use std::cell::Cell;
use std::ffi::CString;
use std::rc::Rc;

use anyhow::{Result, bail};
use netmap_sys::bindings::netmap_ring;
use netmap_sys::bindings::netmap_slot;
use netmap_sys::bindings::nmport_d;
use netmap_sys::bindings::{netmap_if, nmport_close, nmport_open};
use nix::libc;
use nix::sys::time::TimeVal;

pub struct InnerPort {
    pub inner: *mut nmport_d,
}

impl InnerPort {
    pub fn open(portspec: &str) -> Result<Self> {
        // TODO: portspec should be owned?
        let portspec = CString::new(portspec)?;
        let nmport_d = unsafe { nmport_open(portspec.as_ptr()) };
        if nmport_d.is_null() {
            bail!("Failed to prepare port");
        } else {
            Ok(Self { inner: nmport_d })
        }
    }

    pub fn first_tx_ring(&self) -> u16 {
        unsafe { (*self.inner).first_tx_ring }
    }

    pub fn last_tx_ring(&self) -> u16 {
        unsafe { (*self.inner).last_tx_ring }
    }

    pub fn first_rx_ring(&self) -> u16 {
        unsafe { (*self.inner).first_rx_ring }
    }

    pub fn last_rx_ring(&self) -> u16 {
        unsafe { (*self.inner).last_rx_ring }
    }

    pub fn rx_sync(&self) {
        let null: *mut libc::c_void = std::ptr::null_mut();
        if unsafe { libc::ioctl((*self.inner).fd, netmap_sys::constants::NIOCRXSYNC, null) } < 0 {
            panic!("Failed to sync RX");
        }
    }

    pub fn tx_sync(&self) {
        let null: *mut libc::c_void = std::ptr::null_mut();
        if unsafe { libc::ioctl((*self.inner).fd, netmap_sys::constants::NIOCTXSYNC, null) } < 0 {
            panic!("Failed to sync TX");
        }
    }
}

impl Drop for InnerPort {
    fn drop(&mut self) {
        unsafe { nmport_close(self.inner) };
    }
}

#[derive(Clone)]
struct Port(Rc<InnerPort>);

// impl Deref for Port {
//     type Target = InnerPort;
//
//     fn deref(&self) -> &Self::Target {
//         &*self.0
//     }
// }

impl Port {
    pub fn chan(&self) -> (Transmitter<'_>, Receiver<'_>) {
        let tx = Transmitter::new(self.clone());
        let rx = Receiver::new(self.clone());
        (tx, rx)
    }

    fn interface(&self) -> Interface {
        unsafe {
            Interface(Rc::new(InnerInterface {
                inner: (*self.0.inner).nifp,
                port: self.clone(),
            }))
        }
    }
    pub fn tx_buf(&self, ring_idx: usize, buf_idx: usize) -> Option<&[Cell<u8>]> {
        let interface = self.interface();
        let mut ring = interface.tx_ring_at(ring_idx)?;
        let size = unsafe { (*ring.0.inner).nr_buf_size };
        let tmp: *const u8 = unsafe { InnerRing::buffer(&raw const *ring.0, buf_idx) };
        let tmp: *const Cell<u8> = unsafe { std::mem::transmute(tmp) };
        Some(unsafe { std::slice::from_raw_parts(tmp, size as usize) })
    }

    pub fn rx_buf(&self, ring_idx: usize, buf_idx: usize) -> Option<&[u8]> {
        let interface = self.interface();
        let mut ring = interface.rx_ring_at(ring_idx)?;
        let size = unsafe { (*ring.0.inner).nr_buf_size };
        let tmp = unsafe { InnerRing::buffer(&raw const *ring.0, buf_idx) };
        Some(unsafe { std::slice::from_raw_parts(tmp, size as usize) })
    }
}

struct InnerInterface {
    inner: *mut netmap_if,
    port: Port,
}

impl InnerInterface {
    fn ni_rx_rings(&self) -> u32 {
        unsafe { (*self.inner).ni_rx_rings }
    }

