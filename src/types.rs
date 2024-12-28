use std::path::Display;



pub enum CaptureDir {
    In,
    Out,
    InOut,
}

pub enum CaptureMode {
    Default,
    SkbMode,
    DrvMode,
    ZeroCopy,
}


pub enum SocketMode {
    RxTx,
    RxOnly,
    TxOnly,
}


impl Display for SocketMode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SocketMode::RxTx => write!(f, ""),
            SocketMode::RxOnly => write!(f, "/R"),
            SocketMode::TxOnly => write!(f, "/T"),
        }
    }
}


pub struct SocketOptions {
    pub numblocks: u32,
    pub numpackets: u32,
    pub packetsize: u32,
    pub timeout_ms: u32,
    pub dir: CaptureDir,
    pub capture: CaptureMode,
    pub mode: SocketMode,
    pub timestamp: bool,
    pub promisc: bool,
    pub rxhash: bool,
    pub tx_qdisc_bypass: bool,
    pub xdp_prog: Option<String>,
    pub xdp_prog_sec: Option<String>,
    pub xsk_map_name: Option<String>,
    pub reuse_maps: bool,
    pub pin_dir: Option<String>,
}



pub struct Stat {
    rx_packets: u64,
    tx_packets: u64,
    rx_dropped: u64,
    rx_if_dropped: u64,
    rx_invalid: u64,
    tx_invalid: u64,
    freeze: u64,
}
