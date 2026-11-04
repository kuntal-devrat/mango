//! Bumpalo thread-local scratchpad allocator for layout passes.
//!
//! Provides zero-nanosecond bulk deallocation and prevents heap fragmentation
//! across repeated layout passes and text calculations.

use std::cell::RefCell;

use bumpalo::Bump;

thread_local! {
    static LAYOUT_BUMP: RefCell<Bump> = RefCell::new(Bump::with_capacity(64 * 1024));
}

/// Executes a closure with a thread-local `Bump` arena, automatically resetting it on completion.
pub fn with_layout_scratchpad<R>(f: impl FnOnce(&Bump) -> R) -> R {
    LAYOUT_BUMP.with(|cell| {
        let mut bump = cell.borrow_mut();
        bump.reset();
        f(&bump)
    })
}
