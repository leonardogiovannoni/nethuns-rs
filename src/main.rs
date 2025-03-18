mod af_xdp;
mod api;
mod netmap;
mod strategy;
mod meter;
mod forward_mt;
mod forward;
mod dpdk;
use std::time::Duration;

use anyhow::{Result};
use api::Socket;
use strategy::{MpscArgs, MpscStrategy};



fn main() -> Result<()> {
    
    
    let dpdk_flags = dpdk::DpdkFlags {
        strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
    };
    let flags = api::Flags::DpdkFlags(dpdk_flags);
    let mut sock = dpdk::Sock::<MpscStrategy>::create("veth0", None, flags)?;
    
    loop {
        let Ok((pkt, _)) = sock.recv() else {
            continue;
        };
        println!("Received packet: {:?}", &*pkt);
        std::thread::sleep(Duration::from_secs(1));
    }
    Ok(())
}