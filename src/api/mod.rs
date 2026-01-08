use std::{
    fmt::Debug,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};

#[cfg(feature = "af_xdp")]
use crate::af_xdp;
#[cfg(feature = "dpdk")]
use crate::dpdk;
#[cfg(feature = "netmap")]
use crate::netmap;
#[cfg(feature = "pcap")]
use crate::pcap;

pub type Result<T> = std::result::Result<T, crate::errors::Error>;

pub const DEFAULT_BATCH_SIZE: usize = 32;

#[inline]
#[cold]
fn cold() {}
#[inline]
pub fn likely(b: bool) -> bool {
    if !b {
        cold()
    }
    b
}

#[inline]
pub fn unlikely(b: bool) -> bool {
    if b {
        cold()
    }
    b
}

#[derive(Clone, Copy, Debug)]
pub struct BufferDesc(pub(crate) usize);

impl From<usize> for BufferDesc {
    fn from(val: usize) -> Self {
        Self(val)
    }
}

impl From<BufferDesc> for usize {
    fn from(val: BufferDesc) -> usize {
        val.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BufferRef(pub(crate) usize);

impl From<usize> for BufferRef {
    fn from(val: usize) -> Self {
        Self(val)
    }
}

impl From<BufferRef> for usize {
    fn from(val: BufferRef) -> usize {
        val.0
    }
}

#[repr(C)]
pub struct Payload<'ctx, Ctx: Context> {
    pub(crate) token: ManuallyDrop<Token>,
    ctx: &'ctx Ctx,
}

impl<'ctx, Ctx: Context> Payload<'ctx, Ctx> {
    pub fn new(token: Token, ctx: &'ctx Ctx) -> Self {
        Self {
            token: ManuallyDrop::new(token),
            ctx,
        }
    }
}

impl<'ctx, Ctx: Context> Deref for Payload<'ctx, Ctx> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe {
            let buf = self
                .ctx
                .unsafe_buffer(self.token.buffer_desc(), self.token.size() as usize);

            let buf = &*buf;
            &buf[..self.token.size() as usize]
        }
    }
}

impl<'ctx, Ctx: Context> DerefMut for Payload<'ctx, Ctx> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            let buf = self
                .ctx
                .unsafe_buffer(self.token.buffer_desc(), self.token.size() as usize);
            let buf = &mut *buf;
            &mut buf[..self.token.size() as usize]
        }
    }
}

impl<'ctx, Ctx: Context> Drop for Payload<'ctx, Ctx> {
    fn drop(&mut self) {
        self.ctx.release(self.token.buffer_desc());
    }
}

impl<'ctx, Ctx: Context> Payload<'ctx, Ctx> {
    pub fn into_token(self) -> Token {
        let mut me = ManuallyDrop::new(self);
        let token = &mut me.token;
        let rv = unsafe { std::ptr::read(token) };
        ManuallyDrop::into_inner(rv)
    }
}

pub struct Token {
    pub(crate) idx: BufferDesc,
    pub(crate) len: u32,
    pub(crate) buffer_pool: u32,
}

impl Token {
    pub fn new(idx: BufferDesc, buffer_pool: u32, len: u32) -> Self {
        Self {
            idx,
            len,
            buffer_pool,
        }
    }

    pub fn buffer_desc(&self) -> BufferDesc {
        self.idx
    }

    pub fn size(&self) -> u32 {
        self.len
    }

    pub fn pool_id(&self) -> u32 {
        self.buffer_pool
    }

    pub fn check_token<Ctx: Context>(&self, ctx: &Ctx) -> bool {
        ctx.check_token(self)
    }
    pub fn consume<'ctx, Ctx: Context>(self, ctx: &'ctx Ctx) -> Payload<'ctx, Ctx> {
        ctx.packet(self)
    }
}

pub trait Context: Sized + Clone + Send + 'static {
    // type Token: Token<Context = Self>;
    // type Token: Token;

    #[inline(always)]
    fn check_token(&self, token: &Token) -> bool {
        token.pool_id() == self.pool_id()
    }

    fn packet<'ctx>(&'ctx self, token: Token) -> Payload<'ctx, Self> {
        if unlikely(!self.check_token(&token)) {
            panic!("Invalid token");
        }
        Payload::new(token, self)
    }

    fn pool_id(&self) -> u32;

    /// # Safety: this function is safe if Context is !Freeze and the caller ensures correct ownership and lifetime management of the buffer.
    unsafe fn unsafe_buffer(&self, buf_idx: BufferDesc, size: usize) -> *mut [u8];

    fn release(&self, buf_idx: BufferDesc);
}

pub trait Socket: Send + Sized {
    type Context: Context;
    type Metadata: Metadata;
    type Flags: Flags;
    //    #[inline(never)]
    fn recv(&self) -> Result<(Payload<'_, Self::Context>, Self::Metadata)> {
        let (token, meta) = self.recv_token()?;
        Ok((token.consume(self.context()), meta))
    }

    fn recv_token(&self) -> Result<(Token, Self::Metadata)>;

