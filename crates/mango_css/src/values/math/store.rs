//! Thread-safe interned math store with O(1) deduplication and O(1) LRU eviction.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use super::expr::{MathExpr, MathId};
use crate::values::length::Length;

static MATH_STORE: OnceLock<RwLock<MathStoreInner>> = OnceLock::new();

/// Intrusive doubly-linked entry for constant-time LRU ordering.
struct MathStoreEntry {
    expr: MathExpr,
    prev: Option<u32>,
    next: Option<u32>,
}

/// Thread-safe interned-math-expression store with O(1) dedup and O(1) LRU eviction.
struct MathStoreInner {
    entries: HashMap<u32, MathStoreEntry>,
    by_expr: HashMap<MathExpr, u32>,
    head: Option<u32>, // least-recently-used (oldest)
    tail: Option<u32>, // most-recently-used (newest)
    next_id: u32,
}

impl MathStoreInner {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            by_expr: HashMap::new(),
            head: None,
            tail: None,
            next_id: 0,
        }
    }

    /// Moves `id` to the tail (most-recently-used position) in O(1) time.
    fn touch(&mut self, id: u32) {
        if self.tail == Some(id) {
            return;
        }

        let (prev, next) = match self.entries.get(&id) {
            Some(entry) => (entry.prev, entry.next),
            None => return,
        };

        // Unlink from current position
        if let Some(p) = prev {
            if let Some(prev_entry) = self.entries.get_mut(&p) {
                prev_entry.next = next;
            }
        } else {
            self.head = next;
        }

        if let Some(n) = next {
            if let Some(next_entry) = self.entries.get_mut(&n) {
                next_entry.prev = prev;
            }
        } else {
            self.tail = prev;
        }

        // Link at tail
        if let Some(old_tail) = self.tail {
            if let Some(tail_entry) = self.entries.get_mut(&old_tail) {
                tail_entry.next = Some(id);
            }
        } else {
            self.head = Some(id);
        }

        if let Some(entry) = self.entries.get_mut(&id) {
            entry.prev = self.tail;
            entry.next = None;
        }
        self.tail = Some(id);
    }

    /// Evicts the least-recently-used entry from the head in O(1) time.
    fn pop_lru(&mut self) -> Option<(u32, MathExpr)> {
        let head_id = self.head?;
        let entry = self.entries.remove(&head_id)?;
        self.by_expr.remove(&entry.expr);
        self.head = entry.next;
        if let Some(new_head) = self.head {
            if let Some(head_entry) = self.entries.get_mut(&new_head) {
                head_entry.prev = None;
            }
        } else {
            self.tail = None;
        }
        Some((head_id, entry.expr))
    }
}

fn get_math_store() -> &'static RwLock<MathStoreInner> {
    MATH_STORE.get_or_init(|| RwLock::new(MathStoreInner::new()))
}

/// Interns a MathExpr into the thread-safe global store and returns its MathId,
/// deduplicating existing identical expressions in O(1) via a reverse map.
pub fn intern_math(expr: MathExpr) -> MathId {
    const MAX_MATH_STORE_SIZE: usize = 16_384;
    let store = get_math_store();
    let mut guard = store.write().unwrap_or_else(|e| e.into_inner());
    if let Some(&id) = guard.by_expr.get(&expr) {
        guard.touch(id);
        return MathId(id);
    }
    if guard.entries.len() >= MAX_MATH_STORE_SIZE && guard.pop_lru().is_some() {
        eprintln!("mango_css: math expression store full; evicted least-recently-used entry");
    }
    let id = guard.next_id;
    guard.next_id = guard.next_id.wrapping_add(1);

    // Link at tail
    if let Some(old_tail) = guard.tail {
        if let Some(tail_entry) = guard.entries.get_mut(&old_tail) {
            tail_entry.next = Some(id);
        }
    } else {
        guard.head = Some(id);
    }

    let prev = guard.tail;
    guard.tail = Some(id);
    guard.entries.insert(
        id,
        MathStoreEntry {
            expr: expr.clone(),
            prev,
            next: None,
        },
    );
    guard.by_expr.insert(expr, id);
    MathId(id)
}

