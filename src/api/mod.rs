use std::{any::Any, ops::{Deref, DerefMut}};

/*use crate::BufferIndex;

/// A trait representing a packet’s payload data.
pub trait NethunsPayload<'a>: AsRef<[u8]> + AsMut<[u8]> {
    type Ctx: NethunsContext;
    type Token: NethunsToken<Context = Self::Ctx>;

    fn packet(ctx: &'a Self::Ctx, token: Self::Token) -> Self;

}



//impl<'a, T> NethunsPayload<'a> for T where T: AsRef<[u8]> + AsMut<[u8]> {}

/// A token returned by a receive operation. In both implementations,
/// the token carries the buffer index (and, optionally, metadata).
pub trait NethunsContext: Sized + Clone + Send {
    type Token: NethunsToken<Context = Self>;
    //type Payload = Self::Payload;
    /// “Load” the payload from the context. This should return an
    /// instance that implements `Deref<Target = [u8]>` and that on Drop
    /// returns the buffer to the pool.
    //fn packet(&'a self, token: Self::Token) -> Self::Payload;

    fn packet<'ctx, P: NethunsPayload<'ctx>>(&'ctx self, token: Self::Token) -> P {
        P::packet(self, token)
    }
    fn release(&self, buf_idx: BufferIndex);
}

/// A trait representing the buffer pool (or context) that is used by the
/// underlying implementation. It allows obtaining a mutable slice given
/// a buffer index and “releasing” the buffer back to the pool.
pub trait NethunsToken: Sized {
    type Context: NethunsContext<Token = Self>;

    // Given a token, obtain a payload view (and hook in any “release”
    // on drop, if desired).
    //fn load(self, ctx: &'a Self::Context) -> <Self::Context as NethunsContext<'a>>::Payload {
    //    ctx.packet(self)
    //}

    // Release a buffer back to the pool (typically called when a payload
    // is dropped). The argument might be a buffer index.

    // TODO: mark as unsafe
}

/// The common API for a network socket, which can send, receive, and flush.
/// This is implemented by both your AF_XDP and Netmap socket types.
pub trait NethunsSocket: Send {
    type Context: NethunsContext;
    type Token: NethunsToken<Context = Self::Context>;

    /// Receives a packet and returns a token.
    fn recv(&mut self) -> anyhow::Result<Self::Token>;

    /// Sends a packet. The packet is provided as a slice.
    fn send(&mut self, packet: &[u8]) -> anyhow::Result<()>;

    /// Flush any pending operations (for example, ensuring that the TX ring
    /// is synchronized or that completions are processed).
    fn flush(&mut self);

    /// Return a reference to the socket’s context, if needed (for example,
    /// to load a payload token).
    fn context(&self) -> &Self::Context;
}
*/
//use crate::BufferIndex;


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
pub trait NethunsToken: Sized {
    /// Which context type produced this token?
    type Context: NethunsContext<Token = Self>;

    fn load<'ctx>(self, ctx: &'ctx Self::Context) -> <Self::Context as NethunsContext>::Payload<'ctx> {
        ctx.packet(self)
    }

}

/// A trait representing the buffer pool (or context) that is used by the
/// underlying implementation. It allows obtaining a mutable slice given
/// a buffer index and “releasing” the buffer back to the pool.
pub trait NethunsContext: Sized + Clone + Send {
    /// The type of token this context uses.
    type Token: NethunsToken<Context = Self>;

    /// **Generic associated type** for the payload. This means that for each
    /// lifetime `'ctx`, `Payload<'ctx>` is a type that implements `NethunsPayload<'ctx>`.
    ///
    /// We say `Self: 'ctx` to ensure that the context outlives `'ctx`.
    type Payload<'ctx>: NethunsPayload<'ctx, Context = Self, Token = Self::Token>
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
pub trait NethunsPayload<'a>: AsRef<[u8]> + AsMut<[u8]> + Deref<Target = [u8]> + DerefMut<Target = [u8]> {
    /// Which context type is this payload tied to?
    type Context: NethunsContext;
    /// Which token type was used to create it?
    type Token: NethunsToken<Context = Self::Context>;
}

/// The common API for a network socket, which can send, receive, and flush.
pub trait NethunsSocket: Send + Sized {
    /// The associated context.
    type Context: NethunsContext;
    /// The token returned by `recv()`.
    type Token: NethunsToken<Context = Self::Context>;

    type Flags: NethunsFlags + Clone;

    /// Receives a packet and returns a token.
    fn recv(&mut self) -> anyhow::Result<Self::Token>;

    /// Sends a packet. The packet is provided as a slice.
    fn send(&mut self, packet: &[u8]) -> anyhow::Result<()>;

    /// Flush any pending operations (for example, ensuring that the TX ring
    /// is synchronized or that completions are processed).
    fn flush(&mut self);

    /// Return a reference to the socket’s context, if needed (for example,
    /// to load a payload token).

    fn create(portspec: &str, filter: Option<()>, flags: Self::Flags) -> anyhow::Result<(Self::Context, Self)>;
}



pub trait NethunsFlags {}

// pub enum NethunsFlags {
//     Netmap(NetmapFlags),
//     AfXdp(AfXdpFlags),
// }