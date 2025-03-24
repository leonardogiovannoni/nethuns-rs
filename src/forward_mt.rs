use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread;
use std::time::Duration;

// Import the framework APIs.
use crate::{
    af_xdp,
    api::{Flags, Socket, StrategyArgs},
    netmap,
};
use crate::{
    api::{Context, Token},
    strategy::{MpscArgs, MpscStrategy},
};

use ringbuf::{
    traits::{Consumer, Producer, Split},
};
// Use Crossbeam's ArrayQueue for the SPSC queue (add crossbeam-queue to Cargo.toml).

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

/// Netmap–specific arguments.
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

/// AF_XDP–specific arguments.
#[derive(Parser, Debug, Clone)]
struct AfXdpArgs {
    /// Bind flags for AF_XDP.
    #[clap(long, default_value_t = 0)]
    bind_flags: u16,
    /// XDP flags for AF_XDP.
    #[clap(long, default_value_t = 0)]
    xdp_flags: u32,
}

/// A structure holding packet metadata.
/// (Adjust fields to match the actual metadata returned by `recv_local()`.)
struct PacketMeta {
    packet_id: u64,
}

/// A structure that packages a received packet.
struct Packet<Ctx: Context> {
    payload: Ctx::Token,
}

/// Generic function that sets up the SPSC queue, spawns meter and consumer threads,
/// and then runs the producer loop.
fn run_queue<Sock>(flags: Flags, args: &Args, term: Arc<AtomicBool>) -> Result<()>
where
    Sock: Socket<MpscStrategy> + 'static,
{
    // Create a SPSC queue with capacity for 65,536 packets.
    // let queue = Arc::new(ArrayQueue::<Packet<_>>::new(65_536));

    let queue = ringbuf::HeapRb::<Packet<Sock::Context>>::new(65_536);

    // Atomic counters for received and forwarded packets.
    let total_rcv = Arc::new(AtomicU64::new(0));
    let total_fwd = Arc::new(AtomicU64::new(0));

    {
        let total_rcv = total_rcv.clone();
        let total_fwd = total_fwd.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(1));
                let rcv = total_rcv.swap(0, Ordering::SeqCst);
                let fwd = total_fwd.swap(0, Ordering::SeqCst);
                println!("pkt/sec: {}  fwd/sec: {}", rcv, fwd);
            }
        });
    }

    let (mut prod, mut cons) = queue.split();
    let total_fwd_consumer = total_fwd.clone();
    let out_if = args.out_if.clone();
    let flags_consumer = flags.clone();

    let mut in_socket = Sock::create(&args.in_if, args.queue, flags.clone())?;
    let mut out_socket = Sock::create(&out_if, args.queue, flags_consumer)?;
    {
        let in_ctx = in_socket.context().clone();
        thread::spawn(move || -> Result<()> {
            out_socket.send(b"ciaoaaoaoaooaoa").unwrap();
            out_socket.flush();
            loop {
                for packet in cons.pop_iter() {
                    total_fwd_consumer.fetch_add(1, Ordering::SeqCst);
                    let packet = packet.payload;
                    let packet = &*packet.consume(&in_ctx);
                    out_socket.send(packet)?;
                    out_socket.flush();
                    total_fwd_consumer.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
    }

    // Main loop: receive packets and push them into the SPSC queue.
    while !term.load(Ordering::SeqCst) {
        let Ok((payload, meta)) = in_socket.recv() else {
            continue;
        };
        let payload = payload.into_token();
        total_rcv.fetch_add(1, Ordering::SeqCst);
        let mut packet = Packet { payload };
        // Busy-wait until the packet can be pushed into the queue.
        loop {
            if let Err(pkt) = prod.try_push(packet) {
                packet = pkt;
                thread::yield_now();
            } else {
                break;
            }
        }
    }

    Ok(())
}

pub(crate) fn routine() -> Result<()> {
    let args = Args::parse();

    // Set up a termination flag (triggered by Ctrl-C).
    let term = Arc::new(AtomicBool::new(false));
    {
        let term = term.clone();
        ctrlc::set_handler(move || {
            term.store(true, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");
    }

    // Choose the proper framework and call the generic run_queue function.
    match &args.framework {
        Framework::Netmap(netmap_args) => {
            let flags = Flags::Netmap(netmap::NetmapFlags {
                extra_buf: netmap_args.extra_buf,
                strategy_args: StrategyArgs::Mpsc(MpscArgs {
                    consumer_buffer_size: netmap_args.consumer_buffer_size,
                    producer_buffer_size: netmap_args.producer_buffer_size,
                }),
            });
            run_queue::<netmap::Sock<MpscStrategy>>(flags, &args, term)?;
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
            run_queue::<af_xdp::Sock<MpscStrategy>>(flags, &args, term)?;
        }
    }

    Ok(())
}
