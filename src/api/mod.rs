use std::{
    fmt::Debug,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};

use crate::{af_xdp::AfXdpFlags, dpdk::DpdkFlags, netmap::NetmapFlags};

pub type Result<T> = std::result::Result<T, crate::errors::Error>;

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
pub struct BufferRef(usize);

impl From<usize> for BufferRef {
    fn from(val: usize) -> Self {
        Self(val.into())
    }
}

impl From<BufferRef> for usize {
    fn from(val: BufferRef) -> usize {
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
            &*self
                .ctx
                .unsafe_buffer(self.token.buffer_idx(), self.token.size() as usize)
        }
    }
}

impl<'ctx, Ctx: Context> DerefMut for Payload<'ctx, Ctx> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut *self
                .ctx
                .unsafe_buffer(self.token.buffer_idx(), self.token.size() as usize)
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

pub trait Token: Sized + Send {
    type Context: Context<Token = Self>;

    fn consume<'ctx>(self, ctx: &'ctx Self::Context) -> Payload<'ctx, Self::Context> {
        ctx.packet(self)
    }

    fn buffer_idx(&self) -> BufferRef;

    fn size(&self) -> u32;

    fn pool_id(&self) -> u32;
}
pub trait Context: Sized + Clone + Send {
    type Token: Token<Context = Self>;

    #[inline(always)]
    fn check_token(&self, token: &Self::Token) -> bool {
        token.pool_id() == self.pool_id()
    }

    fn packet<'ctx>(&'ctx self, token: Self::Token) -> Payload<'ctx, Self> {
        if unlikely(!self.check_token(&token)) {
            panic!("Invalid token");
        }
        Payload::new(token, self)
    }

    fn pool_id(&self) -> u32;

    unsafe fn unsafe_buffer(&self, buf_idx: BufferRef, size: usize) -> *mut [u8];

    fn release(&self, buf_idx: BufferRef);
}

pub trait Socket: Send + Sized {
    type Context: Context;
    type Metadata: Metadata;
    type Flags: Flags;
    //    #[inline(never)]
    fn recv(&mut self) -> Result<(Payload<'_, Self::Context>, Self::Metadata)> {
        let (token, meta) = self.recv_token()?;
        Ok((token.consume(self.context()), meta))
    }

    fn recv_token(&mut self) -> Result<(<Self::Context as Context>::Token, Self::Metadata)>;

    fn send(&mut self, packet: &[u8]) -> Result<()>;

    fn flush(&mut self);

    fn create(portspec: &str, queue: Option<usize>, flags: Self::Flags) -> Result<Self>;

    fn context(&self) -> &Self::Context;
}

pub trait Metadata {}

pub trait Flags: Clone + Debug {}
//
//#[derive(Clone, Debug)]
//pub enum Flags {
//    Netmap(NetmapFlags),
//    AfXdp(AfXdpFlags),
//    DpdkFlags(DpdkFlags),
//}
