// Flume-based backend distributor implementations
// This module provides implementations of BDistributor traits for the flume channel library.

// =============================================================================
// Flume-based SPMC Distributor Implementation
// =============================================================================

use crate::api::bdistributor::{
    BDistributor,
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
    fn push(&self, batch: [T; BATCH_SIZE]) -> core::result::Result<(), [T; BATCH_SIZE]> {
        // Send each batch individually
        if let Err(err) = self.send(batch) {
            // If sending fails, we can't recover the batches easily
            // This is a limitation of the current design
            panic!("Failed to send batch: {:?}", err);
        }
        Ok(())
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
    fn pop(&self) -> Option<[T; BATCH_SIZE]> {
        self.recv().ok()
    }
}
