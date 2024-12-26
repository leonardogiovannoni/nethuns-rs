

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


pub struct SocketOptions {
    numblocks: u32,
    numpackets: u32,
    packetsize: u32,
    timeout_ms: u32,
    dir: CaptureDir,
    capture: CaptureMode,
    mode: SocketMode,
    timestamp: bool,
    promisc: bool,
    rxhash: bool,
    tx_qdisc_bypass: bool,
    xdp_prog: Option<String>,
    xdp_prog_sec: Option<String>,
    xsk_map_name: Option<String>,
    reuse_maps: bool,
    pin_dir: Option<String>,
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
