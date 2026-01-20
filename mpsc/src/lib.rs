#![feature(portable_simd)]

mod spsc;
mod consumer_registry;

use arrayvec::ArrayVec;
use consumer_registry::{pop_all, ConsumerRegistry};
use std::cell::UnsafeCell;
use std::simd::Simd;
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
    consumer: ConsumerRegistry<Simd<usize, 16>>,
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
    elem: spsc::Producer<Simd<usize, 16>>,
    list: ConsumerRegistry<Simd<usize, 16>>,
}

impl Drop for PerThreadInner {
    fn drop(&mut self) {
        self.list.remove(self.elem.id());
    }
}

// This is a cached producer
pub struct Producer<T> {
    per_thread: Arc<ThreadLocal<UnsafeCell<Option<PerThreadInner>>>>,
    list: ConsumerRegistry<Simd<usize, 16>>,
    local_batch: ArrayVec<usize, 16>,
    _marker: PhantomData<T>,
}

impl<T> Producer<T> {
    fn new(list: ConsumerRegistry<Simd<usize, 16>>) -> Self {
        Self {
            per_thread: Arc::new(ThreadLocal::new()),
            list,
            local_batch: ArrayVec::new(),
            _marker: PhantomData,
        }
    }

    /// Fast path: accumula nel buffer locale (nessun accesso TLS).
    /// Slow path: quando il buffer è pieno, crea/usa l'inner per-thread e drena in blocchi da 16.
    #[inline(always)]
    pub fn push(&mut self, elem: impl Into<usize>) {
        let mut elem = elem.into();
        loop {
            if let Err(e) = self.local_batch.try_push(elem) {
                // buffer pieno: flush
                self.flush();
                elem = e.element();
                continue;
            }
            break;
        }
    }

    /// Drena il buffer locale nell'SPSC del thread corrente.
    /// Nota: come nel codice originale, solo gruppi completi da 16 vengono inviati.
    #[inline(never)]
    #[cold]
    pub fn flush(&mut self) {
        if unlikely(self.local_batch.is_empty()) {
            return;
        }
        let slot = self.per_thread.get_or_default();
        // SAFETY: l'accesso a slot è esclusivo per questo thread
        let guard = unsafe { &mut *slot.get() };
        if unlikely(guard.is_none()) {
            // Primo uso su *questo* thread: crea SPSC e registra un consumer
            let (p, c) = spsc::channel(self.list.single_spsc_len);
            self.list.push(c);
            *guard = Some(PerThreadInner {
                elem: p,
                list: self.list.clone(),
            });
        }
        // SAFETY: abbiamo appena inizializzato l'inner se non esisteva
        let inner = unsafe { guard.as_mut().unwrap_unchecked() };
        // Converte a blocchi da 16; eventuale resto <16 viene scartato (come prima)
        let iter = to_simd(self.local_batch.drain(..));
        let _ = inner.elem.enqueue_many(iter);
    }
}

impl<T> Clone for Producer<T> {
    fn clone(&self) -> Self {
        Self {
            per_thread: self.per_thread.clone(),
            list: self.list.clone(),
            local_batch: ArrayVec::new(), // ogni handle ha il suo buffer fast-path
            _marker: PhantomData,
        }
    }
}

impl<T> Drop for Producer<T> {
    fn drop(&mut self) {
        self.flush(); 
        // We are basically delaying real drop to the entry of the data structure to
        // the destruction the of last global reference to the producer (i.e., when `per_thread`
        // arc count goes to zero).
    }
}

fn to_simd<I: Iterator<Item = usize>>(mut iter: I) -> impl Iterator<Item = Simd<usize, 16>> {
    iter::from_fn(move || {
        let mut values = [0; 16];
        for slot in values.iter_mut() {
            if let Some(val) = iter.next() {
                *slot = val;
            } else {
                return None; // produce solo gruppi completi da 16
            }
        }
        Some(Simd::from_array(values))
    })
}

// Non creiamo una SPSC al momento della creazione del canale:
// le SPSC vengono create p-thread al primo flush().
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
        const LEN: usize = 1024 * 1024 * 4; // multiplo di 16
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

        // drain best-effort (ordine non garantito)
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
