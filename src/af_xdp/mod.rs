mod wrapper;
use crate::api;
use crate::api::Result;
use crate::errors::Error;
use libc::{self, _SC_PAGESIZE, sysconf};
use std::alloc::{self, Layout};
use std::cell::{Cell, RefCell, UnsafeCell};
use std::ffi::CString;
use std::io::{self, ErrorKind};
use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use wrapper::{TxSlot, Umem, XdpDescData, XskSocket};
const RX_BATCH_SIZE: usize = 32;

pub fn resultify(x: i32) -> io::Result<u32> {
    match x >= 0 {
        true => Ok(x as u32),
        false => Err(io::Error::from_raw_os_error(-x)),
    }
}

#[derive(Clone)]
pub struct Ctx {
    buffer: UmemArea,
    producer: RefCell<mpsc::Producer<api::BufferRef>>,
    index: u32,
}

impl Ctx {
    fn new(nbufs: usize, buffer_pool: UmemArea) -> (Self, mpsc::Consumer<api::BufferRef>) {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let (producer, cons) = mpsc::channel(nbufs);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let res = Self {
            buffer: buffer_pool, //: Arc::new(buffer_pool),
            producer: RefCell::new(producer),
            index: counter,
        };
        (res, cons)
    }

    unsafe fn buffer(&self, idx: api::BufferRef, size: usize) -> *mut [u8] {
        let (ptr, _) = self.buffer.raw_parts();
        let offset = usize::from(idx) as usize;
        unsafe {
            let start = ptr.add(offset);
            std::slice::from_raw_parts_mut(start, size)
        }
    }
}

impl api::Context for Ctx {
    type Token = Tok;

    fn release(&self, buf_idx: api::BufferRef) {
        self.producer.borrow_mut().push(buf_idx);
    }

    unsafe fn unsafe_buffer(&self, buf_idx: api::BufferRef, size: usize) -> *mut [u8] {
        unsafe { self.buffer(buf_idx, size) }
    }

    fn pool_id(&self) -> u32 {
        self.index
    }
}

#[derive(Default, Clone, Copy)]
struct StatsRecord {
    timestamp: u64,
    rx_packets: u64,
    rx_bytes: u64,
    tx_packets: u64,
    tx_bytes: u64,
}

#[derive(Clone)]
pub struct UmemArea {
    mem: Arc<UnsafeCell<*mut u8>>,
    size: usize,
}

unsafe impl Send for UmemArea {}

impl UmemArea {
    fn new(packet_buffer_size: usize) -> Result<UmemArea> {
        let packet_buffer = alloc_page_aligned(packet_buffer_size)?;

        let mem = Arc::new(UnsafeCell::new(packet_buffer));

        Ok(Self {
            mem,
            size: packet_buffer_size,
        })
    }

    fn raw_parts(&self) -> (*mut u8, usize) {
        unsafe { (*self.mem.get(), self.size) }
    }
}

/// Wraps the XDP UMEM info.
/// Now handles fill/completion ring and frame addresses internally.
struct UmemManager {
    umem: Umem,
    consumer: mpsc::Consumer<api::BufferRef>,
}

impl UmemManager {
    pub fn create_with_buffer(
        umem: UmemArea,
        consumer: mpsc::Consumer<api::BufferRef>,
    ) -> Result<Self> {
        Ok(Self {
            umem: Umem::new(umem).map_err(|err| Error::Generic(err))?,
            consumer,
        })
    }

    /// Allocates one frame address from our free array.
    fn alloc_frame(&mut self) -> Option<u32> {
        // self.frames.pop()
        self.consumer.pop().map(|idx| usize::from(idx) as u32)
    }

