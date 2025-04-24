use crate::api::Result;
use crate::api::{self, Context};
use crate::errors::Error;
use netmap_rs::context::{BufferPool, Port, Receiver, RxBuf, Transmitter, TxBuf};
use nix::sys::time::TimeVal;
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicU32, Ordering};
use triomphe::Arc;
type RefCell<T> = crate::fake_refcell::FakeRefCell<T>;

#[derive(Clone)]
pub struct Ctx {
    buffer_pool: Arc<BufferPool>,
    producer: RefCell<mpsc::Producer<api::BufferRef>>,
    index: u32,
}

impl Ctx {
    fn new(buffer_pool: BufferPool, indexes: Vec<u32>) -> (Self, mpsc::Consumer<api::BufferRef>) {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let (mut producer, cons) = mpsc::channel(indexes.len());
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let buffer_pool = Arc::new(buffer_pool);
        for idx in indexes {
            producer.push(api::BufferRef::from(idx as usize));
        }
        producer.flush();
        let res = Self {
            buffer_pool,
            producer: RefCell::new(producer),
            index: counter,
        };
        (res, cons)
    }

    unsafe fn buffer(&self, idx: api::BufferRef) -> *mut [u8] {
        unsafe { self.buffer_pool.buffer(usize::from(idx)) }
    }
}

impl api::Context for Ctx {
    type Token = Tok;
    fn release(&self, token: api::BufferRef) {
        self.producer.borrow_mut().push(token);
    }

    unsafe fn unsafe_buffer(&self, buf_idx: api::BufferRef, _size: usize) -> *mut [u8] {
        unsafe { Ctx::buffer(self, buf_idx) }
    }

    fn pool_id(&self) -> u32 {
        self.index
    }
}
//impl api::ContextExt for Ctx {}

struct PacketHeader {
    // index: u32,
    // len: u16,
    // caplen: u16,
    ts: TimeVal,
}

#[must_use]
pub struct Tok {
    idx: api::BufferRef,
    len: u32,
    buffer_pool: u32,
}

impl Tok {
    fn new(idx: u32, buffer_pool: u32, len: u32) -> ManuallyDrop<Self> {
        let idx = api::BufferRef::from(idx as usize);
        ManuallyDrop::new(Self {
            idx,
            len,
            buffer_pool,
        })
    }
}

impl api::Token for Tok {
    type Context = Ctx;

    fn buffer_idx(&self) -> api::BufferRef {
        self.idx
    }

    fn size(&self) -> u32 {
        self.len
    }

    fn pool_id(&self) -> u32 {
        self.buffer_pool
    }
}

impl Drop for Tok {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            panic!("PacketToken must be used");
        }
    }
}

pub struct Sock {
    tx: RefCell<Transmitter>,
    rx: RefCell<Receiver>,
    ctx: Ctx,
    consumer: RefCell<mpsc::Consumer<api::BufferRef>>,
}

impl std::fmt::Debug for Sock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Socket").finish()
    }
}

impl Sock {
    #[inline(always)]
    fn send_inner(&self, scan: TxBuf<'_>, packet: &[u8]) -> Result<()> {
        let TxBuf { ref slot, .. } = scan;
        let token = slot.buf_idx();
        let token = api::BufferRef::from(token as usize);
        let buf = unsafe { Ctx::buffer(&self.ctx, token) };
        let buf = unsafe { &mut (*buf) };
        if packet.len() > buf.len() {
            return Err(Error::TooBigPacket(packet.len()));
        }
        buf[..packet.len()].copy_from_slice(packet);
        Ok(())
    }

    #[inline(always)]
    fn recv_inner(&self, buf: RxBuf<'_>) -> Result<(Tok, Meta)> {
        let RxBuf { slot, ts, .. } = buf;
        let free_idx = self.consumer.borrow_mut().pop().ok_or(Error::NoMemory)?;
        let pkt_idx = slot.buf_idx();
        unsafe {
            slot.update_buffer(|x| *x = usize::from(free_idx) as u32);
        }

        let packet_token = Tok::new(pkt_idx, self.ctx.index, slot.len() as u32);

        let meta = Meta {};
        Ok((ManuallyDrop::into_inner(packet_token), meta))
    }
}

impl api::Socket for Sock {
    type Context = Ctx;
    type Metadata = Meta;
    type Flags = NetmapFlags;

    fn recv_token(&mut self) -> Result<(<Self::Context as Context>::Token, Self::Metadata)> {
        let mut rx = self.rx.borrow_mut();
        if let Some(tmp) = rx.iter_mut().next() {
            self.recv_inner(tmp)
        } else {
            // SAFETY: there are no `RxBuf`s, and so any `Slot`s, in use
            unsafe {
                rx.reset();
            }
            let tmp = rx.iter_mut().next().ok_or(Error::NoPacket)?;
            self.recv_inner(tmp)
        }
    }

    fn send(&mut self, packet: &[u8]) -> Result<()> {
        let mut tx = self.tx.borrow_mut();
        if let Some(next) = tx.iter_mut().next() {
            self.send_inner(next, packet)
        } else {
            // SAFETY: there are no `TxBuf`s, and so any `Slot`s, in use
            unsafe {
                tx.reset();
            }
            let next = tx.iter_mut().next().ok_or(Error::NoMemory)?;
            self.send_inner(next, packet)
        }
    }

    fn flush(&mut self) {
        let mut tx = self.tx.borrow_mut();
        // SAFETY: Any `Slot`s is in use due to the design of the API
        unsafe {
            tx.sync();
        }
    }

    fn create(portspec: &str, queue: Option<usize>, flags: Self::Flags) -> Result<Self> {
        let p = if let Some(q) = queue {
            &format!("{}-{}", portspec, q)
        } else {
            portspec
        };

        let mut port = Port::open(p, flags.extra_buf)?;
        let extra_bufs = unsafe { port.extra_buffers_indexes() };
        let (tx, rx, buffer_pool) = port.split();
        let (ctx, consumer) = Ctx::new(buffer_pool, extra_bufs);
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
pub struct NetmapFlags {
    pub extra_buf: u32,
}

impl api::Flags for NetmapFlags {}

pub struct Meta {}

impl api::Metadata for Meta {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::{Flags, Socket},
        netmap::Sock,
    };

    #[test]
    fn test_send_with_flush() {
        let mut socket0 = Sock::create("vale0", Some(0), NetmapFlags { extra_buf: 1024 }).unwrap();
        let mut socket1 = Sock::create("vale0", Some(1), NetmapFlags { extra_buf: 1024 }).unwrap();
        socket1.send(b"Helloworldmyfriend\0\0\0\0\0\0\0").unwrap();
        socket1.flush();
        let (packet, meta) = socket0.recv().unwrap();
        assert_eq!(&packet[..20], b"Helloworldmyfriend\0\0");
    }
}
