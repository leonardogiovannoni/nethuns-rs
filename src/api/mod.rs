use std::ops::{Deref, DerefMut};

use crate::{
    af_xdp::AfXdpFlags,
    netmap::NetmapFlags,
    strategy::{CrossbeamArgs, MpscArgs, StdArgs},
};

pub trait Strategy: Clone + 'static {
    type Producer: BufferProducer;
    type Consumer: BufferConsumer;
    fn create(args: StrategyArgs) -> (Self::Producer, Self::Consumer);
}

pub trait BufferProducer: Clone + Send {
    fn push(&mut self, elem: BufferIndex);
    fn flush(&mut self);
}

pub trait BufferConsumer: Send {
    fn pop(&mut self) -> Option<BufferIndex>;
    fn available_len(&self) -> usize;
    fn sync(&mut self);
}

#[derive(Clone, Copy, Debug)]
pub struct BufferIndex(u32);

impl From<u32> for BufferIndex {
    fn from(val: u32) -> Self {
        Self(val.into())
    }
}

impl From<BufferIndex> for u32 {
    fn from(val: BufferIndex) -> u32 {
        val.0.into()
    }
}

/// A token returned by a receive operation. In both implementations,
/// the token carries the buffer index (and, optionally, metadata).
pub trait Token: Sized {
    /// Which context type produced this token?
    type Context: Context<Token = Self>;

    fn consume<'ctx>(
        self,
        ctx: &'ctx Self::Context,
    ) -> <Self::Context as Context>::Payload<'ctx> {
        ctx.packet(self)
    }
}

/// A trait representing the buffer pool (or context) that is used by the
/// underlying implementation. It allows obtaining a mutable slice given
/// a buffer index and “releasing” the buffer back to the pool.
pub trait Context: Sized + Clone + Send {
    /// The type of token this context uses.
    type Token: Token<Context = Self>;

    /// **Generic associated type** for the payload. This means that for each
    /// lifetime `'ctx`, `Payload<'ctx>` is a type that implements `NethunsPayload<'ctx>`.
    ///
    /// We say `Self: 'ctx` to ensure that the context outlives `'ctx`.
    type Payload<'ctx>: Payload<'ctx>
    where
        Self: 'ctx;

    /// Create/load a payload from the context using the given token.
    fn packet<'ctx>(&'ctx self, token: Self::Token) -> Self::Payload<'ctx>;

    /// Release a buffer back to the pool (typically called when a payload
    /// is dropped).
    fn release(&self, buf_idx: BufferIndex);
}

/// A trait representing a packet’s payload data. Notice it has a lifetime `'a`:
/// this means the payload may borrow data out of the context for `'a`.
pub trait Payload<'a>:
    AsRef<[u8]> + AsMut<[u8]> + Deref<Target = [u8]> + DerefMut<Target = [u8]>
{
}

/// The common API for a network socket, which can send, receive, and flush.
pub trait Socket<S: Strategy>: Send + Sized {
    /// The associated context.
    type Context: Context;
    type Metadata: Metadata;

    fn recv_local(
        &mut self,
    ) -> anyhow::Result<(
        <Self::Context as Context>::Payload<'_>,
        Self::Metadata,
    )> {
        let (token, meta) = self.recv()?;
        Ok((token.consume(self.context()), meta))
    }

    /// Receives a packet and returns a token.
    fn recv(
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
        filter: Option<()>,
        flags: Flags,
    ) -> anyhow::Result<(Self::Context, Self)>;

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
}
