use std::sync::atomic::AtomicU8;

use crate::{netmap::PkthdrTrait, types::SocketOptions};
use ringbuf::{storage::Heap, SharedRb};
use circular_buffer::CircularBuffer;


pub struct RingSlot<P: PkthdrTrait> {
    status: AtomicU8,
    packet_header: P,
    id: usize,
    //len: usize,
    packet: [u8],
}


//struct NethunsRing(SharedRb<>)
pub enum Filter<P: PkthdrTrait> {
    Closure(Box<dyn Fn(&P, &[u8]) -> bool>),
    Function(fn(&P, &[u8]) -> bool),
}

pub struct InnerSocket<P: PkthdrTrait> {
    pub opt: SocketOptions,
    // todo : Producer<P>
    //pub tx_ring: Option<NethunsRing<P>>,
    //// todo : Consumer<P>
    //pub rx_ring: Option<NethunsRing<P>>,
    pub available_rings: Rings<P>,
    pub devname: String,
    pub queue: Option<usize>,
    pub ifindex: i32,
    pub filter: Filter<P>,
}




pub struct NethunsRing<P: PkthdrTrait> {
    pktsize: usize,
    // rings: SharedRb<Heap<Box<RingSlot<P>>>>,
   ////// rings: CircularBuffer<RingSlot<P>>,
   _phantom: std::marker::PhantomData<P>,
}


impl<P: PkthdrTrait> NethunsRing<P> {
    fn new(num_items: usize, packet_size: usize) -> Self {
        NethunsRing {
            pktsize: packet_size,
            //rings: SharedRb::new(num_items),
            _phantom: std::marker::PhantomData,
        }
    }

    //fn get_slot(&self, index: usize) -> Option<&RingSlot<P>> {

    //    //self.rings.

    //}
}
