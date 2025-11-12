// N-SPSC Backend Distributor traits
// N-way Single Producer, Single Consumer distributor traits for the backend layer.

pub mod ringbuf;
use super::BDistributor;

/// Trait for pushing batches to an N-SPSC distributor (producer side)
/// The index parameter specifies which consumer queue to push to.
pub trait NSPSCBDistributorPusher<const BATCH_SIZE: usize, T> {
    fn push(&self, batch: [T; BATCH_SIZE], index: usize) -> core::result::Result<(), [T; BATCH_SIZE]>;
}

/// Trait for popping batches from an N-SPSC distributor (consumer side)
pub trait NSPSCBDistributorPopper<const BATCH_SIZE: usize, T> {
    fn pop(&self) -> Option<[T; BATCH_SIZE]>;
}

/// Trait for N-way Single Producer Single Consumer distributors
pub trait NSPSCBDistributor<const BATCH_SIZE: usize, T>: BDistributor<BATCH_SIZE, T> + 'static {
    type Pusher: NSPSCBDistributorPusher<{ BATCH_SIZE }, T> + 'static;
    type Popper: NSPSCBDistributorPopper<{ BATCH_SIZE }, T> + 'static;
    fn split(self, n: usize) -> (Self::Pusher, Vec<Self::Popper>);
}
