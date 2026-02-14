//! Context trait and related utilities.

use super::buffer::BufferDesc;
use super::hint::unlikely;
use super::token::{Payload, Token};

/// A context manages a pool of packet buffers.
///
/// Each backend (AF_XDP, netmap, DPDK, pcap) implements this trait to provide
/// buffer management for its specific memory model.
pub trait Context: Sized + Clone + Send + 'static {
    /// Checks if the given token belongs to this context.
    #[inline(always)]
    fn check_token(&self, token: &Token) -> bool {
        token.pool_id() == self.pool_id()
    }

    /// Converts a token into a payload, validating ownership.
    ///
    /// # Panics
    ///
    /// Panics if the token does not belong to this context.
    fn packet<'ctx>(&'ctx self, token: Token) -> Payload<'ctx, Self> {
        if unlikely(!self.check_token(&token)) {
            panic!("Invalid token");
        }
        Payload::new(token, self)
    }

    /// Returns the unique identifier for this context's buffer pool.
    fn pool_id(&self) -> u32;

    /// Returns a raw pointer to the buffer data.
    ///
    /// # Safety
    ///
    /// This function is safe if Context is !Freeze and the caller ensures
    /// correct ownership and lifetime management of the buffer.
    unsafe fn unsafe_buffer(&self, buf_idx: BufferDesc, size: usize) -> *mut [u8];

    /// Releases a buffer back to the pool.
    fn release(&self, buf_idx: BufferDesc);
}
