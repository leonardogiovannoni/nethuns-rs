//mod fake_refcell;
use anyhow::Result;
use anyhow::bail;
use api::{Flags, Socket, Token};
use clap::{Parser, Subcommand};
use etherparse::{NetHeaders, PacketHeaders};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use nethuns_rs::api;
#[cfg(feature = "af_xdp")]
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
    #[cfg(feature = "af_xdp")]
    AfXdp(AfXdpArgs),
    #[cfg(feature = "dpdk")]
    Dpdk(DpdkArgs),
    #[cfg(feature = "pcap")]
    Pcap(PcapArgs),
}


/// Pcap-specific arguments.
#[derive(Parser, Debug)]
#[cfg(feature = "pcap")]
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

/// Netmap-specific arguments.
#[derive(Parser, Debug)]
#[cfg(feature = "netmap")]
struct NetmapArgs {
    /// Extra buffer size for netmap.
    #[clap(long, default_value_t = 1024)]
    extra_buf: u32,

    #[clap(long, default_value_t = 256)]
    consumer_buffer_size: usize,

    #[clap(long, default_value_t = 256)]
    producer_buffer_size: usize,
}

#[derive(Parser, Debug)]
#[cfg(feature = "dpdk")]
struct DpdkArgs {
    /// Extra buffer size for netmap.

    // num_mbufs: 8192,
    #[clap(long, default_value_t = 8192)]
    num_mbufs: u32,

    // mbuf_cache_size: 250,
    #[clap(long, default_value_t = 250)]
    mbuf_cache_size: u32,

    // mbuf_default_buf_size: 2176,
    #[clap(long, default_value_t = 2176)]
    mbuf_default_buf_size: u32,

    #[clap(long, default_value_t = 256)]
    consumer_buffer_size: usize,

    #[clap(long, default_value_t = 256)]
    producer_buffer_size: usize,
}

/// AF_XDP-specific arguments.
#[derive(Parser, Debug)]
#[cfg(feature = "af_xdp")]
struct AfXdpArgs {
    /// Bind flags for AF_XDP.
    #[clap(long, default_value_t = 0)]
    bind_flags: u16,
    /// XDP flags for AF_XDP.
    #[clap(long, default_value_t = 0)]
    xdp_flags: u32,
}

/// Try to parse Ethernet/IP headers using etherparse and return a formatted string.
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
    let mut sockets = Vec::with_capacity(args.sockets);
    for i in 0..args.sockets {
        let portspec = if args.sockets > 1 {
            // In a multi-socket scenario, append the socket id.
            //format!("{}:{}", args.interface, i)
            args.interface.clone()
        } else {
            args.interface.clone()
        };
        let socket = Sock::create(&portspec, Some(i) /*args.queue*/, flags.clone())?;
        // for _ in 0..32 - 1 {
        //     std::mem::forget(socket.context().clone());
        // }
        sockets.push(socket);
    }

    // Start the statistics thread.
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
        for (i, socket) in sockets.into_iter().enumerate() {
            let term_thread = term.clone();
            let counter = totals[i].clone();
            let debug = args.debug;
            let handle = {
                //let ctx = socket.context().clone();
                thread::spawn(move || {
                    let mut local_counter = 0;
                    while !term_thread.load(Ordering::SeqCst) {
                        local_counter += 1;
                        if local_counter == BULK {
                            counter.fetch_add(local_counter, Ordering::SeqCst);
                            local_counter = 0;
                        }

                        let res: Option<()> = (|| {
                            let (pkt, meta) = socket.recv().ok()?;
                            if debug {
                                if let Ok(info) = print_addrs(&pkt) {
                                    println!("Thread {}: {}", i, info);
                                }
                            }
                            Some(())
                        })();
                        if res.is_none() {
                            // Optionally handle the error here.
                        }
                    }
                })
            };
            handles.push(handle);
        }
    } else {
        println!("Single-threaded mode");
        // Single-threaded loop over all sockets.
        let totals = Arc::new(totals);
        let term_loop = term.clone();
        let debug = args.debug;
        let handle = thread::spawn(move || {
            let mut local_counters = vec![0; sockets.len()];
            while !term_loop.load(Ordering::Acquire) {
                for _ in 0..1000 {
                    for (i, socket) in sockets.iter_mut().enumerate() {
                        let res: Option<()> = (|| {
                            let (packet, meta) = socket.recv().ok()?;
                            // let tmp = socket.recv();
                            // let (packet, meta) = match tmp {
                            //     Ok((packet, meta)) => (packet, meta),
                            //     Err(err) => {
                            //         println!("{:?}", err);
                            //         return None;
                            //     }
                            // };

                            local_counters[i] += 1;
                            if local_counters[i] == BULK {
                                totals[i].fetch_add(local_counters[i], Ordering::Relaxed);
                                local_counters[i] = 0;
                            }
                            if debug {
                                if let Ok(info) = print_addrs(&*packet) {
                                    println!("Socket {}: {}", i, info);
                                }
                            }
                            Some(())
                        })();
                        if res.is_none() {
                            // Optionally handle the error here.
                        }
                    }
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all receiver threads and the stats thread to complete.
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
        #[cfg(feature = "af_xdp")]
        Framework::AfXdp(af_xdp_args) => {
            let flags = af_xdp::AfXdpFlags {
                bind_flags: af_xdp_args.bind_flags,
                xdp_flags: af_xdp_args.xdp_flags,
                num_frames: 4096,
                frame_size: 4096,
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
    }
    Ok(())
}
