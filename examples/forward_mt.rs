use anyhow::Result;
use arrayvec::ArrayVec;
use clap::{Parser, Subcommand};
use nethuns_rs::api::{NethunsPusher, Payload, Socket};
use nethuns_rs::pcap::PcapFlags;
use nethuns_rs::{af_xdp, netmap, pcap};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread;
use std::time::Duration;


/// Command-line arguments.
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Input interface name.
    in_if: String,
    /// Output interface name.
    out_if: String,

    // Queue
    ciao: Option<usize>,
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
    /// Use pcap framework.
    Pcap(PcapArgs),
}

/// Netmap–specific arguments.
#[derive(Parser, Debug, Clone)]
struct NetmapArgs {
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


const BATCH_SIZE: usize = 32;
/// Generic function that sets up the SPSC queue, spawns meter and consumer threads,
/// and then runs the producer loop.
fn run_queue<Sock>(flags: Sock::Flags, args: &Args, term: Arc<AtomicBool>) -> Result<()>
where
    Sock: Socket + 'static,
{
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

    let total_fwd_consumer = total_fwd.clone();
    let out_if = args.out_if.clone();
    let flags_consumer = flags.clone();
    let (in_socket, in_pusher, in_popper) =
        Sock::create_with_channel::<{ BATCH_SIZE }>(&args.in_if, args.ciao, flags.clone(), flume::unbounded())?;
    let out_socket = Sock::create(&out_if, args.ciao, flags_consumer)?;
    {
        let _fwd_thread = thread::spawn(move || -> Result<()> {
            for _ in 0..BATCH_SIZE {
                out_socket.send(b"ciaoaaoaoaooaoa")?;
            }
            out_socket.flush();
            loop {
                if let Some(batch) = in_popper.pop() {
                    let len = batch.len();
                    for packet in batch {
                        out_socket.send(&packet)?;
                        out_socket.flush();
                    }
                    total_fwd_consumer.fetch_add(len as u64, Ordering::Relaxed);
                }
            }
        });
    }

    let mut local_total_rcv = 0;

    let mut batch = ArrayVec::new();
    while !term.load(Ordering::SeqCst) {
        let Ok((payload, meta)) = in_socket.recv() else {
            continue;
        };
        local_total_rcv += 1;

        if local_total_rcv >= batch.capacity() {
            total_rcv.fetch_add(local_total_rcv as u64, Ordering::Relaxed);
            local_total_rcv = 0;
        }
        batch.push(payload);
        if batch.len() == batch.capacity() {
            let b = match batch.into_inner() {
                Ok(batch) => batch,
                Err(_) => unreachable!(),
            };
            spin_push::<{ BATCH_SIZE }, Sock>(&in_pusher, b);
            batch = ArrayVec::new();
        }
    }

    Ok(())
}

fn spin_push<'a, const BATCH_SIZE: usize, S: Socket>(
    in_pusher: &'a NethunsPusher<BATCH_SIZE, S::Context>,
    mut batch: [Payload<'a, S::Context>; BATCH_SIZE],
) {
    loop {
        if let Err(b) = in_pusher.push(batch) {
            batch = b;
            thread::yield_now();
            continue;
        }
        break;
    }
}

pub fn main() -> Result<()> {
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
            let flags = netmap::NetmapFlags {
                extra_buf: netmap_args.extra_buf,
            };
            run_queue::<netmap::Sock>(flags, &args, term)?;
        }
        Framework::AfXdp(af_xdp_args) => {
            let flags = af_xdp::AfXdpFlags {
                bind_flags: af_xdp_args.bind_flags,
                xdp_flags: af_xdp_args.xdp_flags,
                num_frames: 4096,
                frame_size: 2048,
                tx_size: 2048,
                rx_size: 2048,
            };
            run_queue::<af_xdp::Sock>(flags, &args, term)?;
        }
        Framework::Pcap(pcap_args) => {
            let flags = PcapFlags {
                snaplen: pcap_args.snaplen,
                promiscuous: pcap_args.promiscuous,
                timeout_ms: pcap_args.timeout_ms,
                immediate: pcap_args.immediate,
                filter: pcap_args.filter.clone(),
                buffer_size: pcap_args.buffer_size,
                buffer_count: pcap_args.buffer_count,
            };
            run_queue::<pcap::Sock>(flags, &args, term)?;
        }
    }

    Ok(())
}
