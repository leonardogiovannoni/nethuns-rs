const RX_RING_SIZE: u16 = 1024;
const NUM_MBUFS: u32 = 8192;
const MBUF_CACHE_SIZE: u32 = 250;
const BURST_SIZE: u16 = 32;
const CUSTOM_ETHER_TYPE: u16 = 0x88B5;
const PORT_ID: u16 = 0;
use anyhow::{Result, bail};
use dpdk_sys::*;
use rand::RngCore;
use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::ffi::CString;
use std::mem;
use std::os::raw::{c_char, c_int};
use std::ptr::{self, NonNull};
use std::rc::Rc;
use std::slice;
use std::sync::Arc;
use std::{env, io};

pub(crate) fn resultify(x: i32) -> io::Result<u32> {
    match x >= 0 {
        true => Ok(x as u32),
        false => Err(io::Error::from_raw_os_error(-x)),
    }
}

/// Initializes a port with the given mempool.
pub(crate) unsafe fn init_port(port: u16, pool: *mut rte_mempool) -> Result<()> {
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
    println!(
        "Server: Port {} started, listening for Ethernet frames (EtherType 0x{:04x})",
        port, CUSTOM_ETHER_TYPE
    );
    Ok(())
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
        port_id: u16,
        queue_id: u16,
    ) -> Result<Self> {
        let file_prefix = rand::rng().next_u64();
        let file_prefix_str = format!("--file-prefix={}", file_prefix);
        let vdev = format!("--vdev=net_af_packet0,iface={}", "veth0");
        let init_args = vec![iface.to_string(), file_prefix_str, vdev];
        //println!("Server: Starting with arguments: {:?}", args);
        let mut cstrings: Vec<CString> = init_args
            .iter()
            .map(|arg| CString::new(arg.as_str()).unwrap())
            .collect();
        // Build a mutable array of *mut c_char pointers.
        let mut c_ptrs: Vec<*mut c_char> = cstrings
            .iter_mut()
            .map(|cstr| cstr.as_ptr() as *mut c_char)
            .collect();
        let argc = c_ptrs.len() as c_int;

        // Initialize the DPDK Environment Abstraction Layer.

        unsafe { resultify(rte_eal_init(argc, c_ptrs.as_mut_ptr()))? };
        let mbuf_pool = unsafe {
            rte_pktmbuf_pool_create(
                std::ptr::null(),
                num_mbufs,
                mbuf_cache_size,
                0,
                mbuf_default_buf_size,
                rte_socket_id() as i32,
            )
        };
        if mbuf_pool.is_null() {
            bail!("Cannot create mbuf pool");
        }
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
        port_id: u16,
        queue_id: u16,
    ) -> Result<(BufferPool, Receiver, Transmitter)> {
        let ctx = Self::inner_new(
            iface,
            num_mbufs,
            mbuf_cache_size,
            mbuf_default_buf_size,
            port_id,
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
        let trasmitter = Transmitter {
            ctx,
            mempool,
            bufs: [ptr::null_mut(); BURST_SIZE as usize],
            index: 0,
        };
        (buffer_pool, receiver, trasmitter)
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            rte_mempool_free(self.ptr);
            rte_eth_dev_stop(self.port_id);
            rte_eth_dev_close(self.port_id);
        }
    }
}

// we can assume to have three different components, which are as usually BufferPool, Trasmitter and Receiver,
// however they are tied to stay in the same thread. This is not restrictive since we are dealing with raw pointers
// and now indexes

pub(crate) struct BufferPool {
    ctx: Arc<UnsafeCell<Context>>,
}

impl BufferPool {
    pub(crate) fn allocate(&mut self) -> *mut rte_mbuf {
        let ctx = unsafe { &mut *self.ctx.get() };
        unsafe { rust_rte_pktmbuf_alloc(ctx.ptr) }
    }

