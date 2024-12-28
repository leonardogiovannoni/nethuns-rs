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
    fn create(dev: &str, opt: SocketOptions, queue: Option<usize>) -> Result<Self>;
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

pub enum Rings<P: PkthdrTrait> {
    RxOnly(NethunsRing<P>),
    TxOnly(NethunsRing<P>),
    RxTx(NethunsRing<P>, NethunsRing<P>),
}

impl Socket for NetmapSocket {
    type Pkthdr = NetmapPkthdr;
    fn create(dev: &str, opt: SocketOptions, queue: Option<usize>) -> Result<Self> {
        let make_ring: &dyn Fn(u32, u32) -> Result<NethunsRing<Self::Pkthdr>> =
            &|nslots, pktsize| todo!();
        let numblocks = opt.numblocks;
        let numpackets = opt.numpackets;

        let nslots = numblocks * numpackets;
        let pktsize = opt.packetsize;

        let rings = match opt.mode {
            SocketMode::RxTx => Rings::RxTx(
                make_ring(nslots, pktsize)?,
                make_ring(nslots, pktsize)?,
            ),
            SocketMode::RxOnly => Rings::RxOnly(make_ring(nslots, pktsize)?),
            SocketMode::TxOnly => Rings::TxOnly(make_ring(nslots, pktsize)?),
        };


        let prefix = if dev.starts_with("vale") {
            "netmap"
        } else {
            ""
        };

        let nm_dev = match queue {
            Some(q) => format!("{}{}-{}{}", prefix, dev, q, flags),
            None => format!("{}{}{}", prefix, dev, flags),
        };

        // TODO: assert_eq!(s->base.rx_ring.size, s->base.tx_ring.size)?
        let extra_bufs = match rings {
            Rings::RxTx(_, _) => 2 * nslots,
            _ => nslots,
        };
        let port = Port::create(&nm_dev, extra_bufs as u32)?;
        // open_desc(...); bail if p->reg.nr_extra_bufs != extra_bufs


        let some_ring = match rings {
            Rings::RxOnly(_) => port.first_rx_ring(),
            Rings::TxOnly(_) => port.first_tx_ring(),
            Rings::RxTx(_, _) => unimplemented!()
        };

        let mut scan = port.interface().ni_bufs_head;
        let mut free_ring = todo!();
        match &rings {
            Rings::RxOnly(rx_ring) => {
                /*
                while scan != 0 {
                    free_ring.push(scan);
                    scan = netmap_buf(scan);
                }
                */
            }
            Rings::TxOnly(rx_ring) => {
                /*
                for tx_slot in tx_ring.iter() {
                    tx_ring.pkhdr.buf_idx.store(scan, atomic::Ordering::Release);
                    scan = netmap_buf(scan);
                }

                */
            }
            Rings::RxTx(_, _) => {
                todo!()
            }
        }
        port.interface().ni_bufs_head = 0;
        let base = InnerSocket {
                  opt,
                  available_rings: rings,
                  queue,
                  devname: dev.to_string(),
                  ifindex:            todo!(), //unsafe { libc::if_nametoindex(self.base.devname.as_ptr()) } as _;
                  filter: Filter::Function(|_, _| true),
              };
        if opt.promisc {
            set_if_promisc(dev, true)?;
        }
        let res = NetmapSocket {
            base,
            port,
            some_ring,
            free_ring: todo!(),
        };
        std::thread::sleep(std::time::Duration::from_secs(2));
        Ok(res)
    }



}
