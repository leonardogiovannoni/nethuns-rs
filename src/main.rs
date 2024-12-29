#![feature(never_type)]
mod netmap;
mod socket;
mod types;
mod circular_queue;
use anyhow::Result;
use netmap::{Socket, NetmapPkthdr, NetmapSocket, PkthdrTrait};
use parking_lot::Mutex;
use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use socket::InnerSocket;

pub struct NethunsNetInfo {
    pub promisc_refcnt: i32,
    pub xdp_prog_refcnt: i32,
    pub xdp_prog_id: u32,
}

static NETINFO_MAP: LazyLock<Mutex<HashMap<Cow<'static, str>, Arc<NethunsNetInfo>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// init can be avoided, since
// - we will use traits to enforce correctness of sizes
// - NETINFO_MAP initialization is lazy


fn lookup_netinfo(dev: &str) -> Option<Arc<NethunsNetInfo>> {
    NETINFO_MAP.lock().get(dev).cloned()
}


fn create_netinfo(s: &str) -> Arc<NethunsNetInfo> {
    let net_info = Arc::new(NethunsNetInfo {
        promisc_refcnt: 0,
        xdp_prog_refcnt: 0,
        xdp_prog_id: 0,
    });

    NETINFO_MAP
        .lock()
        .insert(Cow::Owned(s.to_string()), Arc::clone(&net_info));
    net_info
}

fn main() {
    println!("Hello, world!");
}
