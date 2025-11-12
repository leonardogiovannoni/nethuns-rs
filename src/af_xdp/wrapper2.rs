use crate::af_xdp::{RX_BATCH_SIZE, UmemArea, resultify};
use arrayvec::ArrayVec;
use libc::strerror;
use libxdp_sys::{
    bpf_link, bpf_link__destroy, bpf_map__fd, bpf_object__find_map_by_name, bpf_object_open_opts, bpf_program, bpf_program__attach_xdp, libxdp_get_error, xdp_desc, xdp_multiprog, xdp_multiprog__close, xdp_multiprog__detach, xdp_multiprog__get_from_ifindex, xdp_program__attach, xdp_program__bpf_obj, xdp_program__open_file, xdp_program_opts, xsk_prod_nb_free, xsk_ring_cons, xsk_ring_cons__comp_addr, xsk_ring_cons__peek, xsk_ring_cons__release, xsk_ring_cons__rx_desc, xsk_ring_prod, xsk_ring_prod__fill_addr, xsk_ring_prod__reserve, xsk_ring_prod__submit, xsk_ring_prod__tx_desc, xsk_socket, xsk_socket__create, xsk_socket__delete, xsk_socket__fd, xsk_socket__update_xskmap, xsk_socket_config, xsk_umem, xsk_umem__create, xsk_umem__delete, XSK_LIBBPF_FLAGS__INHIBIT_PROG_LOAD, XSK_RING_CONS__DEFAULT_NUM_DESCS, XSK_RING_PROD__DEFAULT_NUM_DESCS, XSK_UMEM__DEFAULT_FRAME_SIZE
};
use std::io;
use std::mem::size_of;
use std::{
    collections::VecDeque,
    ffi::{CString, c_void},
    mem::zeroed,
    ptr,
    str::FromStr,
};

//
// -------------------------------------------------------------------------
// Constants matching the C code
// -------------------------------------------------------------------------
const NUM_FRAMES: usize = 4096;
const FRAME_SIZE: usize = XSK_UMEM__DEFAULT_FRAME_SIZE as usize;
//const RX_BATCH_SIZE: u32      = 64;
const INVALID_UMEM_FRAME: u64 = u64::MAX;

fn xsk_configure_socket(
    umem_info: &mut Umem,
    xsk_map_fd: i32,
    xdp_flags: u32,
    xsk_bind_flags: u16,
    ifname: CString,
    ifindex: i32,
    xsk_if_queue: u32,
) -> io::Result<XskSocket> {
    let mut xsk_cfg: xsk_socket_config = unsafe { std::mem::zeroed() };
    xsk_cfg.rx_size = XSK_RING_CONS__DEFAULT_NUM_DESCS as u32;
    xsk_cfg.tx_size = XSK_RING_CONS__DEFAULT_NUM_DESCS as u32;
    xsk_cfg.xdp_flags = xdp_flags;
    xsk_cfg.bind_flags = xsk_bind_flags;
    xsk_cfg.__bindgen_anon_1.libbpf_flags = XSK_LIBBPF_FLAGS__INHIBIT_PROG_LOAD;
    let mut xsk = std::ptr::null_mut();
    let ifname = ifname.as_ptr();
    let ifindex = ifindex;
    let xsk_if_queue = xsk_if_queue;
    let umem = (*umem_info).inner;
    let mut rx = unsafe { core::mem::zeroed() };
    let mut tx = unsafe { core::mem::zeroed() };
    //let mut prog_id = 0;
    //println!("ifname: {:?}", ifname);
    //println!("ifindex: {:?}", ifindex);
    //println!("xsk_if_queue: {:?}", xsk_if_queue);
    //println!("umem: {:?}", umem);

    let ret = unsafe {
        xsk_socket__create(
            &mut xsk,
            ifname,
            xsk_if_queue,
            umem,
            &mut rx,
            &mut tx,
            &xsk_cfg,
        )
    };
    if ret != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("xsk_socket__create failed: {}", ret),
        ));
    }

    //if custom_xsk.load(std::sync::atomic::Ordering::SeqCst) {
    let ret = unsafe { xsk_socket__update_xskmap(xsk, xsk_map_fd) };
    if ret != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("xsk_socket__update_xskmap failed: {}", ret),
        ));
    }
    //} else {
    //    let ret = unsafe { bpf_xdp_query_id(ifindex, xdp_flags as i32, &mut prog_id) };
    //    if ret != 0 {
    //        return std::ptr::null_mut();
    //    }
    //}
    let mut umem_frame_addr: [u64; NUM_FRAMES] = [INVALID_UMEM_FRAME; NUM_FRAMES];
    for i in 0..NUM_FRAMES {
        umem_frame_addr[i] = i as u64 * FRAME_SIZE as u64;
    }

    let mut idx = 0;
    let ret = unsafe {
        xsk_ring_prod__reserve(
            &mut (*umem_info).fq,
            XSK_RING_PROD__DEFAULT_NUM_DESCS,
            &mut idx,
        )
    };
    if ret != XSK_RING_PROD__DEFAULT_NUM_DESCS {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("xsk_ring_prod__reserve failed: {}", ret),
        ));
    }

    let mut xsk_info = XskSocket {
        rx: RxRing::new(rx),
        tx: TxRing {
            tx,
            cached: VecDeque::with_capacity(RX_BATCH_SIZE as usize),
            to_flush: 0,
        },
        ifindex,
        inner: xsk,
    };
    for i in 0..XSK_RING_PROD__DEFAULT_NUM_DESCS {
        let mut ring_prod_mut = umem_info.ring_prod_mut();
        let tmp = ring_prod_mut.get_addr(idx);
        *tmp = umem_frame_addr[i as usize];
        idx += 1;
    }
    unsafe {
        xsk_ring_prod__submit(&mut (*umem_info).fq, XSK_RING_PROD__DEFAULT_NUM_DESCS);
    }
    Ok(xsk_info)
}

