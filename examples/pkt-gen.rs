//! pktgen_tx.rs – A minimal high‑speed traffic generator that mimics the
//! behaviour of **netmap/pkt-gen -f tx** (transmit mode) but is written in
//! safe Rust on top of the `nethuns_rs` unified I/O API.  
//!
//! * Compile with the same features you use for the `nethuns_rs` crate — e.g.:
//!   ```bash
//!   cargo run --release --features netmap -- \
//!       -i eth0 -s 4 --multithreading --framework netmap \
//!       --dst-mac 11:22:33:44:55:66 --dst-ip 10.0.0.2 \
//!       --src-ip 10.0.0.1 --len 60 -r 10_000_000
//!   ```
//!
//! * Author: <you>
//! * Licence: BSD‑3‑Clause (match `nethuns_rs`)

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use etherparse::PacketBuilder;
use nethuns_rs::{af_xdp, api::Socket, dpdk, netmap};
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

//──────────────────────────────────────────────────────────────────────────────
// CLI
//──────────────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Network interface name (e.g. "eth0" | "netmap:eth0" | "vale0:1").
    #[clap(short = 'i', long)]
    interface: String,

    /// Queue to bind (default 0 / any).
    #[clap(long)]
    queue: Option<usize>,

    /// Number of independent sockets (and therefore flows) to open.
    #[clap(short, long, default_value_t = 1)]
    sockets: usize,

    /// Spawn one OS thread per socket.
    #[clap(short = 'm', long)]
    multithreading: bool,

    /// Per‑socket statistics (print packets/s for *sockid*).
    #[clap(short = 'S', long)]
    sockstats: Option<usize>,

    /// Enable verbose debug output (prints the first packet template).
    #[clap(short, long)]
    debug: bool,

    // ───────────────────────────────── tx specific ──────────────────────────
    /// Total number of packets to send (0 = unlimited).
    #[clap(short = 'n', long, default_value_t = 0)]
    count: u64,

    /// Desired *approximate* packet rate (pps). 0 = flat‑out.
    #[clap(short = 'r', long, default_value_t = 0)]
    rate: u64,

    /// Frame length in bytes (including all headers).
    #[clap(short = 'l', long, default_value_t = 60)]
    len: usize,

    /// Source MAC address (default: interface MAC).
    #[clap(long)]
    src_mac: Option<String>,

    /// Destination MAC address.
    #[clap(long)]
    dst_mac: String,

    /// Source IPv4 address.
    #[clap(long)]
    src_ip: String,

    /// Destination IPv4 address.
    #[clap(long)]
    dst_ip: String,

    /// UDP source port.
    #[clap(long, default_value_t = 1234)]
    src_port: u16,

    /// UDP destination port.
    #[clap(long, default_value_t = 1234)]
    dst_port: u16,

    /// Underlying packet‑I/O framework.
    #[clap(subcommand)]
    framework: Framework,
}

#[derive(Subcommand, Debug, Clone)]
enum Framework {
    /// Use **netmap**.
    Netmap(NetmapArgs),
    /// Use **AF_XDP**.
    AfXdp(AfXdpArgs),
    /// Use **DPDK**.
    Dpdk(DpdkArgs),
    // Use **pcap** (for testing / low speed).
    Pcap(PcapArgs),
}


// ───────────────────────── pcap specific ──────────────────────────────
#[derive(Parser, Debug, Clone)]
struct PcapArgs {
    /// Snaplen passed to libpcap.
    #[clap(long, default_value_t = 65535)]
    snaplen: i32,
    /// Promiscuous mode.
    #[clap(long, default_value_t = true)]
    promiscuous: bool,
    /// Read timeout in milliseconds (for live captures).
    #[clap(long, default_value_t = 1)]
    timeout_ms: i32,
    /// libpcap immediate mode (deliver packets as soon as they arrive).
    #[clap(long, default_value_t = true)]
    immediate: bool,
    /// Optional BPF filter (tcpdump syntax).
    #[clap(long)]
    filter: Option<String>,
    /// Size of each buffer in the pool (bytes).
    #[clap(long, default_value_t = 2048)]
    buffer_size: usize,
    /// Initial number of buffers to preallocate.
    #[clap(long, default_value_t = 32)]
    buffer_count: usize,
}

// ───────────────────────── netmap specific ────────────────────────────
#[derive(Parser, Debug, Clone)]
struct NetmapArgs {
    #[clap(long, default_value_t = 1024)]
    extra_buf: u32,
    #[clap(long, default_value_t = 256)]
    consumer_buffer_size: usize,
    #[clap(long, default_value_t = 256)]
    producer_buffer_size: usize,
}

