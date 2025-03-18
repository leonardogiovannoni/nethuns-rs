use anyhow::{Result, bail};
use netmap_sys::{
    NS_BUF_CHANGED, netmap_if, netmap_ring, netmap_slot, nmport_close, nmport_d, nmport_open_desc,
    nmport_prepare,
};
use netmap_sys::{NIOCRXSYNC, NIOCTXSYNC};
use nix::sys::time::TimeVal;
use std::cell::{Cell, UnsafeCell};
use std::collections::HashSet;
use std::ffi::CString;
use std::marker::PhantomData;
use std::ops::RangeInclusive;
use std::sync::{Arc, LazyLock, Mutex};

pub struct BufferPool {
    port: Arc<UnsafeCell<Port>>,
}

unsafe impl Send for BufferPool {}
unsafe impl Sync for BufferPool {}

impl BufferPool {
    // this is unsafe since we are returning a mutable pointer to the buffer
    // # Safety
    // The caller should guarantee that this method is called only once for each buffer
    // for all the duration of the buffer usage
    pub unsafe fn buffer(&self, buf_idx: usize) -> *mut [u8] {
        let port = unsafe { &*self.port.get() };
        let (ptr, size) = unsafe { port.buffer(buf_idx) };
        if ptr.is_null() {
            panic!("out of range");
        }
        let ring = unsafe { std::slice::from_raw_parts_mut(ptr, size) };

        ring
    }
}

pub struct Port {
    pub inner: *mut nmport_d,
    a_ring: *mut netmap_ring,
}

unsafe impl Send for Port {}

impl Drop for Port {
    fn drop(&mut self) {
        // Close the nmport_d when Port drops
        unsafe { nmport_close(self.inner) };
    }
}

static ALLOCATED_PORTS: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

fn allocate_port(s: &str) -> bool {
    let mut allocated_ports = ALLOCATED_PORTS.lock().unwrap();
    if allocated_ports.contains(s) {
        return false;
    }
    allocated_ports.insert(s.to_owned());
    true
}

impl Port {
    pub unsafe fn extra_buffers_indexes(&mut self) -> Vec<u32> {
        let mut scan = unsafe { (*(*self.inner).nifp).ni_bufs_head };
        let mut res = Vec::new();
        while scan != 0 {
            let me = &raw mut *self;
            res.push(scan);
            let ptr = unsafe {
                let a_ring = (*me).a_ring;
                let size = (*a_ring).nr_buf_size;
                let buf_ofs = (*a_ring).buf_ofs;
                (a_ring as *mut u8)
                    .add(buf_ofs as usize)
                    .add(scan as usize * size as usize)
            } as *mut u32;
            scan = unsafe { *ptr };
        }
        // assert!(!res.is_empty());
        unsafe { (*(*self.inner).nifp).ni_bufs_head = 0 };
        res
    }

    // This functions is arguably safe, the assumptions that this code needs to keep
    // in order to be considered safe is to not open the same ports in other processes,
    // if this happens, this might be bad. We can at least enforce this to avoid double open
    // of the same port or open of the same port in multiple threads in the same process
    pub fn open(portspec: &str, n_extrabuf: u32) -> Result<Self> {
        if !allocate_port(portspec) {
            bail!("Port is already allocated");
        }

        let cstr = CString::new(portspec)?;

        let p = unsafe { nmport_prepare(cstr.as_ptr()) };
        if p.is_null() {
            bail!("can't prepare port");
        }

        //println!("FD: {}", unsafe { (*p).fd });

        unsafe { (*p).reg.nr_extra_bufs = n_extrabuf }

        let res = unsafe { nmport_open_desc(p) };
        if res < 0 {
            bail!("can't open descriptor");
        }
       
        if unsafe { (*p).reg.nr_extra_bufs != n_extrabuf } {
            bail!("can't allocate extrabuf");
        }

        if p.is_null() {
            bail!("Failed to prepare port");
        } else {
            let mut rv = Self {
                inner: p,
                a_ring: std::ptr::null_mut(),
            };
            let a_ring = rv
                .rx_rings_range()
                .map(|i| rv.rx_ring_at(i).unwrap())
                .chain(rv.tx_rings_range().map(|i| rv.tx_ring_at(i).unwrap()))
                .next()
                .ok_or(anyhow::anyhow!("No ring found"))?;
            rv.a_ring = a_ring.inner;
            Ok(rv)
        }
    }

