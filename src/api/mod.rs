//! Public API for the nethuns-rs networking library.
//!
//! This module provides a unified interface for high-performance packet I/O
//! across multiple backends (AF_XDP, netmap, DPDK, pcap).
//!
//! # Main Types
//!
//! - [`Socket`] - The main trait for network I/O operations
//! - [`Context`] - Manages buffer pools for zero-copy packet access
//! - [`Token`] - Represents ownership of a packet buffer
//! - [`Payload`] - Smart pointer to packet data with automatic cleanup
//!
//! # Example
//!
//! ```ignore
//! use nethuns_rs::api::{Socket, Context};
//!
//! let socket = SomeSocket::create("eth0", Some(0), flags)?;
//! let (payload, meta) = socket.recv()?;
//! println!("Received {} bytes", payload.len());
//! ```

mod buffer;
mod context;
mod hint;
mod metadata;
mod socket;
mod token;

// Re-export all public types
pub use buffer::{BufferDesc, BufferRef};
pub use context::Context;
pub use hint::{likely, unlikely};
pub use metadata::{Metadata, MetadataType};
pub use socket::{Flags, Socket};
pub use token::{Payload, Token};

/// Result type for API operations.
pub type Result<T> = std::result::Result<T, crate::errors::Error>;
