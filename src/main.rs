mod af_xdp;
mod api;
mod dpdk;
mod forward;
mod forward_mt;
mod meter;
mod netmap;
mod strategy;
use std::{io::BufRead, time::Duration};

use anyhow::Result;
use api::{Flags, Socket};
use dpdk::DpdkFlags;
use strategy::{MpscArgs, MpscStrategy};    /*let dpdk_flags = dpdk::DpdkFlags {
        strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
    };
    let flags = api::Flags::DpdkFlags(dpdk_flags);
    let mut sock = dpdk::Sock::<MpscStrategy>::create("veth0", None, flags)?;

    loop {
        let Ok((pkt, _)) = sock.recv() else {
            continue;
        };
        println!("Received packet: {:?}", &*pkt);
        std::thread::sleep(Duration::from_secs(1));
    }
    Ok(())*/
/* 
fn main() -> Result<()> {


    procspawn::init();

    let tmp = procspawn::spawn((), |_| {
        let mut socket1 = dpdk::Sock::<MpscStrategy>::create(
            "veth1",
            None,
            Flags::DpdkFlags(DpdkFlags {
                strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
            }),
        )
        .unwrap();
        println!("Created socket");
        socket1.send(b"Helloworldmyfriend\0\0\0\0\0\0\0").unwrap();
        println!("Sent packet");
        socket1.flush();
        println!("Sent packet");
        std::thread::sleep(Duration::from_secs(1));
    });
    let mut socket0 = dpdk::Sock::<MpscStrategy>::create(
        "veth0",
        None,
        Flags::DpdkFlags(dpdk::DpdkFlags {
            strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
        }),
    )
    .unwrap();

    tmp.join().unwrap();
    
    for _ in 0..100 {
       
        let Ok((packet, meta)) = socket0.recv() else {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        };
        println!("Received packet: {:?}", &*packet);
        assert_eq!(&packet[..20], b"Helloworldmyfriend\0\0");
    }
    Ok(())
}
*/

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <server|client>", args[0]);
        std::process::exit(1);
    }
    let mode = &args[1];

    match mode.as_str() {
        "server" => {
            // Create a socket on "veth0" for receiving
            let mut sock = dpdk::Sock::<MpscStrategy>::create(
                "veth0",
                None,
                Flags::DpdkFlags(dpdk::DpdkFlags {
                    strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
                }),
            )?;
            println!("Server listening on veth0...");

            loop {
                // Try to receive a packet; if none is available, sleep briefly and continue.
                let Ok((packet, _meta)) = sock.recv() else {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                };

                // Attempt to interpret the packet as UTF-8 text for printing
                match std::str::from_utf8(&packet) {
                    Ok(text) => println!("Received: {}", text),
                    Err(_) => println!("Received (non UTF-8): {:?}", &*packet),
                }
            }
        }
        "client" => {
            // Create a socket on "veth1" for sending
            let mut sock = dpdk::Sock::<MpscStrategy>::create(
                "veth1",
                None,
                Flags::DpdkFlags(dpdk::DpdkFlags {
                    strategy_args: api::StrategyArgs::Mpsc(MpscArgs::default()),
                }),
            )?;
            println!("Client running on veth1. Type messages to send.");

            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                let line = line?;
                // Convert the line to bytes (you can append any required null terminators here)
                let data = line.into_bytes();
                sock.send(&data)?;
                sock.flush();
            }
        }
        _ => {
            eprintln!("Invalid mode '{}'. Use 'server' or 'client'.", mode);
            std::process::exit(1);
        }
    }

    Ok(())
}