// ───────────────────────── dpdk specific ──────────────────────────────
#[derive(Parser, Debug, Clone)]
struct DpdkArgs {
    #[clap(long, default_value_t = 8192)]
    num_mbufs: u32,
    #[clap(long, default_value_t = 250)]
    mbuf_cache_size: u32,
    #[clap(long, default_value_t = 2176)]
    mbuf_default_buf_size: u32,
    #[clap(long, default_value_t = 256)]
    consumer_buffer_size: usize,
    #[clap(long, default_value_t = 256)]
    producer_buffer_size: usize,
}

// ───────────────────────── af_xdp specific ────────────────────────────
#[derive(Parser, Debug, Clone)]
struct AfXdpArgs {
    #[clap(long, default_value_t = 0)]
    bind_flags: u16,
    #[clap(long, default_value_t = 0)]
    xdp_flags: u32,
}

//──────────────────────────────────────────────────────────────────────────────
// Helpers
//──────────────────────────────────────────────────────────────────────────────

fn mac_from_str(s: &str) -> Result<[u8; 6]> {
    let parts: Vec<u8> = s
        .split([':', '-'])
        .map(|p| u8::from_str_radix(p, 16))
        .collect::<std::result::Result<_, _>>()?;
    if parts.len() != 6 {
        bail!("Invalid MAC address");
    }
    Ok([parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]])
}

fn ipv4_from_str(s: &str) -> Result<[u8; 4]> {
    Ok(Ipv4Addr::from_str(s)?.octets())
}
/// Build a single UDP/IPv4/Ethernet packet template of the desired `len`.
fn build_packet_template(args: &Args) -> Result<Vec<u8>> {
    let src_mac = args
        .src_mac
        .as_deref()
        .map(mac_from_str)
        .transpose()?
        .unwrap_or([0, 0, 0, 0, 0, 0]); // real MAC lookup omitted
    let dst_mac = mac_from_str(&args.dst_mac)?;
    let src_ip = ipv4_from_str(&args.src_ip)?;
    let dst_ip = ipv4_from_str(&args.dst_ip)?;

    // Build headers with etherparse (checksums auto‑calculated).
    let builder = PacketBuilder::ethernet2(src_mac, dst_mac)
        .ipv4(src_ip, dst_ip, 64) // ttl
        .udp(args.src_port, args.dst_port);

    let header_len = builder.size(0);
    if header_len > args.len {
        bail!(
            "Requested frame length ({}) is smaller than headers ({}).",
            args.len,
            header_len
        );
    }
    let payload_len = args.len - header_len;

    let payload = vec![0u8; payload_len];
    let mut packet = Vec::<u8>::with_capacity(args.len);
    builder.write(&mut packet, &payload)?;
    Ok(packet)
}

//──────────────────────────────────────────────────────────────────────────────
// Core logic
//──────────────────────────────────────────────────────────────────────────────

const FLUSH_EVERY: usize = 64; // how many packets before a flush

