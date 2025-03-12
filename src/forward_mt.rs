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

use ringbuf::HeapCons;
use ringbuf::{
    HeapProd,
    traits::{Consumer, Producer, Split},
};
// Use Crossbeam's ArrayQueue for the SPSC queue (add crossbeam-queue to Cargo.toml).
use crossbeam_queue::ArrayQueue;

/// Command-line arguments.
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Input interface name.
    in_if: String,
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
struct Packet<S: Context> {
    payload: S::Token,
}

/// Generic function that sets up the SPSC queue, spawns meter and consumer threads,
/// and then runs the producer loop.
fn run_queue<Sock>(flags: Flags, args: &Args, term: Arc<AtomicBool>) -> Result<()>
where
    Sock: Socket<MpscStrategy> + 'static,
{
    // Create a SPSC queue with capacity for 65,536 packets.
    // let queue = Arc::new(ArrayQueue::<Packet<_>>::new(65_536));

    let queue = ringbuf::HeapRb::<Packet<_>>::new(65_536);

    // Atomic counters for received and forwarded packets.
    let total_rcv = Arc::new(AtomicU64::new(0));
    let total_fwd = Arc::new(AtomicU64::new(0));

    // Spawn the meter thread.
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

    // Spawn the consumer thread which will create the output socket.
    // let queue_consumer = queue.clone();
    let (mut prod, mut cons) = queue.split();
    let total_fwd_consumer = total_fwd.clone();
    let out_if = args.out_if.clone();
    let flags_consumer = flags.clone();

    let (in_ctx, mut in_socket) = Sock::create(&args.in_if, None, flags.clone())?;

    let (_, mut out_socket) = Sock::create(&out_if, None, flags_consumer)?;
    {
        let in_ctx = in_ctx.clone();
        thread::spawn(move || -> Result<()> {
            out_socket.send(b"ciaoaaoaoaooaoa").unwrap();
            out_socket.flush();
            loop {
                for packet in cons.pop_iter() {
                    total_fwd_consumer.fetch_add(1, Ordering::SeqCst);
                    let packet: <Sock::Context as Context>::Token = packet.payload;
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
        // Assume recv_local() returns a tuple (Vec<u8>, PacketMeta).
        let (payload, meta) = match in_socket.recv() {
            Ok(res) => res,
            Err(e) => {
                continue;
            }
        };

        total_rcv.fetch_add(1, Ordering::SeqCst);
        let mut packet: Packet<Sock::Context> = Packet { payload };
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

    // Normally, you would also close the sockets and join threads.
    // For simplicity (and since the loops are infinite), we return Ok(()) here.
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
                    buffer_size: netmap_args.extra_buf as usize,
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
                    buffer_size: 1024,
                    consumer_buffer_size: 256,
                    producer_buffer_size: 256,
                }),
            });
            run_queue::<af_xdp::Sock<MpscStrategy>>(flags, &args, term)?;
        }
    }

    Ok(())
}
