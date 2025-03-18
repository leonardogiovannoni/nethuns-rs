mod wrapper;
use crate::api::{self, BufferConsumer, BufferProducer, Payload};
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

// Constants from the C code
const NUM_FRAMES: u32 = 4096;
const FRAME_SIZE: u32 = XSK_UMEM__DEFAULT_FRAME_SIZE; // taken from libxdp binding
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
pub struct Ctx<S: api::Strategy> {
    buffer: UmemArea,
    producer: RefCell<S::Producer>,
    index: usize,
}
//
//pub struct PacketData<'a, S: api::Strategy> {
//    packet_idx: api::BufferIndex,
//    size: usize,
//    pool: &'a Ctx<S>,
//}
//
//impl<'a, S: api::Strategy> api::Payload<'a> for PacketData<'a, S> {
//    type Context = Ctx<S>;
//    fn into_token(self) -> <Self::Context as api::Context>::Token {
//        let me = ManuallyDrop::new(self);
//        let rv = Tok::new(u32::from(me.packet_idx) as u64, me.pool.index, me.size as u32);
//        ManuallyDrop::into_inner(rv)
//    }
//}
//
//impl<'a, S: api::Strategy> PacketData<'a, S> {
//    fn as_slice(&self) -> &[u8] {
//        let token = self.packet_idx;
//        let buf = unsafe { self.pool.buffer(token, self.size) };
//        unsafe { &(*buf) }
//    }
//
//    fn as_mut_slice(&mut self) -> &mut [u8] {
//        let token = self.packet_idx;
//        let buf = unsafe { self.pool.buffer(token, self.size) };
//        unsafe { &mut (*buf) }
//    }
//}
//
//impl<S: api::Strategy> AsRef<[u8]> for PacketData<'_, S> {
//    fn as_ref(&self) -> &[u8] {
//        self.as_slice()
//    }
//}
//
//impl<S: api::Strategy> AsMut<[u8]> for PacketData<'_, S> {
//    fn as_mut(&mut self) -> &mut [u8] {
//        self.as_mut_slice()
//    }
//}
//
//impl<S: api::Strategy> Deref for PacketData<'_, S> {
//    type Target = [u8];
//
//    fn deref(&self) -> &Self::Target {
//        self.as_slice()
//    }
//}
//
//impl<S: api::Strategy> DerefMut for PacketData<'_, S> {
//    fn deref_mut(&mut self) -> &mut Self::Target {
//        self.as_mut_slice()
//    }
//}
//
//impl<'a, S: api::Strategy> Drop for PacketData<'a, S> {
//    fn drop(&mut self) {
//        self.pool.release(self.packet_idx);
//    }
//}


//fn pippo<S: api::Strategy>(buffer_pool: UmemArea, args: api::StrategyArgs) -> (Ctx<S>, S::Consumer) {
//    static COUNTER: AtomicUsize = AtomicUsize::new(0);
//    let (producer, cons) = S::create(args);
//    // mpsc::channel::<api::BufferIndex>(buffer_pool.size as usize);
//    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
//    let res = Ctx {
//        buffer: buffer_pool, //: Arc::new(buffer_pool),
//        producer: RefCell::new(producer),
//        index: counter,
//    };
//    (res, cons)
//}

impl<S: api::Strategy> Ctx<S> {
    fn new(buffer_pool: UmemArea, args: api::StrategyArgs) -> (Self, S::Consumer) {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let (producer, cons) = S::create(args);
        // mpsc::channel::<api::BufferIndex>(buffer_pool.size as usize);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let res = Self {
            buffer: buffer_pool, //: Arc::new(buffer_pool),
            producer: RefCell::new(producer),
            index: counter,
        };
        (res, cons)
    }

    unsafe fn buffer(&self, idx: api::BufferIndex, size: usize) -> *mut [u8] {
        let (ptr, _) = self.buffer.raw_parts();
        let offset = u32::from(idx) as usize;
        unsafe {
            let start = ptr.add(offset);
            std::slice::from_raw_parts_mut(start, size)
        }
    }

    fn release(&self, idx: api::BufferIndex) {
        self.producer.borrow_mut().push(idx);
    }
}

impl<S: api::Strategy> api::Context for Ctx<S> {
    type Token = Tok<S>;

    fn release(&self, buf_idx: api::BufferIndex) {
        self.producer.borrow_mut().push(buf_idx);
    }

    unsafe fn unsafe_buffer(&self, buf_idx: api::BufferIndex, size: usize) -> *mut [u8] {
        unsafe { self.buffer(buf_idx, size) }
    }

