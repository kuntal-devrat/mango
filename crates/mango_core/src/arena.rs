//! A typed arena allocator for cache-friendly, index-based data structures.
//!
//! The DOM tree, style tree, and layout tree all use arenas to store nodes.
//! Nodes reference each other via `u32` indices rather than pointers or
//! `Rc<RefCell<>>`, which gives us:
//! - Cache locality (contiguous memory)
//! - Simple serialization
//! - No reference counting overhead
//! - Straightforward ownership (the arena owns everything)
//!
//! # Thread Safety
//!
//! `Arena<T>` is `Send + Sync` when `T` is, enabling parallel layout and style
//! passes. Static assertions enforce this at compile time.

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

/// A strongly-typed, generational index into an [`Arena`].
///
/// The type parameter `T` prevents mixing up indices from different arenas.
/// `Copy`, `Clone`, `PartialEq`, `Eq`, and `Hash` are manually implemented
/// without requiring `T` to satisfy those bounds.
#[derive(Debug)]
pub struct Id<T> {
    index: u32,
    generation: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Copy for Id<T> {}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.generation == 0 || other.generation == 0 {
            self.index == other.index
        } else {
            self.index == other.index && self.generation == other.generation
        }
    }
}

impl<T> Eq for Id<T> {}

impl<T> std::hash::Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        // Hash generation to avoid pathological collisions when slots are
        // recycled. Generation-0 (wildcard) ids hash differently, but that
        // is acceptable because wildcard ids should not be stored in
        // HashMaps — they exist only for transient lookups.
        self.generation.hash(state);
    }
}

impl<T> Id<T> {
    /// Creates a new `Id` from a raw index with wildcard (0) generation.
    ///
    /// Wildcard ids match any generation during lookups, but should **not** be
    /// stored in `HashMap`s or `HashSet`s because their `Hash` output differs
    /// from generation-bearing ids at the same index.
    #[inline]
    pub fn from_raw(index: u32) -> Self {
        Self {
            index,
            generation: 0,
            _marker: PhantomData,
        }
    }

    /// Creates a new `Id` with an explicit index and generation.
    #[inline]
    pub fn from_raw_parts(index: u32, generation: u32) -> Self {
        Self {
            index,
            generation,
            _marker: PhantomData,
        }
    }

    /// Returns the raw index.
    #[inline]
    pub fn raw(self) -> u32 {
        self.index
    }

    /// Returns the generation of this handle.
    #[inline]
    pub fn generation(self) -> u32 {
        self.generation
    }
}

impl<T> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({}:gen{})", self.index, self.generation)
    }
}

#[derive(Debug, Clone)]
struct Slot<T> {
    value: Option<T>,
    generation: u32,
}

/// A typed arena that stores elements contiguously and returns generational [`Id`] handles.
///
/// # Example
/// ```
/// use mango_core::arena::{Arena, Id};
///
/// let mut arena: Arena<String> = Arena::new();
/// let id = arena.alloc("hello".to_string());
/// assert_eq!(arena[id], "hello");
/// ```
#[derive(Debug, Clone)]
pub struct Arena<T> {
    items: Vec<Slot<T>>,
    free_list: Vec<u32>,
    active_count: usize,
}

