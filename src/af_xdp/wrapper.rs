use std::{
    collections::VecDeque,
    ffi::{CString, c_void},
    mem::zeroed,
    ptr,
    sync::Arc,
};

use libc::{RLIM_INFINITY, RLIMIT_MEMLOCK, setrlimit};
use libxdp_sys::{
    XSK_LIBBPF_FLAGS__INHIBIT_PROG_LOAD, XSK_RING_CONS__DEFAULT_NUM_DESCS,
    XSK_RING_PROD__DEFAULT_NUM_DESCS, XSK_UMEM__DEFAULT_FRAME_SIZE, bpf_xdp_query_id, xdp_desc,
    xsk_prod_nb_free, xsk_ring_cons, xsk_ring_cons__comp_addr, xsk_ring_cons__peek,
    xsk_ring_cons__release, xsk_ring_cons__rx_desc, xsk_ring_prod, xsk_ring_prod__fill_addr,
    xsk_ring_prod__reserve, xsk_ring_prod__submit, xsk_ring_prod__tx_desc, xsk_socket,
    xsk_socket__create, xsk_socket__fd, xsk_socket__update_xskmap, xsk_socket_config, xsk_umem,
    xsk_umem__create, xsk_umem__delete,
};

use crate::af_xdp::{RX_BATCH_SIZE, UmemArea, resultify};
use anyhow::Result;
pub struct Umem {
    inner: *mut xsk_umem,
    fq: xsk_ring_prod,
    cq: xsk_ring_cons,
}

unsafe impl Send for Umem {}

pub struct FqMut<'umem> {
    inner: *mut xsk_ring_prod,
    umem: &'umem mut Umem,
}

impl<'umem> FqMut<'umem> {
    // pub fn as_raw(&self) -> *mut xsk_ring_prod {
    //     self.inner
    // }

    // xsk_prod_nb_free
    pub fn nb_free(&mut self, nb: u32) -> u32 {
        unsafe { xsk_prod_nb_free(self.inner, nb) }
    }

    pub fn reserve(&mut self, nb: u32) -> (u32, u32) {
        let mut idx = 0;
        let reserved = unsafe { xsk_ring_prod__reserve(self.inner, nb, &mut idx) };
        (reserved, idx)
    }

    pub fn get_addr(&mut self, idx: u32) -> &mut u64 {
        unsafe { &mut *xsk_ring_prod__fill_addr(self.inner, idx) }
    }

    pub fn submit(&mut self, nb: u32) {
        unsafe {
            xsk_ring_prod__submit(self.inner, nb);
        }
    }
}

pub struct CqMut<'umem> {
    inner: *mut xsk_ring_cons,
    umem: &'umem mut Umem,
}

impl<'umem> CqMut<'umem> {
    // pub fn as_raw(&self) -> *mut xsk_ring_cons {
    //     self.inner
    // }

    pub fn peek(&mut self, nb: u32) -> (u32, u32) {
        let mut idx = 0;
        let peeked = unsafe { xsk_ring_cons__peek(self.inner, nb, &mut idx) };
        (peeked, idx)
    }

    pub fn get_addr(&self, idx: u32) -> u64 {
        unsafe { *xsk_ring_cons__comp_addr(self.inner, idx) }
    }

    pub fn release(&mut self, nb: u32) {
        unsafe {
            xsk_ring_cons__release(self.inner, nb);
        }
    }
}

impl Umem {
    pub fn new(umem: UmemArea) -> Result<Umem> {
        let mut xsk_umem = ptr::null_mut();
        let mut fq = unsafe { zeroed() };
        let mut cq = unsafe { zeroed() };
        let (buffer, size) = umem.raw_parts();
        resultify(unsafe {
            xsk_umem__create(
                &mut xsk_umem,
                buffer as *mut c_void,
                size as u64,
                &mut fq,
                &mut cq,
                ptr::null_mut(),
            )
        })?;
        Ok(Umem {
            inner: xsk_umem,
            fq,
            cq,
        })
    }