    fn rx_rings_range(&self) -> RangeInclusive<usize> {
        let start = self.first_rx_ring() as usize;
        let last = self.last_rx_ring() as usize;
        start..=last
    }

    fn tx_rings_range(&self) -> RangeInclusive<usize> {
        let start = self.first_tx_ring() as usize;
        let last = self.last_tx_ring() as usize;
        start..=last
    }

    pub fn split(self) -> (Transmitter, Receiver, BufferPool) {
        let rc = Arc::new(UnsafeCell::new(self));
        let tx = Transmitter::new(rc.clone());
        let rx = Receiver::new(rc.clone());

        let bp = BufferPool { port: rc.clone() };

        (tx, rx, bp)
    }

    fn extra_bufs(&self) -> usize {
        unsafe { (*self.inner).reg.nr_extra_bufs as usize }
    }

    fn nifp(&self) -> *mut netmap_if {
        unsafe { (*self.inner).nifp }
    }

    fn ni_rx_rings(&self) -> u32 {
        unsafe { (*self.nifp()).ni_rx_rings }
    }

    fn ni_tx_rings(&self) -> u32 {
        unsafe { (*self.nifp()).ni_tx_rings }
    }

    fn ni_host_tx_rings(&self) -> u32 {
        unsafe { (*self.nifp()).ni_host_tx_rings }
    }

    fn tx_rings_offsets(&self) -> &[isize] {
        unsafe {
            let ptr = (*self.nifp()).ring_ofs.as_ptr();
            let len = self.ni_tx_rings() as usize;
            std::slice::from_raw_parts(ptr, len)
        }
    }

    fn rx_rings_offsets(&self) -> &[isize] {
        unsafe {
            let tx_count = self.ni_tx_rings() as usize;
            let host_tx_count = self.ni_host_tx_rings() as usize;
            let ptr = (*self.nifp())
                .ring_ofs
                .as_ptr()
                .add(tx_count + host_tx_count);
            let len = self.ni_rx_rings() as usize;
            std::slice::from_raw_parts(ptr, len)
        }
    }

    unsafe fn raw_tx_ring_at(&self, index: usize) -> *mut netmap_ring {
        let offsets = self.tx_rings_offsets();
        if index >= offsets.len() {
            return std::ptr::null_mut();
        }
        let offset = offsets[index];
        let base = self.nifp() as *mut u8;
        unsafe { base.add(offset as usize) as *mut netmap_ring }
    }

    unsafe fn raw_rx_ring_at(&self, index: usize) -> *mut netmap_ring {
        let offsets = self.rx_rings_offsets();
        if index >= offsets.len() {
            return std::ptr::null_mut();
        }
        let offset = offsets[index];
        let base = self.nifp() as *mut i8;
        unsafe { base.add(offset as usize) as *mut netmap_ring }
    }

    fn tx_ring_at(&self, index: usize) -> Option<RawRing> {
        unsafe {
            let ptr = self.raw_tx_ring_at(index);
            if ptr.is_null() {
                None
            } else {
                Some(RawRing { inner: ptr, index })
            }
        }
    }

