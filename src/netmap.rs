use anyhow::Result;

use crate::types::SocketOptions;

pub trait PkthdrTrait {

}
pub struct NetmapPkthdr {
    ts: !,
    len: u32,
    caplen: u32,
    buf_idx: u32,
}

impl PkthdrTrait for NetmapPkthdr {
}

pub trait Socket: Sized {
    type Pkthdr: PkthdrTrait;
    fn create(opts: SocketOptions) -> Result<Self>;

}

pub struct NetmapSocket {

}

impl Socket for NetmapSocket {
    type Pkthdr = NetmapPkthdr;
    fn create(opts: SocketOptions) -> Result<Self> {
        Ok(NetmapSocket {})
    }


}