    fn send(&self, packet: &[u8]) -> Result<()>;

    fn flush(&self);

    fn create(portspec: &str, queue: Option<usize>, flags: Self::Flags) -> Result<Self>;

    fn create_with_channel<const BATCH_SIZE: usize>(
        portspec: &str,
        queue: Option<usize>,
        flags: Self::Flags,
        channel: impl APINethunsChannel<{ BATCH_SIZE }, Token>,
    ) -> Result<(
        Self,
        NethunsPusher<{ BATCH_SIZE }, Self::Context>,
        NethunsPopper<{ BATCH_SIZE }, Self::Context>,
    )> {
        let socket = Self::create(portspec, queue, flags)?;
        let (inner_pusher, inner_popper) = channel.split();

        let popper = NethunsPopper {
            inner: Box::new(inner_popper),
            ctx: socket.context().clone(),
        };

        let pusher = NethunsPusher {
            inner: Box::new(inner_pusher),
            ctx: socket.context().clone(),
        };
        Ok((socket, pusher, popper))
    }

    fn context(&self) -> &Self::Context;
}

pub struct NethunsPusher<const BATCH_SIZE: usize, Ctx: Context> {
    inner: Box<dyn SPMCPusher<{ BATCH_SIZE }, Token>>,
    ctx: Ctx,
}

impl<const BATCH_SIZE: usize, Ctx: Context> NethunsPusher<{ BATCH_SIZE }, Ctx> {
    #[inline(never)]
    pub fn push(
        &self,
        batch: [Payload<'_, Ctx>; BATCH_SIZE],
    ) -> core::result::Result<(), [Payload<'_, Ctx>; BATCH_SIZE]> {
        let erased = batch.map(|x| {
            let rv = x.into_token();
            if unlikely(!rv.check_token(&self.ctx)) {
                panic!("Wrong payload");
            }
            rv
        });

        self.inner
            .push(erased)
            .map_err(|b| b.map(|x| x.consume(&self.ctx)))
    }
}

pub trait APINethunsPopper<const BATCH_SIZE: usize, T, Ctx: Context>: Send {
    fn pop(&self) -> Option<[T; BATCH_SIZE]>;
}

pub trait APINethunsPusher<const BATCH_SIZE: usize, T, Ctx: Context>: Send {
    fn push(&self, batch: [T; BATCH_SIZE]) -> core::result::Result<(), [T; BATCH_SIZE]>;
}

pub trait APINethunsChannel<const BATCH_SIZE: usize, T> {
    fn split(
        self,
    ) -> (
        impl SPMCPusher<{ BATCH_SIZE }, T> + 'static,
        impl SPMCPopper<{ BATCH_SIZE }, T> + 'static,
    );
}

impl<const BATCH_SIZE: usize, T: Send + 'static> APINethunsChannel<{ BATCH_SIZE }, T>
    for (
        flume::Sender<[T; BATCH_SIZE]>,
        flume::Receiver<[T; BATCH_SIZE]>,
    )
{
    fn split(
        self,
    ) -> (
        impl SPMCPusher<{ BATCH_SIZE }, T> + 'static,
        impl SPMCPopper<{ BATCH_SIZE }, T> + 'static,
    ) {
        self
    }
}

pub struct NethunsPopper<const BATCH_SIZE: usize, Ctx: Context> {
    inner: Box<dyn SPMCPopper<BATCH_SIZE, Token>>,
    ctx: Ctx,
}

impl<const BATCH_SIZE: usize, Ctx: Context> Clone for NethunsPopper<BATCH_SIZE, Ctx> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone_box(),
            ctx: self.ctx.clone(),
        }
    }
}

#[inline(always)]
pub fn unerase_payloads<'ctx, const BATCH_SIZE: usize, Ctx: Context>(
    ctx: &'ctx Ctx,
    mut batch: [ErasedPayload2; BATCH_SIZE],
) -> [Payload<'ctx, Ctx>; BATCH_SIZE] {
    union Transmuter<'a, const N: usize, Ctx: Context> {
        erased: ManuallyDrop<[ErasedPayload2; N]>,
        typed: ManuallyDrop<[Payload<'a, Ctx>; N]>,
    }

    assert!(std::mem::size_of::<ErasedPayload2>() == std::mem::size_of::<Payload<Ctx>>());
    assert!(
        std::mem::size_of::<[ErasedPayload2; BATCH_SIZE]>()
            == std::mem::size_of::<[Payload<Ctx>; BATCH_SIZE]>()
    );

    let ctx = &raw const *ctx;
    for scan in &mut batch {
        scan.ctx = ctx as *const ();
    }
    let rv = unsafe {
        Transmuter {
            erased: ManuallyDrop::new(batch),
        }
        .typed
    };

    ManuallyDrop::into_inner(rv)
}

