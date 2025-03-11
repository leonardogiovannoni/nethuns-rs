mod af_xdp;
mod api;
mod netmap;
mod strategy;
use anyhow::{Result, bail};
use api::{NethunsFlags, NethunsSocket, NethunsToken, Strategy, StrategyArgsEnum};
use clap::{Parser, Subcommand};
use etherparse::{NetHeaders, PacketHeaders};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
use strategy::{CrossbeamArgs, CrossbeamStrategy, MpscArgs, MpscStrategy, StdStrategy};

// Framework-specific flag structures.
pub struct AfXdpFlags {
    pub bind_flags: u16,
    pub xdp_flags: u32,
}

pub struct NetmapFlags {
    pub extra_buf: usize,
}

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

    /// Choose the network framework.
    #[clap(subcommand)]
    framework: Framework,
}

#[derive(Subcommand, Debug)]
enum Framework {
    /// Use netmap framework.
    Netmap(NetmapArgs),
    /// Use AF_XDP framework.
    AfXdp(AfXdpArgs),
}

/// Netmap-specific arguments.
#[derive(Parser, Debug)]
struct NetmapArgs {
    /// Extra buffer size for netmap.
    #[clap(long, default_value_t = 1024)]
    extra_buf: usize,

    #[clap(long, default_value_t = 256)]
    consumer_buffer_size: usize,

    #[clap(long, default_value_t = 256)]
    producer_buffer_size: usize,
}

/// AF_XDP-specific arguments.
#[derive(Parser, Debug)]
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

fn run<S: Strategy, Sock: NethunsSocket<S> + 'static>(
    flags: NethunsFlags,
    args: &Args,
) -> Result<()> {
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
            format!("{}:{}", args.interface, i)
        } else {
            args.interface.clone()
        };
        let socket = Sock::create(&portspec, None, flags.clone())?;
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
                        let res: Result<()> = (|| {
                            let (pkt, meta) = socket.recv_local()?;
                            if debug {
                                if let Ok(info) = print_addrs(&pkt) {
                                    println!("Thread {}: {}", i, info);
                                }
                            }
                            Ok(())
                        })();
                        if res.is_err() {
                            // Optionally handle the error here.
                        }
                    }
                })
            };
            handles.push(handle);
        }
    } else {
        // Single-threaded loop over all sockets.
        let totals = Arc::new(totals);
        let term_loop = term.clone();
        let debug = args.debug;
        let handle = thread::spawn(move || {
            let mut local_counters = vec![0; sockets.len()];
            while !term_loop.load(Ordering::SeqCst) {
                for (i, (ctx, socket)) in sockets.iter_mut().enumerate() {
                    let res: Result<()> = (|| {
                        // let packet = &*socket.recv()?.load(&ctx);
                        let (packet, meta) = socket.recv_local()?;
                        local_counters[i] += 1;
                        if local_counters[i] == BULK {
                            totals[i].fetch_add(local_counters[i], Ordering::SeqCst);
                            local_counters[i] = 0;
                        }
                        if debug {
                            if let Ok(info) = print_addrs(&*packet) {
                                println!("Socket {}: {}", i, info);
                            }
                        }
                        Ok(())
                    })();
                    if res.is_err() {
                        // Optionally handle the error here.
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

// struct NethunsPort {
//     pub ifname: String,
//     pub ifindex: u32,
//     pub queue_id: u32,
// }
//
// fn ifname_to_ifindex(ifname: &str) -> Option<u32> {
//     let c_ifname = CString::new(ifname).ok()?;
//     let index = unsafe { libc::if_nametoindex(c_ifname.as_ptr()) };
//     if index == 0 { None } else { Some(index) }
// }
//
// impl FromStr for NethunsPort {
//     type Err = anyhow::Error;
//     fn from_str(portspec: &str) -> std::result::Result<Self, Self::Err> {
//         let parts: Vec<&str> = portspec.split(':').collect();
//         if parts.len() == 1 {
//             let ifname = parts[0].to_string();
//             let ifindex = ifname_to_ifindex(&ifname)
//                 .ok_or_else(|| anyhow::anyhow!("Invalid ifname"))?;
//             Ok(Self { ifname, ifindex, queue_id: 0 })
//         } else if parts.len() == 2 {
//             let ifname = parts[0].to_string();
//             let queue_id = parts[1].parse()?;
//             let ifindex = ifname_to_ifindex(&ifname)
//                 .ok_or_else(|| anyhow::anyhow!("Invalid ifname"))?;
//             Ok(Self { ifname, ifindex, queue_id })
//         } else {
//             Err(anyhow::anyhow!("Invalid port spec"))
//         }
//     }
// }

fn main() -> Result<()> {
    let args = Args::parse();
    match &args.framework {
        Framework::Netmap(netmap_args) => {
            run::<MpscStrategy, netmap::Socket<_>>(
                // netmap::NetmapFlags {
                //     extra_buf: netmap_args.extra_buf,
                //     strategy_args: Some(MpscArgs {
                //         buffer_size: netmap_args.extra_buf,
                //         consumer_buffer_size: netmap_args.consumer_buffer_size,
                //         producer_buffer_size: netmap_args.producer_buffer_size,
                //     })
                // },
                NethunsFlags::Netmap(netmap::NetmapFlags {
                    extra_buf: netmap_args.extra_buf,
                    strategy_args: StrategyArgsEnum::Mpsc(MpscArgs {
                        buffer_size: netmap_args.extra_buf,
                        consumer_buffer_size: netmap_args.consumer_buffer_size,
                        producer_buffer_size: netmap_args.producer_buffer_size,
                    }),
                }),
                &args,
            )?;

            //            run::<StdStrategy, netmap::Socket<_>>(
            //                netmap::NetmapFlags {
            //                    extra_buf: netmap_args.extra_buf,
            //                    strategy_args: None
            //                },
            //                &args,
            //            )?;

            //            run::<CrossbeamStrategy, netmap::Socket<_>>(
            //                netmap::NetmapFlags {
            //                    extra_buf: netmap_args.extra_buf,
            //                    strategy_args: Some(CrossbeamArgs {
            //                        buffer_size: netmap_args.extra_buf,
            //                    })
            //                },
            //                &args,
            //            )?;
        }
        Framework::AfXdp(af_xdp_args) => {
            // run::<MpscStrategy, af_xdp::Socket<_>>(
            //     af_xdp::AfXdpFlags {
            //         bind_flags: af_xdp_args.bind_flags,
            //         xdp_flags: af_xdp_args.xdp_flags,
            //         strategy_args: None,
            //     },
            //     &args,
            // )?;
            run::<MpscStrategy, af_xdp::Socket<_>>(
                NethunsFlags::AfXdp(af_xdp::AfXdpFlags {
                    bind_flags: af_xdp_args.bind_flags,
                    xdp_flags: af_xdp_args.xdp_flags,
                    strategy_args: StrategyArgsEnum::Mpsc(MpscArgs {
                        buffer_size: 1024,
                        consumer_buffer_size: 256,
                        producer_buffer_size: 256,
                    }),
                }),
                &args,
            )?;
        }
        _ => bail!("Unsupported framework"),
    }
    Ok(())
}
