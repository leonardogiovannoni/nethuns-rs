//! Socket trait and related types.

use std::fmt::Debug;

use super::Result;
use super::context::Context;
use super::metadata::Metadata;
use super::token::{Payload, Token};

/// Trait for backend-specific socket configuration flags.
pub trait Flags: Clone + Debug {}

/// A network socket that can send and receive packets.
///
/// Each backend implements this trait to provide network I/O capabilities.
///
/// # Zero-Copy with Tokens
///
/// For inter-thread packet transfer, use [`recv_token`](Socket::recv_token) to get a [`Token`]
/// that can be sent through any channel (e.g., `ringbuf`, `flume`). The receiving thread
/// can then consume the token with the socket's context to access the packet data:
///
/// ```ignore
/// // Producer thread
/// let (token, meta) = socket.recv_token()?;
/// producer.push(token);
///
/// // Consumer thread  
/// let token = consumer.pop()?;
/// let payload = token.consume(socket.context());
/// ```
pub trait Socket: Send + Sized {
    /// The context type that manages buffer pools for this socket.
    type Context: Context;
    /// The metadata type returned with each received packet.
    type Metadata: Metadata;
    /// The flags type for configuring this socket.
    type Flags: Flags;

    /// Receives a packet, returning the payload and metadata.
    ///
    /// This is a convenience method that calls [`recv_token`](Socket::recv_token) and consumes the token.
    fn recv(&self) -> Result<(Payload<'_, Self::Context>, Self::Metadata)> {
        let (token, meta) = self.recv_token()?;
        Ok((token.consume(self.context()), meta))
    }

    /// Receives a packet, returning a token and metadata.
    ///
    /// The token represents ownership of a packet buffer. It must be either:
    /// - Consumed via [`Token::consume`] to access the packet data, or
    /// - Sent to another thread for processing
    ///
    /// The buffer is automatically released when the resulting [`Payload`] is dropped.
    fn recv_token(&self) -> Result<(Token, Self::Metadata)>;

    /// Sends a packet.
    fn send(&self, packet: &[u8]) -> Result<()>;

    /// Flushes any pending transmissions.
    fn flush(&self);

    /// Creates a new socket bound to the given port specification.
    fn create(portspec: &str, queue: Option<usize>, flags: Self::Flags) -> Result<Self>;

    /// Returns a reference to this socket's context.
    fn context(&self) -> &Self::Context;
}
