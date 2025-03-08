#![feature(associated_type_defaults)]
#![feature(cell_update)]
#![feature(slice_ptr_get)]
mod api;
mod netmap;
mod af_xdp;
use anyhow::{Result, bail};
use mpsc::Producer;
use api::{NethunsContext, NethunsPayload, NethunsSocket, NethunsToken};
use netmap_rs::context::{BufferPool, Port, Receiver, RxBuf, Transmitter, TxBuf};
use nix::sys::time::TimeVal;
use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/* 
#[derive(Clone, Copy, Debug)]
struct U24Repr {
    data: [u8; 3],
}

impl U24Repr {
    fn new(data: [u8; 3]) -> Self {
        Self { data }
    }

    fn from_u32(val: u32) -> Self {
        let data = val.to_be_bytes();
        Self {
            data: [data[1], data[2], data[3]],
        }
    }

    fn as_u32(&self) -> u32 {
        u32::from_be_bytes([0, self.data[0], self.data[1], self.data[2]])
    }
}

impl From<U24Repr> for u32 {
    fn from(val: U24Repr) -> u32 {
        val.as_u32()
    }
}

impl From<u32> for U24Repr {
    fn from(val: u32) -> Self {
        Self::from_u32(val)
    }
}

#[derive(Clone, Copy, Debug)]
struct BufferIndex(u32);

impl From<u32> for BufferIndex {
    fn from(val: u32) -> Self {
        Self(val.into())
    }
}

impl From<BufferIndex> for u32 {
    fn from(val: BufferIndex) -> u32 {
        val.0.into()
    }
}

#[derive(Clone)]
pub struct Context {
    buffer_pool: Arc<BufferPool>,
    producer: RefCell<Producer<BufferIndex>>,
    index: usize,
}

impl Context {
    fn new(buffer_pool: BufferPool, indexes: Vec<u32>) -> (Self, mpsc::Consumer<BufferIndex>) {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let (mut producer, cons) = mpsc::channel::<BufferIndex>(indexes.len());
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let buffer_pool = Arc::new(buffer_pool);
        for idx in indexes {
            producer.push(BufferIndex::from(idx));
        }
        let res = Self {
            buffer_pool,
            producer: RefCell::new(producer),
            index: counter,
        };
        (res, cons)
    }

    unsafe fn buffer(&self, idx: BufferIndex) -> *mut [u8] {
        unsafe { self.buffer_pool.buffer(u32::from(idx) as usize) }
    }

    fn check_token(&self, token: &PayloadToken) -> bool {
        token.buffer_pool == self.index as u32
    }

    fn peek_packet(&self, token: &PayloadToken) -> Payload<'_> {
        if !self.check_token(token) {
            panic!("Invalid token");
        }
        Payload {
            packet_idx: token.idx,
            pool: self,
        }
    }
}

impl NethunsContext for Context {
    //type Payload = Payload<'a>;
    type Token = PayloadToken;

    //fn packet(&'a self, token: Self::Token) -> Payload<'a> {
    //    if !self.check_token(&token) {
    //        panic!("Invalid token");
    //    }
    //    let token = ManuallyDrop::new(token);
    //    Payload {
    //        packet_idx: token.idx,
    //        pool: self,
    //    }
    //}

    fn release(&self, token: BufferIndex) {
        self.producer.borrow_mut().push(token);
    }

    type Payload<'ctx> = Payload<'ctx>;

    fn packet<'ctx>(&'ctx self, token: Self::Token) -> Self::Payload<'ctx> {
        if !self.check_token(&token) {
            panic!("Invalid token");
        }
        let token = ManuallyDrop::new(token);
        Payload {
            packet_idx: token.idx,
            pool: self,
        }
    }
}

struct PacketHeader {
    // index: u32,
    // len: u16,
    // caplen: u16,
    ts: TimeVal,
}

pub struct RecvPacket<'a> {
    ts: TimeVal,
    payload: Payload<'a>,
}

impl<'a> RecvPacket<'a> {
    fn payload(&self) -> &[u8] {
        self.payload.as_slice()
    }

    fn payload_mut(&mut self) -> &mut [u8] {
        self.payload.as_mut_slice()
    }
}

#[must_use]
pub struct PayloadToken {
    idx: BufferIndex,
    buffer_pool: u32,
}

impl PayloadToken {
    fn new(idx: u32, buffer_pool: u32) -> ManuallyDrop<Self> {
        let idx = BufferIndex::from(idx);
        ManuallyDrop::new(Self { idx, buffer_pool })
    }
}

impl NethunsToken for PayloadToken {
    type Context = Context;
}

impl Drop for PayloadToken {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            panic!("PacketToken must be used");
        }
    }
}

struct Socket {
    tx: RefCell<Transmitter>,
    rx: RefCell<Receiver>,
    ctx: Context,
    consumer: RefCell<mpsc::Consumer<BufferIndex>>,
    filter: Option<Filter>,
}

impl std::fmt::Debug for Socket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Socket").finish()
    }
}

pub struct Payload<'a> {
    packet_idx: BufferIndex,
    pool: &'a Context,
}

impl<'a> NethunsPayload<'a> for Payload<'a> {
    type Context = Context;
    type Token = PayloadToken;
}

impl<'a> Payload<'a> {
    fn as_slice(&self) -> &[u8] {
        let token = self.packet_idx;
        let buf = unsafe { self.pool.buffer(token) };
        unsafe { &(*buf) }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        let token = self.packet_idx;
        let buf = unsafe { self.pool.buffer(token) };
        unsafe { &mut (*buf) }
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
impl<'a> AsRef<[u8]> for Payload<'a> {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl<'a> AsMut<[u8]> for Payload<'a> {
    fn as_mut(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
}

impl<'a> Drop for Payload<'a> {
    fn drop(&mut self) {
        self.pool.release(self.packet_idx);
    }
}

pub enum Filter {
    Closure(Box<dyn Fn(&TimeVal, &[u8]) -> Result<(), ()> + Send>),
    Function(fn(&TimeVal, &[u8]) -> Result<(), ()>),
}

impl Filter {
    fn apply(&self, ts: &TimeVal, payload: &[u8]) -> Result<(), ()> {
        match self {
            Filter::Closure(f) => f(ts, payload),
            Filter::Function(f) => f(ts, payload),
        }
    }
}

impl Socket {
    #[inline(always)]
    fn send_inner(&self, scan: TxBuf<'_>, packet: &[u8]) -> Result<()> {
        let TxBuf { ref slot, .. } = scan;
        let token = slot.buf_idx();
        let token = BufferIndex::from(token);
        let buf = unsafe { self.ctx.buffer(token) };
        let buf = unsafe { &mut (*buf) };
        if packet.len() > buf.len() {
            bail!("Packet too big");
        }
        buf[..packet.len()].copy_from_slice(packet);
        Ok(())
    }

    #[inline(always)]
    fn recv_inner(&self, buf: RxBuf<'_>) -> Result<PayloadToken> {
        let RxBuf { slot, ts, .. } = buf;
        let free_idx = self
            .consumer
            .borrow_mut()
            .pop()
            .ok_or_else(|| anyhow::anyhow!("No free buffers"))?;
        let pkt_idx = slot.buf_idx();
        unsafe {
            slot.update_buffer(|x| *x = u32::from(free_idx));
        }

        let packet_token = PayloadToken::new(pkt_idx, self.ctx.index as u32);

        if let Some(filter) = self.filter.as_ref() {
            let aliased_packet = self.ctx.peek_packet(&packet_token);
            if filter.apply(&ts, aliased_packet.as_slice()).is_err() {
                bail!("Filter failed");
            }
        }
        Ok(ManuallyDrop::into_inner(packet_token))
    }

    fn create(portspec: &str, extra_buf: usize, filter: Option<Filter>) -> Result<(Context, Self)> {
        let mut port = Port::open(portspec, extra_buf as u32)?;
        let extra_bufs = unsafe { port.extra_buffers_indexes() };
        let (tx, rx, buffer_pool) = port.split();
        let (ctx, consumer) = Context::new(buffer_pool, extra_bufs);
        Ok((ctx.clone(), Self {
            tx: RefCell::new(tx),
            rx: RefCell::new(rx),
            ctx,
            consumer: RefCell::new(consumer),
            filter,
        }))
    }
}

impl NethunsSocket for Socket {
    type Context = Context;
    type Token = PayloadToken;

    fn recv(&mut self) -> anyhow::Result<Self::Token> {
        let mut rx = self.rx.borrow_mut();
        if let Some(tmp) = rx.iter_mut().next() {
            self.recv_inner(tmp)
        } else {
            // SAFETY: there are no `RxBuf`s, and so any `Slot`s, in use
            unsafe {
                rx.reset();
            }
            let tmp = rx
                .iter_mut()
                .next()
                .ok_or_else(|| anyhow::anyhow!("No packets"))?;
            self.recv_inner(tmp)
        }
    }

    fn send(&mut self, packet: &[u8]) -> Result<()> {
        let mut tx = self.tx.borrow_mut();
        if let Some(next) = tx.iter_mut().next() {
            self.send_inner(next, packet)
        } else {
            // SAFETY: there are no `TxBuf`s, and so any `Slot`s, in use
            unsafe {
                tx.reset();
            }
            let next = tx
                .iter_mut()
                .next()
                .ok_or_else(|| anyhow::anyhow!("No free slots"))?;
            self.send_inner(next, packet)
        }
    }

    fn flush(&mut self) {
        let mut tx = self.tx.borrow_mut();
        // SAFETY: Any `Slot`s is in use due to the design of the API
        unsafe {
            tx.sync();
        }
    }

    fn context(&self) -> &Self::Context {
        &self.ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_without_flush() {
        let (_, mut socket0) = Socket::create("vale0:0", 1024, None).unwrap();
        let (_, mut socket1) = Socket::create("vale0:1", 1024, None).unwrap();
        socket1.send(b"Helloworldmyfriend\0\0").unwrap();
        assert!(socket0.recv().is_err());
    }

    #[test]
    fn test_send_with_flush() {
        let (ctx0, mut socket0) = Socket::create("vale1:0", 1024, None).unwrap();
        let (_, mut socket1) = Socket::create("vale1:1", 1024, None).unwrap();
        socket1.send(b"Helloworldmyfriend\0\0").unwrap();
        socket1.flush();
        let packet = socket0.recv().unwrap().load(&ctx0);
        assert_eq!(&packet.as_slice()[..20], b"Helloworldmyfriend\0\0");
    }
}

fn main2() {
    let (ctx0, mut socket0) = Socket::create("vale1:0", 1024, None).unwrap();
    let (_, mut socket1) = Socket::create("vale1:1", 1024, None).unwrap();
    socket1.send(b"Helloworldmyfriend\0\0").unwrap();
    socket1.flush();
    let payload_token = socket0.recv().unwrap();
    std::thread::spawn(move || {
        let packet = &*payload_token.load(&ctx0);
        assert_eq!(&packet[..20], b"Helloworldmyfriend\0\0");
    });
}
// fn main() {
//     // th
//     let a = std::thread::spawn(|| {
//         pippo2().unwrap();
//     });
//
//     let b = std::thread::spawn(|| {
//         pluto2().unwrap();
//     });
//     a.join().unwrap();
//     b.join().unwrap();
// }
*/
use clap::Parser;
use etherparse::{NetHeaders, PacketHeaders};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::thread;
use std::time::Duration;