#[inline(always)]
pub fn erase_payloads<'ctx, const BATCH_SIZE: usize, Ctx: Context>(
    batch: [Payload<'ctx, Ctx>; BATCH_SIZE],
) -> [ErasedPayload2; BATCH_SIZE] {
    union Transmuter<'a, const N: usize, Ctx: Context> {
        typed: ManuallyDrop<[Payload<'a, Ctx>; N]>,
        erased: ManuallyDrop<[ErasedPayload2; N]>,
    }

    assert!(std::mem::size_of::<ErasedPayload2>() == std::mem::size_of::<Payload<Ctx>>());
    assert!(
        std::mem::size_of::<[ErasedPayload2; BATCH_SIZE]>()
            == std::mem::size_of::<[Payload<Ctx>; BATCH_SIZE]>()
    );

    let rv = unsafe {
        Transmuter {
            typed: ManuallyDrop::new(batch),
        }
        .erased
    };

    ManuallyDrop::into_inner(rv)
}

impl<const BATCH_SIZE: usize, Ctx: Context> NethunsPopper<BATCH_SIZE, Ctx> {
    pub fn pop(&self) -> Option<[Payload<'_, Ctx>; BATCH_SIZE]> {
        let rv = self.inner.pop()?;
        Some(rv.map(|x| x.consume(&self.ctx)))
    }
}

#[repr(C)]
pub struct ErasedPayload2 {
    token: ManuallyDrop<Token>,
    ctx: *const (),
}

unsafe impl Send for ErasedPayload2 {}
const _: () = const {
    struct DummyContext([u8; 0xdeadbeef]);

    impl Clone for DummyContext {
        fn clone(&self) -> Self {
            Self(self.0)
        }
    }

    impl Context for DummyContext {
        fn pool_id(&self) -> u32 {
            todo!()
        }

        unsafe fn unsafe_buffer(&self, buf_idx: BufferDesc, size: usize) -> *mut [u8] {
            todo!()
        }

        fn release(&self, buf_idx: BufferDesc) {
            todo!()
        }
    }

    assert!(size_of::<ErasedPayload2>() == size_of::<Payload<DummyContext>>());
};

pub trait SPMCPusher<const BATCH_SIZE: usize, T>: Send {
    fn push(&self, batch: [T; BATCH_SIZE]) -> core::result::Result<(), [T; BATCH_SIZE]>;
}

pub trait SPMCPopper<const BATCH_SIZE: usize, T>:
    SPMCPopperDynClone<{ BATCH_SIZE }, T> + Send
{
    fn pop(&self) -> Option<[T; BATCH_SIZE]>;
}

pub trait SPMCPopperDynClone<const BATCH_SIZE: usize, T> {
    fn clone_box(&self) -> Box<dyn SPMCPopper<{ BATCH_SIZE }, T>>;
}

impl<const BATCH_SIZE: usize, T: ?Sized, E> SPMCPopperDynClone<{ BATCH_SIZE }, E> for T
where
    T: 'static + SPMCPopper<{ BATCH_SIZE }, E> + Clone,
{
    fn clone_box(&self) -> Box<dyn SPMCPopper<{ BATCH_SIZE }, E>> {
        Box::new(self.clone())
    }
}

impl<const BATCH_SIZE: usize, T> Clone for Box<dyn SPMCPopper<{ BATCH_SIZE }, T>> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub struct FlumeChannel<const BATCH_SIZE: usize, T, Ctx: Context> {
    sender: flume::Sender<[T; BATCH_SIZE]>,
    receiver: flume::Receiver<[T; BATCH_SIZE]>,
    ctx: Option<Ctx>,
}

impl<const BATCH_SIZE: usize, T, Ctx: Context> FlumeChannel<{ BATCH_SIZE }, T, Ctx> {
    pub fn new(
        (sender, receiver): (
            flume::Sender<[T; BATCH_SIZE]>,
            flume::Receiver<[T; BATCH_SIZE]>,
        ),
    ) -> Self {
        Self {
            sender,
            receiver,
            ctx: None,
        }
    }
}

impl<const BATCH_SIZE: usize, T: 'static + Send> SPMCPusher<{ BATCH_SIZE }, T>
    for flume::Sender<[T; BATCH_SIZE]>
{
    fn push(&self, batch: [T; BATCH_SIZE]) -> core::result::Result<(), [T; BATCH_SIZE]> {
        self.send(batch).map_err(|error| error.0)
    }
}

impl<const BATCH_SIZE: usize, T: 'static + Send> SPMCPopper<{ BATCH_SIZE }, T>
    for flume::Receiver<[T; BATCH_SIZE]>
{
    fn pop(&self) -> Option<[T; BATCH_SIZE]> {
        let batch = self.recv().ok()?;
        Some(batch)
    }
}

pub trait Metadata: Send {
    fn into_enum(self) -> MetadataType;
}

pub enum MetadataType {
    #[cfg(feature = "netmap")]
    Netmap(netmap::Meta),
    #[cfg(feature = "af_xdp")]
    AfXdp(af_xdp::Meta),
    #[cfg(feature = "dpdk")]
    Dpdk(dpdk::Meta),
    #[cfg(feature = "pcap")]
    Pcap(pcap::Meta),
}

pub trait Flags: Clone + Debug {}
