// enum_api.rs
// This file contains enum definitions for the nethuns-rs API

use super::{Metadata, Result, Context, Socket, Token};

/// Enum version of Socket trait implementations
#[non_exhaustive]
pub enum EnumSocket {
    #[cfg(feature = "netmap")]
    Netmap(crate::netmap::Sock),
    #[cfg(feature = "af_xdp")]
    AfXdp(crate::af_xdp::Sock),
    #[cfg(feature = "dpdk")]
    Dpdk(crate::dpdk::Sock),
    #[cfg(feature = "pcap")]
    Pcap(crate::pcap::Sock),
}

/// Enum version of Flags implementations
#[non_exhaustive]
pub enum EnumFlags {
    #[cfg(feature = "netmap")]
    Netmap(crate::netmap::NetmapFlags),
    #[cfg(feature = "af_xdp")]
    AfXdp(crate::af_xdp::AfXdpFlags),
    #[cfg(feature = "dpdk")]
    Dpdk(crate::dpdk::DpdkFlags),
    #[cfg(feature = "pcap")]
    Pcap(crate::pcap::PcapFlags),
}

/// Enum version of Metadata trait implementations
#[non_exhaustive]
pub enum EnumMetadata {
    #[cfg(feature = "netmap")]
    Netmap(crate::netmap::Meta),
    #[cfg(feature = "af_xdp")]
    AfXdp(crate::af_xdp::Meta),
    #[cfg(feature = "dpdk")]
    Dpdk(crate::dpdk::Meta),
    #[cfg(feature = "pcap")]
    Pcap(crate::pcap::Meta),
}

/// Enum version of Context implementations
#[non_exhaustive]
pub enum EnumContext {
    #[cfg(feature = "netmap")]
    Netmap(crate::netmap::Ctx),
    #[cfg(feature = "af_xdp")]
    AfXdp(crate::af_xdp::Ctx),
    #[cfg(feature = "dpdk")]
    Dpdk(crate::dpdk::Ctx),
    #[cfg(feature = "pcap")]
    Pcap(crate::pcap::PcapContext),
}

impl EnumSocket {
    /// Create a socket from portspec, queue, and flags
    pub fn create(portspec: &str, queue: Option<usize>, flags: EnumFlags) -> Result<Self> {
        match flags {
            #[cfg(feature = "netmap")]
            EnumFlags::Netmap(netmap_flags) => {
                Ok(EnumSocket::Netmap(crate::netmap::Sock::create(portspec, queue, netmap_flags)?))
            },
            #[cfg(feature = "af_xdp")]
            EnumFlags::AfXdp(af_xdp_flags) => {
                Ok(EnumSocket::AfXdp(crate::af_xdp::Sock::create(portspec, queue, af_xdp_flags)?))
            },
            #[cfg(feature = "dpdk")]
            EnumFlags::Dpdk(dpdk_flags) => {
                Ok(EnumSocket::Dpdk(crate::dpdk::Sock::create(portspec, queue, dpdk_flags)?))
            },
            #[cfg(feature = "pcap")]
            EnumFlags::Pcap(pcap_flags) => {
                Ok(EnumSocket::Pcap(crate::pcap::Sock::create(portspec, queue, pcap_flags)?))
            },
        }
    }

    /// Receive a packet and metadata
    pub fn recv(&self) -> Result<(Token, EnumMetadata)> {
        match self {
            #[cfg(feature = "netmap")]      
            EnumSocket::Netmap(socket) => {
                let (token, meta) = socket.recv_token()?;
                let packet = self.token_to_packet(token);
                Ok((packet.into_token(), EnumMetadata::Netmap(meta)))
            },
            #[cfg(feature = "af_xdp")]
            EnumSocket::AfXdp(socket) => {
                let (packet, meta) = socket.recv()?;
                Ok((packet.into_token(), EnumMetadata::AfXdp(meta)))
            },
            #[cfg(feature = "dpdk")]
            EnumSocket::Dpdk(socket) => {
                let (packet, meta) = socket.recv()?;
                Ok((packet.into_token(), EnumMetadata::Dpdk(meta)))
            },
            #[cfg(feature = "pcap")]
            EnumSocket::Pcap(socket) => {
                let (packet, meta) = socket.recv()?;
                Ok((packet.into_token(), EnumMetadata::Pcap(meta)))
            },
        }
    }

    /// Send a packet
    pub fn send(&self, packet: &[u8]) -> Result<()> {
        match self {
            #[cfg(feature = "netmap")]
            EnumSocket::Netmap(socket) => socket.send(packet),
            #[cfg(feature = "af_xdp")]
            EnumSocket::AfXdp(socket) => socket.send(packet),
            #[cfg(feature = "dpdk")]
            EnumSocket::Dpdk(socket) => socket.send(packet),
            #[cfg(feature = "pcap")]
            EnumSocket::Pcap(socket) => socket.send(packet),
        }
    }

    /// Flush the socket
    pub fn flush(&self) {
        match self {
            #[cfg(feature = "netmap")]
            EnumSocket::Netmap(socket) => socket.flush(),
            #[cfg(feature = "af_xdp")]
            EnumSocket::AfXdp(socket) => socket.flush(),
            #[cfg(feature = "dpdk")]
            EnumSocket::Dpdk(socket) => socket.flush(),
            #[cfg(feature = "pcap")]
            EnumSocket::Pcap(socket) => socket.flush(),
        }
    }