    pub(crate) fn free(&mut self, mbuf: *mut rte_mbuf) {
        unsafe {
            rust_rte_pktmbuf_free(mbuf);
        }
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

impl Receiver {
    pub(crate) fn iter_mut(&mut self) -> ReceiverIterMut {
        ReceiverIterMut { rx: self }
    }
}

// tied to context lifetime
pub(crate) struct RawRteMbuf {
    ptr: *mut rte_mbuf,
}

pub(crate) struct ReceiverIterMut<'a> {
    rx: &'a mut Receiver,
}

impl<'a> ReceiverIterMut<'a> {
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
    len: usize,
}

impl RteMBuf {
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn as_ptr(&self) -> *mut rte_mbuf {
        self.ptr.as_ptr()
    }
}

impl Drop for RteMBuf {
    fn drop(&mut self) {
        unsafe {
            rust_rte_pktmbuf_free(self.ptr.as_ptr());
        }
    }
}

impl<'a> Iterator for ReceiverIterMut<'a> {
    type Item = RteMBuf;

    fn next(&mut self) -> Option<Self::Item> {
        self.advance().map(|ptr| {
            let len = unsafe { (*ptr.as_ptr()).__bindgen_anon_2.__bindgen_anon_1.pkt_len as usize };
            RteMBuf { ptr, len }
        })
    }
}

pub(crate) struct Transmitter {
    ctx: Arc<UnsafeCell<Context>>,
    mempool: *mut rte_mempool,
    bufs: [*mut rte_mbuf; BURST_SIZE as usize],
    index: usize,
}

impl Transmitter {
    fn iter_mut(&mut self) -> TransmitterIterMut {
        TransmitterIterMut { tx: self }
    }

    fn flush(&mut self) {
        let port_id = unsafe { (*self.ctx.get()).port_id };
        let queue_id = unsafe { (*self.ctx.get()).queue_id };
        let sent = unsafe {
            rust_rte_eth_tx_burst(port_id, queue_id, self.bufs.as_mut_ptr(), self.index as u16)
        } as usize;
        let rem = BURST_SIZE as usize - sent;
        if rem > 0 {
            for i in 0..rem {
                self.bufs[i] = self.bufs[sent + i];
            }
            for i in rem..BURST_SIZE as usize {
                self.bufs[i] = ptr::null_mut();
            }

            self.index = sent;
        } else {
            self.index = BURST_SIZE as usize;
        }
    }
}

pub(crate) struct TransmitterIterMut<'a> {
    tx: &'a mut Transmitter,
}

impl<'a> TransmitterIterMut<'a> {
    fn advance(&mut self) -> Option<NonNull<rte_mbuf>> {
        if self.tx.index == BURST_SIZE as usize {
            self.tx.flush();
            let res = unsafe {
                rust_rte_pktmbuf_alloc_bulk(
                    self.tx.mempool,
                    self.tx.bufs.as_mut_ptr(),
                    BURST_SIZE as u32,
                )
            };
            if res == 0 {
                return None;
            }
            self.tx.index = 0;
        } else if self.tx.index >= BURST_SIZE as usize {
            panic!("BUG: index out of bounds");
        }
        let buf = self.tx.bufs[self.tx.index];
        self.tx.index += 1;
        Some(NonNull::new(buf).unwrap())
    }
}

pub(crate) struct RteMBufRef<'a> {
    ptr: NonNull<rte_mbuf>,
    len: usize,
    _marker: std::marker::PhantomData<&'a mut TransmitterIterMut<'a>>,
}

impl<'a> Iterator for TransmitterIterMut<'a> {
    type Item = RteMBufRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let tmp = self.advance()?;
        let len = unsafe { (*tmp.as_ptr()).__bindgen_anon_2.__bindgen_anon_1.pkt_len as usize };
        Some(RteMBufRef {
            ptr: tmp,
            len,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<'a> Drop for TransmitterIterMut<'a> {
    fn drop(&mut self) {
        self.tx.flush();
    }
}