    fn ni_tx_rings(&self) -> u32 {
        unsafe { (*self.inner).ni_tx_rings }
    }

    // this should be invariant over the lifetime of the interface
    fn tx_rings_offsets(&self) -> &[isize] {
        unsafe {
            core::slice::from_raw_parts(
                (*self.inner).ring_ofs.as_ptr(),
                self.ni_tx_rings() as usize,
            )
        }
    }

    fn ni_host_tx_rings(&self) -> u32 {
        unsafe { (*self.inner).ni_host_tx_rings }
    }

    // this should be invariant over the lifetime of the interface
    fn rx_rings_offsets(&self) -> &[isize] {
        unsafe {
            core::slice::from_raw_parts(
                (*self.inner)
                    .ring_ofs
                    .as_ptr()
                    .add(self.ni_tx_rings() as usize)
                    .add(self.ni_host_tx_rings() as usize),
                self.ni_rx_rings() as usize,
            )
        }
    }

    unsafe fn raw_rx_ring_at(&self, index: usize) -> *const netmap_ring {
        let tmp = self.rx_rings_offsets();
        if index >= tmp.len() {
            return std::ptr::null();
        }
        let offset = tmp[index];
        let ptr = self.inner as *const i8;
        let ptr = unsafe { ptr.add(offset as _) };
        ptr as *const _ as *const netmap_ring
    }

    unsafe fn raw_tx_ring_at(&self, index: usize) -> *const netmap_ring {
        let offset = self.tx_rings_offsets().get(index);
        let Some(&offset) = offset else {
            return std::ptr::null();
        };
        let ptr = self.inner as *const u8;
        let ptr = unsafe { ptr.add(offset as _) };
        ptr as *const netmap_ring
    }
}

#[derive(Clone)]
struct Interface(Rc<InnerInterface>);

impl Interface {
    fn rx_ring_at(&self, index: usize) -> Option<Ring> {
        unsafe {
            let ptr = self.0.raw_rx_ring_at(index);
            if ptr.is_null() {
                None
            } else {
                Some(Ring(Rc::new(InnerRing::new(ptr as *mut _, self.clone()))))
            }
        }
    }
    // forse unsafe
    fn tx_ring_at<'iface>(&'iface self, index: usize) -> Option<Ring> {
        unsafe {
            let ptr = self.0.raw_tx_ring_at(index);
            if ptr.is_null() {
                None
            } else {
                Some(Ring(Rc::new(InnerRing::new(ptr as *mut _, self.clone()))))
            }
        }
    }

    fn ni_rx_rings(&self) -> u32 {
        self.0.ni_rx_rings()
    }

    fn ni_tx_rings(&self) -> u32 {
        self.0.ni_tx_rings()
    }

    fn ni_host_tx_rings(&self) -> u32 {
        self.0.ni_host_tx_rings()
    }

    fn rx_rings_offsets(&self) -> &[isize] {
        self.0.rx_rings_offsets()
    }

    fn tx_rings_offsets(&self) -> &[isize] {
        self.0.tx_rings_offsets()
    }
}

// impl Deref for Interface {
//     type Target = InnerInterface;
//
//     fn deref(&self) -> &Self::Target {
//         &*self.0
//     }
// }

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////// #[repr(transparent)]
struct InnerRing {
    pub inner: *mut netmap_ring,
    interface: Interface,
}

impl InnerRing {
    fn new(inner: *mut netmap_ring, interface: Interface) -> Self {
        InnerRing { inner, interface }
    }

    fn iter(&self) -> RingIter<'_> {
        RingIter { ring: self }
    }

    fn num_slots(&self) -> u32 {
        unsafe { (*self.inner).num_slots }
    }

    fn slot_slice(&self) -> &[Slot<'_>] {
        let num_slots = self.num_slots() as usize;
        unsafe {
            let tmp = (*self.inner).slot.as_ptr();
            let tmp = std::mem::transmute(tmp);
            core::slice::from_raw_parts(tmp, num_slots)
        }
    }

    unsafe fn buffer(me: *const Self, index: usize) -> *const u8 {
        let inner = unsafe { (*me).inner };
        unsafe {
            (inner as *const u8)
                .add((*inner).buf_ofs as usize)
                .add(index * (*inner).nr_buf_size as usize)
        }
    }
}

