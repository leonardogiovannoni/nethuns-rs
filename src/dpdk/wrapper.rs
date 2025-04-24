const RX_RING_SIZE: u16 = 1024;
const BURST_SIZE: u16 = 32;

use arrayvec::ArrayVec;
use dpdk_sys::*;
use rand::RngCore;
use std::cell::UnsafeCell;
use std::ffi::CString;
use std::io;
use std::io::StderrLock;
use std::mem;
use std::os::fd::RawFd;
use std::os::raw::{c_char, c_int};
use std::ptr::{self, NonNull};
use std::sync::Arc;

pub(crate) fn resultify(x: i32) -> io::Result<u32> {
    match x >= 0 {
        true => Ok(x as u32),
        false => Err(io::Error::from_raw_os_error(-x)),
    }
}

/// Initializes a port with the given mempool.
pub(crate) unsafe fn init_port(port: u16, pool: *mut rte_mempool) -> io::Result<()> {
    // Zero-initialize the port configuration.
    let mut port_conf: rte_eth_conf = unsafe { mem::zeroed() };
    unsafe { resultify(rte_eth_dev_configure(port, 1, 1, &mut port_conf))? };

    unsafe {
        resultify(rte_eth_rx_queue_setup(
            port,
            0,
            RX_RING_SIZE,
            rte_eth_dev_socket_id(port) as u32,
            ptr::null_mut(),
            pool,
        ))?
    };

    unsafe {
        resultify(rte_eth_tx_queue_setup(
            port,
            0,
            RX_RING_SIZE,
            rte_eth_dev_socket_id(port) as u32,
            ptr::null_mut(),
        ))?
    };

    unsafe { resultify(rte_eth_dev_start(port))? };
    //unsafe { resultify(rte_eth_promiscuous_enable(port))? };

    Ok(())
}

fn find_port(name: &str) -> Option<u16> {
    let nb_ports = unsafe { rte_eth_dev_count_avail() };
    for port_id in 0..nb_ports {
        let port_name = [0; 256];
        if unsafe { rte_eth_dev_get_name_by_port(port_id, port_name.as_ptr() as *mut i8) } == 0 {
            let port_name = unsafe { std::ffi::CStr::from_ptr(port_name.as_ptr() as *const i8) };
            let mut dev_info: rte_eth_dev_info = unsafe { mem::zeroed() };
            unsafe { rte_eth_dev_info_get(port_id, &mut dev_info) };
            if port_name.to_str().unwrap() == name {
                return Some(port_id);
            }
        }
    }
    None
}

struct StderrGuard {
    saved_fd: RawFd,
    std_err_lock: StderrLock<'static>,
}

impl Drop for StderrGuard {
    fn drop(&mut self) {
        unsafe {
            // Restore stderr from the saved file descriptor.
            libc::dup2(self.saved_fd, libc::STDERR_FILENO);
            libc::close(self.saved_fd);
        }
    }
}

fn redirect_stderr_to_null() -> io::Result<StderrGuard> {
    let std_err_lock = std::io::stderr().lock();
    unsafe {
        // Save the original stderr file descriptor.
        let saved_fd = libc::dup(libc::STDERR_FILENO);
        if saved_fd < 0 {
            return Err(io::Error::last_os_error());
        }

        // Open /dev/null.
        let devnull = CString::new("/dev/null").unwrap();
        let fd_devnull = libc::open(devnull.as_ptr(), libc::O_WRONLY);
        if fd_devnull < 0 {
            libc::close(saved_fd);
            return Err(io::Error::last_os_error());
        }

        // Redirect stderr to /dev/null.
        if libc::dup2(fd_devnull, libc::STDERR_FILENO) < 0 {
            libc::close(saved_fd);
            libc::close(fd_devnull);
            return Err(io::Error::last_os_error());
        }

        // Close the extra file descriptor.
        libc::close(fd_devnull);

        Ok(StderrGuard {
            saved_fd,
            std_err_lock,
        })
    }
}

pub(crate) struct Context {
    file_prefix: u64,
    ptr: *mut rte_mempool,
    port_id: u16,
    queue_id: u16,
}

