use std::sync::atomic::AtomicU8;

use crate::{netmap::{PkthdrTrait, Rings}, types::SocketOptions};
use crate::circular_queue::CircularQueue;

pub const RING_SLOT_FREE: u8 = 0;
pub const RING_SLOT_IN_USE: u8 = 1;
pub const RING_SLOT_IN_FLIGHT: u8 = 2;

pub struct RingSlot<P: PkthdrTrait> {
    pub status: AtomicU8,
    pub packet_header: P,
    pub id: usize,
    pub len: usize,
    pub packet: Box<[u8]>,
}

impl<P: PkthdrTrait> RingSlot<P> {
    pub fn packet_header(&self) -> &P {
        &self.packet_header
    }
}

impl<P: PkthdrTrait> RingSlot<P> {
    pub fn is_free(&self) -> bool {
        self.status.load(std::sync::atomic::Ordering::Relaxed) == RING_SLOT_FREE
    }
}





//struct NethunsRing(SharedRb<>)
pub enum Filter<P: PkthdrTrait> {
    Closure(Box<dyn Fn(&P, &[u8]) -> bool>),
    Function(fn(&P, &[u8]) -> bool),
}

pub struct TxOnly;
pub struct RxOnly;
pub struct RxTx;

pub trait Flavor {}

impl Flavor for TxOnly {}
impl Flavor for RxOnly {}
impl Flavor for RxTx {}


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
    rings: CircularQueue<RingSlot<P>>,
   _phantom: std::marker::PhantomData<P>,
}


impl<P: PkthdrTrait> NethunsRing<P> {
    fn new(num_items: usize, packet_size: usize) -> Self {
        NethunsRing {
            pktsize: packet_size,
            rings: CircularQueue::new(num_items),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn pop(&self) -> Option<RingSlot<P>> {
        self.rings.pop()
    }

    pub fn push(&self, slot: RingSlot<P>) -> Result<(), RingSlot<P>> {
        self.rings.push(slot)
    }


    pub fn peek_tail_slot(&self) -> Option<&RingSlot<P>> {
        self.rings.peek_tail()
    }

    pub fn peek_head_slot(&self) -> Option<&RingSlot<P>> {
        self.rings.peek_head()
    }

    // TODO: we should enforce exclusive access to the slot


    //fn get_slot(&self, index: usize) -> Option<&RingSlot<P>> {

    //    //self.rings.

    //}
}