    fn rx_ring_at(&self, index: usize) -> Option<RawRing> {
        unsafe {
            let ptr = self.raw_rx_ring_at(index);
            if ptr.is_null() {
                None
            } else {
                Some(RawRing { inner: ptr, index })
            }
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

    pub fn tx_sync(&self) {
        let null: *mut libc::c_void = std::ptr::null_mut();
        if unsafe { libc::ioctl((*self.inner).fd, NIOCTXSYNC as _, null) } < 0 {
            panic!("Failed to sync TX");
        }
    }

    pub fn rx_sync(&self) {
        let null: *mut libc::c_void = std::ptr::null_mut();
        if unsafe { libc::ioctl((*self.inner).fd, NIOCRXSYNC as _, null) } < 0 {
            panic!("Failed to sync RX");
        }
    }

    unsafe fn buffer(&self, index: usize) -> (*mut u8, usize) {
        let a_ring = self.a_ring;
        let size = unsafe { (*a_ring).nr_buf_size };
        let buf_ofs = unsafe { (*a_ring).buf_ofs };
        let ptr = unsafe {
            (a_ring as *const u8)
                .add(buf_ofs as usize)
                .add(index * size as usize)
        };
        // unsafe { Some(std::slice::from_raw_parts(ptr, size as usize)) }
        (ptr as *mut u8, size as usize)
    }
}

// ---------------------------------------------------------------------
// RawRing & Slot
// ---------------------------------------------------------------------

//#[repr(transparent)]
pub struct RawRing {
    pub inner: *mut netmap_ring,
    index: usize,
}

impl RawRing {
    // pub fn iter_mut(&mut self) -> RingIterMut<'_> {
    //     RingIterMut { ring: self }
    // }

    fn iter_mut(&mut self) -> RingIterMut<'_> {
        RingIterMut {
            ring: self,
            _phantom: PhantomData,
        }
    }

    fn num_slots(&self) -> u32 {
        unsafe { (*self.inner).num_slots }
    }

    fn slot_slice(&self) -> &[RawSlot] {
        let num_slots = self.num_slots() as usize;
        unsafe {
            let ptr = (*self.inner).slot.as_ptr();
            // Turn netmap_slot into our wrapper `Slot`
            std::mem::transmute(std::slice::from_raw_parts(ptr, num_slots))
        }
    }
}

struct RingIterMut<'a> {
    ring: *mut RawRing,
    _phantom: PhantomData<&'a mut RawRing>,
}

impl<'a> Iterator for RingIterMut<'a> {
    type Item = Slot<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let ring = unsafe { (*self.ring).inner };

        unsafe {
            if (*ring).head == (*ring).tail {
                return None;
            }
        }
        let idx = unsafe { (*ring).head };
        let next_idx = unsafe {
            if idx + 1 == (*ring).num_slots {
                0
            } else {
                idx + 1
            }
        };

        unsafe {
            (*ring).head = next_idx;
            (*ring).cur = next_idx;            
        }

        if let Some(slot) = unsafe { (*self.ring).slot_slice().get(idx as usize) } {
            // delegate to drop of Slot the updating of the head
            let slot = Slot {
                raw: unsafe { std::mem::transmute::<&RawSlot, &RawSlot>(slot) },
                ring,
                next_idx,
            };
            Some(slot)
        } else {
            None
        }
    }
}

#[repr(transparent)]
struct RawSlot {
    pub inner: Cell<netmap_slot>,
}

pub struct Slot<'a> {
    raw: &'a RawSlot,
    ring: *mut netmap_ring,
    next_idx: u32,
}

// impl<'a> Drop for Slot<'a> {
//     fn drop(&mut self) {
//         let ring = self.ring;
//         let next_idx = self.next_idx;
//         unsafe {
//             (*ring).head = next_idx;
//             (*ring).cur = next_idx;
//         }
//     }
// }

impl<'a> Slot<'a> {
    pub unsafe fn update_buffer(&self, f: impl FnOnce(&mut u32)) {
        unsafe {
            self.raw.update_buffer(f);
        }
    }

    pub unsafe fn update(&self, f: impl FnOnce(&mut SlotData)) {
        unsafe { self.raw.update(f) };
    }

