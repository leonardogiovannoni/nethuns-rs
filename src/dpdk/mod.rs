mod wrapper;
use crate::api;
use crate::api::Result;
use crate::api::Token;
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

type RefCell<T> = crate::unsafe_refcell::UnsafeRefCell<T>;

#[derive(Clone)]
pub struct Ctx {
    producer: RefCell<mpsc::Producer<api::BufferDesc>>,
    index: u32,
}

impl Ctx {
    unsafe fn buffer(&self, idx: api::BufferDesc) -> *mut [u8] {
        let idx: usize = idx.into();
        let ptr = idx as *mut rte_mbuf;
        // TODO ensure only one segment, i.e. data_len == pkt_len
        let len = unsafe { (*ptr).__bindgen_anon_2.__bindgen_anon_1.data_len as usize };
        let data = unsafe { rust_rte_pktmbuf_mtod(ptr) as *mut u8 };
        unsafe { slice::from_raw_parts_mut(data, len) }
    }
}

impl Ctx {
    fn new(nbufs: usize) -> (Self, mpsc::Consumer<api::BufferDesc>) {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let (producer, cons) = mpsc::channel(nbufs);
        let res = Self {
            producer: RefCell::new(producer),
            index: COUNTER.fetch_add(1, Ordering::SeqCst),
        };
        (res, cons)
    }
}

impl api::Context for Ctx {
    fn pool_id(&self) -> u32 {
        self.index
    }

    unsafe fn unsafe_buffer(&self, buf_idx: api::BufferDesc, _size: usize) -> *mut [u8] {
        unsafe { Ctx::buffer(self, buf_idx) }
    }

    fn release(&self, buf_idx: api::BufferDesc) {
        let mut producer_mut = unsafe { self.producer.borrow_mut() };
        producer_mut.push(buf_idx);
    }
}

pub struct Sock {
    tx: RefCell<Transmitter>,
    rx: RefCell<Receiver>,
    ctx: Ctx,
    consumer: RefCell<mpsc::Consumer<api::BufferDesc>>,
}

pub struct Meta {}

impl api::Metadata for Meta {
    fn into_enum(self) -> api::MetadataType {
        api::MetadataType::Dpdk(self)
    }
}

impl Sock {
    //#[inline(never)]
    //#[cold]
    fn flush_to_memory_pool(&self) {
        let mut consumer = unsafe { self.consumer.borrow_mut() };
        consumer.sync();
        let buf = &mut consumer.cached;
        unsafe { rust_rte_pktmbuf_free_bulk(buf.as_mut_ptr() as *mut _, buf.len() as u32) };
        buf.clear();
    }

    #[inline(always)]
    fn recv_inner(&self, buf: RteMBuf) -> Result<(Token, Meta)> {
        let buf = ManuallyDrop::new(buf);
        let token = buf.as_ptr() as usize;
        let token = api::BufferDesc::from(token);

        let size = {
            let m = buf.as_ptr();
            unsafe { (*m).__bindgen_anon_2.__bindgen_anon_1.data_len as u32 }
        };
        let token = ManuallyDrop::new(Token {
            idx: token,
            len: size,
            buffer_pool: api::Context::pool_id(&self.ctx),
        });
        let meta = Meta {};
        Ok((ManuallyDrop::into_inner(token), meta))
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

    fn recv_token(&self) -> Result<(Token, Self::Metadata)> {
        if let Some(tmp) = unsafe { self.rx.borrow_mut().iter_mut().next() } {
            self.recv_inner(tmp)
        } else {
            self.flush_to_memory_pool();
            let mut rx = unsafe { self.rx.borrow_mut() };
            //let mut consumer = self.consumer.borrow_mut();
            let tmp = rx.iter_mut().next().ok_or(Error::NoPacket)?;
            self.recv_inner(tmp)
        }
    }

    fn send(&self, packet: &[u8]) -> Result<()> {
        let mut tx = unsafe { self.tx.borrow_mut() };
        let scan = tx.iter_mut().next().ok_or(Error::NoPacket)?;
        self.send_inner(scan, packet)
    }

    fn flush(&self) {
        unsafe { self.tx.borrow_mut().flush() };
    }

    fn create(portspec: &str, queue: Option<usize>, flags: Self::Flags) -> Result<Self> {
        let (mut buffer_pool, rx, tx) = Context::create(
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
            let a = unsafe { &mut *ctx.producer.borrow_mut() };
            let tmp = tmp as usize;
            let tmp = api::BufferDesc::from(tmp);
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
        let socket0 = Sock::create(
            "veth0dpdk",
            Some(0),
            DpdkFlags {
                num_mbufs: 8192,
                mbuf_cache_size: 250,
                mbuf_default_buf_size: 2176,
            },
        )
        .unwrap();
        let socket1 = Sock::create(
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
