// Flume-based backend distributor implementations
// This module provides implementations of BDistributor traits for the flume channel library.

// =============================================================================
// Flume-based SPMC Distributor Implementation
// =============================================================================

use crate::api::bdistributor::{
    BDistributor, PopError, PushError, TryPopError, TryPushError,
    spmcbdistributor::{SPMCBDistributor, SPMCBDistributorPopper, SPMCBDistributorPusher},
};

// Implementation for the flume channel tuple (Sender, Receiver) with batched arrays
impl<const BATCH_SIZE: usize, T: Send + 'static> BDistributor<BATCH_SIZE, T>
    for (
        flume::Sender<[T; BATCH_SIZE]>,
        flume::Receiver<[T; BATCH_SIZE]>,
    )
{
    fn create(inner: Self) -> Self {
        inner
    }
}

impl<const BATCH_SIZE: usize, T: Send + 'static> SPMCBDistributor<BATCH_SIZE, T>
    for (
        flume::Sender<[T; BATCH_SIZE]>,
        flume::Receiver<[T; BATCH_SIZE]>,
    )
{
    type Pusher = flume::Sender<[T; BATCH_SIZE]>;
    type Popper = flume::Receiver<[T; BATCH_SIZE]>;

    fn split(self, n: usize) -> (Self::Pusher, Vec<Self::Popper>) {
        let (pusher, popper) = self; 
        let mut poppers = Vec::with_capacity(n);
        poppers.push(popper);
        for _ in 1..n {
            let tmp = poppers[0].clone();
            poppers.push(tmp);
        }

        (pusher, poppers)
    }
}

impl<const BATCH_SIZE: usize, T: Send + 'static> SPMCBDistributorPusher<BATCH_SIZE, T>
    for flume::Sender<[T; BATCH_SIZE]>
{
    fn try_push(
        &self,
        batch: [T; BATCH_SIZE],
    ) -> core::result::Result<(), TryPushError<[T; BATCH_SIZE]>> {
        self.try_send(batch).map_err(|err| match err {
            flume::TrySendError::Full(batch) => TryPushError::Full(batch),
            flume::TrySendError::Disconnected(batch) => TryPushError::Closed(batch),
        })
    }

    fn push(
        &self,
        batch: [T; BATCH_SIZE],
    ) -> impl core::future::Future<Output = core::result::Result<(), PushError<[T; BATCH_SIZE]>>> {
        async move { self.send(batch).map_err(|err| PushError(err.0)) }
    }
}

// Implementation for flume::Receiver
impl<const BATCH_SIZE: usize, T: Send + 'static> BDistributor<BATCH_SIZE, [T; BATCH_SIZE]>
    for flume::Receiver<[T; BATCH_SIZE]>
{
    fn create(inner: Self) -> Self {
        inner
    }
}

impl<const BATCH_SIZE: usize, T: Send + 'static> SPMCBDistributorPopper<BATCH_SIZE, T>
    for flume::Receiver<[T; BATCH_SIZE]>
{
    fn try_pop(&self) -> Result<[T; BATCH_SIZE], TryPopError> {
        self.try_recv().map_err(|err| match err {
            flume::TryRecvError::Empty => TryPopError::Empty,
            flume::TryRecvError::Disconnected => TryPopError::Closed,
        })
    }

    fn pop(
        &self,
    ) -> impl core::future::Future<Output = Result<[T; BATCH_SIZE], PopError>> {
        async move { self.recv().map_err(|_| PopError) }
    }
}
