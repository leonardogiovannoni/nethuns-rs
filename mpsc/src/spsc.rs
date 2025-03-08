use core::cell::UnsafeCell;

use core::sync::atomic;
use std::sync::Arc;

use atomic::{AtomicUsize, Ordering};
use ringbuf::storage::Heap;
use ringbuf::traits::Split;
use ringbuf::{CachingCons, CachingProd, HeapRb, SharedRb};

pub(crate) struct Queue<T>{
    rb: HeapRb<T>
}

pub(crate) struct Producer<T> {
    producer: CachingProd<Arc<SharedRb<Heap<T>>>>,
    id: usize
}

impl<T> Producer<T> {
    pub(crate) fn enqueue(&mut self, value: T) -> Result<(), T> {
        ringbuf::traits::Producer::try_push(&mut self.producer, value)
    }

    pub(crate) fn enqueue_many(&mut self, data: impl Iterator<Item = T>) -> usize {
        ringbuf::traits::Producer::push_iter(&mut self.producer, data)
    }

    pub(crate) fn id(&self) -> usize {
        self.id
    }

    pub(crate) fn new(producer: CachingProd<Arc<SharedRb<Heap<T>>>>, id: usize) -> Self {
        Self {
            producer,
            id
        }
    }

}

pub(crate) struct Consumer<T> {
    // we have to promise that the consumer is only used by one thread
    consumer: UnsafeCell<CachingCons<Arc<SharedRb<Heap<T>>>>>,
    id: usize
}

impl<T> Consumer<T> {
    // # Safety
    // Exclusive access must be enforced by the caller
    pub unsafe fn dequeue_many(&self, data: &mut Vec<T>) {
        let consumer = unsafe { &mut *self.consumer.get() };
        for scan in ringbuf::traits::Consumer::pop_iter(consumer) {
            data.push(scan);
        }
    }

    pub(crate) fn id(&self) -> usize {
        self.id
    }

    pub(crate) fn new(consumer: CachingCons<Arc<SharedRb<Heap<T>>>>, id: usize) -> Self {
        Self {
            consumer: UnsafeCell::new(consumer),
            id
        }
    }
}





impl<T> Queue<T> {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            rb: HeapRb::new(capacity)
        }
    }

    pub(crate) fn split(self) -> (Producer<T>, Consumer<T>) {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let (producer, consumer) = self.rb.split();
        (
            Producer::new(producer, id),
            Consumer::new(consumer, id)
        ) 

        //(Producer { rb: producer }, Consumer { rb: consumer })
    }

}



//pub(crate) trait Idable {
//    fn id(&self) -> usize;
//}
//
//impl<T> Idable for Producer<T> {
//    fn id(&self) -> usize {
//        self.id
//    }
//}
//
//impl<T> Idable for Consumer<T> {
//    fn id(&self) -> usize {
//        self.id
//    }
//}