    pub fn ring_prod_mut(&mut self) -> FqMut<'_> {
        FqMut {
            inner: &mut self.fq,
            umem: self,
        }
    }

    pub fn ring_cons_mut(&mut self) -> CqMut<'_> {
        CqMut {
            inner: &mut self.cq,
            umem: self,
        }
    }

    pub fn as_raw(&self) -> *mut xsk_umem {
        self.inner
    }
}

impl Drop for Umem {
    fn drop(&mut self) {
        unsafe {
            xsk_umem__delete(self.inner);
        }
    }
}

pub struct XdpDescData {
    pub offset: u64,
    pub len: u32,
    pub options: u32,
}

pub struct XskSocket {
    inner: *mut xsk_socket,
    rx: RxRing,
    tx: TxRing,
}

unsafe impl Send for XskSocket {}

fn ifname_to_ifindex(ifname: &str) -> Option<u32> {
    let c_ifname = CString::new(ifname).ok()?;
    let index = unsafe { libc::if_nametoindex(c_ifname.as_ptr()) };
    if index == 0 { None } else { Some(index) }
}

impl XskSocket {

    pub fn as_raw(&self) -> *mut xsk_socket {
        self.inner
    }
    // # Safety
    // Umem should be valid for the lifetime of the XskSocket
    pub unsafe fn create(
        umem: &Umem,
        ifname: &str,
        queue_id: u32,
        xdp_flags: u32,
        bind_flags: u16,
    ) -> Result<Self> {
        let rlim = libc::rlimit {
            rlim_cur: RLIM_INFINITY,
            rlim_max: RLIM_INFINITY,
        };

        resultify(unsafe { setrlimit(RLIMIT_MEMLOCK, &rlim) })?;

        let mut rx = unsafe { zeroed() };
        let mut tx = unsafe { zeroed() };
        // Set up the socket configuration.
        let mut xsk_cfg: xsk_socket_config = unsafe { zeroed() };
        xsk_cfg.rx_size = XSK_RING_CONS__DEFAULT_NUM_DESCS;
        xsk_cfg.tx_size = XSK_RING_PROD__DEFAULT_NUM_DESCS;
        xsk_cfg.xdp_flags = xdp_flags;
        xsk_cfg.bind_flags = bind_flags;
        // Create the xsk socket.
        let mut xsk_ptr = ptr::null_mut();
        let ifname_cstr = CString::new(ifname)?;

        resultify(unsafe {
            xsk_socket__create(
                &mut xsk_ptr,
                ifname_cstr.as_ptr(),
                queue_id,
                umem.as_raw(),
                &mut rx,
                &mut tx,
                &xsk_cfg,
            )
        })?;
        let ifindex = ifname_to_ifindex(ifname).unwrap();
        // If we have a custom XSK program, update the map; otherwise just query the ID.
        unsafe {
            //if CUSTOM_XSK {
            //    resultify(xsk_socket__update_xskmap(xsk_ptr as _, XSK_MAP_FD))?;
            //} else {
            let mut prog_id: u32 = 0;
            resultify(bpf_xdp_query_id(
                ifindex as i32,
                xdp_flags as _,
                &mut prog_id,
            ))?;
            //}
        }

        // Immediately do a one‐time refill of the fill ring, so the kernel has buffers for Rx.
        //umem_manager.refill_fill_ring()?;
        let rx = RxRing::new(rx);
        let tx = TxRing::new(tx, 1024);
        Ok(Self {
            inner: xsk_ptr as _,
            rx,
            tx,
        })
       
    }

    pub fn rx_mut(&mut self) -> &mut RxRing {
        &mut self.rx
    }

    pub fn tx_mut(&mut self) -> &mut TxRing {
        &mut self.tx
    }

    pub fn fd(&self) -> i32 {
        unsafe { xsk_socket__fd(self.inner) }
    }

}


pub struct RxRing {
    rx: xsk_ring_cons,
    cached: VecDeque<XdpDescData>,
}

impl RxRing {
    fn new(rx: xsk_ring_cons) -> Self {
        let v = VecDeque::with_capacity(RX_BATCH_SIZE as usize);
        Self { rx, cached: v }
    }