    pub fn buf_idx(&self) -> u32 {
        self.raw.buf_idx()
    }
    pub fn len(&self) -> u16 {
        self.raw.len()
    }
    pub fn flags(&self) -> u16 {
        self.raw.flags()
    }
    pub fn ptr(&self) -> u64 {
        self.raw.ptr()
    }
}

impl RawSlot {
    pub unsafe fn update_buffer(&self, f: impl FnOnce(&mut u32)) {
        let mut slot_data = SlotData {
            buf_idx: self.inner.get().buf_idx,
            len: self.inner.get().len,
            flags: self.inner.get().flags,
            ptr: self.inner.get().ptr,
        };

        f(&mut slot_data.buf_idx);
        slot_data.flags |= netmap_sys::NS_BUF_CHANGED as u16;

        self.inner.set(netmap_slot {
            buf_idx: slot_data.buf_idx,
            len: slot_data.len,
            flags: slot_data.flags,
            ptr: slot_data.ptr,
        });
    }

    pub unsafe fn update(&self, f: impl FnOnce(&mut SlotData)) {
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

    pub fn buf_idx(&self) -> u32 {
        self.inner.get().buf_idx
    }
    pub fn len(&self) -> u16 {
        self.inner.get().len
    }
    pub fn flags(&self) -> u16 {
        self.inner.get().flags
    }
    pub fn ptr(&self) -> u64 {
        self.inner.get().ptr
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SlotData {
    pub buf_idx: u32,
    pub len: u16,
    pub flags: u16,
    pub ptr: u64,
}

pub struct Receiver {
    port: Arc<UnsafeCell<Port>>,
    synced: bool,
    last_rx_ring: usize,
    ring_idx: usize,
}

unsafe impl Send for Receiver {}

impl Receiver {
    pub fn new(port: Arc<UnsafeCell<Port>>) -> Self {
        let p = unsafe { &*port.get() };
        let ring_idx = p.first_rx_ring() as usize;
        let last_rx_ring = p.last_rx_ring() as usize;
        Self {
            port,
            synced: false,
            last_rx_ring,
            ring_idx,
        }
    }

    pub fn iter_mut(&mut self) -> ReceiverIterMut<'_> {
        ReceiverIterMut { rx: self }
    }

    // # Safety
    // Caller should guarantee that no slots are in use when calling this method
    pub unsafe fn reset(&mut self) {
        self.ring_idx = unsafe { &*self.port.get() }.first_rx_ring() as usize;
        unsafe { self.sync(); }
    }

    // # Safety
    // Caller should guarantee that no slots are in use when calling this method
    pub unsafe fn sync(&mut self) {
        let p = unsafe { &*self.port.get() };
        p.rx_sync();
    }
}

// impl<'a> Drop for ReceiverIterMut<'a> {
//     fn drop(&mut self) {
//     // impl<'a> Drop for Slot<'a> {
//     //     fn drop(&mut self) {
//     //         let ring = self.ring;
//     //         let next_idx = self.next_idx;
//     //         unsafe {
//     //             (*ring).head = next_idx;
//     //             (*ring).cur = next_idx;
//     //         }
//     //     }
//     // } 
//     //unsafe {
//     //    (*self.rx.ring.inner).head = (*self.ring.inner).cur;
//     //}
//     }
// }

pub struct RxBuf<'a> {
    pub slot: Slot<'a>,
    pub ring_idx: u16,
    pub ts: TimeVal,
}

impl<'a> RxBuf<'a> {
    /// # Safety
    /// Caller must ensure `id` is valid and not in use by other slots.
    pub unsafe fn set_buffer_id(&self, id: usize) {
        unsafe {
            self.slot.update(|data| {
                data.buf_idx = id as u32;
                data.flags |= NS_BUF_CHANGED as u16;
            });
        }
    }
}

pub struct ReceiverIterMut<'a> {
    rx: &'a mut Receiver,
}

impl<'a> ReceiverIterMut<'a> {
    pub fn sync(&mut self) {
        let p = unsafe { &*self.rx.port.get() };
        p.rx_sync();
    }
}

