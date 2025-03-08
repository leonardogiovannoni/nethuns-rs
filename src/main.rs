#![feature(associated_type_defaults)]
#![feature(cell_update)]
#![feature(slice_ptr_get)]
mod af_xdp;
mod api;
mod netmap;
use anyhow::{Result, bail};
use api::{NethunsSocket, NethunsToken};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::sync::atomic::Ordering;
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

fn main_netmap<S: NethunsSocket + 'static>(flags: S::Flags) -> Result<()> {
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
    let mut sockets = Vec::with_capacity(args.sockets);
    for i in 0..args.sockets {
        let portspec = if args.sockets > 1 {
            // format!("{}:{}", args.interface, i)
            panic!()
        } else {
            // format!("{}", args.interface)
            args.interface.clone()
        };
        let socket = S::create(&portspec, None, flags.clone())?;
        sockets.push(socket);
    }

    // Start the statistics thread.
    // If a per-socket stat is requested via --sockstats, print that socket’s stats;
    // otherwise print the aggregated count.
    let mut sockets = sockets;
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
    let netmap_flags = netmap::NetmapFlags { extra_buf: 1024 };

    let res = main_netmap::<netmap::Socket>(netmap_flags);
    match res {
        Ok(_) => println!("Success"),
        Err(e) => eprintln!("Error: {:?}", e),
    }
    let af_xdp_flags = af_xdp::AfXdpFlags {
        bind_flags: 0,
        xdp_flags: 0,
    };
    let res = main_netmap::<af_xdp::Socket>(af_xdp_flags);
    match res {
        Ok(_) => println!("Success"),
        Err(e) => eprintln!("Error: {:?}", e),
    }
}
