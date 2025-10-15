use crate::af_xdp::{resultify, UmemArea, RX_BATCH_SIZE};
use arrayvec::ArrayVec;
use aya::maps::Map;
use aya::programs::xdp::XdpLinkId;
use aya::programs::{Xdp, XdpFlags};
use aya::{include_bytes_aligned, Ebpf};
use libxdp_sys::{
    xdp_desc, xsk_prod_nb_free, xsk_ring_cons, xsk_ring_cons__comp_addr, xsk_ring_cons__peek,
    xsk_ring_cons__release, xsk_ring_cons__rx_desc, xsk_ring_prod, xsk_ring_prod__fill_addr,
    xsk_ring_prod__reserve, xsk_ring_prod__submit, xsk_ring_prod__tx_desc, xsk_socket,
    xsk_socket__create, xsk_socket__delete, xsk_socket__fd, xsk_socket__update_xskmap,
    xsk_socket_config, xsk_umem, xsk_umem__create, xsk_umem__delete,
    XSK_LIBBPF_FLAGS__INHIBIT_PROG_LOAD,
};
use std::io;
use std::os::fd::{AsFd, AsRawFd};
use std::ptr::NonNull;
use std::{
    collections::VecDeque,
    ffi::CString,
    mem::zeroed,
    ptr,
};


pub struct Umem {
    inner: NonNull<xsk_umem>,
    fq: xsk_ring_prod,
    cq: xsk_ring_cons,
}

unsafe impl Send for Umem {}

pub struct FqMut<'umem> {
    inner: *mut xsk_ring_prod,
    _umem: &'umem mut Umem,
}

impl<'umem> FqMut<'umem> {
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
    _umem: &'umem mut Umem,
}

impl<'umem> CqMut<'umem> {
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
    pub fn new(umem: UmemArea) -> io::Result<Umem> {
        let mut xsk_umem = ptr::null_mut();
        let mut fq = unsafe { zeroed() };
        let mut cq = unsafe { zeroed() };
        let (buffer, size) = umem.raw_parts();
        resultify(unsafe {
            xsk_umem__create(
                &mut xsk_umem,
                buffer.as_ptr() as *mut _,
                size as u64,
                &mut fq,
                &mut cq,
                ptr::null_mut(),
            )
        })?;
        let xsk_umem = NonNull::new(xsk_umem).expect("Failed to create xsk_umem");
        Ok(Umem {
            inner: xsk_umem,
            fq,
            cq,
        })
    }

    pub fn ring_prod_mut(&mut self) -> FqMut<'_> {
        FqMut {
            inner: &mut self.fq,
            _umem: self,
        }
    }

    pub fn ring_cons_mut(&mut self) -> CqMut<'_> {
        CqMut {
            inner: &mut self.cq,
            _umem: self,
        }
    }
}

impl Drop for Umem {
    fn drop(&mut self) {
        unsafe {
            xsk_umem__delete(self.inner.as_ptr());
        }
    }
}

pub struct XdpDescData {
    pub offset: u64,
    pub len: u32,
    pub options: u32,
}

pub struct XskSocket {
    inner: NonNull<xsk_socket>,
    rx: RxRing,
    tx: TxRing,
    _link: XdpLinkId,
    _bpf: Ebpf,
}

unsafe impl Send for XskSocket {}


static DEFAULT_PROG: &[u8] = include_bytes_aligned!("../../prog.o");

