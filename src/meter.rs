
//mod fake_refcell;
use anyhow::{Result, bail};
use api::{Flags, Socket, Token, Strategy, StrategyArgs};
use clap::{Parser, Subcommand};
use etherparse::{NetHeaders, PacketHeaders};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
use strategy::{CrossbeamArgs, CrossbeamStrategy, MpscArgs, MpscStrategy, StdStrategy};

use crate::{af_xdp, api, netmap, strategy};

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
    Netmap(NetmapArgs),
    /// Use AF_XDP framework.
    AfXdp(AfXdpArgs),
}

/// Netmap-specific arguments.
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

fn run<Sock: Socket<S> + 'static, S: Strategy>(
    flags: Flags,
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
        let socket = Sock::create(&portspec, args.queue, None, flags.clone())?;
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
        for (i, mut socket) in sockets.into_iter().enumerate() {
            let term_thread = term.clone();
            let counter = totals[i].clone();
            let debug = args.debug;
            let handle = {
                let ctx = socket.context().clone();
                thread::spawn(move || {
                    let mut local_counter = 0;
                    while !term_thread.load(Ordering::SeqCst) {
                        local_counter += 1;
                        if local_counter == BULK {
                            counter.fetch_add(local_counter, Ordering::SeqCst);
                            local_counter = 0;
                        }
                        let res: Result<()> = (|| {
                            let (pkt, meta) = socket.recv()?;
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
                for (i, socket) in sockets.iter_mut().enumerate() {
                    let res: Result<()> = (|| {
                        // let packet = &*socket.recv()?.load(&ctx);
                        let (packet, meta) = socket.recv()?;
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



pub(crate) fn routine() -> Result<()> {
    let args = Args::parse();
    match &args.framework {
        Framework::Netmap(netmap_args) => {
            let flags = Flags::Netmap(netmap::NetmapFlags {
                extra_buf: netmap_args.extra_buf,
                strategy_args: StrategyArgs::Mpsc(MpscArgs {
                    buffer_size: netmap_args.extra_buf as usize,
                    consumer_buffer_size: netmap_args.consumer_buffer_size,
                    producer_buffer_size: netmap_args.producer_buffer_size,
                }),
            });
            run::<netmap::Sock<MpscStrategy>, _>(flags, &args)?;
        }
        Framework::AfXdp(af_xdp_args) => {
            let flags = Flags::AfXdp(af_xdp::AfXdpFlags {
                bind_flags: af_xdp_args.bind_flags,
                xdp_flags: af_xdp_args.xdp_flags,
                strategy_args: StrategyArgs::Mpsc(MpscArgs {
                    buffer_size: 1024,
                    consumer_buffer_size: 256,
                    producer_buffer_size: 256,
                }),
            });
            run::<af_xdp::Sock<MpscStrategy>, _>(flags, &args)?;
        }
        _ => bail!("Unsupported framework"),
    }
    Ok(())
}