    fn advance(&mut self) -> Option<XdpDescData> {
        if let Some(desc) = self.cached.pop_front() {
            Some(desc)
        } else {
            let mut idx_rx: u32 = 0;
            let rcvd = unsafe { xsk_ring_cons__peek(&mut self.rx, RX_BATCH_SIZE, &mut idx_rx) };
            if rcvd == 0 {
                return None;
            }
            for _ in 0..rcvd {
                let rx_desc = unsafe { xsk_ring_cons__rx_desc(&mut self.rx, idx_rx) };
                let addr = unsafe { (*rx_desc).addr };
                let len = unsafe { (*rx_desc).len };
                let options = unsafe { (*rx_desc).options };
                self.cached.push_back(XdpDescData {
                    offset: addr,
                    len,
                    options,
                });
                idx_rx += 1;
            }
            unsafe { xsk_ring_cons__release(&mut self.rx, rcvd) };
            Some(self.cached.pop_front().unwrap())
        }
    }
}

impl Iterator for RxRing {
    type Item = XdpDescData;

    fn next(&mut self) -> Option<Self::Item> {
        self.advance()
    }
}

pub struct TxRing {
    tx: xsk_ring_prod,
    cached: VecDeque<*mut xdp_desc>,
    to_flush: u32,
}

impl TxRing {
    fn new(tx: xsk_ring_prod, size: u32) -> Self {
        let v = VecDeque::with_capacity(size as usize);
        Self {
            tx,
            cached: v,
            to_flush: 0,
        }
    }

    pub fn iter(&mut self) -> TxRingIter {
        TxRingIter { ring: self }
    }
}

pub struct TxRingIter<'a> {
    ring: &'a mut TxRing,
}

const TX_BATCH_SIZE: u32 = 64;

pub struct TxSlot<'iter> {
    desc: *mut xdp_desc,
    _phantom: std::marker::PhantomData<&'iter mut TxRingIter<'iter>>,
}

impl<'iter> TxSlot<'iter> {
    pub fn len(&self) -> u32 {
        unsafe { (*self.desc).len }
    }

    pub fn offset(&self) -> u64 {
        unsafe { (*self.desc).addr }
    }

    pub fn options(&self) -> u32 {
        unsafe { (*self.desc).options }
    }

    pub fn len_mut(&mut self) -> &mut u32 {
        unsafe { &mut (*self.desc).len }
    }

    pub fn offset_mut(&mut self) -> &mut u64 {
        unsafe { &mut (*self.desc).addr }
    }

    pub fn options_mut(&mut self) -> &mut u32 {
        unsafe { &mut (*self.desc).options }
    }
}

impl<'a> TxRingIter<'a> {
    pub unsafe fn sync(&mut self) {
        if self.ring.to_flush == 0 {
            return;
        }
        unsafe { xsk_ring_prod__submit(&mut self.ring.tx, self.ring.to_flush) };
        self.ring.to_flush = 0;
    }

    fn advance(&mut self) -> Option<TxSlot<'a>> {
        if let Some(desc) = self.ring.cached.pop_front() {
            self.ring.to_flush += 1;
            Some(TxSlot {
                desc,
                _phantom: std::marker::PhantomData,
            })
        } else {
            unsafe { self.sync() };
            let mut idx_tx: u32 = 0;
            let rcvd =
                unsafe { xsk_ring_prod__reserve(&mut self.ring.tx, TX_BATCH_SIZE, &mut idx_tx) };
            if rcvd == 0 {
                return None;
            }

            for _ in 0..rcvd {
                let tx_desc = unsafe { xsk_ring_prod__tx_desc(&mut self.ring.tx, idx_tx) };
                self.ring.cached.push_back(tx_desc);
                idx_tx += 1;
            }
            let tx_slot = self.ring.cached.pop_front().unwrap();

            self.ring.to_flush += 1;
            Some(TxSlot {
                desc: tx_slot,
                _phantom: std::marker::PhantomData,
            })
        }
    }
}

impl<'a> Iterator for TxRingIter<'a> {
    type Item = TxSlot<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.advance()
    }
}
