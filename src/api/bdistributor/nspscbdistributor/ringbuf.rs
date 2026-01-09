use std::cell::RefCell;
use std::ops::Mul;
use std::sync::Arc;

use ringbuf::storage::Heap;
use ringbuf::traits::Split;
use ringbuf::traits::{Consumer as RbConsumer, Producer as RbProducer};
use ringbuf::{CachingCons, CachingProd, HeapRb, SharedRb};

use crate::api::bdistributor::{BDistributor, PopError, PushError, TryPopError, TryPushError};
use crate::api::bdistributor::nspscbdistributor::{NSPSCBDistributor, NSPSCBDistributorPopper, NSPSCBDistributorPusher};
use crate::api::bdistributor::spmcbdistributor::{SPMCBDistributor, SPMCBDistributorPusher};

pub struct Producer<T> {
    pub(crate) producer: CachingProd<Arc<SharedRb<Heap<T>>>>,
}

impl<T> Producer<T> {
    #[inline]
    pub fn push(&mut self, item: T) -> Result<(), T> {
        RbProducer::try_push(&mut self.producer, item)
    }

    pub(crate) fn new(producer: CachingProd<Arc<SharedRb<Heap<T>>>>) -> Self {
        Self { producer }
    }
}

pub struct Consumer<T> {
    // Safety: caller must enforce exclusive access to the consumer (SPSC).
    consumer: RefCell<CachingCons<Arc<SharedRb<Heap<T>>>>>,
}

impl<T> Consumer<T> {
    /// Pop a single item if available.
    ///
    /// # Safety
    /// Caller must ensure this consumer is used by only one thread.
    #[inline]
    pub fn pop(&self) -> Option<T> {
        RbConsumer::try_pop(&mut *self.consumer.borrow_mut())
    }

    pub(crate) fn new(consumer: CachingCons<Arc<SharedRb<Heap<T>>>>) -> Self {
        Self { consumer: RefCell::new(consumer) }
    }
}

/// Create a single SPSC channel on a heap-backed ring buffer.
fn channel<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    let rb: HeapRb<T> = HeapRb::new(capacity);
    let (prod, cons) = rb.split();
    (Producer::new(prod), Consumer::new(cons))
}

pub struct MultiProducer<T> {
    prods: RefCell<Vec<Producer<T>>>,
}

impl<T> MultiProducer<T> {
    #[inline]
    pub fn push_at(&self, index: usize, item: T) -> Result<(), T> {
        let mut prods = self.prods.borrow_mut();
        assert!(
            index < prods.len(),
            "index out of bounds: {index} >= {}",
            prods.len()
        );
        prods[index].push(item)
    }

    #[inline]
    pub fn producers_mut(&self) -> std::cell::RefMut<'_, Vec<Producer<T>>> {
        self.prods.borrow_mut()
    }
}

pub fn nspsc_channel<T>(capacity: usize, n: usize) -> (MultiProducer<T>, Vec<Consumer<T>>) {
    let mut prod_vec: Vec<Producer<T>> = Vec::with_capacity(n);
    let mut cons_vec: Vec<Consumer<T>> = Vec::with_capacity(n);

    for _ in 0..n {
        let (p, c) = channel::<T>(capacity);
        prod_vec.push(p);
        cons_vec.push(c);
    }

    (MultiProducer { prods: RefCell::new(prod_vec) }, cons_vec)
}


impl<const BATCH_SIZE: usize, T: Send + 'static> BDistributor<BATCH_SIZE, T>
    for (MultiProducer<[T; BATCH_SIZE]>, Vec<Consumer<[T; BATCH_SIZE]>>) 
{
}

impl<const BATCH_SIZE: usize, T: Send + 'static> NSPSCBDistributor<BATCH_SIZE, T>
    for (MultiProducer<[T; BATCH_SIZE]>, Vec<Consumer<[T; BATCH_SIZE]>>)
{
    type Pusher = MultiProducer<[T; BATCH_SIZE]>;
    type Popper = Consumer<[T; BATCH_SIZE]>;

    fn split(self) -> (Self::Pusher, Vec<Self::Popper>) {
        let (pusher, poppers) = self; 
        (pusher, poppers)
    }
}

impl<const BATCH_SIZE: usize, T: Send + 'static> NSPSCBDistributorPusher<BATCH_SIZE, T>
    for MultiProducer<[T; BATCH_SIZE]>
{
    fn try_push(
        &self,
        batch: [T; BATCH_SIZE],
        index: usize,
    ) -> core::result::Result<(), TryPushError<[T; BATCH_SIZE]>> {
        let mut prods = self.prods.borrow_mut();
        assert!(
            index < prods.len(),
            "index out of bounds: {index} >= {}",
            prods.len()
        );
        prods[index].push(batch).map_err(TryPushError::Full)
    }

    fn push(
        &self,
        batch: [T; BATCH_SIZE],
        index: usize,
    ) -> impl core::future::Future<Output = core::result::Result<(), PushError<[T; BATCH_SIZE]>>> {
        async move {
            self.try_push(batch, index)
                .map_err(|err| PushError(err.into_inner()))
        }
    }
}

impl<const BATCH_SIZE: usize, T: Send + 'static> NSPSCBDistributorPopper<BATCH_SIZE, T>
    for Consumer<[T; BATCH_SIZE]>
{
    fn try_pop(&self) -> Result<[T; BATCH_SIZE], TryPopError> {
        Consumer::pop(self).ok_or(TryPopError::Empty)
    }

    fn pop(
        &self,
    ) -> impl core::future::Future<Output = Result<[T; BATCH_SIZE], PopError>> {
        async move { self.try_pop().map_err(|_| PopError) }
    }
}