    // Lo userei quando fallisce in qualche modo la read o la write
    pub fn refill_fill_ring(&mut self) -> io::Result<()> {
        // We cannot exceed the number of free frames we hold.
        let wanted = {
            let mut wanted = self.consumer.available_len();
            if wanted == 0 {
                self.consumer.sync();
                wanted = self.consumer.available_len();
            }
            wanted
        } as u32;

        if wanted == 0 {
            return Ok(());
        }

        let available = self.umem.ring_prod_mut().nb_free(wanted);
        if available == 0 {
            return Ok(()); // ring is full or no space
        }
        let (reserved, idx) = self.umem.ring_prod_mut().reserve(available);
        if reserved != available {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "refill_fill_ring: not enough descriptors reserved",
            ));
        }

        for i in 0..available {
            let addr = self.alloc_frame().ok_or_else(|| {
                io::Error::new(io::ErrorKind::Other, "refill_fill_ring: no free frames")
            })?;
            *self.umem.ring_prod_mut().get_addr(idx + i) = addr as u64;
        }

        self.umem.ring_prod_mut().submit(available);
        Ok(())
    }
} //

fn complete_tx(xsk: &Sock) -> io::Result<()> {
    let umem = &mut xsk.umem_manager.borrow_mut().umem;
    let (completed, mut idx) = umem.ring_cons_mut().peek(64);
    if completed == 0 {
        return Ok(());
    }
    // For each completion, get the “addr” (which was the frame address) and
    // recycle it into your free/producer ring so it can be used again.
    for _ in 0..completed {
        let addr = umem.ring_cons_mut().get_addr(idx);
        idx += 1;
        xsk.ctx
            .producer
            .borrow_mut()
            .push(api::BufferRef::from(addr as usize));
    }
    umem.ring_cons_mut().release(completed);
    xsk.ctx.producer.borrow_mut().flush();

    Ok(())
}

pub struct Tok {
    idx: api::BufferRef,
    len: u32,
    buffer_pool: u32,
}

impl api::Token for Tok {
    type Context = Ctx;

    fn buffer_idx(&self) -> api::BufferRef {
        self.idx
    }

    fn size(&self) -> u32 {
        self.len
    }

    fn pool_id(&self) -> u32 {
        self.buffer_pool
    }
}

impl Tok {
    fn new(idx: u64, buffer_pool: u32, len: u32) -> ManuallyDrop<Self> {
        let idx: usize = idx.try_into().unwrap();
        let idx = api::BufferRef::from(idx);
        ManuallyDrop::new(Self {
            idx,
            len,
            buffer_pool,
        })
    }
}

fn ifname_to_ifindex(ifname: &str) -> Option<u32> {
    let c_ifname = CString::new(ifname).ok()?;
    let index = unsafe { libc::if_nametoindex(c_ifname.as_ptr()) };
    if index == 0 { None } else { Some(index) }
}

/// Wraps an AF_XDP socket.
pub struct Sock {
    ctx: Ctx,
    xsk: RefCell<XskSocket>,
    outstanding_tx: u32,
    umem_manager: RefCell<UmemManager>,
    stats: Cell<StatsRecord>,
    prev_stats: Cell<StatsRecord>,
}

impl Sock {
    #[inline(always)]
    fn recv_inner(&self, slot: XdpDescData) -> Result<(Tok, Meta)> {
        let offset = slot.offset;
        let len = slot.len;
        let options = slot.options;
        let mut stats = self.stats.get();
        stats.rx_bytes += len as u64;
        stats.rx_packets += 1;
        self.stats.set(stats);

        let buffer_pool = self.ctx.index;
        let token = Tok::new(offset, buffer_pool, len);
        let meta = Meta {};
        Ok((ManuallyDrop::into_inner(token), meta))
    }

