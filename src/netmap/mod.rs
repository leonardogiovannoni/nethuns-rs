use anyhow::{Result, bail};
use mpsc::Producer;
use crate::api::{BufferIndex, NethunsContext, NethunsFlags, NethunsPayload, NethunsSocket, NethunsToken};
use netmap_rs::context::{BufferPool, Port, Receiver, RxBuf, Transmitter, TxBuf};
use nix::sys::time::TimeVal;
use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};


#[derive(Clone, Copy, Debug)]
struct U24Repr {
    data: [u8; 3],
}

impl U24Repr {
    fn new(data: [u8; 3]) -> Self {
        Self { data }
    }

    fn from_u32(val: u32) -> Self {
        let data = val.to_be_bytes();
        Self {
            data: [data[1], data[2], data[3]],
        }
    }

    fn as_u32(&self) -> u32 {
        u32::from_be_bytes([0, self.data[0], self.data[1], self.data[2]])
    }
}

impl From<U24Repr> for u32 {
    fn from(val: U24Repr) -> u32 {
        val.as_u32()
    }
}

impl From<u32> for U24Repr {
    fn from(val: u32) -> Self {
        Self::from_u32(val)
    }
}

#[derive(Clone)]
pub struct Context {
    buffer_pool: Arc<BufferPool>,
    producer: RefCell<Producer<BufferIndex>>,
    index: usize,
}

impl Context {
    fn new(buffer_pool: BufferPool, indexes: Vec<u32>) -> (Self, mpsc::Consumer<BufferIndex>) {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let (mut producer, cons) = mpsc::channel::<BufferIndex>(indexes.len());
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let buffer_pool = Arc::new(buffer_pool);
        for idx in indexes {
            producer.push(BufferIndex::from(idx));
        }
        let res = Self {
            buffer_pool,
            producer: RefCell::new(producer),
            index: counter,
        };
        (res, cons)
    }

    unsafe fn buffer(&self, idx: BufferIndex) -> *mut [u8] {
        unsafe { self.buffer_pool.buffer(u32::from(idx) as usize) }
    }

    fn check_token(&self, token: &PayloadToken) -> bool {
        token.buffer_pool == self.index as u32
    }

    fn peek_packet(&self, token: &PayloadToken) -> Payload<'_> {
        if !self.check_token(token) {
            panic!("Invalid token");
        }
        Payload {
            packet_idx: token.idx,
            pool: self,
        }
    }
}

impl NethunsContext for Context {
    //type Payload = Payload<'a>;
    type Token = PayloadToken;

    //fn packet(&'a self, token: Self::Token) -> Payload<'a> {
    //    if !self.check_token(&token) {
    //        panic!("Invalid token");
    //    }
    //    let token = ManuallyDrop::new(token);
    //    Payload {
    //        packet_idx: token.idx,
    //        pool: self,
    //    }
    //}

    fn release(&self, token: BufferIndex) {
        self.producer.borrow_mut().push(token);
    }

    type Payload<'ctx> = Payload<'ctx>;

    fn packet<'ctx>(&'ctx self, token: Self::Token) -> Self::Payload<'ctx> {
        if !self.check_token(&token) {
            panic!("Invalid token");
        }
        let token = ManuallyDrop::new(token);
        Payload {
            packet_idx: token.idx,
            pool: self,
        }
    }
}

struct PacketHeader {
    // index: u32,
    // len: u16,
    // caplen: u16,
    ts: TimeVal,
}

pub struct RecvPacket<'a> {
    ts: TimeVal,
    payload: Payload<'a>,
}

impl<'a> RecvPacket<'a> {
    fn payload(&self) -> &[u8] {
        self.payload.as_slice()
    }

    fn payload_mut(&mut self) -> &mut [u8] {
        self.payload.as_mut_slice()
    }
}

#[must_use]
pub struct PayloadToken {
    idx: BufferIndex,
    buffer_pool: u32,
}

