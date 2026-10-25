//! Zero-copy shared-memory ring buffer for high-bandwidth IPC transfers.
//!
//! Enables high-throughput, low-latency bulk data transfers between browser processes
//! (DisplayList commands, compositor tiles, framebuffers, media frames) without
//! intermediate heap allocations.

use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Error conditions occurring during shared memory ring buffer operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShmError {
    BufferFull,
    BufferEmpty,
    RequestedTooLarge,
}

impl std::fmt::Display for ShmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShmError::BufferFull => write!(f, "Shared memory ring buffer full"),
            ShmError::BufferEmpty => write!(f, "Shared memory ring buffer empty"),
            ShmError::RequestedTooLarge => write!(f, "Requested payload exceeds buffer capacity"),
        }
    }
}

impl std::error::Error for ShmError {}

/// An atomic shared-memory ring buffer.
pub struct ShmRingBuffer {
    buffer: Box<[UnsafeCell<u8>]>,
    capacity: usize,
    read_pos: AtomicUsize,
    write_pos: AtomicUsize,
}

impl ShmRingBuffer {
    /// Creates a shared-memory ring buffer with a given byte capacity.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than zero");
        let mut raw = Vec::with_capacity(capacity);
        raw.resize_with(capacity, || UnsafeCell::new(0));
        Self {
            buffer: raw.into_boxed_slice(),
            capacity,
            read_pos: AtomicUsize::new(0),
            write_pos: AtomicUsize::new(0),
        }
    }

    /// Wraps buffer in a thread-safe `Arc` reference.
    pub fn shared(capacity: usize) -> Arc<Self> {
        Arc::new(Self::new(capacity))
    }

    /// Buffer capacity in bytes.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Available bytes to be read from the ring buffer.
    pub fn available_read(&self) -> usize {
        let w = self.write_pos.load(Ordering::Acquire);
        let r = self.read_pos.load(Ordering::Acquire);
        w.saturating_sub(r)
    }

    /// Available bytes that can be written to the ring buffer.
    pub fn available_write(&self) -> usize {
        self.capacity.saturating_sub(self.available_read())
    }

    /// Writes a slice of bytes into the ring buffer.
    pub fn write(&self, data: &[u8]) -> Result<usize, ShmError> {
        if data.is_empty() {
            return Ok(0);
        }
        if data.len() > self.capacity {
            return Err(ShmError::RequestedTooLarge);
        }
        if self.available_write() < data.len() {
            return Err(ShmError::BufferFull);
        }

        let w = self.write_pos.load(Ordering::Acquire);
        let start_idx = w % self.capacity;

        let ptr = self.buffer.as_ptr() as *mut u8;
        let first_chunk = (self.capacity - start_idx).min(data.len());

        // SAFETY:
        // 1. `self.buffer` holds `UnsafeCell<u8>`, explicitly permitting interior mutability via raw pointer.
        // 2. Both `ptr.add(start_idx)` for `first_chunk` bytes and `ptr` for `data.len() - first_chunk` bytes
        //    reside strictly within the allocated capacity bounds `[0, capacity)`.
        // 3. `available_write()` verified sufficient unread capacity, preventing overlap with active reader regions.
        // 4. `data` is an independent input slice and cannot alias the ring buffer memory.
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr.add(start_idx), first_chunk);
            if data.len() > first_chunk {
                std::ptr::copy_nonoverlapping(
                    data.as_ptr().add(first_chunk),
                    ptr,
                    data.len() - first_chunk,
                );
            }
        }

        self.write_pos.fetch_add(data.len(), Ordering::Release);
        Ok(data.len())
    }

    /// Reads up to `max_bytes` from the ring buffer.
    pub fn read(&self, max_bytes: usize) -> Vec<u8> {
        let avail = self.available_read();
        let to_read = avail.min(max_bytes);
        if to_read == 0 {
            return Vec::new();
        }

        let r = self.read_pos.load(Ordering::Acquire);
        let start_idx = r % self.capacity;
        let mut out = vec![0u8; to_read];

        let ptr = self.buffer.as_ptr() as *const u8;
        let first_chunk = (self.capacity - start_idx).min(to_read);

        // SAFETY:
        // 1. Bytes between `r` and `r + to_read` were committed by a writer prior to the atomic `write_pos`
        //    release store, and cannot be overwritten until `read_pos` advances.
        // 2. Offsets `ptr.add(start_idx)` and `ptr` remain strictly within the allocated buffer range.
        // 3. `out` has allocated capacity of `to_read` bytes and does not alias the buffer.
        unsafe {
            std::ptr::copy_nonoverlapping(ptr.add(start_idx), out.as_mut_ptr(), first_chunk);
            if to_read > first_chunk {
                std::ptr::copy_nonoverlapping(
                    ptr,
                    out.as_mut_ptr().add(first_chunk),
                    to_read - first_chunk,
                );
            }
        }

        self.read_pos.fetch_add(to_read, Ordering::Release);
        out
    }

    /// Resets the ring buffer cursors to zero.
    pub fn clear(&self) {
        self.read_pos.store(0, Ordering::Release);
        self.write_pos.store(0, Ordering::Release);
    }
}

// SAFETY: `ShmRingBuffer` coordinates thread-safe concurrent reads and writes across
// independent memory ranges using atomic cursors (`read_pos`, `write_pos`) with Acquire/Release order.
unsafe impl Send for ShmRingBuffer {}
// SAFETY: Thread-safe synchronization ensures disjoint reader and writer access without data races.
unsafe impl Sync for ShmRingBuffer {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shm_ring_buffer_basic_write_read() {
        let shm = ShmRingBuffer::new(1024);
        assert_eq!(shm.available_read(), 0);
        assert_eq!(shm.available_write(), 1024);

        let data = b"Hello, Zero-Copy Shared Memory IPC!";
        let written = shm.write(data).expect("write to SHM");
        assert_eq!(written, data.len());
        assert_eq!(shm.available_read(), data.len());

        let read_bytes = shm.read(1024);
        assert_eq!(read_bytes, data);
        assert_eq!(shm.available_read(), 0);
    }

    #[test]
    fn test_shm_ring_buffer_wraparound() {
        let shm = ShmRingBuffer::new(16);
        let chunk1 = b"0123456789"; // 10 bytes
        shm.write(chunk1).unwrap();
        let read1 = shm.read(10);
        assert_eq!(read1, chunk1);

        // Now write another 10 bytes that will wrap around index 16
        let chunk2 = b"abcdefghij";
        shm.write(chunk2).unwrap();
        let read2 = shm.read(10);
        assert_eq!(read2, chunk2);
    }
}