impl Context {
    pub(crate) fn inner_new(
        iface: &str,
        num_mbufs: u32,
        mbuf_cache_size: u32,
        mbuf_default_buf_size: u16,
        queue_id: u16,
    ) -> io::Result<Self> {
        let file_prefix = rand::rng().next_u64();
        let file_prefix_str = format!("--file-prefix={}", "server");
        let tmp = "-a".to_string();

        let tmp2 = iface.to_string(); // file_prefix);
        let init_args = vec![file_prefix_str, tmp, tmp2];
        let mut cstrings: Vec<CString> = init_args
            .iter()
            .map(|arg| CString::new(arg.as_str()).unwrap())
            .collect();
        let mut c_ptrs: Vec<*mut c_char> = cstrings
            .iter_mut()
            .map(|cstr| cstr.as_ptr() as *mut c_char)
            .collect();
        let argc = c_ptrs.len() as c_int;

        unsafe { resultify(rte_eal_init(argc, c_ptrs.as_mut_ptr()))? };

        let random_name = rand::rng().next_u64().to_string();

        let mbuf_pool = unsafe {
            rte_pktmbuf_pool_create(
                CString::new(random_name).unwrap().as_ptr(),
                num_mbufs,
                mbuf_cache_size,
                0,
                mbuf_default_buf_size,
                rte_socket_id() as i32,
            )
        };
        if mbuf_pool.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Cannot create mbuf pool",
            ));
        }
        let port_id = 0;
        unsafe {
            init_port(port_id, mbuf_pool)?;
        }
        Ok(Context {
            file_prefix,
            ptr: mbuf_pool,
            port_id,
            queue_id,
        })
    }

    pub(crate) fn new(
        iface: &str,
        num_mbufs: u32,
        mbuf_cache_size: u32,
        mbuf_default_buf_size: u16,
        queue_id: u16,
    ) -> io::Result<(BufferPool, Receiver, Transmitter)> {
        let ctx = Self::inner_new(
            iface,
            num_mbufs,
            mbuf_cache_size,
            mbuf_default_buf_size,
            queue_id,
        )?;
        Ok(Self::split(ctx))
    }

    pub(crate) fn split(self) -> (BufferPool, Receiver, Transmitter) {
        let port_id = self.port_id;
        let queue_id = self.queue_id;
        let mempool = self.ptr;
        let ctx = Arc::new(UnsafeCell::new(self));
        let buffer_pool = BufferPool {
            ctx: Arc::clone(&ctx),
        };
        let receiver = Receiver {
            ctx: Arc::clone(&ctx),
            bufs: [ptr::null_mut(); BURST_SIZE as usize],
            nb_rx: 0,
            index: 0,
            port_id,
            queue_id,
        };

        let trasmitter = Transmitter::new(ctx, mempool);

        (buffer_pool, receiver, trasmitter)
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            rte_eth_dev_stop(self.port_id);
            rte_eth_dev_close(self.port_id);
            rte_mempool_free(self.ptr);
        }
    }
}

// we can assume to have three different components, which are as usually BufferPool, Trasmitter and Receiver,
// however they are tied to stay in the same thread. This is not restrictive since we are dealing with raw pointers
// and now indexes

pub(crate) struct BufferPool {
    ctx: Arc<UnsafeCell<Context>>,
}

unsafe impl Send for BufferPool {}

impl BufferPool {
    pub(crate) fn allocate(&mut self) -> *mut rte_mbuf {
        let ctx = unsafe { &mut *self.ctx.get() };
        unsafe { rust_rte_pktmbuf_alloc(ctx.ptr) }
    }
}

pub(crate) struct Receiver {
    ctx: Arc<UnsafeCell<Context>>,
    bufs: [*mut rte_mbuf; BURST_SIZE as usize],
    nb_rx: usize,
    index: usize,
    port_id: u16,
    queue_id: u16,
}

unsafe impl Send for Receiver {}

impl Receiver {
    pub(crate) fn iter_mut<'a>(&'a mut self) -> ReceiverIterMut<'a> {
        ReceiverIterMut { rx: self }
    }
}

pub(crate) struct ReceiverIterMut<'a> {
    rx: &'a mut Receiver,
}

impl<'a> ReceiverIterMut<'a> {
    #[inline(always)]
    fn advance(&mut self) -> Option<NonNull<rte_mbuf>> {
        if self.rx.index == self.rx.nb_rx {
            let port_id = self.rx.port_id;
            let queue_id = self.rx.queue_id;
            let res = unsafe {
                rust_rte_eth_rx_burst(port_id, queue_id, self.rx.bufs.as_mut_ptr(), BURST_SIZE)
            };

            self.rx.index = 0;
            self.rx.nb_rx = res as usize;
            if res == 0 {
                return None;
            }
        } else if self.rx.index >= self.rx.nb_rx {
            panic!("BUG: index out of bounds");
        }
        let buf = self.rx.bufs[self.rx.index];
        self.rx.index += 1;
        Some(NonNull::new(buf).unwrap())
    }
}