    fn send_inner<'a>(&self, mut slot: TxSlot<'a>, payload: &[u8]) -> Result<()> {
        let frame_addr = self
            .umem_manager
            .borrow_mut()
            .alloc_frame()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No free frames for TX"))?;

        // Assign the descriptor’s address
        *slot.offset_mut() = frame_addr as u64;
        *slot.len_mut() = payload.len() as u32;

        // Actually copy the packet into UMEM
        let buffer_index = api::BufferRef::from(frame_addr as usize);
        let buf = unsafe { self.ctx.buffer(buffer_index, payload.len()) };

        unsafe {
            (*buf).copy_from_slice(payload);
        }

        // Update stats, etc.
        // self.stats.update(|mut s| {
        //     s.tx_bytes += payload.len() as u64;
        //     s.tx_packets += 1;
        //     s
        // });
        let mut stats = self.stats.get();
        stats.tx_bytes += payload.len() as u64;
        stats.tx_packets += 1;
        self.stats.set(stats);

        Ok(())
    }

    fn print_stats(&self) {
        fn calc_period(r: &StatsRecord, p: &StatsRecord) -> f64 {
            let period = r.timestamp - p.timestamp;
            if period > 0 {
                (period as f64) / 1e9
            } else {
                0.0
            }
        }
        let stats_rec = &self.stats.get();
        let stats_prev = &self.prev_stats.get();
        let mut period = calc_period(stats_rec, stats_prev);
        if period == 0.0 {
            period = 1.0;
        }
        let rx_pkts = stats_rec.rx_packets - stats_prev.rx_packets;
        let rx_pps = rx_pkts as f64 / period;
        let rx_bytes = stats_rec.rx_bytes - stats_prev.rx_bytes;
        let rx_bps = (rx_bytes * 8) as f64 / period / 1_000_000.0;
        println!(
            "AF_XDP RX: {:11} pkts ({:10.0} pps) {:11} Kbytes ({:6.0} Mbits/s) period:{}",
            stats_rec.rx_packets,
            rx_pps,
            stats_rec.rx_bytes / 1000,
            rx_bps,
            period
        );

        let tx_pkts = stats_rec.tx_packets - stats_prev.tx_packets;
        let tx_pps = tx_pkts as f64 / period;
        let tx_bytes = stats_rec.tx_bytes - stats_prev.tx_bytes;
        let tx_bps = (tx_bytes * 8) as f64 / period / 1_000_000.0;
        println!(
            "       TX: {:11} pkts ({:10.0} pps) {:11} Kbytes ({:6.0} Mbits/s) period:{}",
            stats_rec.tx_packets,
            tx_pps,
            stats_rec.tx_bytes / 1000,
            tx_bps,
            period
        );
        println!();
    }
}

impl api::Socket for Sock {
    type Context = Ctx;
    type Metadata = Meta;
    type Flags = AfXdpFlags;
    fn recv_token(&mut self) -> Result<(<Self::Context as api::Context>::Token, Self::Metadata)> {
        let mut rx = self.xsk.borrow_mut();
        if let Some(slot) = rx.rx_mut().next() {
            self.recv_inner(slot)
        } else {
            self.umem_manager.borrow_mut().refill_fill_ring()?;
            let tmp = rx
                .rx_mut()
                .next()
                .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No packets"))?;
            self.recv_inner(tmp)
        }
    }

    fn send(&mut self, packet: &[u8]) -> Result<()> {
        if let Some(slot) = self.xsk.borrow_mut().tx_mut().iter().next() {
            self.send_inner(slot, packet)?
        } else {
            self.flush();
            if let Some(slot) = self.xsk.borrow_mut().tx_mut().iter().next() {
                self.send_inner(slot, packet)?
            } else {
                return Err(Error::NoMemory);
            }
        }
        Ok(())
    }

    fn flush(&mut self) {
        unsafe {
            self.xsk.borrow_mut().tx_mut().iter().sync();
        }

        complete_tx(self).unwrap();
        unsafe {
            libc::sendto(
                self.xsk.borrow().fd(),
                std::ptr::null_mut(),
                0,
                libc::MSG_DONTWAIT,
                std::ptr::null_mut(),
                0,
            )
        };
    }

