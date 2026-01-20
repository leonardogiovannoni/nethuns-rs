//! Token and Payload types for zero-copy packet access.

use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};

use super::buffer::BufferDesc;
use super::context::Context;

/// A token representing ownership of a packet buffer.
/// 
/// The token must be consumed (via [`Token::consume`]) or released back to the pool.
/// Dropping a token without consuming it will panic in debug builds.
pub struct Token {
    pub(crate) idx: BufferDesc,
    pub(crate) len: u32,
    pub(crate) buffer_pool: u32,
}

impl Token {
    /// Creates a new token.
    pub fn new(idx: BufferDesc, buffer_pool: u32, len: u32) -> Self {
        Self {
            idx,
            len,
            buffer_pool,
        }
    }

    /// Returns the buffer descriptor for this token.
    pub fn buffer_desc(&self) -> BufferDesc {
        self.idx
    }

    /// Returns the size of the packet data.
    pub fn size(&self) -> u32 {
        self.len
    }

    /// Returns the pool ID this token belongs to.
    pub fn pool_id(&self) -> u32 {
        self.buffer_pool
    }

    /// Validates that this token belongs to the given context.
    pub fn check_token<Ctx: Context>(&self, ctx: &Ctx) -> bool {
        ctx.check_token(self)
    }

    /// Consumes the token and returns a [`Payload`] that provides access to the packet data.
    pub fn consume<'ctx, Ctx: Context>(self, ctx: &'ctx Ctx) -> Payload<'ctx, Ctx> {
        ctx.packet(self)
    }
}

/// A smart pointer to packet data that automatically releases the buffer on drop.
/// 
/// `Payload` implements [`Deref`] and [`DerefMut`] to provide access to the underlying
/// packet bytes as a `[u8]` slice.
#[repr(C)]
pub struct Payload<'ctx, Ctx: Context> {
    pub(crate) token: ManuallyDrop<Token>,
    ctx: &'ctx Ctx,
}

impl<'ctx, Ctx: Context> Payload<'ctx, Ctx> {
    /// Creates a new payload from a token and context.
    pub fn new(token: Token, ctx: &'ctx Ctx) -> Self {
        Self {
            token: ManuallyDrop::new(token),
            ctx,
        }
    }

    /// Converts this payload back into a token without releasing the buffer.
    /// 
    /// This is useful when you need to transfer ownership to another context.
    pub fn into_token(self) -> Token {
        let mut me = ManuallyDrop::new(self);
        let token = &mut me.token;
        let rv = unsafe { std::ptr::read(token) };
        ManuallyDrop::into_inner(rv)
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
