mod wrapper;
use anyhow::{Result, bail};
use dpdk_sys::*;
use etherparse::err::packet;
use std::cell::RefCell;
use std::cell::UnsafeCell;
use std::mem;
use std::mem::ManuallyDrop;
use std::slice;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use wrapper::BufferPool;
use wrapper::Context;
use wrapper::Receiver;
use wrapper::RteMBuf;
use wrapper::RteMBufRef;
use wrapper::Transmitter;

use crate::api;
use crate::api::BufferConsumer;
use crate::api::BufferProducer;
const RX_RING_SIZE: u16 = 1024;
const NUM_MBUFS: u32 = 8192;
const MBUF_CACHE_SIZE: u32 = 250;
const BURST_SIZE: u16 = 32;
const CUSTOM_ETHER_TYPE: u16 = 0x88B5;
const PORT_ID: u16 = 0;

type Filter = ();

#[derive(Clone)]
pub struct Ctx<S: api::Strategy> {
    producer: RefCell<S::Producer>,
    index: usize,
}

impl<S: api::Strategy> Ctx<S> {
    unsafe fn buffer(&self, idx: api::BufferIndex) -> *mut [u8] {
        let idx: usize = idx.into();
        let ptr = idx as *mut rte_mbuf;
        // TODO ensure only one segment, i.e. data_len == pkt_len
        let len = unsafe { (*ptr).__bindgen_anon_2.__bindgen_anon_1.data_len as usize };
        let data = unsafe { rust_rte_pktmbuf_mtod(ptr) as *mut u8 };
        unsafe { slice::from_raw_parts_mut(data, len) }
    }
}

impl<S: api::Strategy> Ctx<S> {
    fn new(nbufs: usize, strategy_args: api::StrategyArgs) -> (Self, S::Consumer) {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let (producer, cons) = S::create(nbufs, strategy_args);
        let res = Self {
            producer: RefCell::new(producer),
            index: COUNTER.fetch_add(1, Ordering::SeqCst),
        };
        (res, cons)
    }
}

pub struct Tok<S: api::Strategy> {
    idx: api::BufferIndex,
    pool_id: usize,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: api::Strategy> Tok<S> {
    fn new(idx: api::BufferIndex, pool_id: usize) -> ManuallyDrop<Self> {
        ManuallyDrop::new(Self {
            idx,
            pool_id,
            _phantom: std::marker::PhantomData,
        })
    }
}

impl<S: api::Strategy> api::Token for Tok<S> {
    type Context = Ctx<S>;

    fn buffer_idx(&self) -> api::BufferIndex {
        self.idx
    }

    fn size(&self) -> usize {
        let idx: usize = self.idx.into();
        let ptr = idx as *mut rte_mbuf;
        unsafe { (*ptr).__bindgen_anon_2.__bindgen_anon_1.data_len as usize }
    }