impl PayloadToken {
    fn new(idx: u32, buffer_pool: u32) -> ManuallyDrop<Self> {
        let idx = BufferIndex::from(idx);
        ManuallyDrop::new(Self { idx, buffer_pool })
    }
}

impl NethunsToken for PayloadToken {
    type Context = Context;
}

impl Drop for PayloadToken {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            panic!("PacketToken must be used");
        }
    }
}

pub struct Socket {
    tx: RefCell<Transmitter>,
    rx: RefCell<Receiver>,
    ctx: Context,
    consumer: RefCell<mpsc::Consumer<BufferIndex>>,
    filter: Option<Filter>,
}

impl std::fmt::Debug for Socket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Socket").finish()
    }
}

pub struct Payload<'a> {
    packet_idx: BufferIndex,
    pool: &'a Context,
}

impl<'a> NethunsPayload<'a> for Payload<'a> {
    type Context = Context;
    type Token = PayloadToken;
}

impl<'a> Payload<'a> {
    fn as_slice(&self) -> &[u8] {
        let token = self.packet_idx;
        let buf = unsafe { self.pool.buffer(token) };
        unsafe { &(*buf) }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        let token = self.packet_idx;
        let buf = unsafe { self.pool.buffer(token) };
        unsafe { &mut (*buf) }
    }
}

impl Deref for Payload<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for Payload<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}
impl<'a> AsRef<[u8]> for Payload<'a> {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl<'a> AsMut<[u8]> for Payload<'a> {
    fn as_mut(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
}

impl<'a> Drop for Payload<'a> {
    fn drop(&mut self) {
        self.pool.release(self.packet_idx);
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

impl Socket {
    #[inline(always)]
    fn send_inner(&self, scan: TxBuf<'_>, packet: &[u8]) -> Result<()> {
        let TxBuf { ref slot, .. } = scan;
        let token = slot.buf_idx();
        let token = BufferIndex::from(token);
        let buf = unsafe { self.ctx.buffer(token) };
        let buf = unsafe { &mut (*buf) };
        if packet.len() > buf.len() {
            bail!("Packet too big");
        }
        buf[..packet.len()].copy_from_slice(packet);
        Ok(())
    }

    #[inline(always)]
    fn recv_inner(&self, buf: RxBuf<'_>) -> Result<PayloadToken> {
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

        let packet_token = PayloadToken::new(pkt_idx, self.ctx.index as u32);

        if let Some(filter) = self.filter.as_ref() {
            let aliased_packet = self.ctx.peek_packet(&packet_token);
            if filter.apply(&ts, aliased_packet.as_slice()).is_err() {
                bail!("Filter failed");
            }
        }
        Ok(ManuallyDrop::into_inner(packet_token))
    }

    //pub fn create(portspec: &str, extra_buf: usize, filter: Option<Filter>) -> Result<(Context, Self)> {
    //   
    //}
}

impl NethunsSocket for Socket {
    type Context = Context;
    type Token = PayloadToken;
    type Flags = NetmapFlags;

    fn recv(&mut self) -> anyhow::Result<Self::Token> {
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
    
    fn create(portspec: &str, filter: Option<()>, flags: Self::Flags) -> anyhow::Result<(Self::Context, Self)> {
        let mut port = Port::open(portspec, flags.extra_buf as u32)?;
        let extra_bufs = unsafe { port.extra_buffers_indexes() };
        let (tx, rx, buffer_pool) = port.split();
        let (ctx, consumer) = Context::new(buffer_pool, extra_bufs);
        Ok((ctx.clone(), Self {
            tx: RefCell::new(tx),
            rx: RefCell::new(rx),
            ctx,
            consumer: RefCell::new(consumer),
            filter: None, // TODO
        }))
    }

    fn context(&self) -> &Self::Context {
        &self.ctx
    }
    
}



#[derive(Clone, Copy, Debug)]
pub struct NetmapFlags {
    pub extra_buf: usize,
}

impl NethunsFlags for NetmapFlags {}