// N-SPSC Backend Distributor traits
// N-way Single Producer, Single Consumer distributor traits for the backend layer.

pub mod aspsc;
pub mod ringbuf;
use super::{BDistributor, PopError, PushError, TryPopError, TryPushError};

/// Trait for pushing batches to an N-SPSC distributor (producer side)
/// The index parameter specifies which consumer queue to push to.
pub trait NSPSCBDistributorPusher<const BATCH_SIZE: usize, T> {
    fn try_push(
        &self,
        batch: [T; BATCH_SIZE],
        index: usize,
    ) -> core::result::Result<(), TryPushError<[T; BATCH_SIZE]>>;

    fn push(
        &self,
        batch: [T; BATCH_SIZE],
        index: usize,
    ) -> impl core::future::Future<Output = core::result::Result<(), PushError<[T; BATCH_SIZE]>>>;
}

/// Trait for popping batches from an N-SPSC distributor (consumer side)
pub trait NSPSCBDistributorPopper<const BATCH_SIZE: usize, T> {
    fn try_pop(&self) -> Result<[T; BATCH_SIZE], TryPopError>;

    fn pop(
        &self,
    ) -> impl core::future::Future<Output = Result<[T; BATCH_SIZE], PopError>>;
}

/// Trait for N-way Single Producer Single Consumer distributors
pub trait NSPSCBDistributor<const BATCH_SIZE: usize, T>: BDistributor<BATCH_SIZE, T> + 'static {
    type Pusher: NSPSCBDistributorPusher<{ BATCH_SIZE }, T> + 'static;
    type Popper: NSPSCBDistributorPopper<{ BATCH_SIZE }, T> + 'static;
    fn split(self, n: usize) -> (Self::Pusher, Vec<Self::Popper>);
}
