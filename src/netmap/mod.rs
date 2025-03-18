use crate::api::{self, BufferConsumer, BufferProducer, Context, Payload};
use anyhow::{Result, bail};
use netmap_rs::context::{BufferPool, Port, Receiver, RxBuf, Transmitter, TxBuf};
use nix::sys::time::TimeVal;
use std::cell::RefCell;
//use std::cell::RefCell;
use crate::api::ContextExt;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

//type RefCell<T> = crate::fake_refcell::FakeRefCell<T>;
#[derive(Clone)]
pub struct Ctx<S: api::Strategy> {
    buffer_pool: Arc<BufferPool>,
    producer: RefCell<S::Producer>,
    index: usize,
}

impl<S: api::Strategy> Ctx<S> {
    fn new(
        buffer_pool: BufferPool,
        indexes: Vec<u32>,
        strategy_args: api::StrategyArgs,
    ) -> (Self, S::Consumer) {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let (mut producer, cons) = S::create(strategy_args);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let buffer_pool = Arc::new(buffer_pool);
        for idx in indexes {
            producer.push(api::BufferIndex::from(idx));
        }
        producer.flush();
        let res = Self {
            buffer_pool,
            producer: RefCell::new(producer),
            index: counter,
        };
        (res, cons)
    }

    unsafe fn buffer(&self, idx: api::BufferIndex) -> *mut [u8] {
        unsafe { self.buffer_pool.buffer(u32::from(idx) as usize) }
    }
}

impl<S: api::Strategy> api::Context for Ctx<S> {
    type Token = Tok<S>;
    fn release(&self, token: api::BufferIndex) {
        self.producer.borrow_mut().push(token);
    }

    unsafe fn unsafe_buffer(&self, buf_idx: api::BufferIndex, _size: usize) -> *mut [u8] {
        unsafe { Ctx::buffer(self, buf_idx) }
    }

    fn pool_id(&self) -> usize {
        self.index
    }
}
impl<S: api::Strategy> api::ContextExt for Ctx<S> {}

struct PacketHeader {
    // index: u32,
    // len: u16,
    // caplen: u16,
    ts: TimeVal,
}

#[must_use]
pub struct Tok<S: api::Strategy> {
    idx: api::BufferIndex,
    buffer_pool: usize,
    _strategy: std::marker::PhantomData<S>,
}

impl<S: api::Strategy> Tok<S> {
    fn new(idx: u32, buffer_pool: usize) -> ManuallyDrop<Self> {
        let idx = api::BufferIndex::from(idx);
        ManuallyDrop::new(Self {
            idx,
            buffer_pool,
            _strategy: std::marker::PhantomData,
        })
    }
}

impl<S: api::Strategy> api::Token for Tok<S> {
    type Context = Ctx<S>;

    fn buffer_idx(&self) -> api::BufferIndex {
        self.idx
    }

    fn size(&self) -> usize {
        // TODO: currently not used
        0xdeadbeef
    }

    fn pool_id(&self) -> usize {
        self.buffer_pool
    }
}

impl<S: api::Strategy> api::TokenExt for Tok<S> {
    fn clone(&self) -> Self {
        ManuallyDrop::into_inner(Self::new(self.idx.into(), self.buffer_pool))
    }
}

impl<S: api::Strategy> Drop for Tok<S> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            panic!("PacketToken must be used");
        }
    }
}

pub struct Sock<S: api::Strategy> {
    tx: RefCell<Transmitter>,
    rx: RefCell<Receiver>,
    ctx: Ctx<S>,
    consumer: RefCell<S::Consumer>,
    filter: Option<Filter>,
}

impl<S: api::Strategy> std::fmt::Debug for Sock<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Socket").finish()
    }
}

pub enum Filter {
    Closure(Box<dyn Fn(&TimeVal, &[u8]) -> Result<(), ()> + Send>),
    Function(fn(&TimeVal, &[u8]) -> Result<(), ()>),
}

impl Filter {
    fn apply(&self, ts: &TimeVal, payload: &[u8]) -> Result<(), ()> {
        match self {
            Filter::Closure(f) => f(ts, payload),
            Filter::Function(f) => f(ts, payload),
        }
    }
}

impl<S: api::Strategy> Sock<S> {
    #[inline(always)]
    fn send_inner(&self, scan: TxBuf<'_>, packet: &[u8]) -> Result<()> {
        let TxBuf { ref slot, .. } = scan;
        let token = slot.buf_idx();
        let token = api::BufferIndex::from(token);
        let buf = unsafe { self.ctx.buffer(token) };
        let buf = unsafe { &mut (*buf) };
        if packet.len() > buf.len() {
            bail!("Packet too big");
        }
        buf[..packet.len()].copy_from_slice(packet);
        Ok(())
    }

    #[inline(always)]
    fn recv_inner(&self, buf: RxBuf<'_>) -> Result<(Tok<S>, Meta)> {
        let RxBuf { slot, ts, .. } = buf;
        let free_idx = self
            .consumer
            .borrow_mut()
            .pop()
            .ok_or_else(|| anyhow::anyhow!("No free buffers"))?;
        let pkt_idx = slot.buf_idx();
        unsafe {
            slot.update_buffer(|x| *x = u32::from(free_idx));
        }

        let packet_token = Tok::new(pkt_idx, self.ctx.index);

        if let Some(filter) = self.filter.as_ref() {
            let aliased_packet = self.ctx.peek_packet(&packet_token);
            let aliased_packet = ManuallyDrop::new(aliased_packet);
            if filter.apply(&ts, &*aliased_packet).is_err() {
                bail!("Filter failed");
            }
        }
        Ok((ManuallyDrop::into_inner(packet_token), Meta {}))
    }
}

impl<S: api::Strategy> api::Socket<S> for Sock<S> {
    type Context = Ctx<S>;
    type Metadata = Meta;

    fn recv_token(&mut self) -> anyhow::Result<(<Self::Context as api::Context>::Token, Self::Metadata)> {
        let mut rx = self.rx.borrow_mut();
        if let Some(tmp) = rx.iter_mut().next() {
            self.recv_inner(tmp)
        } else {
            // SAFETY: there are no `RxBuf`s, and so any `Slot`s, in use
            unsafe {
                rx.reset();
            }
            let tmp = rx
                .iter_mut()
                .next()
                .ok_or_else(|| anyhow::anyhow!("No packets"))?;
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
            let next = tx
                .iter_mut()
                .next()
                .ok_or_else(|| anyhow::anyhow!("No free slots"))?;
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

    fn create(portspec: &str, filter: Option<()>, flags: api::Flags) -> anyhow::Result<Self> {
        let flags = match flags {
            api::Flags::Netmap(flags) => flags,
            _ => panic!("Invalid flags"),
        };
        let mut port = Port::open(portspec, flags.extra_buf)?;
        let extra_bufs = unsafe { port.extra_buffers_indexes() };
        let (tx, rx, buffer_pool) = port.split();
        let (ctx, consumer) = Ctx::new(buffer_pool, extra_bufs, flags.strategy_args);
        Ok(Self {
            tx: RefCell::new(tx),
            rx: RefCell::new(rx),
            ctx,
            consumer: RefCell::new(consumer),
            filter: None, // TODO
        })
    }

    fn context(&self) -> &Self::Context {
        &self.ctx
    }
}

#[derive(Clone, Debug)]
pub struct NetmapFlags {
    pub extra_buf: u32,
    pub strategy_args: api::StrategyArgs,
}

pub struct Meta {}

impl api::Metadata for Meta {}

#[cfg(test)]
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
