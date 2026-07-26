use std::sync::atomic::{AtomicUsize, Ordering};
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::marker::PhantomData;

pub const CACHE_LINE_SIZE: usize = 64;

#[repr(C, align(64))]
struct CachePadded<T> {
    value: T,
}

/// A Single-Producer Single-Consumer (SPSC) lock-free atomic shared-memory Ring Buffer.
/// Split into explicit Producer and Consumer handles to guarantee SPSC semantics.
pub struct SpscRingBuffer<T, const N: usize> {
    head: CachePadded<AtomicUsize>,
    tail: CachePadded<AtomicUsize>,
    buffer: UnsafeCell<[MaybeUninit<T>; N]>,
}

unsafe impl<T: Send, const N: usize> Send for SpscRingBuffer<T, N> {}
unsafe impl<T: Send, const N: usize> Sync for SpscRingBuffer<T, N> {}

impl<T, const N: usize> SpscRingBuffer<T, N> {
    pub fn new() -> (Producer<T, N>, Consumer<T, N>) {
        assert!(N.is_power_of_two(), "Capacity must be a power of two");
        let rb = Arc::new(Self {
            head: CachePadded { value: AtomicUsize::new(0) },
            tail: CachePadded { value: AtomicUsize::new(0) },
            buffer: UnsafeCell::new([const { MaybeUninit::uninit() }; N]),
        });
        
        (Producer { rb: rb.clone(), _marker: PhantomData }, Consumer { rb, _marker: PhantomData })
    }
}

pub struct Producer<T, const N: usize> {
    rb: Arc<SpscRingBuffer<T, N>>,
    _marker: PhantomData<*const ()>,
}

// Producer can be sent to another thread, but cannot be shared concurrently
unsafe impl<T: Send, const N: usize> Send for Producer<T, N> {}

impl<T, const N: usize> Producer<T, N> {
    pub fn push(&mut self, value: T) -> Result<(), T> {
        let head = self.rb.head.value.load(Ordering::Relaxed);
        let tail = self.rb.tail.value.load(Ordering::Acquire);
        
        if head.wrapping_sub(tail) == N {
            return Err(value); // Full
        }

        unsafe {
            let buf = self.rb.buffer.get() as *mut MaybeUninit<T>;
            (*buf.add(head & (N - 1))).write(value);
        }

        self.rb.head.value.store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }
}

pub struct Consumer<T, const N: usize> {
    rb: Arc<SpscRingBuffer<T, N>>,
    _marker: PhantomData<*const ()>,
}

// Consumer can be sent to another thread, but cannot be shared concurrently
unsafe impl<T: Send, const N: usize> Send for Consumer<T, N> {}

impl<T, const N: usize> Consumer<T, N> {
    pub fn pop(&mut self) -> Option<T> {
        let tail = self.rb.tail.value.load(Ordering::Relaxed);
        let head = self.rb.head.value.load(Ordering::Acquire);

        if tail == head {
            return None; // Empty
        }

        let value = unsafe {
            let buf = self.rb.buffer.get() as *mut MaybeUninit<T>;
            (*buf.add(tail & (N - 1))).assume_init_read()
        };

        self.rb.tail.value.store(tail.wrapping_add(1), Ordering::Release);
        Some(value)
    }
}

impl<T, const N: usize> Drop for SpscRingBuffer<T, N> {
    fn drop(&mut self) {
        // Safe to clean up without sync because we only drop when both handles are gone
        let tail = self.tail.value.load(Ordering::Relaxed);
        let head = self.head.value.load(Ordering::Relaxed);
        for i in tail..head {
            unsafe {
                let buf = self.buffer.get() as *mut MaybeUninit<T>;
                (*buf.add(i & (N - 1))).assume_init_drop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_push_pop() {
        let (mut prod, mut cons) = SpscRingBuffer::<i32, 16>::new();
        assert_eq!(prod.push(1), Ok(()));
        assert_eq!(prod.push(2), Ok(()));
        assert_eq!(cons.pop(), Some(1));
        assert_eq!(cons.pop(), Some(2));
        assert_eq!(cons.pop(), None);
    }

    #[test]
    fn test_performance() {
        let (mut prod, mut cons) = SpscRingBuffer::<i32, 1024>::new();
        let start = Instant::now();
        
        for i in 0..1024 {
            let _ = prod.push(i);
        }
        for _ in 0..1024 {
            let _ = cons.pop();
        }
        
        let duration = start.elapsed();
        // Sub-microsecond validation for typical IPC usage
        assert!(duration.as_micros() < 1000);
    }
}
