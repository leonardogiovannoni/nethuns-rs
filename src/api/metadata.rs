//! Metadata types for different backends.

#[cfg(feature = "af-xdp")]
use crate::af_xdp;
#[cfg(feature = "dpdk")]
use crate::dpdk;
#[cfg(feature = "netmap")]
use crate::netmap;

/// Trait for per-packet metadata from different backends.
pub trait Metadata: Send {
    /// Converts backend-specific metadata into the unified enum type.
    fn into_enum(self) -> MetadataType;
}

/// Unified enum containing metadata from all supported backends.
pub enum MetadataType {
    /// Metadata from netmap backend.
    #[cfg(feature = "netmap")]
    Netmap(netmap::Meta),
    /// Metadata from AF_XDP backend.
    #[cfg(feature = "af-xdp")]
    AfXdp(af_xdp::Meta),
    /// Metadata from DPDK backend.
    #[cfg(feature = "dpdk")]
    Dpdk(dpdk::Meta),
    /// Metadata from pcap backend.
    #[cfg(feature = "pcap")]
    Pcap(crate::pcap::Meta),
}
