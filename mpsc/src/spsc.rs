use core::cell::UnsafeCell;

use core::sync::atomic;
use std::sync::Arc;

use arrayvec::ArrayVec;
use atomic::{AtomicUsize, Ordering};
use ringbuf::storage::Heap;
use ringbuf::traits::Split;
use ringbuf::wrap::Wrap;
use ringbuf::{CachingCons, CachingProd, HeapRb, SharedRb};


pub(crate) struct Producer<T> {
    producer: CachingProd<Arc<SharedRb<Heap<T>>>>,
}

impl<T> Producer<T> {
    pub(crate) fn enqueue_many(&mut self, data: impl Iterator<Item = T>) -> usize {
        ringbuf::traits::Producer::push_iter(&mut self.producer, data)
    }

    pub(crate) fn id(&self) -> usize {
        Arc::as_ptr(&self.producer.rb_ref()) as usize
    }

    pub(crate) fn new(producer: CachingProd<Arc<SharedRb<Heap<T>>>>, id: usize) -> Self {
        Self { producer }
    }
}

pub(crate) struct Consumer<T> {
    // we have to promise that the consumer is only used by one thread
    pub(crate) consumer: UnsafeCell<CachingCons<Arc<SharedRb<Heap<T>>>>>,
}

impl<T> Consumer<T> {
    // # Safety
    // Exclusive access must be enforced by the caller
    // #[inline(always)]
    // pub unsafe fn dequeue_many<const N: usize>(&self, data: &mut ArrayVec<T, { N }>) {
    //     let consumer = unsafe { &mut *self.consumer.get() };
    //     let remaining = data.capacity() - data.len();
    //     for scan in ringbuf::traits::Consumer::pop_iter(consumer).take(remaining) {
    //         unsafe { data.push_unchecked(scan) };
    //     }
    // }

    pub(crate) unsafe fn id(&self) -> usize {
        let tmp = unsafe { (*self.consumer.get()).rb_ref() };
        Arc::as_ptr(tmp) as usize
    }

    pub(crate) fn new(consumer: CachingCons<Arc<SharedRb<Heap<T>>>>, id: usize) -> Self {
        Self {
            consumer: UnsafeCell::new(consumer),
        }
    }
}

pub(crate) fn channel<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    let rb = HeapRb::new(capacity);
    static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (producer, consumer) = rb.split();
    (Producer::new(producer, id), Consumer::new(consumer, id))
}
