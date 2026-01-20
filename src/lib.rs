//! # nethuns-rs
//!
//! A high-performance packet I/O library for Rust, providing a unified interface
//! across multiple networking backends.
//!
//! ## Supported Backends
//!
//! - **AF_XDP** - Linux kernel XDP socket support (feature: `af-xdp`)
//! - **netmap** - High-speed packet I/O framework (feature: `netmap`)
//! - **DPDK** - Data Plane Development Kit (feature: `dpdk`)
//! - **pcap** - libpcap-based capture/injection (feature: `pcap`, enabled by default)
//!
//! ## Quick Start
//!
//! ```ignore
//! use nethuns_rs::api::{Socket, Context};
//! // Choose your backend (e.g., pcap)
//! use nethuns_rs::pcap::{Sock, PcapFlags};
//!
//! let socket = Sock::create("eth0", None, PcapFlags::default())?;
//! let (packet, meta) = socket.recv()?;
//! println!("Received {} bytes", packet.len());
//! ```

// Backend modules (conditionally compiled)
#[cfg(feature = "af-xdp")]
pub mod af_xdp;
#[cfg(feature = "dpdk")]
pub mod dpdk;
#[cfg(feature = "netmap")]
pub mod netmap;
#[cfg(feature = "pcap")]
pub mod pcap;

// Core API
pub mod api;

// Internal utilities
pub mod errors;
pub(crate) mod unsafe_refcell;
