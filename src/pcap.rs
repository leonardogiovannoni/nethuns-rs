// pcap_backend.rs
//! Glue to fit the `pcap` crate into the provided Socket/Context/Token API.

use std::{
    cell::RefCell,
    fs::File,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use crossbeam_queue::ArrayQueue;
use pcap::{Active, Capture, Device, Packet};
use pcap_parser::{create_reader, traits::PcapReaderIterator, PcapBlockOwned, PcapError};

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
    // Thread-safe MPMC queue for buffer recycling.
    // Stores raw pointers (as usize) to buffers.
    pool: Arc<ArrayQueue<usize>>,
}

impl PcapContext {
    fn new(buf_size: usize, buf_count: usize) -> Self {
        let pool_id = NEXT_POOL_ID.fetch_add(1, Ordering::Relaxed) as u32;
        let pool = Arc::new(ArrayQueue::new(buf_count));

        // Pre-allocate buffers
        for _ in 0..buf_count {
            let buf = vec![0u8; buf_size].into_boxed_slice();
            let ptr = Box::into_raw(buf) as *mut u8 as usize;
            let _ = pool.push(ptr);
        }

        Self {
            pool_id,
            buf_capacity: buf_size,
            pool,
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
        let ptr = usize::from(buf_idx);
        // Try to return the buffer to the pool.
        if let Err(returned_ptr) = self.pool.push(ptr) {
            // If the pool is full (e.g., due to extra allocations during bursts),
            // we must deallocate the buffer to avoid leaks.
            unsafe {
                let _ = Box::from_raw(std::slice::from_raw_parts_mut(
                    returned_ptr as *mut u8,
                    self.buf_capacity,
                ));
            }
        }
    }
}

/// -------- Socket -------------------------------------------------------------------

enum PcapInner {
    Live(Capture<Active>),
    Offline(Box<dyn PcapReaderIterator + Send>),
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

    fn next_packet_offline(
        reader: &mut Box<dyn PcapReaderIterator + Send>,
        buffer: &mut [u8],
    ) -> std::result::Result<(u32, Meta), crate::errors::Error> {
        loop {
            match reader.next() {
                Ok((offset, block)) => {
                    match block {
                        PcapBlockOwned::Legacy(packet) => {
                            let len = packet.origlen;
                            let caplen = packet.data.len() as u32;
                            let copy_len = std::cmp::min(caplen as usize, buffer.len());
                            buffer[..copy_len].copy_from_slice(&packet.data[..copy_len]);

                            let meta = Meta {
                                timestamp: libc::timeval {
                                    tv_sec: packet.ts_sec as i64,
                                    tv_usec: packet.ts_usec as i32,
                                },
                                len,
                                caplen,
                            };
                            reader.consume(offset);
                            return Ok((copy_len as u32, meta));
                        }
                        PcapBlockOwned::NG(block) => {
                            match block {
                                pcap_parser::Block::EnhancedPacket(packet) => {
                                    let len = packet.origlen;
                                    let caplen = packet.data.len() as u32;
                                    let copy_len = std::cmp::min(caplen as usize, buffer.len());
                                    buffer[..copy_len].copy_from_slice(&packet.data[..copy_len]);

                                    let raw_ts = (packet.ts_high as u64) << 32 | (packet.ts_low as u64);
                                    
                                    // Try to determine if timestamp is microseconds or nanoseconds.
                                    // Modern timestamps (e.g. 2024) in microseconds are ~1.7e15
                                    // In nanoseconds they are ~1.7e18
                                    // We use a threshold of 1e16 to distinguish.
                                    let (tv_sec, tv_usec) = if raw_ts < 10_000_000_000_000_000 {
                                        // Microseconds
                                        ((raw_ts / 1_000_000) as i64, (raw_ts % 1_000_000) as i32)
                                    } else {
                                        // Nanoseconds
                                        ((raw_ts / 1_000_000_000) as i64, ((raw_ts % 1_000_000_000) / 1000) as i32)
                                    };

                                    let meta = Meta {
                                        timestamp: libc::timeval {
                                            tv_sec,
                                            tv_usec: tv_usec as _,
                                        },
                                        len,
                                        caplen,
                                    };
                                    reader.consume(offset);
                                    return Ok((copy_len as u32, meta));
                                }
                                pcap_parser::Block::SimplePacket(packet) => {
                                    let len = packet.origlen;
                                    let caplen = packet.data.len() as u32;
                                    let copy_len = std::cmp::min(caplen as usize, buffer.len());
                                    buffer[..copy_len].copy_from_slice(&packet.data[..copy_len]);

                                    let meta = Meta {
                                        timestamp: libc::timeval {
                                            tv_sec: 0,
                                            tv_usec: 0,
                                        },
                                        len,
                                        caplen,
                                    };
                                    reader.consume(offset);
                                    return Ok((copy_len as u32, meta));
                                }
                                _ => {
                                    // Skip other blocks (headers, interfaces, stats)
                                    reader.consume(offset);
                                    continue;
                                }
                            }
                        }
                        PcapBlockOwned::LegacyHeader(_) => {
                            reader.consume(offset);
                            continue;
                        }
                    }
                }
                Err(PcapError::Eof) => {
                    return Err(crate::errors::Error::Pcap(pcap::Error::NoMorePackets));
                }
                Err(PcapError::Incomplete(_)) => {
                    reader.refill().map_err(|e| {
                        crate::errors::Error::Pcap(pcap::Error::PcapError(format!("{:?}", e)))
                    })?;
                    continue;
                }
                Err(e) => {
                    // Map pcap-parser error to something generic or print it
                    eprintln!("Pcap parser error: {:?}", e);
                    return Err(crate::errors::Error::Pcap(pcap::Error::PcapError(
                        "Parser error".to_string(),
                    )));
                }
            }
        }
    }
}

impl Socket for Sock {
    type Context = PcapContext;
    type Metadata = Meta;
    type Flags = PcapFlags;

