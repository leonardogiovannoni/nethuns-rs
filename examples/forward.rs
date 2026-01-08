use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread;
use std::time::Duration;

// Import the framework APIs.
use nethuns_rs::api::{Flags, Socket};
#[cfg(feature = "af_xdp")]
use nethuns_rs::af_xdp;
#[cfg(feature = "dpdk")]
use nethuns_rs::dpdk;
#[cfg(feature = "netmap")]
use nethuns_rs::netmap;
#[cfg(feature = "pcap")]
use nethuns_rs::pcap;

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
    #[cfg(feature = "netmap")]
    Netmap(NetmapArgs),
    /// Use AF_XDP framework.
    #[cfg(feature = "af_xdp")]
    AfXdp(AfXdpArgs),
    /// Use DPDK framework.
    #[cfg(feature = "dpdk")]
    Dpdk(DpdkArgs),
    /// Use pcap framework.
    #[cfg(feature = "pcap")]
    Pcap(PcapArgs),
}

/// Netmap-specific arguments.
#[derive(Parser, Debug, Clone)]
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

/// AF_XDP-specific arguments.
#[derive(Parser, Debug, Clone)]
#[cfg(feature = "af_xdp")]
struct AfXdpArgs {
    /// Bind flags for AF_XDP.
    #[clap(long, default_value_t = 0)]
    bind_flags: u16,
    /// XDP flags for AF_XDP.
    #[clap(long, default_value_t = 0)]
    xdp_flags: u32,
}

/// DPDK-specific arguments.
#[derive(Parser, Debug, Clone)]
#[cfg(feature = "dpdk")]
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

/// Pcap-specific arguments.
#[derive(Parser, Debug, Clone)]
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

pub fn main() -> Result<()> {
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
        #[cfg(feature = "netmap")]
        Framework::Netmap(netmap_args) => {
            let flags = netmap::NetmapFlags {
                extra_buf: netmap_args.extra_buf,
            };
            run_forwarder::<netmap::Sock>(flags, &args, term)
        }
        #[cfg(feature = "af_xdp")]
        Framework::AfXdp(af_xdp_args) => {
            let flags = af_xdp::AfXdpFlags {
                bind_flags: af_xdp_args.bind_flags,
                xdp_flags: af_xdp_args.xdp_flags,
                num_frames: 4096,
                frame_size: 2048,
                tx_size: 2048,
                rx_size: 2048,
            };
            run_forwarder::<af_xdp::Sock>(flags, &args, term)
        }
        #[cfg(feature = "dpdk")]
        Framework::Dpdk(dpdk_args) => {
            let flags = dpdk::DpdkFlags {
                num_mbufs: dpdk_args.num_mbufs,
                mbuf_cache_size: dpdk_args.mbuf_cache_size,
                mbuf_default_buf_size: dpdk_args.mbuf_default_buf_size as u16,
            };
            run_forwarder::<dpdk::Sock>(flags, &args, term)
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
            run_forwarder::<pcap::Sock>(flags, &args, term)
        }
    }
}

/// The main packet-forwarding routine.
///
/// This function creates an input and an output socket, spawns a meter thread,
/// then enters a loop where it receives a packet on the input interface, and forwards it
/// to the output interface using a retry loop.
fn run_forwarder<Sock>(flags: Sock::Flags, args: &Args, term: Arc<AtomicBool>) -> Result<()>
where
    Sock: Socket + 'static,
{
    println!("Starting packet forwarder:");
    println!("  Input interface: {}", args.in_if);
    println!("  Output interface: {}", args.out_if);

    // Create the input and output sockets using the selected framework.
    let in_socket = Sock::create(&args.in_if, args.queue, flags.clone())?;
    let out_socket = Sock::create(&args.out_if, args.queue, flags.clone())?;

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
