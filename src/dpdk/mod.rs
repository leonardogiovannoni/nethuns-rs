mod wrapper;
use crate::api;
use crate::api::Result;
use crate::errors::Error;
use dpdk_sys::*;
use std::mem::ManuallyDrop;
use std::slice;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use wrapper::Context;
use wrapper::Receiver;
use wrapper::RteMBuf;
use wrapper::RteMBufRef;
use wrapper::Transmitter;

type RefCell<T> = crate::fake_refcell::FakeRefCell<T>;

#[derive(Clone)]
pub struct Ctx {
    producer: RefCell<mpsc::Producer<api::BufferRef>>,
    index: u32,
}

impl Ctx {
    unsafe fn buffer(&self, idx: api::BufferRef) -> *mut [u8] {
        let idx: usize = idx.into();
        let ptr = idx as *mut rte_mbuf;
        // TODO ensure only one segment, i.e. data_len == pkt_len
        let len = unsafe { (*ptr).__bindgen_anon_2.__bindgen_anon_1.data_len as usize };
        let data = unsafe { rust_rte_pktmbuf_mtod(ptr) as *mut u8 };
        unsafe { slice::from_raw_parts_mut(data, len) }
    }
}

impl Ctx {
    fn new(nbufs: usize) -> (Self, mpsc::Consumer<api::BufferRef>) {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let (producer, cons) = mpsc::channel(nbufs);
        // S::create(nbufs, strategy_args);
        let res = Self {
            producer: RefCell::new(producer),
            index: COUNTER.fetch_add(1, Ordering::SeqCst),
        };
        (res, cons)
    }
}

pub struct Tok {
    idx: api::BufferRef,
    pool_id: u32,
}

impl Tok {
    fn new(idx: api::BufferRef, pool_id: u32) -> ManuallyDrop<Self> {
        ManuallyDrop::new(Self { idx, pool_id })
    }
}

impl api::Token for Tok {
    type Context = Ctx;

    fn buffer_idx(&self) -> api::BufferRef {
        self.idx
    }

    fn size(&self) -> u32 {
        let idx: usize = self.idx.into();
        let ptr = idx as *mut rte_mbuf;
        unsafe { (*ptr).__bindgen_anon_2.__bindgen_anon_1.data_len as u32 }
    }

    fn pool_id(&self) -> u32 {
        self.pool_id
    }
}

//impl api::TokenExt for Tok {
//    fn duplicate(&self) -> Self {
//        Self {
//            idx: self.idx,
//            pool_id: self.pool_id,
//            _phantom: std::marker::PhantomData,
//        }
//    }
//}

impl api::Context for Ctx {
    type Token = Tok;

    fn pool_id(&self) -> u32 {
        self.index
    }

    unsafe fn unsafe_buffer(&self, buf_idx: api::BufferRef, _size: usize) -> *mut [u8] {
        unsafe { Ctx::buffer(self, buf_idx) }
    }

    fn release(&self, buf_idx: api::BufferRef) {
        self.producer.borrow_mut().push(buf_idx);
    }
}

pub struct Sock {
    tx: RefCell<Transmitter>,
    rx: RefCell<Receiver>,
    ctx: Ctx,
    consumer: RefCell<mpsc::Consumer<api::BufferRef>>,
}

pub struct Meta {}

impl api::Metadata for Meta {}

impl Sock {
    //#[inline(never)]
    //#[cold]
    fn flush_to_memory_pool(&mut self) {
        let mut consumer = self.consumer.borrow_mut();
        consumer.sync();
        let buf = &mut consumer.cached;
        unsafe { rust_rte_pktmbuf_free_bulk(buf.as_mut_ptr() as *mut _, buf.len() as u32) };
        buf.clear();
    }

    #[inline(always)]
    fn recv_inner(&self, buf: RteMBuf) -> Result<(Tok, Meta)> {
        let buf = ManuallyDrop::new(buf);
        let token = buf.as_ptr() as usize;
        let token = api::BufferRef::from(token);
        let token = Tok::new(token, api::Context::pool_id(&self.ctx));
        let meta = Meta {};
        Ok((ManuallyDrop::into_inner(token), Meta {}))
    }

    fn send_inner(&self, scan: RteMBufRef, packet: &[u8]) -> Result<()> {
        let m = scan.as_ptr().as_ptr();
        let len = packet.len() as u16;
        let buf = unsafe {
            if len > (*m).__bindgen_anon_2.__bindgen_anon_1.buf_len {
                return Err(Error::TooBigPacket(len as usize));
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

impl api::Socket for Sock {
    type Context = Ctx;
    type Metadata = Meta;
    type Flags = DpdkFlags;

    fn recv_token(&mut self) -> Result<(<Self::Context as api::Context>::Token, Self::Metadata)> {
        if let Some(tmp) = self.rx.borrow_mut().iter_mut().next() {
            self.recv_inner(tmp)
        } else {
            self.flush_to_memory_pool();
            let mut rx = self.rx.borrow_mut();
            //let mut consumer = self.consumer.borrow_mut();
            let tmp = rx.iter_mut().next().ok_or(Error::NoPacket)?;
            self.recv_inner(tmp)
        }
    }

    fn send(&mut self, packet: &[u8]) -> Result<()> {
        let mut tx = self.tx.borrow_mut();
        let scan = tx.iter_mut().next().ok_or(Error::NoPacket)?;
        self.send_inner(scan, packet)
    }

    fn flush(&mut self) {
        self.tx.borrow_mut().flush();
    }

    fn create(portspec: &str, queue: Option<usize>, flags: Self::Flags) -> Result<Self> {
        let (mut buffer_pool, rx, tx) = Context::new(
            portspec,
            flags.num_mbufs,
            flags.mbuf_cache_size,
            flags.mbuf_default_buf_size,
            queue.unwrap_or(0) as u16,
        )?;

        let (ctx, consumer) = Ctx::new(flags.num_mbufs as usize);
        loop {
            let tmp = buffer_pool.allocate();
            if tmp.is_null() {
                break;
            }
            assert!(!tmp.is_null());
            let a = &mut *ctx.producer.borrow_mut();
            let tmp = tmp as usize;
            let tmp = api::BufferRef::from(tmp);
            a.push(tmp);
        }
        Ok(Self {
            tx: RefCell::new(tx),
            rx: RefCell::new(rx),
            ctx,
            consumer: RefCell::new(consumer),
        })
    }

    fn context(&self) -> &Self::Context {
        &self.ctx
    }
}

#[derive(Clone, Debug)]
pub struct DpdkFlags {
    pub num_mbufs: u32,
    pub mbuf_cache_size: u32,
    pub mbuf_default_buf_size: u16,
}

impl api::Flags for DpdkFlags {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Socket;

    #[test]
    fn test_send_with_flush() {
        let mut socket0 = Sock::create(
            "veth0dpdk",
            Some(0),
            DpdkFlags {
                num_mbufs: 8192,
                mbuf_cache_size: 250,
                mbuf_default_buf_size: 2176,
            },
        )
        .unwrap();
        let mut socket1 = Sock::create(
            "veth1dpdk",
            Some(0),
            DpdkFlags {
                num_mbufs: 8192,
                mbuf_cache_size: 250,
                mbuf_default_buf_size: 2176,
            },
        )
        .unwrap();
        socket1.send(b"Helloworldmyfriend\0\0\0\0\0\0\0").unwrap();
        socket1.flush();
        let (packet, meta) = socket0.recv().unwrap();
        assert_eq!(&packet[..20], b"Helloworldmyfriend\0\0");
    }
}