    fn recv_token(&self) -> Result<(Token, Self::Metadata)> {
        let ctx = &self.ctx;
        let mut inner = self.inner.borrow_mut();

        // 1. Acquire a buffer from the pool (or allocate if empty)
        let ptr = if let Some(addr) = ctx.pool.pop() {
            addr as *mut u8
        } else {
            // Fallback: pool is empty, allocate a new buffer.
            let buf = vec![0u8; ctx.buf_capacity].into_boxed_slice();
            Box::into_raw(buf) as *mut u8
        };

        // 2. Read packet from pcap directly into buffer
        // SAFETY: We own the buffer `ptr`.
        let (len, meta) = unsafe {
            let slice = std::slice::from_raw_parts_mut(ptr, ctx.buf_capacity);
            match &mut *inner {
                PcapInner::Live(cap) => {
                    let pkt = Self::next_packet(cap).map_err(crate::errors::Error::from)?;
                    let meta = Meta {
                        timestamp: pkt.header.ts,
                        len: pkt.header.len,
                        caplen: pkt.header.caplen,
                    };
                    let copy_len = std::cmp::min(pkt.data.len(), slice.len());
                    slice[..copy_len].copy_from_slice(&pkt.data[..copy_len]);
                    (copy_len as u32, meta)
                }
                PcapInner::Offline(reader) => Self::next_packet_offline(reader, slice)?,
            }
        };

        // 3. Create Token
        let buf_desc = BufferDesc(ptr as usize);
        let token = Token::new(buf_desc, ctx.pool_id(), len);

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
            let file = File::open(path).map_err(|e| {
                crate::errors::Error::Pcap(pcap::Error::PcapError(e.to_string()))
            })?;

            // Create reader using pcap-parser's autodetection.
            // Requires pcap-parser >= 0.16.0 (or 0.17.0) to ensure Send trait on return type.
            let reader = create_reader(1000000, file).map_err(|e| {
                crate::errors::Error::Pcap(pcap::Error::PcapError(format!("{:?}", e)))
            })?;

            PcapInner::Offline(reader)
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