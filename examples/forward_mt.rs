//! Multi-threaded forwarder using ringbuf channel between Rx and Tx threads.
//!
//! One thread receives packets and sends tokens through a ring buffer,
//! while a consumer thread receives tokens, consumes them to access packet data,
//! and forwards them to the output interface.

use anyhow::Result;
use clap::{Parser, Subcommand};
use nethuns_rs::api::{Socket, Token};
use ringbuf::{
    HeapRb,
    traits::{Consumer, Producer, Split},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread;
use std::time::Duration;

#[cfg(feature = "af-xdp")]
use nethuns_rs::af_xdp;
#[cfg(feature = "netmap")]
use nethuns_rs::netmap;
#[cfg(feature = "pcap")]
use nethuns_rs::pcap;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Input interface name.
    in_if: String,
    /// Output interface name.
    out_if: String,
    /// Queue index to bind (defaults to backend choice).
    queue: Option<usize>,
    /// Ring buffer capacity (number of tokens).
    #[clap(long, default_value_t = 1024)]
    ring_size: usize,
    /// Choose the network framework.
    #[clap(subcommand)]
    framework: Framework,
}

#[derive(Subcommand, Debug, Clone)]
enum Framework {
    #[cfg(feature = "netmap")]
    Netmap(NetmapArgs),
    #[cfg(feature = "af-xdp")]
    AfXdp(AfXdpArgs),
    #[cfg(feature = "pcap")]
    Pcap(PcapArgs),
}

#[cfg(feature = "netmap")]
#[derive(Parser, Debug, Clone)]
struct NetmapArgs {
    #[clap(long, default_value_t = 1024)]
    extra_buf: u32,
}

#[cfg(feature = "af-xdp")]
#[derive(Parser, Debug, Clone)]
struct AfXdpArgs {
    #[clap(long, default_value_t = 0)]
    bind_flags: u16,
    #[clap(long, default_value_t = 0)]
    xdp_flags: u32,
}

#[cfg(feature = "pcap")]
#[derive(Parser, Debug, Clone)]
struct PcapArgs {
    #[clap(long, default_value_t = 65535)]
    snaplen: i32,
    #[clap(long, default_value_t = true)]
    promiscuous: bool,
    #[clap(long, default_value_t = 1)]
    timeout_ms: i32,
    #[clap(long, default_value_t = true)]
    immediate: bool,
    #[clap(long)]
    filter: Option<String>,
    #[clap(long, default_value_t = 2048)]
    buffer_size: usize,
    #[clap(long, default_value_t = 32)]
    buffer_count: usize,
}

/// Run the producer/consumer pipeline with ringbuf.
fn run_queue<Sock>(flags: Sock::Flags, args: &Args, term: Arc<AtomicBool>) -> Result<()>
where
    Sock: Socket + 'static,
{
    let total_rcv = Arc::new(AtomicU64::new(0));
    let total_fwd = Arc::new(AtomicU64::new(0));

    // Stats printer thread
    {
        let total_rcv = total_rcv.clone();
        let total_fwd = total_fwd.clone();
        let term = term.clone();
        thread::spawn(move || {
            while !term.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_secs(1));
                let rcv = total_rcv.swap(0, Ordering::SeqCst);
                let fwd = total_fwd.swap(0, Ordering::SeqCst);
                println!("rcv/sec: {}  fwd/sec: {}", rcv, fwd);
            }
        });
    }

    // Create sockets
    let in_socket = Sock::create(&args.in_if, args.queue, flags.clone())?;
    let out_socket = Sock::create(&args.out_if, args.queue, flags)?;

    // Clone context for consumer thread (Context is Clone + Send + 'static)
    let ctx = in_socket.context().clone();

    // Create ringbuf channel for tokens
    let rb = HeapRb::<Token>::new(args.ring_size);
    let (mut producer, mut consumer) = rb.split();

    let total_fwd_consumer = total_fwd.clone();
    let term_consumer = term.clone();

    // Consumer thread: receives tokens, consumes them to get payloads, forwards packets
    let consumer_handle = thread::spawn(move || -> Result<()> {
        while !term_consumer.load(Ordering::SeqCst) {
            // Try to pop a token from the ring
            if let Some(token) = consumer.try_pop() {
                // Consume the token with the context to get the payload
                let payload = token.consume(&ctx);

                // Forward the packet
                if let Err(e) = out_socket.send(&payload) {
                    // eprintln!("Send error: {:?}", e);
                    continue;
                }
                out_socket.flush();

                total_fwd_consumer.fetch_add(1, Ordering::Relaxed);
            } else {
                // No token available, yield to avoid busy spinning
                thread::yield_now();
            }
        }
        Ok(())
    });

    // Producer: receives packets, sends tokens to consumer
    while !term.load(Ordering::SeqCst) {
        let (token, _meta) = match in_socket.recv_token() {
            Ok(t) => t,
            Err(_) => continue,
        };

        total_rcv.fetch_add(1, Ordering::Relaxed);

        // Push token to ring, spin if full
        let mut tok = token;
        loop {
            match producer.try_push(tok) {
                Ok(()) => break,
                Err(t) => {
                    tok = t;
                    thread::yield_now();
                }
            }
        }
    }

    // Wait for consumer to finish
    let _ = consumer_handle.join();

    Ok(())
}

pub fn main() -> Result<()> {
    let args = Args::parse();

    let term = Arc::new(AtomicBool::new(false));
    {
        let term = term.clone();
        ctrlc::set_handler(move || {
            term.store(true, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");
    }

    match &args.framework {
        #[cfg(feature = "netmap")]
        Framework::Netmap(netmap_args) => {
            let flags = netmap::NetmapFlags {
                extra_buf: netmap_args.extra_buf,
            };
            run_queue::<netmap::Sock>(flags, &args, term)?;
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
            run_queue::<af_xdp::Sock>(flags, &args, term)?;
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
            run_queue::<pcap::Sock>(flags, &args, term)?;
        }
    }

    Ok(())
}