fn run_tx<Sock: Socket + 'static>(flags: Sock::Flags, args: &Args) -> Result<()> {
    println!(
        "pktgen_tx – interface {} ({} sockets, {})",
        args.interface,
        args.sockets,
        if args.multithreading {
            "multithreaded"
        } else {
            "single‑threaded"
        }
    );

    // Ctrl‑C terminator.
    let term = Arc::new(AtomicBool::new(false));
    {
        let term = term.clone();
        ctrlc::set_handler(move || {
            term.store(true, Ordering::SeqCst);
        })?;
    }

    // Counters for stats.
    let totals: Vec<_> = (0..args.sockets)
        .map(|_| Arc::new(AtomicU64::new(0)))
        .collect();

    // Pre‑built packet template so threads don’t allocate.
    let pkt_template = build_packet_template(args)?;

    // Open sockets (one per queue / per socket).
    let mut sockets = Vec::with_capacity(args.sockets);
    for i in 0..args.sockets {
        let portspec = if args.sockets > 1 {
            args.interface.clone() // you might want to append ":{i}" depending on driver
        } else {
            args.interface.clone()
        };
        let socket = Sock::create(&portspec, args.queue.or(Some(i)), flags.clone())?;
        // Clone contexts to avoid contention (see nethuns docs)
        sockets.push(socket);
    }

    // Stats printer.
    let totals_stats = totals.clone();
    let term_stats = term.clone();
    let stats_handle = if let Some(sockid) = args.sockstats {
        thread::spawn(move || {
            let mut prev = 0u64;
            while !term_stats.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_secs(1));
                let cur = totals_stats[sockid].load(Ordering::SeqCst);
                prev = cur;
            }
        })
    } else {
        thread::spawn(move || {
            let mut prev = 0u64;
            while !term_stats.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_secs(1));
                let cur: u64 = totals_stats.iter().map(|c| c.load(Ordering::SeqCst)).sum();
                println!("total: {} pkt/s", cur - prev);
                prev = cur;
            }
        })
    };

    // Worker(s).
    let mut handles = Vec::new();

    let rate_pps = args.rate; // 0 == unlimited
    let spacing = if rate_pps > 0 {
        Some(Duration::from_secs_f64(1.0 / rate_pps as f64))
    } else {
        None
    };

    if args.multithreading {
        for (sock_id, mut sock) in sockets.into_iter().enumerate() {
            let term = term.clone();
            let counter = totals[sock_id].clone();
            let pkt = pkt_template.clone();
            let spacing = spacing.clone();
            let count_limit = args.count;
            let handle = thread::spawn(move || {
                let mut sent: u64 = 0;
                let mut local_queue = 0;
                let mut next_ts = Instant::now();

                while !term.load(Ordering::Acquire) {
                    if count_limit > 0 && sent >= count_limit {
                        break;
                    }

                    // simple rate limiting
                    if let Some(d) = spacing {
                        let now = Instant::now();
                        if now < next_ts {
                            std::thread::sleep(next_ts - now);
                        }
                        next_ts += d;
                    }

                    match sock.send(&pkt) {
                        Ok(_) => {
                            sent += 1;
                            local_queue += 1;
                            if local_queue >= FLUSH_EVERY {
                                let _ = sock.flush();
                                local_queue = 0;
                            }
                            // aggregated counters
                            counter.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(err) => {
                        }
                    }
                }
                // final flush
                let _ = sock.flush();
            });
            handles.push(handle);
        }
    } else {
        // single‑thread covering all sockets
        let term = term.clone();
        let totals_ref = totals.clone();
        let args = args.clone();
        let handle = thread::spawn(move || {
            let mut sent_total = 0u64;
            let mut next_ts = Instant::now();
            loop {
                if term.load(Ordering::Acquire) {
                    break;
                }
                for (idx, sock) in sockets.iter_mut().enumerate() {
                    if args.count > 0 && sent_total >= args.count {
                        term.store(true, Ordering::SeqCst);
                        break;
                    }
                    if let Some(d) = spacing {
                        let now = Instant::now();
                        if now < next_ts {
                            std::thread::sleep(next_ts - now);
                        }
                        next_ts += d;
                    }
                    match sock.send(&pkt_template) {
                        Ok(_) => {
                            sent_total += 1;
                            totals_ref[idx].fetch_add(1, Ordering::Relaxed);
                            if sent_total % FLUSH_EVERY as u64 == 0 {
                                let _ = sock.flush();
                            }
                        }
                        Err(err) => {
                        }
                    }
                }
            }
            // final flushes
            for sock in &mut sockets {
                let _ = sock.flush();
            }
        });
        handles.push(handle);
    }

    // Wait for workers & stats.
    for h in handles {
        h.join().expect("worker panicked");
    }
    stats_handle.join().expect("stats panicked");

    Ok(())
}

//──────────────────────────────────────────────────────────────────────────────
// Entrypoint
//──────────────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let args = Args::parse();

    match &args.framework {
        Framework::Netmap(nm) => {
            let flags = netmap::NetmapFlags {
                extra_buf: nm.extra_buf,
            };
            run_tx::<netmap::Sock>(flags, &args)?;
        }
        Framework::AfXdp(xdp) => {
            let flags = af_xdp::AfXdpFlags {
                bind_flags: xdp.bind_flags,
                xdp_flags: xdp.xdp_flags,
                num_frames: 4096 * 8,
                frame_size: 2048,
                tx_size: 2048,
                rx_size: 2048,
            };
            run_tx::<af_xdp::Sock>(flags, &args)?;
        }
        Framework::Dpdk(dp) => {
            let flags = dpdk::DpdkFlags {
                num_mbufs: dp.num_mbufs,
                mbuf_cache_size: dp.mbuf_cache_size,
                mbuf_default_buf_size: dp.mbuf_default_buf_size as u16,
            };
            run_tx::<dpdk::Sock>(flags, &args)?;
        }
        Framework::Pcap(pcap) => {
            let flags = nethuns_rs::pcap::PcapFlags {
                snaplen: pcap.snaplen,
                promiscuous: pcap.promiscuous,
                timeout_ms: pcap.timeout_ms,
                immediate: pcap.immediate,
                filter: None,
                buffer_size: pcap.buffer_size,
                buffer_count: pcap.buffer_count,
            };
            run_tx::<nethuns_rs::pcap::Sock>(flags, &args)?;
        }
    }
    Ok(())
}