struct Ring(Rc<InnerRing>);

impl Ring {
    pub fn iter(&self) -> RingIter<'_> {
        RingIter { ring: &*self.0 }
    }

    pub fn num_slots(&self) -> u32 {
        self.0.num_slots()
    }

    pub fn slot_slice(&self) -> &[Slot<'_>] {
        self.0.slot_slice()
    }

    pub fn interface(&self) -> Interface {
        self.0.interface.clone()
    }
}

pub struct RingIter<'a> {
    ring: &'a InnerRing,
}

impl<'a> Iterator for RingIter<'a> {
    type Item = (&'a Slot<'a>, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        let ring = unsafe { &*self.ring.inner };
        if ring.head == ring.tail {
            return None;
        }
        let idx = ring.head;
        let next_idx = if idx + 1 == self.ring.num_slots() {
            0
        } else {
            idx + 1
        };

        unsafe {
            (*self.ring.inner).head = next_idx;
            (*self.ring.inner).cur = next_idx;
        }
        let size = ring.nr_buf_size;
        if let Some(tmp) = self.ring.slot_slice().get(idx as usize) {
            let buf_idx = tmp.inner.get().buf_idx;
            let buf = unsafe { InnerRing::buffer(self.ring, buf_idx as usize) };
            let buf = unsafe { std::slice::from_raw_parts(buf, size as usize) };
            let tmp = &raw const *tmp;
            Some((unsafe { &*tmp }, buf))
        } else {
            None
        }
    }
}

#[repr(transparent)]
pub struct Slot<'port> {
    pub inner: Cell<netmap_slot>,
    _phantom: std::marker::PhantomData<&'port ()>,
}

impl<'port> Slot<'port> {
    pub fn update(&self, f: impl FnOnce(&mut SlotData)) {
        let mut slot_data = SlotData {
            buf_idx: self.inner.get().buf_idx,
            len: self.inner.get().len,
            flags: self.inner.get().flags,
            ptr: self.inner.get().ptr,
        };

        f(&mut slot_data);

        self.inner.set(netmap_slot {
            buf_idx: slot_data.buf_idx,
            len: slot_data.len,
            flags: slot_data.flags,
            ptr: slot_data.ptr,
        });
    }
}

pub struct Receiver<'port> {
    pub port: Port,
    _phantom: std::marker::PhantomData<&'port InnerPort>,
}

impl<'port> Receiver<'port> {
    pub(crate) fn new(port: Port) -> Self {
        Receiver {
            port,
            _phantom: std::marker::PhantomData,
        }
    }

    fn first_rx_ring(&self) -> u16 {
        self.port.0.first_rx_ring()
    }

    pub fn last_rx_ring(&self) -> u16 {
        self.port.0.last_rx_ring()
    }

    pub fn iter_mut(&mut self) -> ReceiverIterMut<'_, 'port> {
        let ring_idx = self.first_rx_ring() as usize;
        let last_rx_ring = self.last_rx_ring() as usize;
        ReceiverIterMut {
            receiver: self,
            synced: false,
            last_rx_ring,
            ring_idx,
        }
    }
}
pub struct ReceiverIterMut<'a, 'port> {
    receiver: &'a mut Receiver<'port>,
    synced: bool,
    last_rx_ring: usize,
    ring_idx: usize,
}

pub struct RxBuf<'a, 'port> {
    pub slot: &'a Slot<'port>,
    pub buf: &'a [u8],
    pub ts: TimeVal,
}

impl<'a, 'port> Iterator for ReceiverIterMut<'a, 'port> {
    type Item = RxBuf<'a, 'port>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.synced {
            self.receiver.port.0.rx_sync();
            self.synced = true;
        }