pub(crate) struct RteMBuf {
    ptr: NonNull<rte_mbuf>,
}

impl RteMBuf {
    pub(crate) fn as_ptr(&self) -> *mut rte_mbuf {
        self.ptr.as_ptr()
    }
}

impl<'a> Iterator for ReceiverIterMut<'a> {
    type Item = RteMBuf;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.advance().map(|ptr| {
            //let len = unsafe { (*ptr.as_ptr()).__bindgen_anon_2.__bindgen_anon_1.pkt_len as usize };
            RteMBuf { ptr }
        })
    }
}

pub(crate) struct Transmitter {
    ctx: Arc<UnsafeCell<Context>>,
    mempool: *mut rte_mempool,
    bufs: ArrayVec<*mut rte_mbuf, { BURST_SIZE as usize }>,
    ready_bufs: ArrayVec<NonNull<rte_mbuf>, { BURST_SIZE as usize }>,
    port_id: u16,
    queue_id: u16,
}

impl Transmitter {
    pub(crate) fn iter_mut(&mut self) -> TransmitterIterMut {
        TransmitterIterMut { tx: self }
    }

    pub(crate) fn flush(&mut self) {
        let sent = unsafe {
            let len = self.ready_bufs.len();
            let ready_bufs: *mut *mut rte_mbuf = self.ready_bufs.as_mut_ptr() as *mut *mut _;
            rust_rte_eth_tx_burst(self.port_id, self.queue_id, ready_bufs, len as u16)
        } as usize;
        self.ready_bufs.drain(..sent);
    }

    fn new(ctx: Arc<UnsafeCell<Context>>, mempool: *mut rte_mempool) -> Self {
        let mut bufs = ArrayVec::new();
        while !bufs.is_full() {
            bufs.push(ptr::null_mut());
        }

        let ret = unsafe {
            rust_rte_pktmbuf_alloc_bulk(
                mempool,
                bufs.as_mut_ptr() as *mut *mut _,
                bufs.capacity() as u32,
            )
        };

        if ret != 0 {
            panic!("Cannot allocate mbufs");
        }

        let port_id = unsafe { (*ctx.get()).port_id };
        let queue_id = unsafe { (*ctx.get()).queue_id };

        Self {
            ctx,
            mempool,
            bufs,
            ready_bufs: ArrayVec::new(),
            port_id,
            queue_id,
        }
    }
}

unsafe impl Send for Transmitter {}

pub(crate) struct TransmitterIterMut<'a> {
    tx: &'a mut Transmitter,
}

impl<'a> TransmitterIterMut<'a> {
    fn advance(&mut self) -> Option<NonNull<rte_mbuf>> {
        if self.tx.ready_bufs.is_full() {
            self.tx.flush();
            let old_len = self.tx.ready_bufs.len();
            let can_ask = self.tx.bufs.capacity() - old_len;
            while !self.tx.bufs.is_full() {
                self.tx.bufs.push(ptr::null_mut());
            }

            let slice = &mut self.tx.bufs[old_len..];
            let res = unsafe {
                rust_rte_pktmbuf_alloc_bulk(
                    self.tx.mempool,
                    slice.as_mut_ptr() as *mut *mut _,
                    can_ask as u32,
                )
            };
            if res != 0 {
                for _ in 0..can_ask {
                    self.tx.bufs.pop().unwrap();
                }
                return None;
            }
        }
        self.tx.bufs.pop().map(|buf| NonNull::new(buf).unwrap())
    }
}

pub(crate) struct RteMBufRef<'a> {
    ptr: NonNull<rte_mbuf>,
    _marker: std::marker::PhantomData<&'a mut TransmitterIterMut<'a>>,
    tx_iter: *mut TransmitterIterMut<'a>,
}

impl<'a> RteMBufRef<'a> {
    pub(crate) fn as_ptr(&self) -> NonNull<rte_mbuf> {
        self.ptr
    }
}

impl<'a> Iterator for TransmitterIterMut<'a> {
    type Item = RteMBufRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let tmp = self.advance()?;
        Some(RteMBufRef {
            ptr: tmp,
            _marker: std::marker::PhantomData,
            tx_iter: self,
        })
    }
}

impl<'a> Drop for RteMBufRef<'a> {
    fn drop(&mut self) {
        let tx = unsafe { &mut (*self.tx_iter).tx };
        tx.ready_bufs.push(self.ptr);
    }
}

impl<'a> Drop for TransmitterIterMut<'a> {
    fn drop(&mut self) {
        self.tx.flush();
    }
}