    fn pool_id(&self) -> usize {
        self.pool_id
    }
}

impl<S: api::Strategy> api::TokenExt for Tok<S> {
    fn duplicate(&self) -> Self {
        Self {
            idx: self.idx,
            pool_id: self.pool_id,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<S: api::Strategy> api::Context for Ctx<S> {
    type Token = Tok<S>;

    fn pool_id(&self) -> usize {
        self.index
    }

    unsafe fn unsafe_buffer(&self, buf_idx: api::BufferIndex, size: usize) -> *mut [u8] {
        unsafe { Ctx::buffer(self, buf_idx) }
    }

    fn release(&self, buf_idx: api::BufferIndex) {
        self.producer.borrow_mut().push(buf_idx);
    }
}

pub struct Sock<S: api::Strategy> {
    tx: RefCell<Transmitter>,
    rx: RefCell<Receiver>,
    ctx: Ctx<S>,
    consumer: RefCell<S::Consumer>,
    filter: Option<Filter>,
}

pub struct Meta {}

impl api::Metadata for Meta {}

impl<S: api::Strategy> Sock<S> {
    fn flush_to_memory_pool(&mut self) {
        let mut consumer = self.consumer.borrow_mut();
        loop {
            let mut b = false;
            while let Some(val) = consumer.pop() {
                // self.ctx.free(val);
                let ptr = usize::from(val) as *mut rte_mbuf;

                unsafe { rust_rte_pktmbuf_free(ptr) };
                b = true;
            }
            if !b {
                break;
            }

            consumer.sync();
        }
    }

    fn recv_inner(&self, buf: RteMBuf) -> Result<(Tok<S>, Meta)> {
        let token = buf.as_ptr() as usize;
        let token = api::BufferIndex::from(token);
        let token = Tok::new(token, api::Context::pool_id(&self.ctx));

        // TODO: filter stuff..

        Ok((ManuallyDrop::into_inner(token), Meta {}))
    }

    fn send_inner(&self, scan: RteMBufRef, packet: &[u8]) -> Result<()> {
        let m = scan.as_ptr().as_ptr();
        let len = packet.len() as u16;
        let buf = unsafe {
            if len > (*m).__bindgen_anon_2.__bindgen_anon_1.buf_len {
                bail!("Packet too big");
            }
            (*m).__bindgen_anon_1.__bindgen_anon_1.data_off = 0;
            (*m).__bindgen_anon_2.__bindgen_anon_1.data_len += len;
            (*m).__bindgen_anon_2.__bindgen_anon_1.pkt_len += len as u32;
            (*m).buf_addr as *mut u8
        };
        let slice_mut = unsafe { slice::from_raw_parts_mut(buf, len as usize) };
        slice_mut.copy_from_slice(packet);
        Ok(())
    }
}

impl<S: api::Strategy> api::Socket<S> for Sock<S> {
    type Context = Ctx<S>;
    type Metadata = Meta;

    fn recv_token(
        &mut self,
    ) -> anyhow::Result<(<Self::Context as api::Context>::Token, Self::Metadata)> {
        if let Some(tmp) = self.rx.borrow_mut().iter_mut().next() {
            self.recv_inner(tmp)
        } else {
            self.flush_to_memory_pool();
            let mut rx = self.rx.borrow_mut();
            let tmp = rx
                .iter_mut()
                .next()
                .ok_or_else(|| anyhow::anyhow!("No packets"))?;
            self.recv_inner(tmp)
        }
    }

    fn send(&mut self, packet: &[u8]) -> anyhow::Result<()> {
        if let Some(scan) = self.tx.borrow_mut().iter_mut().next() {
            self.send_inner(scan, packet)
        } else {
            self.flush_to_memory_pool();
            let mut tx = self.tx.borrow_mut();
            let scan = tx
                .iter_mut()
                .next()
                .ok_or_else(|| anyhow::anyhow!("No packets"))?;
            self.send_inner(scan, packet)
        }
    }

    fn flush(&mut self) {
        self.tx.borrow_mut().flush();
    }

    fn create(portspec: &str, queue: Option<usize>, filter: Option<()>, flags: api::Flags) -> anyhow::Result<Self> {
        let flags = match flags {
            api::Flags::DpdkFlags(flags) => flags,
            _ => panic!("Invalid flags"),
        };

        let (_, rx, tx) = Context::new(
            portspec,
            flags.num_mbufs,
            flags.mbuf_cache_size,
            flags.mbuf_default_buf_size,
            queue.unwrap_or(0) as u16, 
        )?;

        let (ctx, consumer) = Ctx::new(flags.num_mbufs as usize, flags.strategy_args);
        Ok(Self {
            tx: RefCell::new(tx),
            rx: RefCell::new(rx),
            ctx,
            consumer: RefCell::new(consumer),
            filter,
        })
    }

    fn context(&self) -> &Self::Context {
        &self.ctx
    }
}

#[derive(Clone, Debug)]
pub struct DpdkFlags {
    pub strategy_args: api::StrategyArgs,
    pub num_mbufs: u32,
    pub mbuf_cache_size: u32,
    pub mbuf_default_buf_size: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::{self, Flags, Socket},
        strategy::{MpscArgs, MpscStrategy},
    };

    #[test]
    fn test_send_with_flush() {
        let mut socket0 = Sock::<MpscStrategy>::create(
            "veth0dpdk",
            Some(0),
            None,
            Flags::DpdkFlags(DpdkFlags {
                strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
                num_mbufs: 8192,
                mbuf_cache_size: 250,
                mbuf_default_buf_size: 2176,
            }),
        )
        .unwrap();
        let mut socket1 = Sock::<MpscStrategy>::create(
            "veth1dpdk",
            Some(0),
            None,
            Flags::DpdkFlags(DpdkFlags {
                strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
                num_mbufs: 8192,
                mbuf_cache_size: 250,
                mbuf_default_buf_size: 2176,
            }),
        )
        .unwrap();
        socket1.send(b"Helloworldmyfriend\0\0\0\0\0\0\0").unwrap();
        socket1.flush();
        let (packet, meta) = socket0.recv().unwrap();
        assert_eq!(&packet[..20], b"Helloworldmyfriend\0\0");
    }
}