        let interface = self.receiver.port.interface();
        loop {
            let ring = interface.rx_ring_at(self.ring_idx)?;
            let ts = unsafe { (*ring.0.inner).ts };
            let mut ring = ring.iter();
            if let Some((slot, buf)) = ring.next() {
                break Some(RxBuf {
                    slot: unsafe { std::mem::transmute(slot) },
                    buf: unsafe { std::mem::transmute(buf) },
                    ts: TimeVal::new(ts.tv_sec, ts.tv_usec),
                });
            } else {
                self.ring_idx += 1;
                if self.ring_idx >= self.last_rx_ring {
                    return None;
                }
            }
        }
    }
}

pub struct Transmitter<'port> {
    pub port: Port,
    _phantom: std::marker::PhantomData<&'port InnerPort>,
}

impl<'port> Transmitter<'port> {
    pub(crate) fn new(port: Port) -> Self {
        Transmitter {
            port,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn first_tx_ring(&self) -> u16 {
        unsafe { (*self.port.0).first_tx_ring() }
    }

    pub fn last_tx_ring(&self) -> u16 {
        unsafe { (*self.port.0).last_tx_ring() }
    }

    pub fn iter_mut(&mut self) -> TransmitterIterMut<'_, 'port> {
        TransmitterIterMut::new(self)
    }

    pub fn send(&mut self, packet: &[u8]) -> Result<()> {
        let mut iter = self.iter_mut();
        let mut slot = iter
            .next()
            .ok_or_else(|| anyhow::anyhow!("No available slots"))?;
        slot.write(packet)?;
        Ok(())
    }

    pub fn flush(&mut self) {
        unsafe {
            (*self.port.0).tx_sync();
        }
    }
}

pub struct TransmitterIterMut<'a, 'port> {
    pub transmitter: &'a Transmitter<'port>,
    synced: bool,
    last_tx_ring: usize,
    ring_idx: usize,
}

impl<'a, 'port> TransmitterIterMut<'a, 'port> {
    fn new(transmitter: &'a Transmitter<'port>) -> Self {
        let last_tx_ring = transmitter.last_tx_ring() as usize;
        let ring_idx = transmitter.first_tx_ring() as usize;
        Self {
            transmitter,
            synced: false,
            last_tx_ring,
            ring_idx,
        }
    }

    pub fn sync(&mut self) {
        unsafe {
            (*self.transmitter.port.0).tx_sync();
        }
    }
}
pub struct TxBuf<'a, 'port> {
    pub slot: &'a Slot<'port>,
    pub buf: &'a [Cell<u8>],
}

impl<'a, 'port> TxBuf<'a, 'port> {
    fn new(slot: &'a Slot<'port>, buf: &'a [Cell<u8>]) -> Self {
        Self { slot, buf }
    }

    pub fn write(&mut self, packet: &[u8]) -> Result<()> {
        if packet.len() > self.buf.len() {
            bail!("Packet too big");
        }
        for i in 0..packet.len() {
            self.buf[i].set(packet[i]);
        }
        self.slot.update(|x| {
            x.len = packet.len() as u16;
        });
        Ok(())
    }
}

impl<'a, 'port> Drop for TxBuf<'a, 'port> {
    fn drop(&mut self) {
        self.slot.update(|x| {
            x.flags = 0;
        });
    }
}

impl<'a, 'port> Iterator for TransmitterIterMut<'a, 'port> {
    type Item = TxBuf<'a, 'port>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.synced {
            self.sync();
            self.synced = true;
        }

        let interface = self.transmitter.port.interface();
        loop {
            let ring = interface.tx_ring_at(self.ring_idx)?;
            let mut ring = ring.iter();
            if let Some((slot, buf)) = ring.next() {
                unsafe {
                    break Some(TxBuf::new(
                        std::mem::transmute(slot),
                        std::mem::transmute(buf),
                    ));
                };
            } else {
                self.ring_idx += 1;
                if self.ring_idx >= self.last_tx_ring {
                    return None;
                }
            }
        }
    }
}

impl<'a, 'port> Drop for TransmitterIterMut<'a, 'port> {
    fn drop(&mut self) {
        self.sync();
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SlotData {
    pub buf_idx: u32,
    pub len: u16,
    pub flags: u16,
    pub ptr: u64,
}