/// Clears all interned math expressions (useful for test harnesses or memory recovery).
pub fn clear_math_store() {
    let store = get_math_store();
    let mut guard = store.write().unwrap_or_else(|e| e.into_inner());
    guard.entries.clear();
    guard.by_expr.clear();
    guard.head = None;
    guard.tail = None;
    guard.next_id = 0;
}

/// Resolves an interned MathId to concrete pixels.
pub fn eval_math(
    id: MathId,
    font_size: f32,
    root_font_size: f32,
    container_size: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> f32 {
    let store = get_math_store();
    let guard = store.read().unwrap_or_else(|e| e.into_inner());
    if let Some(entry) = guard.entries.get(&id.0) {
        entry.expr.to_px_with_viewports(
            font_size,
            root_font_size,
            container_size,
            viewport_width,
            viewport_height,
        )
    } else {
        0.0
    }
}

/// Retrieves a clone of the interned MathExpr for a MathId.
pub fn get_math(id: MathId) -> Option<MathExpr> {
    let store = get_math_store();
    let guard = store.read().unwrap_or_else(|e| e.into_inner());
    guard.entries.get(&id.0).map(|e| e.expr.clone())
}

/// Creates a Length from a MathExpr, performing constant folding to Length::Px or CalcLength if possible.
pub fn make_math_length(expr: MathExpr) -> Length {
    let simplified = expr.simplify();
    if simplified.is_dimensionless_number() {
        Length::Math(intern_math(simplified))
    } else if let Some(calc) = simplified.try_into_calc_length() {
        Length::Calc(calc)
    } else if let Some(px) = simplified.try_fold_px() {
        Length::Px(px)
    } else {
        Length::Math(intern_math(simplified))
    }
}

thread_local! {
    static ANCHOR_NAMES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn intern_anchor_name(name: &str) -> u32 {
    ANCHOR_NAMES.with(|names| {
        let mut list = names.borrow_mut();
        if let Some(pos) = list.iter().position(|n| n == name) {
            pos as u32
        } else {
            let id = list.len() as u32;
            list.push(name.to_string());
            id
        }
    })
}

pub fn get_interned_anchor_name(id: u32) -> Option<String> {
    ANCHOR_NAMES.with(|names| names.borrow().get(id as usize).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_math_store_lru_intrusive_linked_list() {
        let mut inner = MathStoreInner::new();
        let expr1 = MathExpr::Number(1.0);
        let expr2 = MathExpr::Number(2.0);
        let expr3 = MathExpr::Number(3.0);

        // Insert expr1
        inner.entries.insert(
            1,
            MathStoreEntry {
                expr: expr1.clone(),
                prev: None,
                next: None,
            },
        );
        inner.by_expr.insert(expr1, 1);
        inner.head = Some(1);
        inner.tail = Some(1);

        // Insert expr2 at tail
        inner.entries.get_mut(&1).unwrap().next = Some(2);
        inner.entries.insert(
            2,
            MathStoreEntry {
                expr: expr2.clone(),
                prev: Some(1),
                next: None,
            },
        );
        inner.by_expr.insert(expr2, 2);
        inner.tail = Some(2);

        // Insert expr3 at tail
        inner.entries.get_mut(&2).unwrap().next = Some(3);
        inner.entries.insert(
            3,
            MathStoreEntry {
                expr: expr3.clone(),
                prev: Some(2),
                next: None,
            },
        );
        inner.by_expr.insert(expr3, 3);
        inner.tail = Some(3);

        assert_eq!(inner.head, Some(1));
        assert_eq!(inner.tail, Some(3));

        // Touch expr1 (oldest) -> moves to tail, new head is expr2
        inner.touch(1);
        assert_eq!(inner.head, Some(2));
        assert_eq!(inner.tail, Some(1));

        // Pop LRU (should evict expr2)
        let (evicted_id, _) = inner.pop_lru().expect("must pop head");
        assert_eq!(evicted_id, 2);
        assert_eq!(inner.head, Some(3));
        assert_eq!(inner.tail, Some(1));
    }
}