// Assume that your own network I/O API (from the rest of the file)
// provides the following types and methods:
//use my_net_api::{Filter, RecvPacket, Socket}; // adjust the module path as needed

/// Command line options.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Network interface name.
    #[clap(short, long)]
    interface: String,

    /// Number of sockets to use (default 1).
    #[clap(short, long, default_value_t = 1)]
    sockets: usize,

    /// Enable multithreading (one thread per socket).
    #[clap(short, long)]
    multithreading: bool,

    /// Enable per-socket statistics; provide the socket id.
    #[clap(short = 'S', long)]
    sockstats: Option<usize>,

    /// Enable debug printing (e.g. parsed IP addresses).
    #[clap(short, long)]
    debug: bool,
}

/// Try to parse Ethernet/IP headers using etherparse and return a formatted string.
fn print_addrs(frame: &[u8]) -> Result<String> {
    let packet_header = PacketHeaders::from_ethernet_slice(frame).unwrap();

    // Get reference to IP header
    let ip_header = &packet_header
        .net
        .ok_or(anyhow::anyhow!("Error: IP header not found"))?;

    match ip_header {
        NetHeaders::Ipv4(hdr, _) => Ok(format!(
            "IP: {} > {}",
            Ipv4Addr::from(hdr.source),
            Ipv4Addr::from(hdr.destination)
        )),
        NetHeaders::Ipv6(hdr, _) => Ok(format!(
            "IP: {} > {}",
            Ipv6Addr::from(hdr.source),
            Ipv6Addr::from(hdr.destination)
        )),
        _ => bail!("Error: IP header not found"),
    }
}

