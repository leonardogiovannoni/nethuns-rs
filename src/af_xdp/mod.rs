mod wrapper;
use crate::api::{
    BufferIndex, NethunsContext, NethunsFlags, NethunsPayload, NethunsSocket, NethunsToken,
};
use anyhow::{Result, bail};
use libc::{self, _SC_PAGESIZE, sysconf};
use libxdp_sys::XSK_UMEM__DEFAULT_FRAME_SIZE;
use std::alloc::{self, Layout};
use std::cell::{Cell, RefCell, UnsafeCell};
use std::ffi::CString;
use std::io::{self, Error, ErrorKind};
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::os::raw::c_int;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use wrapper::{TxSlot, Umem, XdpDescData, XskSocket};

//use libxdp_sys::*; // assume libxdp_sys crate exposes the raw C bindings

// Constants from the C code
const NUM_FRAMES: usize = 4096;
const FRAME_SIZE: u64 = XSK_UMEM__DEFAULT_FRAME_SIZE as u64; // taken from libxdp binding
const RX_BATCH_SIZE: u32 = 64;

// Global variables (unsafe globals in Rust, used only during startup)
static mut CUSTOM_XSK: bool = false;
static mut XSK_MAP_FD: c_int = -1;

//
// Data structures
//

pub fn resultify(x: i32) -> io::Result<u32> {
    match x >= 0 {
        true => Ok(x as u32),
        false => Err(io::Error::from_raw_os_error(-x)),
    }
}

#[derive(Clone)]
pub struct Context {
    buffer: UmemArea,
    producer: RefCell<mpsc::Producer<BufferIndex>>,
    index: usize,
}

pub struct Payload<'a> {
    packet_idx: BufferIndex,
    size: usize,
    pool: &'a Context,
}

impl<'a> NethunsPayload<'a> for Payload<'a> {
    type Context = Context;
    type Token = PayloadToken;
}

impl<'a> Payload<'a> {
    fn as_slice(&self) -> &[u8] {
        let token = self.packet_idx;
        let buf = unsafe { self.pool.buffer(token, self.size) };
        unsafe { &(*buf) }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        let token = self.packet_idx;
        let buf = unsafe { self.pool.buffer(token, self.size) };
        unsafe { &mut (*buf) }
    }
}

impl AsRef<[u8]> for Payload<'_> {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl AsMut<[u8]> for Payload<'_> {
    fn as_mut(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
}

impl Deref for Payload<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for Payload<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<'a> Drop for Payload<'a> {
    fn drop(&mut self) {
        self.pool.release(self.packet_idx);
    }
}

impl Context {
    fn new(buffer_pool: UmemArea) -> (Self, mpsc::Consumer<BufferIndex>) {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let (producer, cons) = mpsc::channel::<BufferIndex>(buffer_pool.size as usize);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let res = Self {
            buffer: buffer_pool, //: Arc::new(buffer_pool),
            producer: RefCell::new(producer),
            index: counter,
        };
        (res, cons)
    }

    fn get_packet(&self, token: PayloadToken) -> Payload<'_> {
        Payload {
            packet_idx: token.idx,
            size: token.len as usize,
            pool: self,
        }
    }

    unsafe fn buffer(&self, idx: BufferIndex, size: usize) -> *mut [u8] {
        let (ptr, _) = self.buffer.raw_parts();
        let offset = u32::from(idx) as usize;
        unsafe {
            let start = ptr.add(offset);
            std::slice::from_raw_parts_mut(start, size)
        }
    }

    fn release(&self, idx: BufferIndex) {
        self.producer.borrow_mut().push(idx);
    }
}

impl NethunsContext for Context {
    type Token = PayloadToken;

    type Payload<'ctx> = Payload<'ctx>;