impl<T> Arena<T> {
    /// Creates a new, empty arena.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            free_list: Vec::new(),
            active_count: 0,
        }
    }

    /// Creates a new arena with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            free_list: Vec::new(),
            active_count: 0,
        }
    }

    /// Allocates a new item in the arena and returns its generational [`Id`].
    /// Reuses slots from the free list if available, bumping their generation.
    pub fn alloc(&mut self, item: T) -> Id<T> {
        if let Some(free_idx) = self.free_list.pop() {
            let slot = &mut self.items[free_idx as usize];
            slot.generation = slot.generation.wrapping_add(1);
            let generation = slot.generation;
            slot.value = Some(item);
            self.active_count += 1;
            Id::from_raw_parts(free_idx, generation)
        } else {
            let index = u32::try_from(self.items.len())
                .expect("Arena capacity overflow: exceeds u32::MAX elements");
            self.items.push(Slot {
                value: Some(item),
                generation: 1,
            });
            self.active_count += 1;
            Id::from_raw_parts(index, 1)
        }
    }

    /// Allocates a new item using a closure that receives the assigned [`Id`].
    /// Reuses slots from the free list if available, bumping their generation.
    pub fn alloc_with<F>(&mut self, f: F) -> Id<T>
    where
        F: FnOnce(Id<T>) -> T,
    {
        if let Some(free_idx) = self.free_list.pop() {
            let slot = &mut self.items[free_idx as usize];
            slot.generation = slot.generation.wrapping_add(1);
            let generation = slot.generation;
            let id = Id::from_raw_parts(free_idx, generation);
            slot.value = Some(f(id));
            self.active_count += 1;
            id
        } else {
            let index = u32::try_from(self.items.len())
                .expect("Arena capacity overflow: exceeds u32::MAX elements");
            let id = Id::from_raw_parts(index, 1);
            self.items.push(Slot {
                value: Some(f(id)),
                generation: 1,
            });
            self.active_count += 1;
            id
        }
    }

    /// Frees an item in the arena, placing its slot back onto the free-list.
    ///
    /// The generation is **not** bumped here — it is bumped in [`alloc`] when
    /// the slot is reused. This avoids wasting half the generation space on a
    /// free→alloc cycle (previously generation was bumped in both `free` and
    /// `alloc`).
    pub fn free(&mut self, id: Id<T>) -> Option<T> {
        let slot = self.items.get_mut(id.index as usize)?;
        if id.generation != 0 && slot.generation != id.generation {
            return None; // Stale ID or already freed
        }
        if let Some(val) = slot.value.take() {
            self.free_list.push(id.index);
            self.active_count = self.active_count.saturating_sub(1);
            Some(val)
        } else {
            None
        }
    }

    /// Returns `true` if the item at the given id is currently active (not freed or superseded).
    #[inline]
    pub fn is_alive(&self, id: Id<T>) -> bool {
        if let Some(slot) = self.items.get(id.index as usize)
            && (id.generation == 0 || slot.generation == id.generation)
        {
            return slot.value.is_some();
        }
        false
    }

    /// Returns the number of active items in the arena.
    #[inline]
    pub fn len(&self) -> usize {
        self.active_count
    }

    /// Returns `true` if the arena contains no active items.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.active_count == 0
    }

    /// Returns the number of freed slots waiting to be reused.
    #[inline]
    pub fn free_slots(&self) -> usize {
        self.free_list.len()
    }

    /// Returns the total number of allocated slot positions.
    #[inline]
    pub fn total_slots(&self) -> usize {
        self.items.len()
    }

    /// Returns a reference to the item at the given id, or `None` if out of bounds, freed, or stale.
    #[inline]
    pub fn get(&self, id: Id<T>) -> Option<&T> {
        let slot = self.items.get(id.index as usize)?;
        if id.generation == 0 || slot.generation == id.generation {
            slot.value.as_ref()
        } else {
            None
        }
    }

    /// Returns a mutable reference to the item at the given id, or `None` if out of bounds, freed, or stale.
    #[inline]
    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        let slot = self.items.get_mut(id.index as usize)?;
        if id.generation == 0 || slot.generation == id.generation {
            slot.value.as_mut()
        } else {
            None
        }
    }

    /// Returns an iterator over all active items with their generational ids.
    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> {
        self.items.iter().enumerate().filter_map(|(i, slot)| {
            slot.value
                .as_ref()
                .map(|v| (Id::from_raw_parts(i as u32, slot.generation), v))
        })
    }

    /// Returns a mutable iterator over all active items with their generational ids.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id<T>, &mut T)> {
        self.items.iter_mut().enumerate().filter_map(|(i, slot)| {
            let generation = slot.generation;
            slot.value
                .as_mut()
                .map(|v| (Id::from_raw_parts(i as u32, generation), v))
        })
    }

    /// Removes all items from the arena, resetting it to an empty state.
    ///
    /// This preserves the allocated capacity for reuse. Generations are
    /// **not** reset, so stale `Id` handles from before the clear remain
    /// correctly invalidated.
    pub fn clear(&mut self) {
        for slot in &mut self.items {
            slot.value = None;
        }
        self.free_list.clear();
        // Rebuild the free list from all slots so they can be reused.
        // Iterate in reverse so that the lowest indices are popped first
        // (LIFO), preserving allocation-order locality.
        for i in (0..self.items.len()).rev() {
            self.free_list.push(i as u32);
        }
        self.active_count = 0;
    }

    /// Retains only the items for which the predicate returns `true`.
    ///
    /// Items removed by the predicate are freed and their slots added to
    /// the free list for reuse.
    pub fn retain<F>(&mut self, mut predicate: F)
    where
        F: FnMut(Id<T>, &T) -> bool,
    {
        for i in 0..self.items.len() {
            let slot = &self.items[i];
            let dominated = if let Some(ref val) = slot.value {
                let id = Id::from_raw_parts(i as u32, slot.generation);
                !predicate(id, val)
            } else {
                false
            };
            if dominated {
                self.items[i].value = None;
                self.free_list.push(i as u32);
                self.active_count = self.active_count.saturating_sub(1);
            }
        }
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Index<Id<T>> for Arena<T> {
    type Output = T;

    #[inline]
    #[track_caller]
    fn index(&self, id: Id<T>) -> &T {
        self.get(id)
            .expect("Attempted to index a freed, stale, or unallocated Arena slot")
    }
}

impl<T> IndexMut<Id<T>> for Arena<T> {
    #[inline]
    #[track_caller]
    fn index_mut(&mut self, id: Id<T>) -> &mut T {
        self.get_mut(id)
            .expect("Attempted to index a freed, stale, or unallocated Arena slot")
    }
}

// Compile-time assertions: Arena<T> is Send+Sync when T is.
const _: () = {
    #[allow(dead_code)]
    fn assert_send<T: Send>() {}
    #[allow(dead_code)]
    fn assert_sync<T: Sync>() {}
    #[allow(dead_code)]
    fn assertions() {
        assert_send::<Arena<String>>();
        assert_sync::<Arena<String>>();
        assert_send::<Id<String>>();
        assert_sync::<Id<String>>();
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_alloc_and_access() {
        let mut arena = Arena::new();
        let id1 = arena.alloc(42);
        let id2 = arena.alloc(99);

        assert_eq!(arena[id1], 42);
        assert_eq!(arena[id2], 99);
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn test_arena_get_out_of_bounds() {
        let arena: Arena<i32> = Arena::new();
        let bad_id = Id::from_raw(999);
        assert!(arena.get(bad_id).is_none());
    }

    #[test]
    fn test_arena_mutation() {
        let mut arena = Arena::new();
        let id = arena.alloc("hello".to_string());
        arena[id] = "world".to_string();
        assert_eq!(arena[id], "world");
    }

    #[test]
    fn test_arena_iter() {
        let mut arena = Arena::new();
        arena.alloc(10);
        arena.alloc(20);
        arena.alloc(30);

        let values: Vec<i32> = arena.iter().map(|(_, v)| *v).collect();
        assert_eq!(values, vec![10, 20, 30]);
    }

    #[test]
    fn test_arena_free_and_slot_recycling() {
        let mut arena = Arena::new();
        let _a = arena.alloc("first");
        let b = arena.alloc("second");
        let _c = arena.alloc("third");
        assert_eq!(arena.len(), 3);
        assert_eq!(arena.total_slots(), 3);

        // Free slot b
        let freed_b = arena.free(b);
        assert_eq!(freed_b, Some("second"));
        assert_eq!(arena.len(), 2);
        assert_eq!(arena.free_slots(), 1);
        assert!(!arena.is_alive(b));
        assert!(arena.get(b).is_none());

        // Allocate a new item — should recycle slot b
        let d = arena.alloc("recycled");
        assert_eq!(d.raw(), b.raw(), "Should reuse the freed slot index");
        assert_ne!(
            d.generation(),
            b.generation(),
            "Generations must differ to invalidate stale handles"
        );
        assert_eq!(arena[d], "recycled");
        assert!(
            arena.get(b).is_none(),
            "Stale handle b must not access newly recycled item d"
        );
        assert_eq!(arena.len(), 3);
        assert_eq!(arena.free_slots(), 0);
        assert_eq!(arena.total_slots(), 3, "No new slot allocation needed");
    }

    #[test]
    fn test_arena_clear() {
        let mut arena = Arena::new();
        let a = arena.alloc("one");
        let _b = arena.alloc("two");
        arena.clear();
        assert_eq!(arena.len(), 0);
        assert!(arena.is_empty());
        assert!(arena.get(a).is_none());
        // Slots are preserved for reuse
        assert_eq!(arena.total_slots(), 2);
        assert_eq!(arena.free_slots(), 2);
        // New allocation reuses slot
        let c = arena.alloc("three");
        assert_eq!(arena[c], "three");
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn test_arena_retain() {
        let mut arena = Arena::new();
        arena.alloc(1);
        arena.alloc(2);
        arena.alloc(3);
        arena.alloc(4);
        arena.retain(|_, &v| v % 2 == 0);
        assert_eq!(arena.len(), 2);
        let values: Vec<i32> = arena.iter().map(|(_, v)| *v).collect();
        assert_eq!(values, vec![2, 4]);
    }

    #[test]
    fn test_id_display() {
        let id: Id<i32> = Id::from_raw_parts(42, 3);
        assert_eq!(format!("{id}"), "Id(42:gen3)");
    }

    #[test]
    fn test_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Arena<i32>>();
        assert_send_sync::<Id<i32>>();
    }
}
