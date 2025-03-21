use std::{mem::ManuallyDrop, ops::{Deref, DerefMut}};

use crate::{
    af_xdp::AfXdpFlags, dpdk::DpdkFlags, netmap::NetmapFlags, strategy::{CrossbeamArgs, MpscArgs, StdArgs}
};


pub trait Strategy: Send + Clone + 'static {
    type Producer: BufferProducer;
    type Consumer: BufferConsumer;
    fn create(args: StrategyArgs) -> (Self::Producer, Self::Consumer);
}

pub(crate) trait BufferProducer: Clone + Send {
    fn push(&mut self, elem: BufferIndex);
    fn flush(&mut self);
}

pub(crate) trait BufferConsumer: Send {
    fn pop(&mut self) -> Option<BufferIndex>;
    fn available_len(&self) -> usize;
    fn sync(&mut self);
}

#[derive(Clone, Copy, Debug)]
pub struct BufferIndex(usize);


impl From<usize> for BufferIndex {
    fn from(val: usize) -> Self {
        Self(val.into())
    }
}

impl From<BufferIndex> for usize {
    fn from(val: BufferIndex) -> usize {
        val.0.into()
    }
}


pub struct Payload<'ctx, Ctx: Context> {
    token: ManuallyDrop<Ctx::Token>,
    ctx: &'ctx Ctx,
}


impl<'ctx, Ctx: Context> Payload<'ctx, Ctx> {
    pub fn new(token: Ctx::Token, ctx: &'ctx Ctx) -> Self {
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
            &*self.ctx.unsafe_buffer(self.token.buffer_idx(), self.token.size())
        }
    }
}


impl<'ctx, Ctx: Context> DerefMut for Payload<'ctx, Ctx> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut *self.ctx.unsafe_buffer(self.token.buffer_idx(), self.token.size())
        }
    }
}

impl<'ctx, Ctx: Context> Drop for Payload<'ctx, Ctx> {
    fn drop(&mut self) {
        self.ctx.release(self.token.buffer_idx());
    }
}


impl<'ctx, Ctx: Context> Payload<'ctx, Ctx> {
    pub fn into_token(self) -> Ctx::Token {
        let mut me = ManuallyDrop::new(self);
        let token = &mut me.token;
        let rv = unsafe { std::ptr::read(token) };
        ManuallyDrop::into_inner(rv)
    }
}

/// A token returned by a receive operation. In both implementations,
/// the token carries the buffer index (and, optionally, metadata).
pub trait Token: Sized + Send {
    /// Which context type produced this token?
    type Context: Context<Token = Self>;

    fn consume<'ctx>(
        self,
        ctx: &'ctx Self::Context,
    ) -> Payload<'ctx, Self::Context> {
        ctx.packet(self)
    }

    fn buffer_idx(&self) -> BufferIndex;

    fn size(&self) -> usize;

    fn pool_id(&self) -> usize;
}

pub(crate) trait TokenExt {
    fn duplicate(&self) -> Self;
}

/// A trait representing the buffer pool (or context) that is used by the
/// underlying implementation. It allows obtaining a mutable slice given
/// a buffer index and “releasing” the buffer back to the pool.
pub trait Context: Sized + Clone + Send {
    /// The type of token this context uses.
    type Token: Token<Context = Self> + TokenExt;

    fn check_token(&self, token: &Self::Token) -> bool {
        token.pool_id() == self.pool_id()
    }

    /// Create/load a payload from the context using the given token.
    fn packet<'ctx>(&'ctx self, token: Self::Token) -> Payload<'ctx, Self> {
        if !self.check_token(&token) {
            panic!("Invalid token");
        }
        Payload::new(token, self)
    }


    fn pool_id(&self) -> usize;

    unsafe fn unsafe_buffer(&self, buf_idx: BufferIndex, size: usize) -> *mut [u8];

    fn release(&self, buf_idx: BufferIndex);
}

pub(crate) trait ContextExt: Context {
    fn peek_packet(&self, token: &Self::Token) -> Payload<'_, Self> {
        if !self.check_token(token) {
            panic!("Invalid token");
        }
        let token = TokenExt::duplicate(token);
        Payload::new(token, self)
    }
}

/*pub trait Socket<S, F>
where
    S: Strategy,
    F: FnOnce(Self::Metadata, Payload<'_, Self::Context>) -> bool + Send,
{ */
/// The common API for a network socket, which can send, receive, and flush.
pub trait Socket<S: Strategy>: Send + Sized {
    /// The associated context.
    type Context: Context;
    type Metadata: Metadata;

    fn recv(
        &mut self,
    ) -> anyhow::Result<(
        Payload<'_, Self::Context>,
        Self::Metadata,
    )> {
        let (token, meta) = self.recv_token()?;
        Ok((token.consume(self.context()), meta))
    }

    /// Receives a packet and returns a token.
    fn recv_token(
        &mut self,
    ) -> anyhow::Result<(<Self::Context as Context>::Token, Self::Metadata)>;

    /// Sends a packet. The packet is provided as a slice.
    fn send(&mut self, packet: &[u8]) -> anyhow::Result<()>;

    /// Flush any pending operations (for example, ensuring that the TX ring
    /// is synchronized or that completions are processed).
    fn flush(&mut self);

    /// Return a reference to the socket’s context, if needed (for example,
    /// to load a payload token).

    fn create(
        portspec: &str,
        queue: Option<usize>,
        filter: Option<()>,
        flags: Flags,
    ) -> anyhow::Result<Self>;

    fn context(&self) -> &Self::Context;
}

pub trait Metadata {}

#[derive(Clone, Debug)]
pub enum StrategyArgs {
    Std(StdArgs),
    Mpsc(MpscArgs),
    Crossbeam(CrossbeamArgs),
}

#[derive(Clone, Debug)]
pub enum Flags {
    Netmap(NetmapFlags),
    AfXdp(AfXdpFlags),
    DpdkFlags(DpdkFlags)
}
