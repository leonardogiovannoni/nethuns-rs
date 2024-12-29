use std::collections::VecDeque;

use crate::socket::{Filter, InnerSocket, NethunsRing, RingSlot, RING_SLOT_IN_USE};
use crate::types::{SocketMode, SocketOptions};
use anyhow::{bail, Result};
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
    fn send(&mut self, packet: &[u8]) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
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

impl<P: PkthdrTrait> Rings<P> {
    fn tx_ring(&self) -> Result<&NethunsRing<P>> {
        match self {
            Rings::TxOnly(tx_ring) => Ok(tx_ring),
            Rings::RxTx(_, tx_ring) => Ok(tx_ring),
            _ => Err(anyhow::anyhow!("Not a TxOnly ring")),
        }
    }

    fn rx_ring(&self) -> Result<&NethunsRing<P>> {
        match self {
            Rings::RxOnly(rx_ring) => Ok(rx_ring),
            Rings::RxTx(rx_ring, _) => Ok(rx_ring),
            _ => Err(anyhow::anyhow!("Not a RxOnly ring")),
        }
    }

    fn tx_ring_mut(&mut self) -> Result<&mut NethunsRing<P>> {
        match self {
            Rings::TxOnly(tx_ring) => Ok(tx_ring),
            Rings::RxTx(_, tx_ring) => Ok(tx_ring),
            _ => Err(anyhow::anyhow!("Not a TxOnly ring")),
        }
    }

    fn rx_ring_mut(&mut self) -> Result<&mut NethunsRing<P>> {
        match self {
            Rings::RxOnly(rx_ring) => Ok(rx_ring),
            Rings::RxTx(rx_ring, _) => Ok(rx_ring),
            _ => Err(anyhow::anyhow!("Not a RxOnly ring")),
        }
    }
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

    fn send(&mut self, packet: &[u8]) -> Result<()> {
        let tx = self.base.available_rings.tx_ring_mut()?;
        let slot = RingSlot {
            status: std::sync::atomic::AtomicU32::new(0),
            len: 0,
            packet_header: NetmapPkthdr {
                ts: todo!(),
                len: packet.len() as u32,
                caplen: packet.len() as u32,
                buf_idx: 0,
            },
            id: todo!(),
            len: todo!(),
            packet: Vec::new(),
        };

        slot.status.store(RING_SLOT_IN_USE, std::sync::atomic::Ordering::Release);

        let buf_idx = slot.packet_header().buf_idx as usize;
        let dst = nethuns_get_buf_addr_netmap(self.some_ring, buf_idx);
        *dst = packet;
        slot.len = packet.len();
        tx.push(slot);
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        let tx_ring = self.base.available_rings.tx_ring_mut()?;
        let mut prev_tails = Vec::new();
        for nm_tx_ring in self.port.tx_rings() {
            prev_tails.push(nm_tx_ring.tail);

            loop {
                if nm_tx_ring.is_empty() {
                    break;
                }
                let Some(slot) = tx_ring.pop() else {
                    // TODO: handle the case
                    panic!();
                };
                if slot.status.load(std::sync::atomic::Ordering::Acquire) != RING_SLOT_IN_USE {
                    break;
                }
                slot.status.store(RING_SLOT_IN_FLIGHT, std::sync::atomic::Ordering::Relaxed);

                let mut netmap_slot = todo!("get head slot");
            }

        }

        // let tmp = self.port.
        Ok(())
    }

}

fn nethuns_get_buf_addr_netmap(some_ring: usize, buf_index: usize) -> ! {
    // some_ring.nr_buf_size
    netmap_buf(some_ring, buf_index);
    todo!()
}