impl<'a> Iterator for ReceiverIterMut<'a> {
    type Item = RxBuf<'a>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        // First time, do an RX sync

        if !self.rx.synced {
            let p = unsafe { &*self.rx.port.get() };
            p.rx_sync();
            self.rx.synced = true;
        }
        loop {
            let p = unsafe { &*self.rx.port.get() };
            let mut ring = p.rx_ring_at(self.rx.ring_idx)?;
            // get timestamp from netmap ring
            let ts = unsafe { (*ring.inner).ts };
            let mut ring_iter = ring.iter_mut();
            if let Some(slot) = ring_iter.next() {
                // let raw_slot_trans = unsafe { std::mem::transmute(slot) };
                // let slot = Slot { raw: raw_slot_trans };
                let slot = unsafe { std::mem::transmute::<Slot, Slot>(slot) };
                //let buf_trans = unsafe { std::mem::transmute(buf) };
                break Some(RxBuf {
                    slot,
                    ring_idx: self.rx.ring_idx as u16,
                    ts: TimeVal::new(ts.tv_sec, ts.tv_usec),
                });
            } else {
                self.rx.ring_idx += 1;
                if self.rx.ring_idx > self.rx.last_rx_ring {
                    return None;
                }
            }
        }
    }
}

pub struct TransmitterIterMut<'a> {
    tx: &'a mut Transmitter,
}

pub struct TxBuf<'a> {
    pub slot: Slot<'a>,
    pub ring_idx: u16,
}

impl<'a> Drop for TxBuf<'a> {
    fn drop(&mut self) {
        unsafe {
            self.slot.update(|data| {
                data.flags = 0;
            });
        }
    }
}

impl<'a> TransmitterIterMut<'a> {
    pub fn sync(&mut self) {
        let p = unsafe { &*self.tx.port.get() };
        p.tx_sync();
    }
}

impl<'a> Iterator for TransmitterIterMut<'a> {
    type Item = TxBuf<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // On first call, do a TX sync
        if !self.tx.synced {
            let p = unsafe { &*self.tx.port.get() };
            p.tx_sync();
            self.tx.synced = true;
        }

        loop {
            let p = unsafe { &*self.tx.port.get() };
            let mut ring = p.tx_ring_at(self.tx.ring_idx)?;
            let mut ring_iter = ring.iter_mut();
            if let Some(slot) = ring_iter.next() {
                let slot = unsafe { std::mem::transmute::<Slot, Slot>(slot) };
                break Some(TxBuf {
                    slot,
                    ring_idx: self.tx.ring_idx as u16,
                });
            } else {
                self.tx.ring_idx += 1;
                if self.tx.ring_idx > self.tx.last_tx_ring {
                    return None;
                }
            }
        }
    }
}

pub struct Transmitter {
    port: Arc<UnsafeCell<Port>>,
    synced: bool,
    last_tx_ring: usize,
    ring_idx: usize,
}

unsafe impl Send for Transmitter {}

impl Transmitter {
    pub fn iter_mut(&mut self) -> TransmitterIterMut<'_> {
        TransmitterIterMut { tx: self }
    }
    fn new(port: Arc<UnsafeCell<Port>>) -> Self {
        let p = unsafe { &*port.get() };
        let ring_idx = p.first_tx_ring() as usize;
        let last_tx_ring = p.last_tx_ring() as usize;
        Transmitter {
            port,
            synced: false,
            last_tx_ring,
            ring_idx,
        }
    }

    // # Safety
    // Caller should guarantee that no slots are in use when calling this method
    pub unsafe fn reset(&mut self) {
        self.ring_idx = unsafe { &*self.port.get() }.first_tx_ring() as usize;
        unsafe { self.sync(); }
    }

    // # Safety
    // Caller should guarantee that no slots are in use when calling this method
    pub unsafe fn sync(&mut self) {
        let p = unsafe { &*self.port.get() };
        p.tx_sync();
    }
}
