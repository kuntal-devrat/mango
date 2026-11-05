//! Bumpalo thread-local scratchpad allocator for layout passes.
//!
//! Provides zero-nanosecond bulk deallocation and prevents heap fragmentation
//! across repeated layout passes and text calculations.

use std::cell::RefCell;
use std::collections::HashMap;

use bumpalo::Bump;

thread_local! {
    static LAYOUT_BUMP: RefCell<Bump> = RefCell::new(Bump::with_capacity(64 * 1024));
    static INTRINSIC_WIDTH_CACHE: RefCell<HashMap<usize, (f32, f32)>> =
        RefCell::new(HashMap::with_capacity(1024));
}

/// Executes a closure with a thread-local `Bump` arena, automatically resetting it on completion.
pub fn with_layout_scratchpad<R>(f: impl FnOnce(&Bump) -> R) -> R {
    INTRINSIC_WIDTH_CACHE.with(|c| c.borrow_mut().clear());
    let res = LAYOUT_BUMP.with(|cell| {
        let mut bump = cell.borrow_mut();
        bump.reset();
        f(&bump)
    });
    INTRINSIC_WIDTH_CACHE.with(|c| c.borrow_mut().clear());
    res
}

/// Executes a closure with the thread-local intrinsic width measurement cache.
pub fn with_intrinsic_width_cache<R>(f: impl FnOnce(&mut HashMap<usize, (f32, f32)>) -> R) -> R {
    INTRINSIC_WIDTH_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        f(&mut cache)
    })
}
