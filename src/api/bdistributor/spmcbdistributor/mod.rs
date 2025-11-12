// SPMC Backend Distributor traits
// Single Producer, Multiple Consumer distributor traits for the backend layer.
pub mod flume;
use super::BDistributor;

/// Trait for pushing batches to an SPMC distributor (producer side)
pub trait SPMCBDistributorPusher<const BATCH_SIZE: usize, T> {
    fn push(&self, batch: [T; BATCH_SIZE]) -> core::result::Result<(), [T; BATCH_SIZE]>;
}

/// Trait for popping batches from an SPMC distributor (consumer side)
pub trait SPMCBDistributorPopper<const BATCH_SIZE: usize, T>: Clone {
    fn pop(&self) -> Option<[T; BATCH_SIZE]>;
}

/// Combined trait for SPMC inner distributors that can be split into pusher/popper
pub trait SPMCBDistributor<const BATCH_SIZE: usize, T>:
    BDistributor<BATCH_SIZE, T> + 'static
{
    type Pusher: SPMCBDistributorPusher<{ BATCH_SIZE }, T> + 'static;
    type Popper: SPMCBDistributorPopper<{ BATCH_SIZE }, T> + 'static;

    fn split(self, n: usize) -> (Self::Pusher, Vec<Self::Popper>);
}
