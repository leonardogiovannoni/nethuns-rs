// Backend Distributor logic for nethuns-rs
// This module contains all the backend (inner) traits and implementations for distributing packets
// across multiple consumers.

pub mod spmcbdistributor;
pub mod nspscbdistributor;

// Re-export SPMC traits
//pub use spmcbdistributor::{SPMCBDistributor, SPMCBDistributorPopper, SPMCBDistributorPusher};

// Re-export N-SPSC traits
// pub use nspscbdistributor::{NSPSCBDistributor, NSPSCBDistributorPopper, NSPSCBDistributorPusher};

// =============================================================================
// Base Backend Distributor Trait
// =============================================================================

/// Base trait for inner distributors that work with the backend
pub trait BDistributor<const N: usize, T>: Send + Sized {
    fn create(inner: Self) -> Self;
    // fn get_ctx(&self) -> &Ctx;
}
