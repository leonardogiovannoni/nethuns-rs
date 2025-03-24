mod af_xdp;
mod api;
mod dpdk;
mod fake_refcell;
mod forward;
mod forward_mt;
mod meter;
mod netmap;
mod strategy;
use std::{io::BufRead, time::Duration};

use anyhow::Result;
use api::{Flags, Socket};
use dpdk::DpdkFlags;
use strategy::{MpscArgs, MpscStrategy}; 

fn main2() -> Result<()> {
    let mut socket0 = dpdk::Sock::<MpscStrategy>::create(
        "veth0",
        Some(0),
        Flags::DpdkFlags(dpdk::DpdkFlags {
            strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
            num_mbufs: 8192,
            mbuf_cache_size: 250,
            mbuf_default_buf_size: 2176,
        }),
    )
    .unwrap();
    let mut socket1 = dpdk::Sock::<MpscStrategy>::create(
        "veth1",
        Some(0),
        Flags::DpdkFlags(DpdkFlags {
            strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
            num_mbufs: 8192,
            mbuf_cache_size: 250,
            mbuf_default_buf_size: 2176,
        }),
    )
    .unwrap();
    socket0.send(b"Helloworldmyfriend\0\0\0\0\0\0\0").unwrap();
    socket0.flush();

    let (payload, meta) = socket1.recv()?;
    assert_eq!(&payload[..20], b"Helloworldmyfriend\0\0");
    println!("Received packet: {:?}", &*payload);
    Ok(())
}

fn main3() -> Result<()> {
    let mut socket0 = af_xdp::Sock::<MpscStrategy>::create(
        "veth0",
        Some(0),
        Flags::AfXdp(af_xdp::AfXdpFlags {
            xdp_flags: 0,
            bind_flags: 0,
            strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
            num_frames: 4096,
            frame_size: 2048,
        }),
    )
    .unwrap();
    let mut socket1 = af_xdp::Sock::<MpscStrategy>::create(
        "veth1",
        Some(0),
        Flags::AfXdp(af_xdp::AfXdpFlags {
            xdp_flags: 0,
            bind_flags: 0,
            strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
            num_frames: 4096,
            frame_size: 2048,
        }),
    )
    .unwrap();
    socket1.send(b"Helloworldmyfriend\0\0\0\0\0\0\0").unwrap();
    socket1.flush();
    let (packet, meta) = socket0.recv().unwrap();
    assert_eq!(&packet[..20], b"Helloworldmyfriend\0\0");
    Ok(())
}

fn main() {
    meter::routine().unwrap();
}
