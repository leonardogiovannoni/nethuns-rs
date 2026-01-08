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
}

/// An error returned from popping operations.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum TryPopError {
    /// The channel is empty but not closed.
    Empty,
    /// The channel is empty and closed.
    Closed,
}

impl TryPopError {
    pub fn is_empty(&self) -> bool {
        matches!(self, TryPopError::Empty)
    }

    pub fn is_closed(&self) -> bool {
        matches!(self, TryPopError::Closed)
    }
}

impl std::fmt::Display for TryPopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            TryPopError::Empty => write!(f, "popping from an empty channel"),
            TryPopError::Closed => write!(f, "popping from an empty and closed channel"),
        }
    }
}

impl std::error::Error for TryPopError {}

/// An error returned from async popping operations.
/// Returned when the channel is empty and closed.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct PopError;

impl std::fmt::Display for PopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "async popping from an empty and closed channel")
    }
}

impl std::error::Error for PopError {}

/// An error returned from async pushing operations.
/// Contains the batch that couldn't be pushed.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct PushError<T>(pub T);

impl<T> PushError<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::fmt::Display for PushError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "async pushing into a closed channel")
    }
}

impl<T: std::fmt::Debug> std::error::Error for PushError<T> {}

/// An error returned from non-async pushing operations.
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum TryPushError<T> {
    /// The channel is full but not closed.
    Full(T),
    /// The channel is closed.
    Closed(T),
}

impl<T> TryPushError<T> {
    pub fn into_inner(self) -> T {
        match self {
            TryPushError::Full(batch) => batch,
            TryPushError::Closed(batch) => batch,
        }
    }

    pub fn is_full(&self) -> bool {
        matches!(self, TryPushError::Full(_))
    }

    pub fn is_closed(&self) -> bool {
        matches!(self, TryPushError::Closed(_))
    }
}

impl<T> std::fmt::Display for TryPushError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            TryPushError::Full(_) => write!(f, "pushing into a full channel"),
            TryPushError::Closed(_) => write!(f, "pushing into a closed channel"),
        }
    }
}

impl<T: std::fmt::Debug> std::error::Error for TryPushError<T> {}
