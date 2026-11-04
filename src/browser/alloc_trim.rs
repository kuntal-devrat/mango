//! Cross-platform allocator trimming and page reclamation.
//!
//! Frees cached unused heap pages back to the operating system kernel
//! via `mimalloc`'s `mi_collect` and glibc's `malloc_trim`.

/// Flushes thread-local allocator caches and returns unused physical pages to the OS.
pub fn trim_allocator_memory() {
    #[cfg(not(target_env = "msvc"))]
    {
        // SAFETY: Calling mi_collect is thread-safe and instructs mimalloc to release
        // freed heap chunks back to the OS via madvise/VirtualFree.
        unsafe extern "C" {
            fn mi_collect(force: bool);
        }
        unsafe {
            mi_collect(true);
        }
    }

    #[cfg(target_os = "linux")]
    {
        // SAFETY: malloc_trim is an idempotent glibc function that releases
        // unmapped top-of-heap memory back to the Linux kernel.
        unsafe extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        unsafe {
            malloc_trim(0);
        }
    }
}