    fn pool_id(&self) -> usize {
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
    fn new(packet_buffer_size: usize) -> Result<UmemArea, anyhow::Error> {
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
struct UmemManager<S: api::Strategy> {
    umem: Umem,
    consumer: S::Consumer,
}

impl<S: api::Strategy> UmemManager<S> {
    pub fn create_with_buffer(umem: UmemArea, consumer: S::Consumer) -> Result<Self> {
        Ok(Self {
            umem: Umem::new(umem)?,
            consumer,
        })
    }

    /// Allocates one frame address from our free array.
    fn alloc_frame(&mut self) -> Option<u32> {
        // self.frames.pop()
        self.consumer.pop().map(|idx| u32::from(idx))
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

fn complete_tx<S: api::Strategy>(xsk: &Sock<S>) -> io::Result<()> {
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
            .push(api::BufferIndex::from(addr as u32));
    }
    umem.ring_cons_mut().release(completed);
    xsk.ctx.producer.borrow_mut().flush();

    Ok(())
}

pub struct Tok<S: api::Strategy> {
    idx: api::BufferIndex,
    len: u32,
    buffer_pool: usize,
    _marker: std::marker::PhantomData<S>,
}

impl<S: api::Strategy> api::Token for Tok<S> {
    type Context = Ctx<S>;

    fn buffer_idx(&self) -> api::BufferIndex {
        self.idx
    }

    fn size(&self) -> usize {
        self.len as usize
    }

    fn pool_id(&self) -> usize {
        self.buffer_pool
    }
}

impl<S: api::Strategy> api::TokenExt for Tok<S> {
    fn clone(&self) -> Self {
        Tok {
            idx: self.idx,
            len: self.len,
            buffer_pool: self.buffer_pool,
            _marker: std::marker::PhantomData,
        }
    }
}


impl<S: api::Strategy> Tok<S> {
    fn new(idx: u64, buffer_pool: usize, len: u32) -> ManuallyDrop<Self> {
        let idx: u32 = idx.try_into().unwrap();
        let idx = api::BufferIndex::from(idx);
        ManuallyDrop::new(Self {
            idx,
            len,
            buffer_pool,
            _marker: std::marker::PhantomData,
        })
    }

    //fn load(self, pool: &Ctx<S>) -> PacketData<'_, S> {
    //    pool.packet(self)
    //}
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
pub struct Sock<S: api::Strategy> {
    ctx: Ctx<S>,
    xsk: RefCell<XskSocket>,
    outstanding_tx: u32,
    umem_manager: RefCell<UmemManager<S>>,
    stats: Cell<StatsRecord>,
    prev_stats: Cell<StatsRecord>,
}

impl<S: api::Strategy> Sock<S> {
    #[inline(always)]
    fn recv_inner(&self, slot: XdpDescData) -> Result<(Tok<S>, Meta)> {
        let offset = slot.offset;
        let len = slot.len;
        let options = slot.options;
        // self.stats.update(|mut s| {
        //     s.rx_bytes += len as u64;
        //     s.rx_packets += 1;
        //     s
        // });
        let mut stats = self.stats.get();
        stats.rx_bytes += len as u64;
        stats.rx_packets += 1;
        self.stats.set(stats);

        let buffer_pool = self.ctx.index;
        let token = Tok::new(offset, buffer_pool, len);
        Ok((ManuallyDrop::into_inner(token), Meta {}))
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
        let buffer_index = api::BufferIndex::from(frame_addr);
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

impl<S: api::Strategy> api::Socket<S> for Sock<S> {
    type Context = Ctx<S>;
    type Metadata = Meta;

    fn recv_token(&mut self) -> anyhow::Result<(<Self::Context as api::Context>::Token, Self::Metadata)> {
        let mut rx = self.xsk.borrow_mut();
        if let Some(slot) = rx.rx_mut().next() {
            self.recv_inner(slot)
        } else {
            self.umem_manager.borrow_mut().refill_fill_ring()?;
            let tmp = rx
                .rx_mut()
                .next()
                .ok_or_else(|| anyhow::anyhow!("No packets"))?;
            self.recv_inner(tmp)
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
        flags: api::Flags,
    ) -> anyhow::Result<Self> {
        let flags = match flags {
            api::Flags::AfXdp(flags) => flags,
            _ => panic!("Invalid flags"),
        };
        let xdp_flags = flags.xdp_flags;
        let bind_flags = flags.bind_flags;
        let umem_bytes_len = NUM_FRAMES * FRAME_SIZE;
        let umem = UmemArea::new(umem_bytes_len as usize)?;
        let (ctx, consumer) = Ctx::new(umem.clone(), flags.strategy_args);

        for i in 0..NUM_FRAMES {
            let prod: &mut <S as api::Strategy>::Producer = &mut *ctx.producer.borrow_mut();
            prod.push(api::BufferIndex::from((i as u32) * FRAME_SIZE as u32));
        }
        {
            let prod: &mut <S as api::Strategy>::Producer = &mut *ctx.producer.borrow_mut();
            prod.flush();
        }
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
        Ok(
            Self {
                ctx,
                xsk: RefCell::new(socket),
                outstanding_tx: 0,
                umem_manager: RefCell::new(umem_manager),
                stats: Cell::new(StatsRecord::default()),
                prev_stats: Cell::new(StatsRecord::default()),
            },
        )
    }

    fn context(&self) -> &Self::Context {
        &self.ctx
    }
}

#[derive(Clone, Debug)]
pub struct AfXdpFlags {
    pub bind_flags: u16,
    pub xdp_flags: u32,
    pub strategy_args: api::StrategyArgs,
}

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
        return Err(Error::new(ErrorKind::InvalidInput, "Invalid size"));
    }
    let page_size = unsafe { sysconf(_SC_PAGESIZE) };
    if page_size < 0 {
        return Err(Error::last_os_error());
    }
    let page_size = page_size as usize;

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

pub struct Meta {}

impl api::Metadata for Meta {}

#[cfg(test)]
mod tests {
    use crate::{
        api::{Flags, Socket},
        strategy::{MpscArgs, MpscStrategy},
    };

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
        let mut socket0 = Sock::<MpscStrategy>::create(
            "veth_test0:0",
            None,
            Flags::AfXdp(AfXdpFlags {
                xdp_flags: 0,
                bind_flags: 0,
                strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
            }),
        )
        .unwrap();
        let mut socket1 = Sock::<MpscStrategy>::create(
            "veth_test1:0",
            None,
            Flags::AfXdp(AfXdpFlags {
                xdp_flags: 0,
                bind_flags: 0,
                strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
            }),
        )
        .unwrap();
        socket1.send(b"Helloworldmyfriend\0\0\0\0\0\0\0").unwrap();
        socket1.flush();
        let (packet, meta) = socket0.recv().unwrap();
        assert_eq!(&packet[..20], b"Helloworldmyfriend\0\0");
    }
}
