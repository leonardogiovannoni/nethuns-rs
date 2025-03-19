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
use crate::api::Socket;
const RX_RING_SIZE: u16 = 1024;
const NUM_MBUFS: u32 = 8192;
const MBUF_CACHE_SIZE: u32 = 250;
const BURST_SIZE: u16 = 32;
const CUSTOM_ETHER_TYPE: u16 = 0x88B5;
const PORT_ID: u16 = 0;

type Filter = ();

#[derive(Clone)]
pub struct Ctx<S: api::Strategy> {
    buffer_pool: Arc<UnsafeCell<BufferPool>>,
    producer: RefCell<S::Producer>,
    index: usize,
}

// (* Temporary

unsafe impl<S: api::Strategy> Send for Ctx<S> {}
unsafe impl<S: api::Strategy> Sync for Ctx<S> {}

// Temporary *)

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
    fn new(buffer_pool: BufferPool, strategy_args: api::StrategyArgs) -> (Self, S::Consumer) {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let (mut producer, cons) = S::create(strategy_args);
        let res = Self {
            buffer_pool: Arc::new(UnsafeCell::new(buffer_pool)),
            producer: RefCell::new(producer),
            index: COUNTER.fetch_add(1, Ordering::SeqCst),
        };
        (res, cons)
    }

    fn free(&mut self, token: api::BufferIndex) {
        let token: usize = token.into();
        let ptr = token as *mut rte_mbuf;
        unsafe { (*self.buffer_pool.get()).free(ptr) };
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
    fn clone(&self) -> Self {
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

// (* Temporary

unsafe impl<S: api::Strategy> Send for Sock<S> {}

// Temporary *)

pub struct Meta {}

impl api::Metadata for Meta {}

fn init_data(m: *mut rte_mbuf, len: u16) -> *mut u8 {
    unsafe {
        (*m).__bindgen_anon_1.__bindgen_anon_1.data_off = 0;
        if len > (*m).__bindgen_anon_2.__bindgen_anon_1.buf_len {
            return std::ptr::null_mut();
        }
        (*m).__bindgen_anon_2.__bindgen_anon_1.data_len += len;
        (*m).__bindgen_anon_2.__bindgen_anon_1.pkt_len += len as u32;
        (*m).buf_addr as *mut u8
    }
}

impl<S: api::Strategy> Sock<S> {
    fn flush_to_memory_pool(&mut self) {
        let mut consumer = self.consumer.borrow_mut();
        loop {
            let mut b = false;
            while let Some(val) = consumer.pop() {
                self.ctx.free(val);
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

    fn create(portspec: &str, filter: Option<()>, flags: api::Flags) -> anyhow::Result<Self> {
        let flags = match flags {
            api::Flags::DpdkFlags(flags) => flags,
            _ => panic!("Invalid flags"),
        };

        let (buffer_pool, rx, tx) = Context::new(
            portspec,
            NUM_MBUFS,
            MBUF_CACHE_SIZE,
            RTE_MBUF_DEFAULT_BUF_SIZE as u16,
            PORT_ID,
            0,
        )?;

        let (ctx, consumer) = Ctx::new(buffer_pool, flags.strategy_args);
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

fn pippo() {
    // Convert command-line arguments into C-style strings.
    // let args: Vec<String> = env::args().collect();
    //
    // // Note: In the original C code argc/argv are adjusted, but we do not need that in Rust.
    //
    // // Create the mbuf pool.
    // let pool_name = CString::new("MBUF_POOL").unwrap();
    // let mbuf_pool = rte_pktmbuf_pool_create(
    //     pool_name.as_ptr(),
    //     NUM_MBUFS,
    //     MBUF_CACHE_SIZE,        let data = unsafe { (*ptr).__bindgen_anon_2.__bindgen_anon_1.buf_addr as *mut u8 };

    //     0,
    //     RTE_MBUF_DEFAULT_BUF_SIZE as u16,
    //     rte_socket_id() as i32,
    // );
    // if mbuf_pool.is_null() {
    //     rte_exit(
    //         EXIT_FAILURE as i32,
    //         CString::new("Cannot create mbuf pool\n").unwrap().as_ptr(),
    //     );
    // }
    //
    // // Initialize the port.
    // init_port(PORT_ID, mbuf_pool);

    let (buffer_pool, mut receiver, trasmitter) = Context::new(
        "veth0",
        NUM_MBUFS,
        MBUF_CACHE_SIZE,
        RTE_MBUF_DEFAULT_BUF_SIZE as u16,
        PORT_ID,
        0,
    )
    .unwrap();
    // Allocate an array for burst packet reception.

    loop {
        if let Some(mbuf) = receiver.iter_mut().next() {
            let eth_hdr = unsafe { rust_rte_pktmbuf_mtod(mbuf.as_ptr()) as *mut rte_ether_hdr };
            println!("{}", mbuf.len());
            let ether_type = u16::from_be(unsafe { (*eth_hdr).ether_type });
            if ether_type == CUSTOM_ETHER_TYPE {
                let payload_ptr = unsafe { eth_hdr.offset(1) as *mut u8 };
                let payload_len = unsafe {
                    (*mbuf.as_ptr()).__bindgen_anon_2.__bindgen_anon_1.data_len as usize
                        - mem::size_of::<rte_ether_hdr>()
                };
                let copy_len = if payload_len < 1023 {
                    payload_len
                } else {
                    1023
                };
                let payload_slice = unsafe { slice::from_raw_parts(payload_ptr, copy_len) };
                let received = String::from_utf8_lossy(payload_slice);
                println!("Received: {}", received);
            }
        }
    }
    // Cleanup (never reached in this infinite loop)
}

#[derive(Clone, Debug)]
pub struct DpdkFlags {
    pub strategy_args: api::StrategyArgs,
}


/*#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::{self, Flags, Socket},
        netmap::Sock,
        strategy::{MpscArgs, MpscStrategy},
    };

    #[test]
    fn test_send_with_flush() {
        let mut socket0 = Sock::<MpscStrategy>::create(
            "vale0:0",
            None,
            Flags::Netmap(NetmapFlags {
                extra_buf: 1024,
                strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
            }),
        )
        .unwrap();
        let mut socket1 = Sock::<MpscStrategy>::create(
            "vale0:1",
            None,
            Flags::Netmap(NetmapFlags {
                extra_buf: 1024,
                strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
            }),
        )
        .unwrap();
        socket1.send(b"Helloworldmyfriend\0\0\0\0\0\0\0").unwrap();
        socket1.flush();
        let (packet, meta) = socket0.recv().unwrap();
        assert_eq!(&packet[..20], b"Helloworldmyfriend\0\0");
    }
}
 */

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
            "veth0",
            None,
            Flags::DpdkFlags(DpdkFlags {
                strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
            }),
        )
        .unwrap();
        let mut socket1 = Sock::<MpscStrategy>::create(
            "veth1",
            None,
            Flags::DpdkFlags(DpdkFlags {
                strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
            }),
        )
        .unwrap();
        socket1.send(b"Helloworldmyfriend\0\0\0\0\0\0\0").unwrap();
        socket1.flush();
        let (packet, meta) = socket0.recv().unwrap();
        assert_eq!(&packet[..20], b"Helloworldmyfriend\0\0");
    }
}