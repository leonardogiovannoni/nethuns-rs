//! Token-based packet meter example.
//!
//! This variant exercises recv_token/packet handling by round-robining
//! payloads across cloned contexts.
use anyhow::{Result, bail};
use nethuns_rs::api::{Flags, Socket, Token};
use clap::{Parser, Subcommand};
use etherparse::{NetHeaders, PacketHeaders};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use nethuns_rs::api::Context;
#[cfg(feature = "af-xdp")]
use nethuns_rs::af_xdp;
#[cfg(feature = "dpdk")]
use nethuns_rs::dpdk;
#[cfg(feature = "netmap")]
use nethuns_rs::netmap;
#[cfg(feature = "pcap")]
use nethuns_rs::pcap;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Network interface name.
    #[clap(short, long)]
    interface: String,

    /// Queue index to bind (defaults to backend choice).
    #[clap(long)]
    queue: Option<usize>,

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

    /// Choose the network framework.
    #[clap(subcommand)]
    framework: Framework,
}

#[derive(Subcommand, Debug)]
enum Framework {
    /// Use netmap framework.
    #[cfg(feature = "netmap")]
    Netmap(NetmapArgs),
    /// Use AF_XDP framework.
    #[cfg(feature = "af-xdp")]
    AfXdp(AfXdpArgs),
    /// Use DPDK.
    #[cfg(feature = "dpdk")]
    Dpdk(DpdkArgs),
    /// Use pcap.
    #[cfg(feature = "pcap")]
    Pcap(PcapArgs),
}

/// Netmap-specific arguments.
#[cfg(feature = "netmap")]
#[derive(Parser, Debug)]
struct NetmapArgs {
    /// Extra buffer size for netmap.
    #[clap(long, default_value_t = 1024)]
    extra_buf: u32,

    #[clap(long, default_value_t = 256)]
    consumer_buffer_size: usize,

    #[clap(long, default_value_t = 256)]
    producer_buffer_size: usize,
}

#[cfg(feature = "dpdk")]
#[derive(Parser, Debug)]
struct DpdkArgs {
    /// Number of mbufs to allocate.
    #[clap(long, default_value_t = 8192)]
    num_mbufs: u32,

    /// Per-core mbuf cache size.
    #[clap(long, default_value_t = 250)]
    mbuf_cache_size: u32,

    /// Default mbuf data size.
    #[clap(long, default_value_t = 2176)]
    mbuf_default_buf_size: u32,

    #[clap(long, default_value_t = 256)]
    consumer_buffer_size: usize,

    #[clap(long, default_value_t = 256)]
    producer_buffer_size: usize,
}

/// AF_XDP-specific arguments.
#[cfg(feature = "af-xdp")]
#[derive(Parser, Debug)]
struct AfXdpArgs {
    /// Bind flags for AF_XDP.
    #[clap(long, default_value_t = 0)]
    bind_flags: u16,
    /// XDP flags for AF_XDP.
    #[clap(long, default_value_t = 0)]
    xdp_flags: u32,
}

/// Pcap-specific arguments.
#[cfg(feature = "pcap")]
#[derive(Parser, Debug)]
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

/// Parse Ethernet/IP headers and return a short source/destination string.
fn print_addrs(frame: &[u8]) -> Result<String> {
    let packet_header = PacketHeaders::from_ethernet_slice(frame).unwrap();
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

/// Receive tokens and report per-second rates.
fn run<Sock: Socket + 'static>(flags: Sock::Flags, args: &Args) -> Result<()> {
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

    let term = Arc::new(AtomicBool::new(false));
    {
        let term = term.clone();
        ctrlc::set_handler(move || {
            term.store(true, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");
    }

    let totals: Vec<Arc<AtomicU64>> = (0..args.sockets)
        .map(|_| Arc::new(AtomicU64::new(0)))
        .collect();

    let mut sockets = Vec::with_capacity(args.sockets);
    let mut ctxs: Vec<_> = Vec::new();
    for i in 0..args.sockets {
        let portspec = if args.sockets > 1 {
            format!("{}:{}", args.interface, i)
        } else {
            args.interface.clone()
        };
        let socket = Sock::create(&portspec, args.queue, flags.clone())?;
        for _ in 0..64 - 1 {
            ctxs.push(socket.context().clone());
        }
        sockets.push(socket);
    }

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

    let mut handles = Vec::new();
    const BULK: u64 = 10000;
    println!("Single-threaded mode");
    let totals = Arc::new(totals);
    let term_loop = term.clone();
    let debug = args.debug;

    let mut j = 0;
    let handle = thread::spawn(move || {
        let mut local_counters = vec![0; sockets.len()];
        while !term_loop.load(Ordering::Acquire) {
            for _ in 0..1000 {
                for (i, socket) in sockets.iter_mut().enumerate() {
                    let res: Result<()> = (|| {
                        let (token, meta) = socket.recv_token()?;
                        local_counters[i] += 1;
                        if local_counters[i] == BULK {
                            totals[i].fetch_add(local_counters[i], Ordering::Relaxed);
                            local_counters[i] = 0;
                        }
                        let payload = ctxs[j].packet(token);
                        j = (j + 1) % ctxs.len();
                        Ok(())
                    })();
                    if res.is_err() {
                    }
                }
            }
        }
    });
    handles.push(handle);

    for handle in handles {
        handle.join().expect("Receiver thread panicked");
    }
    stats_handle.join().expect("Stats thread panicked");

    Ok(())
}

pub fn main() -> Result<()> {
    let args = Args::parse();
    match &args.framework {
        #[cfg(feature = "netmap")]
        Framework::Netmap(netmap_args) => {
            let flags = netmap::NetmapFlags {
                extra_buf: netmap_args.extra_buf,
            };
            run::<netmap::Sock>(flags, &args)?;
        }
        #[cfg(feature = "af-xdp")]
        Framework::AfXdp(af_xdp_args) => {
            let flags = af_xdp::AfXdpFlags {
                bind_flags: af_xdp_args.bind_flags,
                xdp_flags: af_xdp_args.xdp_flags,
                num_frames: 4096,
                frame_size: 2048,
                tx_size: 2048,
                rx_size: 2048,
            };
            run::<af_xdp::Sock>(flags, &args)?;
        }
        #[cfg(feature = "dpdk")]
        Framework::Dpdk(dpdk_args) => {
            let flags = dpdk::DpdkFlags {
                num_mbufs: dpdk_args.num_mbufs,
                mbuf_cache_size: dpdk_args.mbuf_cache_size,
                mbuf_default_buf_size: dpdk_args.mbuf_default_buf_size as u16,
            };
            run::<dpdk::Sock>(flags, &args)?;
        }
        #[cfg(feature = "pcap")]
        Framework::Pcap(pcap_args) => {
            let flags = pcap::PcapFlags {
                snaplen: pcap_args.snaplen,
                promiscuous: pcap_args.promiscuous,
                timeout_ms: pcap_args.timeout_ms,
                immediate: pcap_args.immediate,
                filter: pcap_args.filter.clone(),
                buffer_size: pcap_args.buffer_size,
                buffer_count: pcap_args.buffer_count,
            };
            run::<pcap::Sock>(flags, &args)?;
        }
        _ => bail!("Unsupported framework"),
    }
    Ok(())
}