impl XskSocket {
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn create(
        umem: &mut Umem,
        ifname: &str,
        queue_id: u32,
        xdp_flags: u32,
        bind_flags: u16,
        rx_size: u32,
        tx_size: u32,
    ) -> io::Result<Self> {
        let ifn = CString::new(ifname)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid interface name"))?;

        let mut bpf = Ebpf::load(DEFAULT_PROG)
            .map_err(|e| io::Error::other(format!("Failed to load BPF object: {e}")))?;

        let prog: &mut Xdp = bpf
            .program_mut("xdp_sock_prog")
            .expect("xdp_sock_prog not found in BPF object")
            .try_into()
            .expect("xdp_sock_prog is not an Xdp program");

        prog.load()
            .map_err(|e| io::Error::other(format!("Failed to load XDP program: {e}")))?;

        let link_id = prog
            .attach(ifname, XdpFlags::from_bits_truncate(xdp_flags))
            .map_err(|e| io::Error::other(format!("Failed to attach XDP program: {e}")))?;

        let xsks_map = bpf
            .map_mut("xsks_map")
            .expect("xsks_map not found in BPF object");
        let Map::XskMap(xsks_map) = xsks_map else {
            panic!("xsks_map is not an XskMap");
        };
        let mut xsk_cfg: xsk_socket_config = unsafe { std::mem::zeroed() };
        xsk_cfg.rx_size = rx_size;
        xsk_cfg.tx_size = tx_size;
        xsk_cfg.xdp_flags = xdp_flags;
        xsk_cfg.bind_flags = bind_flags;
        xsk_cfg.__bindgen_anon_1.libbpf_flags = XSK_LIBBPF_FLAGS__INHIBIT_PROG_LOAD;
        let mut xsk = std::ptr::null_mut();
        let xsk_if_queue = queue_id;
        let mut rx = unsafe { core::mem::zeroed() };
        let mut tx = unsafe { core::mem::zeroed() };

        resultify(unsafe {
            xsk_socket__create(
                &mut xsk,
                ifn.as_ptr(),
                xsk_if_queue,
                umem.inner.as_ptr(),
                &mut rx,
                &mut tx,
                &xsk_cfg,
            )
        })?;

        let xsk = NonNull::new(xsk).expect("Failed to create xsk_socket");
        let xsk_map_fd = xsks_map.fd().as_fd().as_raw_fd();
        resultify(unsafe { xsk_socket__update_xskmap(xsk.as_ptr(), xsk_map_fd) })?;
        Ok(XskSocket {
            rx: RxRing::new(rx),
            tx: TxRing {
                tx,
                cached: VecDeque::with_capacity(RX_BATCH_SIZE),
                to_flush: 0,
            },
            inner: xsk,
            _link: link_id,
            _bpf: bpf,
        })
    }

    pub fn rx_mut(&mut self) -> &mut RxRing {
        &mut self.rx
    }

    pub fn tx_mut(&mut self) -> &mut TxRing {
        &mut self.tx
    }

    pub fn fd(&self) -> i32 {
        unsafe { xsk_socket__fd(self.inner.as_ptr()) }
    }
}

impl Drop for XskSocket {
    fn drop(&mut self) {
        unsafe {
            xsk_socket__delete(self.inner.as_ptr());
        }
    }
}

pub struct RxRing {
    rx: xsk_ring_cons,
    cached: ArrayVec<XdpDescData, { RX_BATCH_SIZE }>,
}

impl RxRing {
    fn new(rx: xsk_ring_cons) -> Self {
        Self {
            rx,
            cached: ArrayVec::new(),
        }
    }

    #[inline(always)]
    fn advance(&mut self) -> Option<XdpDescData> {
        if let Some(desc) = self.cached.pop() {
            Some(desc)
        } else {
            let mut idx_rx: u32 = 0;
            let rcvd =
                unsafe { xsk_ring_cons__peek(&mut self.rx, RX_BATCH_SIZE as u32, &mut idx_rx) };

            // rcvd <= RX_BATCH_SIZE
            if rcvd == 0 {
                return None;
            }
            for i in (idx_rx..(rcvd + idx_rx)).rev() {
                let rx_desc = unsafe { xsk_ring_cons__rx_desc(&self.rx, i) };
                let addr = unsafe { (*rx_desc).addr };
                let len = unsafe { (*rx_desc).len };
                let options = unsafe { (*rx_desc).options };
                // SAFETY: rcvd <= RX_BATCH_SIZE, cached empty at the start
                unsafe {
                    self.cached.push_unchecked(XdpDescData {
                        offset: addr,
                        len,
                        options,
                    });
                }
            }
            unsafe { xsk_ring_cons__release(&mut self.rx, rcvd) };
            // SAFETY: self.cached is guaranteed to have at least one element here
            let rv = unsafe { self.cached.pop().unwrap_unchecked() };
            Some(rv)
        }
    }
}

impl Iterator for RxRing {
    type Item = XdpDescData;

    #[inline(always)]
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

    #[inline(always)]
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
