use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread;
use std::time::Duration;

// Import the framework APIs.
use crate::strategy::{MpscArgs, MpscStrategy};
use crate::{
    af_xdp,
    api::{Flags, Socket, Strategy, StrategyArgs},
    netmap,
};

/// Command-line arguments.
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Input interface name.
    in_if: String,

    // Queue
    queue: Option<usize>,

    /// Output interface name.
    out_if: String,

    /// Choose the network framework.
    #[clap(subcommand)]
    framework: Framework,
}

#[derive(Subcommand, Debug, Clone)]
enum Framework {
    /// Use netmap framework.
    Netmap(NetmapArgs),
    /// Use AF_XDP framework.
    AfXdp(AfXdpArgs),
}

/// Netmap-specific arguments.
#[derive(Parser, Debug, Clone)]
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
#[derive(Parser, Debug, Clone)]
struct AfXdpArgs {
    /// Bind flags for AF_XDP.
    #[clap(long, default_value_t = 0)]
    bind_flags: u16,
    /// XDP flags for AF_XDP.
    #[clap(long, default_value_t = 0)]
    xdp_flags: u32,
}

pub(crate) fn routine() -> Result<()> {
    // Parse command-line arguments.
    let args = Args::parse();

    // Set up a termination flag triggered on Ctrl-C.
    let term = Arc::new(AtomicBool::new(false));
    {
        let term = term.clone();
        ctrlc::set_handler(move || {
            term.store(true, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");
    }

    // Choose the proper framework and run the forwarder.
    match args.framework.clone() {
        Framework::Netmap(netmap_args) => {
            let flags = Flags::Netmap(netmap::NetmapFlags {
                extra_buf: netmap_args.extra_buf,
                strategy_args: StrategyArgs::Mpsc(MpscArgs {
                    consumer_buffer_size: netmap_args.consumer_buffer_size,
                    producer_buffer_size: netmap_args.producer_buffer_size,
                }),
            });
            run_forwarder::<netmap::Sock<MpscStrategy>>(flags, &args, term)
        }
        Framework::AfXdp(af_xdp_args) => {
            let flags = Flags::AfXdp(af_xdp::AfXdpFlags {
                bind_flags: af_xdp_args.bind_flags,
                xdp_flags: af_xdp_args.xdp_flags,
                strategy_args: StrategyArgs::Mpsc(MpscArgs {
                    consumer_buffer_size: 256,
                    producer_buffer_size: 256,
                }),
                num_frames: 4096,
                frame_size: 2048,
            });
            run_forwarder::<af_xdp::Sock<MpscStrategy>>(flags, &args, term)
        }
    }
}

/// The main packet-forwarding routine.
///
/// This function creates an input and an output socket, spawns a meter thread,
/// then enters a loop where it receives a packet on the input interface, and forwards it
/// to the output interface using a retry loop.
fn run_forwarder<Sock>(flags: Flags, args: &Args, term: Arc<AtomicBool>) -> Result<()>
where
    Sock: Socket<MpscStrategy> + 'static,
{
    println!("Starting packet forwarder:");
    println!("  Input interface: {}", args.in_if);
    println!("  Output interface: {}", args.out_if);

    // Create the input and output sockets using the selected framework.
    let mut in_socket = Sock::create(&args.in_if, args.queue, flags.clone())?;
    let mut out_socket = Sock::create(&args.out_if, args.queue, flags.clone())?;

    // Atomic counters for received and forwarded packets.
    let total_rcv = Arc::new(AtomicU64::new(0));
    let total_fwd = Arc::new(AtomicU64::new(0));

    // Spawn a meter thread that prints packet rates every second.
    {
        let total_rcv = total_rcv.clone();
        let total_fwd = total_fwd.clone();
        let term_meter = term.clone();
        thread::spawn(move || {
            let mut prev_rcv = 0;
            let mut prev_fwd = 0;
            while !term_meter.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_secs(1));
                let curr_rcv = total_rcv.load(Ordering::SeqCst);
                let curr_fwd = total_fwd.load(Ordering::SeqCst);
                println!(
                    "pkt/sec: {} fwd/sec: {}",
                    curr_rcv.saturating_sub(prev_rcv),
                    curr_fwd.saturating_sub(prev_fwd)
                );
                prev_rcv = curr_rcv;
                prev_fwd = curr_fwd;
            }
        });
    }

    // Forwarding loop.
    while !term.load(Ordering::SeqCst) {
        // Receive a packet from the input socket.
        let (packet, meta) = match in_socket.recv() {
            Ok((p, m)) => (p, m),
            Err(e) => {
                eprintln!("Receive error: {:?}", e);
                continue;
            }
        };
        total_rcv.fetch_add(1, Ordering::SeqCst);

        // Forward the packet with a retry loop.
        loop {
            match out_socket.send(&packet) {
                // On success, exit the retry loop.
                Ok(_) => break,
                Err(e) => {
                    out_socket.flush();
                }
            }
        }
        total_fwd.fetch_add(1, Ordering::SeqCst);

        // Release the packet from the input socket.
        // (Assumes that meta contains a packet identifier for release.)
    }

    Ok(())
}
