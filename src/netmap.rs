use std::collections::VecDeque;

use crate::socket::{Filter, InnerSocket, NethunsRing};
use crate::types::{SocketMode, SocketOptions};
use anyhow::Result;
use netmap_rs::port::Port;
pub trait PkthdrTrait {}
pub struct NetmapPkthdr {
    ts: !,
    len: u32,
    caplen: u32,
    buf_idx: u32,
}

impl PkthdrTrait for NetmapPkthdr {}

pub trait Socket: Sized {
    type Pkthdr: PkthdrTrait;
    fn create(opts: SocketOptions) -> Result<Self>;
}

/*

struct nethuns_socket_netmap
{
    struct nethuns_socket_base base;
    struct nmport_d *p;
    struct netmap_ring *some_ring;
    uint32_t *free_ring;
    uint64_t free_mask;
    uint64_t free_head;
    uint64_t free_tail;
    bool tx;
    bool rx;
};*/

/*
#[derive(Debug)]
pub struct NethunsSocketNetmap {
    base: NethunsSocketBase,
    p: NmPortDescriptor,
    some_ring: NetmapRing,
    free_ring: CircularQueue<u32>,
}
*/

pub type CircularQueue = !;
pub struct NetmapSocket {
    base: InnerSocket<NetmapPkthdr>,
    port: Port,
    // keep track of the ring using the index
    some_ring: usize,
    free_ring: CircularQueue, /*<u32>*/
}

impl Socket for NetmapSocket {
    type Pkthdr = NetmapPkthdr;
    fn create(opts: SocketOptions) -> Result<Self> {
        //  Ok(NetmapSocket {})
        let make_ring: &dyn Fn(u32, u32) -> Result<NethunsRing<Self::Pkthdr>> =
            &|nslots, pktsize| todo!();
        let numblocks = opts.numblocks;
        let numpackets = opts.numpackets;
        let pktsize = opts.packetsize;
        let (rx, tx) = match opts.mode {
            SocketMode::RxTx => (Some(make_ring(numblocks * numpackets, pktsize)?), Some(make_ring(numblocks * numpackets, pktsize)?)),
            SocketMode::RxOnly => (Some(make_ring(numblocks * numpackets, pktsize)?), None),
            SocketMode::TxOnly => (None, Some(make_ring(numblocks * numpackets, pktsize)?)),
        };
        let port = Port::create(todo!())?;
        let base = InnerSocket {
            opt: opts,
            tx_ring: tx,
            rx_ring: rx,
            // devname: opts.devname.clone(),
            // queue: opts.queue,
            // ifindex: opts.ifindex,
            filter: Filter::Function(|_, _| true),
        };

        Ok(NetmapSocket {
            base,
            port,
            some_ring: 0,
            free_ring: todo!(),
        })

    }
}