struct XdpMultiprog {
    inner: *mut xdp_multiprog,
}

impl XdpMultiprog {
    pub fn new(ifindex: i32) -> io::Result<XdpMultiprog> {
        let mut mp = unsafe { xdp_multiprog__get_from_ifindex(ifindex) };
        if mp.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("xdp_multiprog__get_from_ifindex failed"),
            ));
        }
        Ok(XdpMultiprog { inner: mp })
    }

    fn detach(&mut self) -> io::Result<()> {
        resultify(unsafe { xdp_multiprog__detach(self.inner) }).map(|_| ())
    }
}

impl Drop for XdpMultiprog {
    fn drop(&mut self) {
        unsafe {
            xdp_multiprog__close(self.inner);
        }
    }
}

fn do_unload(ifindex: i32) -> io::Result<()> {
    let mut opts: bpf_object_open_opts = unsafe { zeroed() };
    opts.sz = size_of::<bpf_object_open_opts>();
    let mut mp = XdpMultiprog::new(ifindex)?;
    mp.detach()?;
    Ok(())
}

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
    //link: *mut bpf_link,
    ifindex: i32,
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
        umem2: &mut Umem,
        ifname: &str,
        queue_id: u32,
        xdp_flags: u32,
        bind_flags: u16,
    ) -> io::Result<Self> {
        let filename = CString::from_str("prog.o").unwrap();

        let mut opts: bpf_object_open_opts = unsafe { core::mem::zeroed() };
        opts.sz = core::mem::size_of::<bpf_object_open_opts>();
        let mut xdp_opts: xdp_program_opts = unsafe { core::mem::zeroed() };
        xdp_opts.sz = core::mem::size_of::<xdp_program_opts>();
        xdp_opts.open_filename = filename.as_ptr();
        let prog_name: Option<String> = None;
        xdp_opts.prog_name = if let Some(prog_name) = prog_name {
            prog_name.as_ptr() as *const i8
        } else {
            std::ptr::null_mut()
        };

        let attach_mode = 0x2;

        let ifindex = ifname_to_ifindex(ifname).ok_or(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to get ifindex for {}", ifname),
        ))?;
        xdp_opts.opts = &mut opts;
        //do_unload(ifindex as i32).unwrap();
        //if unsafe { *conf.progname } != 0 {
        //    xdp_opts.open_filename = conf.filename;
        //    xdp_opts.prog_name = conf.progname;
        //    xdp_opts.opts = &mut opts;
        //    prog = unsafe { xdp_program__create(&mut xdp_opts) };
        //} else {
        let prog =
            unsafe { xdp_program__open_file(filename.as_ptr(), std::ptr::null_mut(), &mut opts) };
        // }
        let err = unsafe { libxdp_get_error(prog as *const c_void) };
        if err != 0 {
            //bail!("ERR: loading program: ..");
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("ERR: loading program: {}", err),
            ));
        }

        let err = unsafe { xdp_program__attach(prog, ifindex as i32, attach_mode, 0) };

        if err != 0 {
            let tmp = unsafe { strerror(err) };
            let tmp = unsafe { std::ffi::CStr::from_ptr(tmp) };
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Couldn't attach XDP program on iface '?' : {} ({})",
                    tmp.to_str().unwrap(),
                    err
                ),
            ));
        }


        let s = CString::from_str("xsks_map").unwrap();
        let map = unsafe { bpf_object__find_map_by_name(xdp_program__bpf_obj(prog), s.as_ptr()) };
        let xsk_map_fd = unsafe { bpf_map__fd(map) };
        if xsk_map_fd < 0 {
            let tmp = unsafe { strerror(xsk_map_fd) };
            let tmp = unsafe { std::ffi::CStr::from_ptr(tmp) };
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("ERROR: no xsks map found: {}", tmp.to_str().unwrap()),
            ));
        }

        xsk_configure_socket(
            umem2,
            xsk_map_fd,
            0,
            0,
            CString::from_str(ifname).unwrap(),
            ifindex as i32,
            queue_id,
        )
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

impl Drop for XskSocket {
    fn drop(&mut self) {
        unsafe {
            xsk_socket__delete(self.inner);
            do_unload(self.ifindex).unwrap();
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
            if rcvd == 0 {
                return None;
            }
            for i in (idx_rx..(rcvd + idx_rx)).rev() {
                let rx_desc = unsafe { xsk_ring_cons__rx_desc(&mut self.rx, i) };
                let addr = unsafe { (*rx_desc).addr };
                let len = unsafe { (*rx_desc).len };
                let options = unsafe { (*rx_desc).options };
                self.cached.push(XdpDescData {
                    offset: addr,
                    len,
                    options,
                });
            }
            unsafe { xsk_ring_cons__release(&mut self.rx, rcvd) };
            Some(self.cached.pop().unwrap())
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
