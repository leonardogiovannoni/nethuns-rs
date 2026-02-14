// pcap_backend.rs
//! Glue to fit the `pcap` crate into the provided Socket/Context/Token API.

use std::{
    cell::RefCell,
    sync::atomic::{AtomicUsize, Ordering},
};

use pcap::{Active, Capture, Device, Offline, Packet};

use crate::api::{
    BufferDesc, Context, Flags as FlagsTrait, Metadata, MetadataType, Result, Socket, Token,
};

/// -------- Flags ------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct PcapFlags {
    /// Snaplen passed to libpcap.
    pub snaplen: i32,
    /// Promiscuous mode.
    pub promiscuous: bool,
    /// Read timeout in milliseconds (for live captures).
    pub timeout_ms: i32,
    /// libpcap immediate mode (deliver packets as soon as they arrive).
    pub immediate: bool,
    /// Optional BPF filter (tcpdump syntax).
    pub filter: Option<String>,
    /// Size of each buffer in the pool (bytes).
    pub buffer_size: usize,
    /// Initial number of buffers to preallocate.
    pub buffer_count: usize,
}

impl Default for PcapFlags {
    fn default() -> Self {
        Self {
            snaplen: 65535,
            promiscuous: true,
            timeout_ms: 1,
            immediate: true,
            filter: None,
            buffer_size: 2048,
            buffer_count: 32,
        }
    }
}

impl FlagsTrait for PcapFlags {}

/// -------- Metadata ----------------------------------------------------------------

// If you need per-packet pcap metadata, create a PcapMeta and add
// a `MetadataType::Pcap(PcapMeta)` variant to your enum. For now we
// supply unit `()` and mark conversion as unreachable.

pub struct Meta {
    pub timestamp: libc::timeval,
    pub len: u32,
    pub caplen: u32,
}

impl Metadata for Meta {
    fn into_enum(self) -> MetadataType {
        MetadataType::Pcap(self)
    }
}

/// -------- Context + Pool -----------------------------------------------------------

static NEXT_POOL_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone, Debug)]
pub struct PcapContext {
    pool_id: u32,
    buf_capacity: usize,
}

impl PcapContext {
    fn new(buf_size: usize, _count: usize) -> Self {
        let pool_id = NEXT_POOL_ID.fetch_add(1, Ordering::Relaxed) as u32;
        Self {
            pool_id,
            buf_capacity: buf_size,
        }
    }
}

impl Context for PcapContext {
    fn pool_id(&self) -> u32 {
        self.pool_id
    }

    /// # Safety
    /// Caller (your `Payload`) guarantees exclusive access to this buffer
    /// until `release()` is called via `Drop`.
    unsafe fn unsafe_buffer(&self, buf_idx: BufferDesc, size: usize) -> *mut [u8] {
        let ptr = usize::from(buf_idx) as *mut u8;
        std::ptr::slice_from_raw_parts_mut(ptr, size)
    }

    fn release(&self, buf_idx: BufferDesc) {
        let ptr = usize::from(buf_idx) as *mut u8;
        unsafe {
            let _ = Box::from_raw(std::slice::from_raw_parts_mut(ptr, self.buf_capacity));
        }
    }
}

/// -------- Socket -------------------------------------------------------------------

enum PcapInner {
    Live(Capture<Active>),
    Offline(Capture<Offline>),
}

pub struct Sock {
    ctx: PcapContext,
    inner: RefCell<PcapInner>,
}

impl Sock {
    fn next_packet<'a>(
        cap: &'a mut Capture<Active>,
    ) -> std::result::Result<Packet<'a>, pcap::Error> {
        cap.next_packet()
    }

    fn next_packet_offline<'a>(
        cap: &'a mut Capture<Offline>,
    ) -> std::result::Result<Packet<'a>, pcap::Error> {
        cap.next_packet()
    }
}

impl Socket for Sock {
    type Context = PcapContext;
    type Metadata = Meta;
    type Flags = PcapFlags;

    fn recv_token(&self) -> Result<(Token, Self::Metadata)> {
        let ctx = &self.ctx;
        let mut inner = self.inner.borrow_mut();
        let pkt = match &mut *inner {
            PcapInner::Live(cap) => Self::next_packet(cap).map_err(crate::errors::Error::from)?,
            PcapInner::Offline(cap) => {
                Self::next_packet_offline(cap).map_err(crate::errors::Error::from)?
            }
        };
        let data = pkt.data;
        let len = data.len();
        let meta = Meta {
            timestamp: pkt.header.ts,
            len: pkt.header.len,
            caplen: pkt.header.caplen,
        };

        let mut res = vec![0u8; ctx.buf_capacity].into_boxed_slice();
        res[..len].copy_from_slice(data);
        let ptr = Box::into_raw(res) as *mut u8;
        let buf_desc = BufferDesc(ptr as usize);
        let token = Token::new(buf_desc, ctx.pool_id(), len as u32);
        Ok((token, meta))
    }

    fn send(&self, packet: &[u8]) -> Result<()> {
        match &mut *self.inner.borrow_mut() {
            PcapInner::Live(cap) => cap.sendpacket(packet).map_err(crate::errors::Error::from),
            PcapInner::Offline(_) => {
                // Err(err_str("pcap offline captures cannot send packets")),
                panic!("pcap offline captures cannot send packets")
            }
        }
    }

    fn flush(&self) {
        // libpcap doesn't buffer sends in a way we can flush here; no-op.
    }

    fn create(portspec: &str, _queue: Option<usize>, flags: Self::Flags) -> Result<Self> {
        let ctx = PcapContext::new(flags.buffer_size, flags.buffer_count);

        // Offline path?
        let is_file = portspec.starts_with("file:")
            || portspec.ends_with(".pcap")
            || portspec.ends_with(".pcapng");

        let inner = if is_file {
            let path = portspec.strip_prefix("file:").unwrap_or(portspec);
            let cap = Capture::from_file(path).map_err(crate::errors::Error::from)?;
            PcapInner::Offline(cap)
        } else {
            // Live device
            // Accept both a literal device name or "any".
            let dev = Device::from(portspec);
            let mut inactive = Capture::from_device(dev).map_err(crate::errors::Error::from)?;
            inactive = inactive
                .promisc(flags.promiscuous)
                .snaplen(flags.snaplen)
                .timeout(flags.timeout_ms);
            if flags.immediate {
                // not all libpcap builds support immediate mode; ignore if unsupported
                inactive = inactive.immediate_mode(true);
            }
            let mut cap = inactive.open().map_err(crate::errors::Error::from)?;

            if let Some(expr) = flags.filter.as_deref() {
                // Optimize=true, netmask=0 lets libpcap query it
                cap.filter(expr, true).map_err(crate::errors::Error::from)?;
            }

            PcapInner::Live(cap)
        };

        let inner = RefCell::new(inner);

        Ok(Self { ctx, inner })
    }

    fn context(&self) -> &Self::Context {
        &self.ctx
    }
}