    fn packet<'ctx>(&'ctx self, token: Self::Token) -> Self::Payload<'ctx> {
        todo!()
    }

    fn release(&self, buf_idx: BufferIndex) {
        todo!()
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
    fn new(packet_buffer_size: u64) -> Result<UmemArea, anyhow::Error> {
        let packet_buffer = alloc_page_aligned(packet_buffer_size as usize)?;

        let mem = Arc::new(UnsafeCell::new(packet_buffer));

        Ok(Self {
            mem,
            size: packet_buffer_size as usize,
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
    consumer: mpsc::Consumer<BufferIndex>,
}

impl UmemManager {
    /// Create a new XskUmemInfo, set up the UMEM, and initialize the internal frame store.
    pub fn create_with_buffer(
        umem: UmemArea,
        consumer: mpsc::Consumer<BufferIndex>,
    ) -> Result<Self> {
        Ok(Self {
            umem: Umem::new(umem)?,
            consumer,
        })
    }

    /// Allocates one frame address from our free array.
    fn alloc_frame(&mut self) -> Option<u64> {
        // self.frames.pop()
        self.consumer.pop().map(|idx| u32::from(idx) as u64)
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
            *self.umem.ring_prod_mut().get_addr(idx + i) = addr;
        }

        self.umem.ring_prod_mut().submit(available);
        Ok(())
    }

    // Reads the Completion Ring (CQ) to reclaim frames that were used for Tx.
    // Returns how many frames were reclaimed.

    // lo userò quando ho fallisce la read o la write
} //

fn complete_tx(xsk: &Socket) -> io::Result<()> {
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
            .push(BufferIndex::from(addr as u32));
    }
    umem.ring_cons_mut().release(completed);
    // Also be sure to flush your free list if you’re using e.g. a lockfree ring
    xsk.ctx.producer.borrow_mut().flush();

    Ok(())
}

pub struct PayloadToken {
    idx: BufferIndex,
    len: u32,
    buffer_pool: u32,
}

impl NethunsToken for PayloadToken {
    type Context = Context;

    fn load<'ctx>(
        self,
        ctx: &'ctx Self::Context,
    ) -> <Self::Context as NethunsContext>::Payload<'ctx> {
        ctx.packet(self)
    }
}

impl PayloadToken {
    fn new(idx: u32, buffer_pool: u32, len: u32) -> ManuallyDrop<Self> {
        let idx = BufferIndex::from(idx);
        ManuallyDrop::new(Self {
            idx,
            len,
            buffer_pool,
        })
    }

    fn load(self, pool: &Context) -> Payload<'_> {
        pool.get_packet(self)
    }
}

struct Port {
    pub ifname: String,
    pub ifindex: u32,
    pub queue_id: u32,
}

fn ifname_to_ifindex(ifname: &str) -> Option<u32> {
    let c_ifname = CString::new(ifname).ok()?;
    let index = unsafe { libc::if_nametoindex(c_ifname.as_ptr()) };
    if index == 0 { None } else { Some(index) }
}

impl Port {
    fn parse(portspec: &str) -> Result<Self> {
        let parts: Vec<&str> = portspec.split(':').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid port spec"));
        }
        let ifname = parts[0].to_string();
        let queue_id = parts[1].parse()?;
        let ifindex =
            ifname_to_ifindex(&ifname).ok_or_else(|| anyhow::anyhow!("Invalid ifname"))?;
        Ok(Self {
            ifname,
            ifindex,
            queue_id,
        })
    }
}

/// Wraps an AF_XDP socket.
pub struct Socket {
    /// The Rx and Tx rings (part of the XDP socket).
    // rx: RefCell<RxRing>,
    // tx: RefCell<TxRing>,
    ctx: Context,
    /// The UMEM info. Now also handles fill/completion ring & frame addresses.

    /// The pointer to the underlying xsk_socket (from libxdp_sys).
    // xsk: *mut xsk_socket,
    xsk: RefCell<XskSocket>,
    /// Outstanding Tx frames (to know how many completions to expect).
    outstanding_tx: u32,

    umem_manager: RefCell<UmemManager>,
    //consumer: mpsc::Consumer<BufferIndex>,
    /// Stats
    stats: Cell<StatsRecord>,
    prev_stats: Cell<StatsRecord>,
}

type Filter = ();

impl Socket {
    // pub fn create(
    //     portspec: &str,
    //     extra_buf: usize,
    //     filter: Option<Filter>,
    //     xdp_flags: u32,
    //     bind_flags: u16,
    // ) -> Result<(Context, Self)> {
    //
    // }

    fn recv_inner(&self, slot: XdpDescData) -> Result<PayloadToken> {
        let offset = slot.offset;
        let len = slot.len as usize;
        let options = slot.options;
        self.stats.update(|mut s| {
            s.rx_bytes += len as u64;
            s.rx_packets += 1;
            s
        });
        let buffer_pool = self.ctx.index;
        let token = PayloadToken::new(offset as u32, buffer_pool as u32, len as u32);
        Ok(ManuallyDrop::into_inner(token))
    }