fn main_af_xdp() -> Result<()> {
    // Parse command-line arguments.
    let args = Args::parse();

    println!("Test {} started with parameters:", args.interface);
    println!("* interface: {}", args.interface);
    println!("* sockets: {}", args.sockets);
    println!(
        "* multithreading: {}",
        if args.multithreading { "ON" } else { "OFF" }
    );
    if let Some(sockid) = args.sockstats {
        println!("* sockstats: ON for socket {}", sockid);
    } else {
        println!("* sockstats: OFF, aggregated stats only");
    }
    println!("* debug: {}", if args.debug { "ON" } else { "OFF" });

    // Setup a termination flag (triggered on Ctrl+C).
    let term = Arc::new(AtomicBool::new(false));
    {
        let term = term.clone();
        ctrlc::set_handler(move || {
            term.store(true, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");
    }

    // Create per-socket packet counters.
    let totals: Vec<Arc<AtomicU64>> = (0..args.sockets)
        .map(|_| Arc::new(AtomicU64::new(0)))
        .collect();

    // Create a socket for each requested socket.
    // Here we adapt the port spec to your API: if more than one socket,
    // append the socket id; otherwise use queue 0.
    let extra_buf = 1024;
    let mut sockets = Vec::with_capacity(args.sockets);
    for i in 0..args.sockets {
        let portspec = if args.sockets > 1 {
            // format!("{}:{}", args.interface, i)
            panic!()
        } else {
            // format!("{}", args.interface)
            args.interface.clone()
        };
        let socket = af_xdp::Socket::create(&portspec, extra_buf, None, 0, 0)?;
        sockets.push(socket);
    }

    // Start the statistics thread.
    // If a per-socket stat is requested via --sockstats, print that socket’s stats;
    // otherwise print the aggregated count.
    do_main(args, term, totals, sockets)
}


fn main_netmap() -> Result<()> {
    // Parse command-line arguments.
    let args = Args::parse();

    println!("Test {} started with parameters:", args.interface);
    println!("* interface: {}", args.interface);
    println!("* sockets: {}", args.sockets);
    println!(
        "* multithreading: {}",
        if args.multithreading { "ON" } else { "OFF" }
    );
    if let Some(sockid) = args.sockstats {
        println!("* sockstats: ON for socket {}", sockid);
    } else {
        println!("* sockstats: OFF, aggregated stats only");
    }
    println!("* debug: {}", if args.debug { "ON" } else { "OFF" });

    // Setup a termination flag (triggered on Ctrl+C).
    let term = Arc::new(AtomicBool::new(false));
    {
        let term = term.clone();
        ctrlc::set_handler(move || {
            term.store(true, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");
    }

    // Create per-socket packet counters.
    let totals: Vec<Arc<AtomicU64>> = (0..args.sockets)
        .map(|_| Arc::new(AtomicU64::new(0)))
        .collect();

    // Create a socket for each requested socket.
    // Here we adapt the port spec to your API: if more than one socket,
    // append the socket id; otherwise use queue 0.
    let extra_buf = 1024;
    let mut sockets = Vec::with_capacity(args.sockets);
    for i in 0..args.sockets {
        let portspec = if args.sockets > 1 {
            // format!("{}:{}", args.interface, i)
            panic!()
        } else {
            // format!("{}", args.interface)
            args.interface.clone()
        };
        let socket = netmap::Socket::create(&portspec, extra_buf, None)?;
        sockets.push(socket);
    }

    // Start the statistics thread.
    // If a per-socket stat is requested via --sockstats, print that socket’s stats;
    // otherwise print the aggregated count.
    do_main(args, term, totals, sockets)
}


fn do_main<S: NethunsSocket + 'static>(args: Args, term: Arc<AtomicBool>, totals: Vec<Arc<AtomicU64>>, mut sockets: Vec<(S::Context, S)>) -> std::result::Result<(), anyhow::Error> {
    let totals_stats = totals.clone();
    let term_stats = term.clone();
    let stats_handle = if let Some(sockid) = args.sockstats {
        thread::spawn(move || {
            let mut old_total = 0;
            while !term_stats.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_secs(1));
                let count = totals_stats[sockid].load(Ordering::SeqCst);
                println!(
                    "Socket {} pkt/sec: {}",
                    sockid,
                    count.saturating_sub(old_total)
                );
                old_total = count;
            }
        })
    } else {
        thread::spawn(move || {
            let mut old_total = 0;
            while !term_stats.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_secs(1));
                let sum: u64 = totals_stats.iter().map(|c| c.load(Ordering::SeqCst)).sum();
                println!("Aggregated pkt/sec: {}", sum.saturating_sub(old_total));
                old_total = sum;
            }
        })
    };

    // Spawn packet-receiving threads.
    let mut handles = Vec::new();
    const BULK: u64 = 10000;
    if args.multithreading {
        // One thread per socket.
        for (i, (ctx, mut socket)) in sockets.into_iter().enumerate() {
            let term_thread = term.clone();
            let counter = totals[i].clone();
            let debug = args.debug;
            let handle = {
                let ctx = ctx.clone();
                thread::spawn(move || {
                    let mut local_counter = 0;

                    while !term_thread.load(Ordering::SeqCst) {
                        local_counter += 1;
                        if local_counter == BULK {
                            counter.fetch_add(local_counter, Ordering::SeqCst);
                            local_counter = 0;
                        }
                        match (|| {
                            let pkt = &*socket.recv()?.load(&ctx);
                            //counter.fetch_add(1, Ordering::SeqCst);
                            if debug {
                                if let Ok(info) = print_addrs(&pkt) {
                                    println!("Thread {}: {}", i, info);
                                }
                            }
                            Ok::<(), anyhow::Error>(())
                        })() {
                            Err(e) => {}
                            _ => (),
                        }
                    }
                })
            };
            handles.push(handle);
        }
    } else {
        // Single-threaded loop over all sockets.
        // Wrap sockets and totals in Arcs for shared access.
        let totals = Arc::new(totals);
        println!("Single-threaded loop over all sockets");
        let term_loop = term.clone();
        println!("term_loop: {:?}", term_loop);
        let debug = args.debug;
        println!("debug: {:?}", debug);
        let handle = thread::spawn(move || {
            //println!("thread::spawn");

            let mut local_counters = vec![0; sockets.len()];
            while !term_loop.load(Ordering::SeqCst) {
                for (i, (ctx, socket)) in sockets.iter_mut().enumerate() {
                    match (|| {
                        let packet = &*socket.recv()?.load(&ctx);
                        local_counters[i] += 1;
                        if local_counters[i] == BULK {
                            totals[i].fetch_add(local_counters[i], Ordering::SeqCst);
                            local_counters[i] = 0;
                        }
                        if debug {
                            if let Ok(info) = print_addrs(&packet) {
                                println!("Socket {}: {}", i, info);
                            }
                        }
                        Ok::<(), anyhow::Error>(())
                    })() {
                        Err(e) => {}
                        Ok(_) => {
                            // println!("routine() ok");
                        }
                    }
                }
            }
        });
        println!("handle: {:?}", handle);
        handles.push(handle);
    }

    // Wait for all receiver threads and the stats thread to complete.
    for handle in handles {
        handle.join().expect("Receiver thread panicked");
    }
    stats_handle.join().expect("Stats thread panicked");

    // Sockets are closed automatically when dropped.
    Ok(())
}

fn main() {
    let res = main_netmap();
    match res {
        Ok(_) => println!("Success"),
        Err(e) => eprintln!("Error: {:?}", e),
    }
    let res = main_af_xdp();
    match res {
        Ok(_) => println!("Success"),
        Err(e) => eprintln!("Error: {:?}", e),
    }
}