    /// Get the context
    pub fn context(&self) -> EnumContext {
        match self {
            #[cfg(feature = "netmap")]
            EnumSocket::Netmap(socket) => EnumContext::Netmap(socket.context().clone()),
            #[cfg(feature = "af_xdp")]
            EnumSocket::AfXdp(socket) => EnumContext::AfXdp(socket.context().clone()),
            #[cfg(feature = "dpdk")]
            EnumSocket::Dpdk(socket) => EnumContext::Dpdk(socket.context().clone()),
            #[cfg(feature = "pcap")]
            EnumSocket::Pcap(socket) => EnumContext::Pcap(socket.context().clone()),
        }
    }

   // /// Convert a token to a packet with the appropriate context
   // pub fn token_to_packet<'a>(&'a self, token: Token) -> Packet<'a, EnumContext> {
   //     let ctx = self.context();
   //     Packet::new(token, &ctx)
   // }
}

impl Clone for EnumFlags {
    fn clone(&self) -> Self {
        match self {
            #[cfg(feature = "netmap")]
            EnumFlags::Netmap(flags) => EnumFlags::Netmap(flags.clone()),
            #[cfg(feature = "af_xdp")]
            EnumFlags::AfXdp(flags) => EnumFlags::AfXdp(flags.clone()),
            #[cfg(feature = "dpdk")]
            EnumFlags::Dpdk(flags) => EnumFlags::Dpdk(flags.clone()),
            #[cfg(feature = "pcap")]
            EnumFlags::Pcap(flags) => EnumFlags::Pcap(flags.clone()),
        }
    }
}

impl std::fmt::Debug for EnumFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "netmap")]
            EnumFlags::Netmap(flags) => flags.fmt(f),
            #[cfg(feature = "af_xdp")]
            EnumFlags::AfXdp(flags) => flags.fmt(f),
            #[cfg(feature = "dpdk")]
            EnumFlags::Dpdk(flags) => flags.fmt(f),
            #[cfg(feature = "pcap")]
            EnumFlags::Pcap(flags) => flags.fmt(f),
        }
    }
}

impl Metadata for EnumMetadata {
    fn into_enum(self) -> super::MetadataType {
        match self {
            #[cfg(feature = "netmap")]
            EnumMetadata::Netmap(meta) => meta.into_enum(),
            #[cfg(feature = "af_xdp")]
            EnumMetadata::AfXdp(meta) => meta.into_enum(),
            #[cfg(feature = "dpdk")]
            EnumMetadata::Dpdk(meta) => meta.into_enum(),
            #[cfg(feature = "pcap")]
            EnumMetadata::Pcap(meta) => meta.into_enum(),
        }
    }
}

impl Clone for EnumContext {
    fn clone(&self) -> Self {
        match self {
            #[cfg(feature = "netmap")]
            EnumContext::Netmap(ctx) => EnumContext::Netmap(ctx.clone()),
            #[cfg(feature = "af_xdp")]
            EnumContext::AfXdp(ctx) => EnumContext::AfXdp(ctx.clone()),
            #[cfg(feature = "dpdk")]
            EnumContext::Dpdk(ctx) => EnumContext::Dpdk(ctx.clone()),
            #[cfg(feature = "pcap")]
            EnumContext::Pcap(ctx) => EnumContext::Pcap(ctx.clone()),
        }
    }
}

impl Context for EnumContext {
    fn pool_id(&self) -> u32 {
        match self {
            #[cfg(feature = "netmap")]
            EnumContext::Netmap(ctx) => ctx.pool_id(),
            #[cfg(feature = "af_xdp")]
            EnumContext::AfXdp(ctx) => ctx.pool_id(),
            #[cfg(feature = "dpdk")]
            EnumContext::Dpdk(ctx) => ctx.pool_id(),
            #[cfg(feature = "pcap")]
            EnumContext::Pcap(ctx) => ctx.pool_id(),
        }
    }

    unsafe fn unsafe_buffer(&self, buf_idx: super::BufferDesc, size: usize) -> *mut [u8] {
        match self {
            #[cfg(feature = "netmap")]
            EnumContext::Netmap(ctx) => unsafe { ctx.unsafe_buffer(buf_idx, size) },
            #[cfg(feature = "af_xdp")]
            EnumContext::AfXdp(ctx) => unsafe { ctx.unsafe_buffer(buf_idx, size) },
            #[cfg(feature = "dpdk")]
            EnumContext::Dpdk(ctx) => unsafe { ctx.unsafe_buffer(buf_idx, size) },
            #[cfg(feature = "pcap")]
            EnumContext::Pcap(ctx) => unsafe { ctx.unsafe_buffer(buf_idx, size) },
        }
    }

    fn release(&self, buf_idx: super::BufferDesc) {
        match self {
            #[cfg(feature = "netmap")]
            EnumContext::Netmap(ctx) => ctx.release(buf_idx),
            #[cfg(feature = "af_xdp")]
            EnumContext::AfXdp(ctx) => ctx.release(buf_idx),
            #[cfg(feature = "dpdk")]
            EnumContext::Dpdk(ctx) => ctx.release(buf_idx),
            #[cfg(feature = "pcap")]
            EnumContext::Pcap(ctx) => ctx.release(buf_idx),
        }
    }
}
