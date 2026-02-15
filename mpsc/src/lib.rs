#![cfg_attr(feature = "simd", feature(portable_simd))]

mod spsc;
mod consumer_registry;
mod simd_type;

use arrayvec::ArrayVec;
use consumer_registry::{pop_all, ConsumerRegistry};
use std::cell::UnsafeCell;
use simd_type::SimdUsize16;
use std::marker::PhantomData;
use std::iter;

use std::usize;

use thread_local::ThreadLocal;
use std::sync::Arc;

#[inline]
#[cold]
fn cold() {}

#[inline]
pub fn likely(b: bool) -> bool {
    if !b { cold() }
    b
}

#[inline]
pub fn unlikely(b: bool) -> bool {
    if b { cold() }
    b
}

// This is a cached consumer
pub struct Consumer<T> {
    consumer: ConsumerRegistry<SimdUsize16>,
    cached: ArrayVec<usize, 1024>,
    _marker: PhantomData<T>,
}

impl<T> Consumer<T> {
    pub fn pop(&mut self) -> Option<usize> {
        if unlikely(self.cached.is_empty()) {
            self.sync();
        }
        self.cached.pop()
    }

    pub fn cached(&mut self) -> &mut ArrayVec<usize, 1024> {
        &mut self.cached
    }

    pub fn available_len(&self) -> usize {
        self.cached.len()
    }

    pub fn sync(&mut self) {
        pop_all(&mut self.consumer, &mut self.cached);
    }
}

// ===== TLS per-thread + fast path =====

struct PerThreadInner {
    elem: spsc::Producer<SimdUsize16>,
    list: ConsumerRegistry<SimdUsize16>,
}

impl Drop for PerThreadInner {
    fn drop(&mut self) {
        self.list.remove(self.elem.id());
    }
}

// This is a cached producer
pub struct Producer<T> {
    per_thread: Arc<ThreadLocal<UnsafeCell<Option<PerThreadInner>>>>,
    list: ConsumerRegistry<SimdUsize16>,
    local_batch: ArrayVec<usize, 16>,
    _marker: PhantomData<T>,
}

impl<T> Producer<T> {
    fn new(list: ConsumerRegistry<SimdUsize16>) -> Self {
        Self {
            per_thread: Arc::new(ThreadLocal::new()),
            list,
            local_batch: ArrayVec::new(),
            _marker: PhantomData,
        }
    }

    /// Fast path: accumulate in local buffer (no TLS access).
    /// Slow path: when buffer is full, create/use per-thread inner and drain in blocks of 16.
    #[inline(always)]
    pub fn push(&mut self, elem: impl Into<usize>) {
        let mut elem = elem.into();
        loop {
            if let Err(e) = self.local_batch.try_push(elem) {
                // Buffer full: flush
                self.flush();
                elem = e.element();
                continue;
            }
            break;
        }
    }

    /// Drain the local buffer into the current thread's SPSC.
    /// Note: as in the original code, only complete groups of 16 are sent.
    #[inline(never)]
    #[cold]
    pub fn flush(&mut self) {
        if unlikely(self.local_batch.is_empty()) {
            return;
        }
        let slot = self.per_thread.get_or_default();
        // SAFETY: slot access is exclusive to this thread
        let guard = unsafe { &mut *slot.get() };
        if unlikely(guard.is_none()) {
            // First use on *this* thread: create SPSC and register a consumer
            let (p, c) = spsc::channel(self.list.single_spsc_len);
            self.list.push(c);
            *guard = Some(PerThreadInner {
                elem: p,
                list: self.list.clone(),
            });
        }
        // SAFETY: we just initialized the inner if it didn't exist
        let inner = unsafe { guard.as_mut().unwrap_unchecked() };

        let val_opt = {
            let mut iter = to_simd(self.local_batch.iter().cloned());
            iter.next()
        };

        if let Some(val) = val_opt {
            loop {
                if inner.elem.enqueue_many(iter::once(val)) > 0 {
                    break;
                }
                std::hint::spin_loop();
            }
            self.local_batch.clear();
        }
    }
}

impl<T> Clone for Producer<T> {
    fn clone(&self) -> Self {
        Self {
            per_thread: self.per_thread.clone(),
            list: self.list.clone(),
            local_batch: ArrayVec::new(), // each handle has its own fast-path buffer
            _marker: PhantomData,
        }
    }
}

impl<T> Drop for Producer<T> {
    fn drop(&mut self) {
        self.flush();
        // We are basically delaying real drop to the entry of the data structure to
        // the destruction of the last global reference to the producer (i.e., when `per_thread`
        // arc count goes to zero).
    }
}



fn to_simd<I: Iterator<Item = usize>>(mut iter: I) -> impl Iterator<Item = SimdUsize16> {
    iter::from_fn(move || {
        let mut values = [0; 16];
        for slot in values.iter_mut() {
            if let Some(val) = iter.next() {
                *slot = val;
            } else {
                return None; // produce only complete groups of 16
            }
        }
        Some(simd_type::from_array(values))
    })
}

// We don't create a SPSC at channel creation time:
// SPSCs are created per-thread at first flush().
pub fn channel<T>(size: usize) -> (Producer<T>, Consumer<T>) {
    let list = ConsumerRegistry::new(size);
    (
        Producer::new(list.clone()),
        Consumer {
            consumer: list,
            cached: ArrayVec::new(),
            _marker: PhantomData,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_fastpath() {
        const LEN: usize = 1024 * 1024 * 4; // multiple of 16
        let (producer, mut consumer) = channel::<usize>(LEN);
        let threads = num_cpus::get();
        let mut handles = Vec::new();
        let mut producers = Vec::new();
        for _ in 0..threads {
            producers.push(producer.clone());
        }
        for mut p in producers {
            let handle = std::thread::spawn(move || {
                for i in 0..LEN {
                    p.push(i as usize);
                }
            });
            handles.push(handle);
        }

        // Drain best-effort (order not guaranteed)
        let mut got = 0usize;
        let expected = threads * LEN;
        while got < expected {
            if let Some(_v) = consumer.pop() {
                got += 1;
            } else {
                std::thread::yield_now();
            }
        }

        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(got, expected);
    }
}