    fn send_inner<'a>(&self, mut slot: TxSlot<'a>, payload: &[u8]) -> Result<()> {
        let frame_addr = self
            .umem_manager
            .borrow_mut()
            .alloc_frame()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No free frames for TX"))?;

        // Assign the descriptor’s address
        *slot.offset_mut() = frame_addr;
        *slot.len_mut() = payload.len() as u32;

        // Actually copy the packet into UMEM
        let buffer_index = BufferIndex::from(frame_addr as u32);
        let buf = unsafe { self.ctx.buffer(buffer_index, payload.len()) };

        unsafe {
            (*buf).copy_from_slice(payload);
        }

        // Update stats, etc.
        self.stats.update(|mut s| {
            s.tx_bytes += payload.len() as u64;
            s.tx_packets += 1;
            s
        });

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

impl NethunsSocket for Socket {
    type Context = Context;
    type Token = PayloadToken;
    type Flags = AfXdpFlags;

    fn recv(&mut self) -> anyhow::Result<Self::Token> {
        if let Some(slot) = self.xsk.borrow_mut().rx_mut().next() {
            self.recv_inner(slot)
        } else {
            self.umem_manager.borrow_mut().refill_fill_ring()?;
            if let Some(slot) = self.xsk.borrow_mut().rx_mut().next() {
                self.recv_inner(slot)
            } else {
                bail!("No inbound packets");
            }
        }
    }

    fn send(&mut self, packet: &[u8]) -> anyhow::Result<()> {
        if let Some(slot) = self.xsk.borrow_mut().tx_mut().iter().next() {
            self.send_inner(slot, packet)?
        } else {
            self.flush();
            if let Some(slot) = self.xsk.borrow_mut().tx_mut().iter().next() {
                self.send_inner(slot, packet)?
            } else {
                bail!("No inbound packets");
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

    fn create(
        portspec: &str,
        filter: Option<()>,
        flags: Self::Flags,
    ) -> anyhow::Result<(Self::Context, Self)> {
        let xdp_flags = flags.xdp_flags;
        let bind_flags = flags.bind_flags;
        let umem_bytes_len = NUM_FRAMES as u64 * FRAME_SIZE;
        let umem = UmemArea::new(umem_bytes_len)?;

        let (ctx, consumer) = Context::new(umem.clone());

        for i in 0..NUM_FRAMES {
            ctx.producer
                .borrow_mut()
                .push(BufferIndex::from((i as u32) * FRAME_SIZE as u32));
        }
        ctx.producer.borrow_mut().flush();
        let mut umem_manager = UmemManager::create_with_buffer(umem.clone(), consumer)?;

        let port = Port::parse(portspec)?;

        let socket = unsafe {
            XskSocket::create(
                &umem_manager.umem,
                &port.ifname,
                port.queue_id,
                xdp_flags,
                bind_flags,
            )?
        };

        umem_manager.refill_fill_ring()?;
        Ok((ctx.clone(), Self {
            ctx,
            xsk: RefCell::new(socket),
            outstanding_tx: 0,
            umem_manager: RefCell::new(umem_manager),
            stats: Cell::new(StatsRecord::default()),
            prev_stats: Cell::new(StatsRecord::default()),
        }))
    }
}

#[derive(Clone)]
pub struct AfXdpFlags {
    pub bind_flags: u16,
    pub xdp_flags: u32,
}

impl NethunsFlags for AfXdpFlags {}

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
    // In purely std-only Rust, there's no direct API to get the OS page size,
    // so we'll pretend it's 4096. In real code, you might call libc::sysconf or use a crate.
    //let page_size = 4096;
    let page_size = unsafe { sysconf(_SC_PAGESIZE) } as usize;

    // Build a Layout describing a block of `size` bytes with `page_size` alignment.
    let layout = Layout::from_size_align(size, page_size)
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "Invalid layout"))?;

    // Allocate the memory.
    let ptr = unsafe { alloc::alloc(layout) };

    // Check for allocation failure (ptr::null_mut on OOM).
    if ptr.is_null() {
        Err(Error::new(
            ErrorKind::Other,
            "Allocation with page alignment failed",
        ))
    } else {
        Ok(ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    //#[test]
    //fn test_send_without_flush() {
    //    let (_, socket0) = Socket::create("vale0:0", 1024, None, 0, 0).unwrap();
    //    let (_, socket1) = Socket::create("vale0:1", 1024, None, 0, 0).unwrap();
    //    socket1.send(b"Helloworldmyfriend\0\0").unwrap();
    //    assert!(socket0.recv().is_err());
    //}

    #[test]
    fn test_send_with_flush() {
        let (ctx0, mut socket0) = Socket::create("veth_test0:0", None, AfXdpFlags { xdp_flags: 0, bind_flags: 0 }).unwrap();
        let (_, mut socket1) = Socket::create("veth_test1:0", None, AfXdpFlags { xdp_flags: 0, bind_flags: 0 }).unwrap();
        socket1.send(b"Helloworldmyfriend\0\0\0\0\0\0\0").unwrap();
        socket1.flush();
        let packet = socket0.recv().unwrap().load(&ctx0);
        assert_eq!(&packet.as_slice()[..20], b"Helloworldmyfriend\0\0");
    }
}