    fn create(portspec: &str, queue: Option<usize>, flags: Self::Flags) -> Result<Self> {
        let xdp_flags = flags.xdp_flags;
        let bind_flags = flags.bind_flags;
        let num_frames = flags.num_frames;
        let frame_size = flags.frame_size;
        let umem_bytes_len = (num_frames * frame_size) as usize;
        let umem = UmemArea::new(umem_bytes_len as usize)?;
        let (ctx, consumer) = Ctx::new(num_frames as usize, umem.clone());

        for i in 0..num_frames {
            let prod = &mut *ctx.producer.borrow_mut();
            prod.push(api::BufferRef::from((i as usize) * frame_size as usize));
        }
        {
            let prod = &mut *ctx.producer.borrow_mut();
            prod.flush();
        }
        let mut umem_manager = UmemManager::create_with_buffer(umem.clone(), consumer)?;

        let socket = unsafe {
            XskSocket::create(
                &mut umem_manager.umem,
                portspec,
                queue.unwrap_or(0) as u32,
                xdp_flags,
                bind_flags,
            )?
        };

        let ifindex = ifname_to_ifindex(&portspec).unwrap();

        umem_manager.refill_fill_ring()?;
        Ok(Self {
            ctx,
            xsk: RefCell::new(socket),
            outstanding_tx: 0,
            umem_manager: RefCell::new(umem_manager),
            stats: Cell::new(StatsRecord::default()),
            prev_stats: Cell::new(StatsRecord::default()),
        })
    }

    fn context(&self) -> &Self::Context {
        &self.ctx
    }
}

#[derive(Clone, Debug)]
pub struct AfXdpFlags {
    pub bind_flags: u16,
    pub xdp_flags: u32,
    pub num_frames: u32,
    pub frame_size: u32,
}

impl api::Flags for AfXdpFlags {}

/// Get the current time in nanoseconds (monotonic clock).
fn gettime() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let res = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    if res < 0 {
        eprintln!("Error with clock_gettime");
        std::process::exit(1);
    }
    (ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64)
}

pub fn alloc_page_aligned(size: usize) -> io::Result<*mut u8> {
    if size == 0 {
        return Err(io::Error::new(ErrorKind::InvalidInput, "Invalid size"));
    }
    let page_size = unsafe { sysconf(_SC_PAGESIZE) };
    if page_size < 0 {
        return Err(io::Error::last_os_error());
    }
    let page_size = page_size as usize;

    let layout = Layout::from_size_align(size, page_size)
        .map_err(|_| io::Error::new(ErrorKind::InvalidInput, "Invalid layout"))?;

    // Allocate the memory.
    let ptr = unsafe { alloc::alloc(layout) };

    // Check for allocation failure (ptr::null_mut on OOM).
    if ptr.is_null() {
        Err(io::Error::new(
            ErrorKind::Other,
            "Allocation with page alignment failed",
        ))
    } else {
        Ok(ptr)
    }
}

pub struct Meta {}

impl api::Metadata for Meta {}

#[cfg(test)]
mod tests {
    use libxdp_sys::XSK_UMEM__DEFAULT_FRAME_SIZE;

    use crate::api::Socket;

    use super::*;

    #[test]
    fn test_send_with_flush() {
        let mut socket0 = Sock::create(
            "veth0af_xdp",
            Some(0),
            AfXdpFlags {
                xdp_flags: 0,
                bind_flags: 0,
                frame_size: XSK_UMEM__DEFAULT_FRAME_SIZE,
                num_frames: 4096 * 8,
            },
        )
        .unwrap();
        let mut socket1 = Sock::create(
            "veth1af_xdp",
            Some(0),
            AfXdpFlags {
                xdp_flags: 0,
                bind_flags: 0,
                frame_size: XSK_UMEM__DEFAULT_FRAME_SIZE,
                num_frames: 4096,
            },
        )
        .unwrap();
        socket1.send(b"Helloworldmyfriend\0\0\0\0\0\0\0").unwrap();
        socket1.flush();
        let (packet, meta) = socket0.recv().unwrap();
        assert_eq!(&packet[..20], b"Helloworldmyfriend\0\0");
    }
}
