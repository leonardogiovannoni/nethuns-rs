#[cfg(any(
    all(feature = "netmap", feature = "dpdk"),
    all(feature = "netmap", feature = "af_xdp"),
    all(feature = "netmap", feature = "pcap"),
    all(feature = "dpdk", feature = "af_xdp"),
    all(feature = "dpdk", feature = "pcap"),
    all(feature = "af_xdp", feature = "pcap"),
))]
compile_error!("Enable only one backend feature: netmap, dpdk, af_xdp, pcap.");

#[cfg(feature = "af_xdp")]
pub mod af_xdp;
pub mod api;
#[cfg(feature = "dpdk")]
pub mod dpdk;
pub mod errors;
pub mod unsafe_refcell;
#[cfg(feature = "netmap")]
pub mod netmap;
#[cfg(feature = "pcap")]
pub mod pcap;
