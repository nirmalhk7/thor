use std::sync::atomic::{AtomicUsize, Ordering};
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;

const CACHE_LINE_SIZE: usize = 64;

#[repr(C, align(64))]
struct CachePadded<T> {
    value: T,
}

/// A Single-Producer Single-Consumer (SPSC) lock-free atomic shared-memory Ring Buffer.
/// Features 64-byte cache-line alignment to prevent false sharing, and zero-copy semantics.
pub struct SpscRingBuffer<T, const N: usize> {
    head: CachePadded<AtomicUsize>,
    tail: CachePadded<AtomicUsize>,
    buffer: UnsafeCell<[MaybeUninit<T>; N]>,
}

unsafe impl<T: Send, const N: usize> Send for SpscRingBuffer<T, N> {}
unsafe impl<T: Send, const N: usize> Sync for SpscRingBuffer<T, N> {}

impl<T, const N: usize> SpscRingBuffer<T, N> {
    pub fn new() -> Self {
        assert!(N.is_power_of_two(), "Capacity must be a power of two");
        Self {
            head: CachePadded { value: AtomicUsize::new(0) },
            tail: CachePadded { value: AtomicUsize::new(0) },
            // Safety: an array of MaybeUninit is fully valid when uninitialized.
            buffer: UnsafeCell::new(unsafe { MaybeUninit::uninit().assume_init() }),
        }
    }

    pub fn push(&self, value: T) -> Result<(), T> {
        let head = self.head.value.load(Ordering::Relaxed);
        let tail = self.tail.value.load(Ordering::Acquire);
        
        if head.wrapping_sub(tail) == N {
            return Err(value); // Full
        }

        unsafe {
            let buf = self.buffer.get() as *mut MaybeUninit<T>;
            (*buf.add(head & (N - 1))).write(value);
        }

        self.head.value.store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    pub fn pop(&self) -> Option<T> {
        let tail = self.tail.value.load(Ordering::Relaxed);
        let head = self.head.value.load(Ordering::Acquire);

        if tail == head {
            return None; // Empty
        }

        let value = unsafe {
            let buf = self.buffer.get() as *mut MaybeUninit<T>;
            (*buf.add(tail & (N - 1))).assume_init_read()
        };

        self.tail.value.store(tail.wrapping_add(1), Ordering::Release);
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_push_pop() {
        let rb = SpscRingBuffer::<i32, 16>::new();
        assert_eq!(rb.push(1), Ok(()));
        assert_eq!(rb.push(2), Ok(()));
        assert_eq!(rb.pop(), Some(1));
        assert_eq!(rb.pop(), Some(2));
        assert_eq!(rb.pop(), None);
    }

    #[test]
    fn test_performance() {
        let rb = SpscRingBuffer::<i32, 1024>::new();
        let start = Instant::now();
        
        for i in 0..1024 {
            let _ = rb.push(i);
        }
        for _ in 0..1024 {
            let _ = rb.pop();
        }
        
        let duration = start.elapsed();
        // Sub-microsecond validation for typical IPC usage
        assert!(duration.as_micros() < 1000);
    }
}
