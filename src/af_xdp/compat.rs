/*XDP_ALWAYS_INLINE __u32 xsk_prod_nb_free(struct xsk_ring_prod *r, __u32 nb)
{
    __u32 free_entries = r->cached_cons - r->cached_prod;

    if (free_entries >= nb)
        return free_entries;

    r->cached_cons = __atomic_load_n(r->consumer, __ATOMIC_ACQUIRE);
    r->cached_cons += r->size;

    return r->cached_cons - r->cached_prod;
}

XDP_ALWAYS_INLINE __u32 xsk_ring_prod__reserve(struct xsk_ring_prod *prod, __u32 nb, __u32 *idx)
{
    if (xsk_prod_nb_free(prod, nb) < nb)
        return 0;

    *idx = prod->cached_prod;
    prod->cached_prod += nb;

    return nb;
}
*/

use std::{
    collections::HashSet, ffi::c_void, sync::{atomic::{AtomicU32, Ordering}, LazyLock, Mutex}
};

use libc::{
    getsockopt, mmap, munmap, setsockopt, sockaddr, sockaddr_xdp, xdp_mmap_offsets_v1, EINVAL, IFNAMSIZ, MAP_FAILED, MAP_POPULATE, MAP_SHARED, PROT_READ, PROT_WRITE, SOL_XDP, XDP_MMAP_OFFSETS, XDP_UMEM_COMPLETION_RING, XDP_UMEM_FILL_RING, XDP_UMEM_PGOFF_COMPLETION_RING, XDP_UMEM_PGOFF_FILL_RING
};
use libxdp_sys::{xdp_desc, xdp_mmap_offsets, xdp_program, xsk_ring_cons, xsk_ring_prod, xsk_socket_config, xsk_umem_config, XSK_LIBXDP_FLAGS__INHIBIT_PROG_LOAD, XSK_RING_CONS__DEFAULT_NUM_DESCS, XSK_RING_PROD__DEFAULT_NUM_DESCS};
use libxdp_sys::__xsk_setup_xdp_prog;

pub unsafe fn xsk_prod_nb_free(r: &mut xsk_ring_prod, nb: u32) -> u32 {
    let free_entries = r.cached_cons - r.cached_prod;

    if free_entries >= nb {
        return free_entries;
    }

    let cons = r.consumer as *const AtomicU32;
    let cons = unsafe { &*cons };
    r.cached_cons = cons.load(Ordering::Acquire);
    r.cached_cons += r.size;

    return r.cached_cons - r.cached_prod;
}

pub unsafe fn xsk_ring_prod__reserve(prod: &mut xsk_ring_prod, nb: u32, idx: &mut u32) -> u32 {
    if xsk_prod_nb_free(prod, nb) < nb {
        return 0;
    }

    *idx = prod.cached_prod;
    prod.cached_prod += nb;

    return nb;
}

/*XDP_ALWAYS_INLINE void xsk_ring_prod__submit(struct xsk_ring_prod *prod, __u32 nb)
{
    /* Make sure everything has been written to the ring before indicating
     * this to the kernel by writing the producer pointer.
     */
    __atomic_store_n(prod->producer, *prod->producer + nb, __ATOMIC_RELEASE);
} */

pub unsafe fn xsk_ring_prod__submit(prod: &mut xsk_ring_prod, nb: u32) {
    let producer = prod.producer as *const AtomicU32;
    let producer = unsafe { &*producer };
    let tmp = producer.load(Ordering::Relaxed) + nb;
    producer.store(tmp, Ordering::Release);
}

/*XDP_ALWAYS_INLINE const __u64 *
xsk_ring_cons__comp_addr(const struct xsk_ring_cons *comp, __u32 idx)
{
    const __u64 *addrs = (const __u64 *)comp->ring;

    return &addrs[idx & comp->mask];
}*/

pub unsafe fn xsk_ring_cons__comp_addr(comp: &xsk_ring_cons, idx: u32) -> *const u64 {
    let addrs = comp.ring as *const u64;
    return addrs.offset((idx & comp.mask) as isize);
}

/*XDP_ALWAYS_INLINE __u32 xsk_cons_nb_avail(struct xsk_ring_cons *r, __u32 nb)
{
    __u32 entries = r->cached_prod - r->cached_cons;

    if (entries == 0) {
        r->cached_prod = __atomic_load_n(r->producer, __ATOMIC_ACQUIRE);
        entries = r->cached_prod - r->cached_cons;
    }

    return (entries > nb) ? nb : entries;
} */

pub unsafe fn xsk_cons_nb_avail(r: &mut xsk_ring_cons, nb: u32) -> u32 {
    let mut entries = r.cached_prod - r.cached_cons;

    if entries == 0 {
        let prod = r.producer as *const AtomicU32;
        let prod = unsafe { &*prod };
        r.cached_prod = prod.load(Ordering::Acquire);
        entries = r.cached_prod - r.cached_cons;
    }

    if entries > nb { nb } else { entries }
}

/*XDP_ALWAYS_INLINE __u32 xsk_ring_cons__peek(struct xsk_ring_cons *cons, __u32 nb, __u32 *idx)
{
    __u32 entries = xsk_cons_nb_avail(cons, nb);

    if (entries > 0) {
        *idx = cons->cached_cons;
        cons->cached_cons += entries;
    }

    return entries;
}
 */

pub unsafe fn xsk_ring_cons__peek(cons: &mut xsk_ring_cons, nb: u32, idx: &mut u32) -> u32 {
    let entries = xsk_cons_nb_avail(cons, nb);

    if entries > 0 {
        *idx = cons.cached_cons;
        cons.cached_cons += entries;
    }

    return entries;
}

/*XDP_ALWAYS_INLINE void xsk_ring_cons__release(struct xsk_ring_cons *cons, __u32 nb)
{
    /* Make sure data has been read before indicating we are done
     * with the entries by updating the consumer pointer.
     */
    __atomic_store_n(cons->consumer, *cons->consumer + nb, __ATOMIC_RELEASE);
}
 */

pub unsafe fn xsk_ring_cons__release(cons: &mut xsk_ring_cons, nb: u32) {
    let consumer = cons.consumer as *const AtomicU32;
    let consumer = unsafe { &*consumer };
    let tmp = consumer.load(Ordering::Relaxed) + nb;
    consumer.store(tmp, Ordering::Release);
}

/*XDP_ALWAYS_INLINE const struct xdp_desc *
xsk_ring_cons__rx_desc(const struct xsk_ring_cons *rx, __u32 idx)
{
    const struct xdp_desc *descs = (const struct xdp_desc *)rx->ring;

    return &descs[idx & rx->mask];
}
 */

pub unsafe fn xsk_ring_cons__rx_desc(rx: &mut xsk_ring_cons, idx: u32) -> *const xdp_desc {
    let descs = rx.ring as *const xdp_desc;
    return descs.offset((idx & rx.mask) as isize);
}

/*static void xsk_mmap_offsets_v1(struct xdp_mmap_offsets *off)
{
    struct xdp_mmap_offsets_v1 off_v1;

    /* getsockopt on a kernel <= 5.3 has no flags fields.
     * Copy over the offsets to the correct places in the >=5.4 format
     * and put the flags where they would have been on that kernel.
     */
    memcpy(&off_v1, off, sizeof(off_v1));

    off->rx.producer = off_v1.rx.producer;
    off->rx.consumer = off_v1.rx.consumer;
    off->rx.desc = off_v1.rx.desc;
    off->rx.flags = off_v1.rx.consumer + sizeof(__u32);

    off->tx.producer = off_v1.tx.producer;
    off->tx.consumer = off_v1.tx.consumer;
    off->tx.desc = off_v1.tx.desc;
    off->tx.flags = off_v1.tx.consumer + sizeof(__u32);

    off->fr.producer = off_v1.fr.producer;
    off->fr.consumer = off_v1.fr.consumer;
    off->fr.desc = off_v1.fr.desc;
    off->fr.flags = off_v1.fr.consumer + sizeof(__u32);

    off->cr.producer = off_v1.cr.producer;
    off->cr.consumer = off_v1.cr.consumer;
    off->cr.desc = off_v1.cr.desc;
    off->cr.flags = off_v1.cr.consumer + sizeof(__u32);
} */

fn xsk_mmap_offsets_v1(off: &mut xdp_mmap_offsets) {
    off.rx.producer = off.rx.producer;
    off.rx.consumer = off.rx.consumer;
    off.rx.desc = off.rx.desc;
    off.rx.flags = off.rx.consumer + size_of::<u32>() as u64;

    off.tx.producer = off.tx.producer;
    off.tx.consumer = off.tx.consumer;
    off.tx.desc = off.tx.desc;
    off.tx.flags = off.tx.consumer + size_of::<u32>() as u64;

    off.fr.producer = off.fr.producer;
    off.fr.consumer = off.fr.consumer;
    off.fr.desc = off.fr.desc;
    off.fr.flags = off.fr.consumer + size_of::<u32>() as u64;

    off.cr.producer = off.cr.producer;
    off.cr.consumer = off.cr.consumer;
    off.cr.desc = off.cr.desc;
    off.cr.flags = off.cr.consumer + size_of::<u32>() as u64;
}

/*
static int xsk_get_mmap_offsets(int fd, struct xdp_mmap_offsets *off)
{
    socklen_t optlen;
    int err;

    optlen = sizeof(*off);
    err = getsockopt(fd, SOL_XDP, XDP_MMAP_OFFSETS, off, &optlen);
    if (err)
        return err;

    if (optlen == sizeof(*off))
        return 0;

    if (optlen == sizeof(struct xdp_mmap_offsets_v1)) {
        xsk_mmap_offsets_v1(off);
        return 0;
    }

    return -EINVAL;
}
 */

fn xsk_get_mmap_offsets(fd: i32, off: &mut xdp_mmap_offsets) -> i32 {
    let mut optlen = size_of::<xdp_mmap_offsets>() as u32;
    let err = unsafe {
        getsockopt(
            fd,
            SOL_XDP,
            XDP_MMAP_OFFSETS,
            off as *mut xdp_mmap_offsets as *mut c_void,
            &mut optlen,
        )
    };
    if err != 0 {
        return err;
    }

    if optlen == size_of::<xdp_mmap_offsets>() as u32 {
        return 0;
    }

    if optlen == size_of::<xdp_mmap_offsets_v1>() as u32 {
        xsk_mmap_offsets_v1(off);
        return 0;
    }

    return -EINVAL;
}

/*struct xsk_umem {
    struct xsk_ring_prod *fill_save;
    struct xsk_ring_cons *comp_save;
    char *umem_area;
    struct xsk_umem_config config;
    int fd;
    int refcount;
    struct list_head ctx_list;
    bool rx_ring_setup_done;
    bool tx_ring_setup_done;
}; */

#[repr(C)]
pub struct xsk_umem {
    fill_save: *mut xsk_ring_prod,
    comp_save: *mut xsk_ring_cons,
    umem_area: *mut i8,
    config: xsk_umem_config,
    fd: i32,
    refcount: i32,
    //ctx_list: list_head,
    rx_ring_setup_done: bool,
    tx_ring_setup_done: bool,
}

/*

static int xsk_create_umem_rings(struct xsk_umem *umem, int fd,
                 struct xsk_ring_prod *fill,
                 struct xsk_ring_cons *comp)
{
    struct xdp_mmap_offsets off;
    void *map;
    int err;

    err = setsockopt(fd, SOL_XDP, XDP_UMEM_FILL_RING,
             &umem->config.fill_size,
             sizeof(umem->config.fill_size));
    if (err)
        return -errno;

    err = setsockopt(fd, SOL_XDP, XDP_UMEM_COMPLETION_RING,
             &umem->config.comp_size,
             sizeof(umem->config.comp_size));
    if (err)
        return -errno;

    err = xsk_get_mmap_offsets(fd, &off);
    if (err)
        return -errno;

    map = mmap(NULL, off.fr.desc + umem->config.fill_size * sizeof(__u64),
           PROT_READ | PROT_WRITE, MAP_SHARED | MAP_POPULATE, fd,
           XDP_UMEM_PGOFF_FILL_RING);
    if (map == MAP_FAILED)
        return -errno;

    fill->mask = umem->config.fill_size - 1;
    fill->size = umem->config.fill_size;
    fill->producer = map + off.fr.producer;
    fill->consumer = map + off.fr.consumer;
    fill->flags = map + off.fr.flags;
    fill->ring = map + off.fr.desc;
    fill->cached_cons = umem->config.fill_size;

    map = mmap(NULL, off.cr.desc + umem->config.comp_size * sizeof(__u64),
           PROT_READ | PROT_WRITE, MAP_SHARED | MAP_POPULATE, fd,
           XDP_UMEM_PGOFF_COMPLETION_RING);
    if (map == MAP_FAILED) {
        err = -errno;
        goto out_mmap;
    }

    comp->mask = umem->config.comp_size - 1;
    comp->size = umem->config.comp_size;
    comp->producer = map + off.cr.producer;
    comp->consumer = map + off.cr.consumer;
    comp->flags = map + off.cr.flags;
    comp->ring = map + off.cr.desc;

    return 0;

out_mmap:
    munmap(map, off.fr.desc + umem->config.fill_size * sizeof(__u64));
    return err;
}

*/

unsafe fn xsk_create_umem_rings(
    umem: &mut xsk_umem,
    fd: i32,
    fill: &mut xsk_ring_prod,
    comp: &mut xsk_ring_cons,
) -> i32 {
    let mut off: xdp_mmap_offsets = unsafe { std::mem::zeroed() };
    let mut err: i32;

    err = unsafe {
        setsockopt(
            fd,
            SOL_XDP,
            XDP_UMEM_FILL_RING,
            &raw mut umem.config.fill_size as *mut c_void,
            size_of::<u32>() as u32,
        )
    };
    if err != 0 {
        return -1;
    }

    err = unsafe {
        setsockopt(
            fd,
            SOL_XDP,
            XDP_UMEM_COMPLETION_RING,
            &raw mut umem.config.comp_size as *mut c_void,
            size_of::<u32>() as u32,
        )
    };
    if err != 0 {
        return -1;
    }

    err = xsk_get_mmap_offsets(fd, &mut off);
    if err != 0 {
        return -1;
    }

    let map = unsafe {
        mmap(
            std::ptr::null_mut(),
            off.fr.desc as usize + umem.config.fill_size as usize * size_of::<u64>(),
            PROT_READ | PROT_WRITE,
            MAP_SHARED | MAP_POPULATE,
            fd,
            XDP_UMEM_PGOFF_FILL_RING as i64,
        )
    };
    if map == MAP_FAILED {
        return -1;
    }

    fill.mask = umem.config.fill_size - 1;
    fill.size = umem.config.fill_size;
    fill.producer = (map as *mut u8).offset(off.fr.producer as isize) as *mut u32;
    fill.consumer = (map as *mut u8).offset(off.fr.consumer as isize) as *mut u32;
    fill.flags = (map as *mut u8).offset(off.fr.flags as isize) as *mut u32;
    fill.ring = (map as *mut u8).offset(off.fr.desc as isize) as *mut c_void;
    fill.cached_cons = umem.config.fill_size;

    let map = unsafe {
        mmap(
            std::ptr::null_mut(),
            off.cr.desc as usize + (umem.config.comp_size as usize) * size_of::<u64>(),
            PROT_READ | PROT_WRITE,
            MAP_SHARED | MAP_POPULATE,
            fd,
            XDP_UMEM_PGOFF_COMPLETION_RING as i64,
        )
    };
    if map == MAP_FAILED {
		
        unsafe {
            munmap(
                map,
                off.fr.desc as usize + (umem.config.fill_size as usize) * size_of::<u64>() as usize,
            )
        };
        return err;
    }

    comp.mask = umem.config.comp_size - 1;
    comp.size = umem.config.comp_size;
    comp.producer = (map as *mut u8).offset(off.cr.producer as isize) as *mut u32;
    comp.consumer = (map as *mut u8).offset(off.cr.consumer as isize) as *mut u32;
    comp.flags = (map as *mut u8).offset(off.cr.flags as isize) as *mut u32;
    comp.ring = (map as *mut u8).offset(off.cr.desc as isize) as *mut c_void;

    return 0;
}

/*
static int xsk_set_xdp_socket_config(struct xsk_socket_config *cfg,
                     const struct xsk_socket_opts *opts)
{
    __u32 libxdp_flags;

    libxdp_flags = OPTS_GET(opts, libxdp_flags, 0);
    if (libxdp_flags & ~XSK_LIBXDP_FLAGS__INHIBIT_PROG_LOAD)
        return -EINVAL;

    cfg->rx_size = OPTS_GET(opts, rx_size, 0) ?: XSK_RING_CONS__DEFAULT_NUM_DESCS;
    cfg->tx_size = OPTS_GET(opts, tx_size, 0) ?: XSK_RING_PROD__DEFAULT_NUM_DESCS;
    cfg->libxdp_flags = libxdp_flags;
    cfg->xdp_flags = OPTS_GET(opts, xdp_flags, 0);
    cfg->bind_flags = OPTS_GET(opts, bind_flags, 0);

    return 0;
}
*/

/*struct xsk_socket_opts {
	size_t sz;
	struct xsk_ring_cons *rx;
	struct xsk_ring_prod *tx;
	struct xsk_ring_prod *fill;
	struct xsk_ring_cons *comp;
	__u32 rx_size;
	__u32 tx_size;
	__u32 libxdp_flags;
	__u32 xdp_flags;
	__u16 bind_flags;
	size_t :0;
}; */

#[repr(C)]
pub struct xsk_socket_opts {
	sz: usize,
	rx: *mut xsk_ring_cons,
	tx: *mut xsk_ring_prod,
	fill: *mut xsk_ring_prod,
	comp: *mut xsk_ring_cons,
	rx_size: u32,
	tx_size: u32,
	libxdp_flags: u32,
	xdp_flags: u32,
	bind_flags: u16,
}

fn xsk_set_xdp_socket_config(cfg: &mut xsk_socket_config, opts: &xsk_socket_opts) -> i32 {
	let libxdp_flags = opts.libxdp_flags;

	if libxdp_flags & !XSK_LIBXDP_FLAGS__INHIBIT_PROG_LOAD != 0 {
		return -EINVAL;
	}
	cfg.rx_size = if opts.rx_size != 0 { opts.rx_size } else { XSK_RING_CONS__DEFAULT_NUM_DESCS };
	cfg.tx_size = if opts.tx_size != 0 { opts.tx_size } else { XSK_RING_PROD__DEFAULT_NUM_DESCS };
	cfg.__bindgen_anon_1.libxdp_flags = libxdp_flags;
	cfg.xdp_flags = opts.xdp_flags;
	cfg.bind_flags = opts.bind_flags;
	return 0;
}

/*
struct xsk_socket *xsk_socket__create_opts(const char *ifname,
                       __u32 queue_id,
                       struct xsk_umem *umem,
                       struct xsk_socket_opts *opts)
{
    bool rx_setup_done = false, tx_setup_done = false;
    void *rx_map = NULL, *tx_map = NULL;
    struct sockaddr_xdp sxdp = {};
    struct xdp_mmap_offsets off;
    struct xsk_ring_prod *fill;
    struct xsk_ring_cons *comp;
    struct xsk_ring_cons *rx;
    struct xsk_ring_prod *tx;
    struct xsk_socket *xsk;
    struct xsk_ctx *ctx;
    int err, ifindex;
    __u64 netns_cookie;
    socklen_t optlen;
    bool unmap;

    if (!OPTS_VALID(opts, xsk_socket_opts)) {
        err = -EINVAL;
        goto err;
    }
    rx = OPTS_GET(opts, rx, NULL);
    tx = OPTS_GET(opts, tx, NULL);
    fill = OPTS_GET(opts, fill, NULL);
    comp = OPTS_GET(opts, comp, NULL);

    if (!umem || !(rx || tx) || (fill == NULL) ^ (comp == NULL)) {
        err = -EFAULT;
        goto err;
    }
    if (!fill && !comp) {
        fill = umem->fill_save;
        comp = umem->comp_save;
    }

    xsk = calloc(1, sizeof(*xsk));
    if (!xsk) {
        err = -ENOMEM;
        goto err;
    }

    err = xsk_set_xdp_socket_config(&xsk->config, opts);
    if (err)
        goto out_xsk_alloc;

    ifindex = if_nametoindex(ifname);
    if (!ifindex) {
        err = -errno;
        goto out_xsk_alloc;
    }

    if (umem->refcount++ > 0) {
        xsk->fd = socket(AF_XDP, SOCK_RAW, 0);
        if (xsk->fd < 0) {
            err = -errno;
            goto out_xsk_alloc;
        }
    } else {
        xsk->fd = umem->fd;
        rx_setup_done = umem->rx_ring_setup_done;
        tx_setup_done = umem->tx_ring_setup_done;
    }

    optlen = sizeof(netns_cookie);
    err = getsockopt(xsk->fd, SOL_SOCKET, SO_NETNS_COOKIE, &netns_cookie, &optlen);
    if (err) {
        if (errno != ENOPROTOOPT) {
            err = -errno;
            goto out_socket;
        }
        netns_cookie = INIT_NS;
    }

    ctx = xsk_get_ctx(umem, netns_cookie, ifindex, queue_id);
    if (!ctx) {
        if (!fill || !comp) {
            err = -EFAULT;
            goto out_socket;
        }

        ctx = xsk_create_ctx(xsk, umem, netns_cookie, ifindex, ifname, queue_id,
                     fill, comp);
        if (!ctx) {
            err = -ENOMEM;
            goto out_socket;
        }
    }
    xsk->ctx = ctx;

    if (rx && !rx_setup_done) {
        err = setsockopt(xsk->fd, SOL_XDP, XDP_RX_RING,
                 &xsk->config.rx_size,
                 sizeof(xsk->config.rx_size));
        if (err) {
            err = -errno;
            goto out_put_ctx;
        }
        if (xsk->fd == umem->fd)
            umem->rx_ring_setup_done = true;

    }
    if (tx && !tx_setup_done) {
        err = setsockopt(xsk->fd, SOL_XDP, XDP_TX_RING,
                 &xsk->config.tx_size,
                 sizeof(xsk->config.tx_size));
        if (err) {
            err = -errno;
            goto out_put_ctx;
        }
        if (xsk->fd == umem->fd)
            umem->tx_ring_setup_done = true;
    }

    err = xsk_get_mmap_offsets(xsk->fd, &off);
    if (err) {
        err = -errno;
        goto out_put_ctx;
    }

    if (rx) {
        rx_map = mmap(NULL, off.rx.desc +
                  xsk->config.rx_size * sizeof(struct xdp_desc),
                  PROT_READ | PROT_WRITE, MAP_SHARED | MAP_POPULATE,
                  xsk->fd, XDP_PGOFF_RX_RING);
        if (rx_map == MAP_FAILED) {
            err = -errno;
            goto out_put_ctx;
        }

        rx->mask = xsk->config.rx_size - 1;
        rx->size = xsk->config.rx_size;
        rx->producer = rx_map + off.rx.producer;
        rx->consumer = rx_map + off.rx.consumer;
        rx->flags = rx_map + off.rx.flags;
        rx->ring = rx_map + off.rx.desc;
        rx->cached_prod = *rx->producer;
        rx->cached_cons = *rx->consumer;
    }
    xsk->rx = rx;

    if (tx) {
        tx_map = mmap(NULL, off.tx.desc +
                  xsk->config.tx_size * sizeof(struct xdp_desc),
                  PROT_READ | PROT_WRITE, MAP_SHARED | MAP_POPULATE,
                  xsk->fd, XDP_PGOFF_TX_RING);
        if (tx_map == MAP_FAILED) {
            err = -errno;
            goto out_mmap_rx;
        }

        tx->mask = xsk->config.tx_size - 1;
        tx->size = xsk->config.tx_size;
        tx->producer = tx_map + off.tx.producer;
        tx->consumer = tx_map + off.tx.consumer;
        tx->flags = tx_map + off.tx.flags;
        tx->ring = tx_map + off.tx.desc;
        tx->cached_prod = *tx->producer;
        /* cached_cons is r->size bigger than the real consumer pointer
         * See xsk_prod_nb_free
         */
        tx->cached_cons = *tx->consumer + xsk->config.tx_size;
    }
    xsk->tx = tx;

    sxdp.sxdp_family = PF_XDP;
    sxdp.sxdp_ifindex = ctx->ifindex;
    sxdp.sxdp_queue_id = ctx->queue_id;
    if (umem->refcount > 1) {
        sxdp.sxdp_flags |= XDP_SHARED_UMEM;
        sxdp.sxdp_shared_umem_fd = umem->fd;
    } else {
        sxdp.sxdp_flags = xsk->config.bind_flags;
    }

    err = bind(xsk->fd, (struct sockaddr *)&sxdp, sizeof(sxdp));
    if (err) {
        err = -errno;
        goto out_mmap_tx;
    }

    if (!(xsk->config.libxdp_flags & XSK_LIBXDP_FLAGS__INHIBIT_PROG_LOAD)) {
        err = __xsk_setup_xdp_prog(xsk, NULL);
        if (err)
            goto out_mmap_tx;
    }

    umem->fill_save = NULL;
    umem->comp_save = NULL;
    return xsk;

out_mmap_tx:
    if (tx)
        munmap(tx_map, off.tx.desc +
               xsk->config.tx_size * sizeof(struct xdp_desc));
out_mmap_rx:
    if (rx)
        munmap(rx_map, off.rx.desc +
               xsk->config.rx_size * sizeof(struct xdp_desc));
out_put_ctx:
    unmap = umem->fill_save != fill;
    xsk_put_ctx(ctx, unmap);
out_socket:
    if (--umem->refcount)
        close(xsk->fd);
out_xsk_alloc:
    free(xsk);
err:
    return libxdp_err_ptr(err, true);
}
*/

/*struct xsk_ctx {
	struct xsk_ring_prod *fill;
	struct xsk_ring_cons *comp;
	struct xsk_umem *umem;
	__u32 queue_id;
	int refcount;
	int ifindex;
	__u64 netns_cookie;
	int xsks_map_fd;
	struct list_head list;
	struct xdp_program *xdp_prog;
	int refcnt_map_fd;
	char ifname[IFNAMSIZ];
};
 */

#[repr(C)]
struct xsk_ctx {
	fill: *mut xsk_ring_prod,
	comp: *mut xsk_ring_cons,
	umem: *mut xsk_umem,
	queue_id: u32,
	refcount: i32,
	ifindex: i32,
	netns_cookie: u64,
	xsks_map_fd: i32,
	xdp_prog: *mut xdp_program,
	refcnt_map_fd: i32,
	ifname: [u8; IFNAMSIZ],
}

/*struct xsk_socket {
	struct xsk_ring_cons *rx;
	struct xsk_ring_prod *tx;
	struct xsk_ctx *ctx;
	struct xsk_socket_config config;
	int fd;
}; */

#[repr(C)]
pub struct xsk_socket {
	rx: *mut xsk_ring_cons,
	tx: *mut xsk_ring_prod,
	ctx: *mut xsk_ctx,
	config: xsk_socket_config,
	fd: i32,
}



unsafe fn xsk_create_ctx(
	xsk: *mut xsk_socket,
	umem: *mut xsk_umem,
	netns_cookie: u64,
	ifindex: i32,
	ifname: *const i8,
	queue_id: u32,
	fill: *mut xsk_ring_prod,
	comp: *mut xsk_ring_cons,
) -> *mut xsk_ctx {
	
	let ctx_ident = CtxIdent { netns_cookie, ifindex, queue_id };
	let mut active_ctx = ACTIVE_CTX.lock().unwrap();
	if active_ctx.contains(&ctx_ident) {
		return std::ptr::null_mut();
	}
	active_ctx.insert(ctx_ident);
	drop(active_ctx);
	let ctx = std::alloc::alloc(std::alloc::Layout::new::<xsk_ctx>()) as *mut xsk_ctx;
	if ctx.is_null() {
		return std::ptr::null_mut();
	}

	let mut err: i32;

	if (*umem).fill_save.is_null() {
		err = xsk_create_umem_rings(&mut *umem, (*xsk).fd, &mut *fill, &mut *comp);
		if err != 0 {
			std::alloc::dealloc(ctx as *mut u8, std::alloc::Layout::new::<xsk_ctx>());
			return std::ptr::null_mut();
		}
	} else if (*umem).fill_save != fill || (*umem).comp_save != comp {
		std::ptr::copy_nonoverlapping(fill, (*umem).fill_save, std::mem::size_of::<xsk_ring_prod>());
		std::ptr::copy_nonoverlapping(comp, (*umem).comp_save, std::mem::size_of::<xsk_ring_cons>());
	}

	(*ctx).netns_cookie = netns_cookie;
	(*ctx).ifindex = ifindex;
	(*ctx).refcount = 1;
	(*ctx).umem = umem;
	(*ctx).queue_id = queue_id;
	std::ptr::copy_nonoverlapping(ifname, (*ctx).ifname.as_mut_ptr() as _, IFNAMSIZ - 1);
	(*ctx).ifname[IFNAMSIZ - 1] = 0;

	(*ctx).fill = fill;
	(*ctx).comp = comp;
	//list_add(&ctx->list, &umem->ctx_list);
	return ctx;
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CtxIdent {
	netns_cookie: u64,
	ifindex: i32,
	queue_id: u32,
}

static ACTIVE_CTX: LazyLock<Mutex<HashSet<CtxIdent>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

const SO_NETNS_COOKIE: i32 = 71;

pub unsafe fn xsk_socket__create_opts(ifname: *const i8, queue_id: u32, umem: *mut xsk_umem, opts: *mut xsk_socket_opts) -> *mut xsk_socket {
	let mut rx_setup_done = false;
	let mut tx_setup_done = false;
	let mut rx_map: *mut c_void = std::ptr::null_mut();
	let mut tx_map: *mut c_void = std::ptr::null_mut();
	let mut sxdp: sockaddr_xdp = std::mem::zeroed();
	let mut off: xdp_mmap_offsets = std::mem::zeroed();
	let mut fill: *mut xsk_ring_prod;
	let mut comp: *mut xsk_ring_cons;
	let mut rx: *mut xsk_ring_cons;
	let mut tx: *mut xsk_ring_prod;
	let mut xsk: *mut xsk_socket;
	let mut ctx: *mut xsk_ctx;
	let mut err: i32;
	let mut ifindex: i32;
	let mut netns_cookie: u64 = 0;
	let mut optlen: u32;
	let mut unmap: bool;

	if !opts.is_null() {
		rx = (*opts).rx;
		tx = (*opts).tx;
		fill = (*opts).fill;
		comp = (*opts).comp;
	} else {
		return std::ptr::null_mut();
	}

	if umem.is_null() || (rx.is_null() && tx.is_null()) || (fill.is_null() ^ comp.is_null()) {
		return std::ptr::null_mut();
	}
	if fill.is_null() && comp.is_null() {
		fill = (*umem).fill_save;
		comp = (*umem).comp_save;
	}

	xsk = std::alloc::alloc(std::alloc::Layout::new::<xsk_socket>()) as *mut xsk_socket;
	if xsk.is_null() {
		return std::ptr::null_mut();
	}

	err = xsk_set_xdp_socket_config(&mut (*xsk).config, &*opts);
	if err != 0 {
		return std::ptr::null_mut();
	}

	ifindex = libc::if_nametoindex(ifname as *const i8) as i32;
	if ifindex == 0 {
		return std::ptr::null_mut();
	}

	if (*umem).refcount > 0 {
		(*xsk).fd = libc::socket(libc::AF_XDP, libc::SOCK_RAW, 0);
		if (*xsk).fd < 0 {
			return std::ptr::null_mut();
		}
	} else {
		(*xsk).fd = (*umem).fd;
		rx_setup_done = (*umem).rx_ring_setup_done;
		tx_setup_done = (*umem).tx_ring_setup_done;
	}

	optlen = std::mem::size_of::<u64>() as u32;
	err = libc::getsockopt((*xsk).fd, libc::SOL_SOCKET, SO_NETNS_COOKIE, &mut netns_cookie as *mut u64 as *mut c_void, &mut optlen);
	if err != 0 {
		if nix::errno::errno() != libc::ENOPROTOOPT {
			return std::ptr::null_mut();
		}
		netns_cookie = 1;
	}

	// ctx = xsk_get_ctx(umem, netns_cookie, ifindex, queue_id);
	// if ctx.is_null() {
	// 	if fill.is_null() || comp.is_null() {
	// 		return std::ptr::null_mut();
	// 	}
// 
	// 	ctx = xsk_create_ctx(xsk, umem, netns_cookie, ifindex, ifname, queue_id, fill, comp);
	// 	if ctx.is_null() {
	// 		return std::ptr::null_mut();
	// 	}
	// }

	ctx = xsk_create_ctx(xsk, umem, netns_cookie, ifindex, ifname, queue_id, fill, comp);

	(*xsk).ctx = ctx;

	if !rx.is_null() && !rx_setup_done {
		err = libc::setsockopt((*xsk).fd, libc::SOL_XDP, libc::XDP_RX_RING, &mut (*xsk).config.rx_size as *mut u32 as *mut c_void, std::mem::size_of::<u32>() as u32);
		if err != 0 {
			return std::ptr::null_mut();
		}
		if (*xsk).fd == (*umem).fd {
			(*umem).rx_ring_setup_done = true;
		}
	}

	if !tx.is_null() && !tx_setup_done {
		err = libc::setsockopt((*xsk).fd, libc::SOL_XDP, libc::XDP_TX_RING, &mut (*xsk).config.tx_size as *mut u32 as *mut c_void, std::mem::size_of::<u32>() as u32);
		if err != 0 {
			return std::ptr::null_mut();
		}
		if (*xsk).fd == (*umem).fd {
			(*umem).tx_ring_setup_done = true;
		}
	}

	err = xsk_get_mmap_offsets((*xsk).fd, &mut off);
	if err != 0 {
		return std::ptr::null_mut();
	}

	if !rx.is_null() {
		rx_map = libc::mmap(std::ptr::null_mut(), off.rx.desc as usize + (*xsk).config.rx_size as usize * std::mem::size_of::<xdp_desc>(), libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED | libc::MAP_POPULATE, (*xsk).fd, libc::XDP_PGOFF_RX_RING as i64);
		if rx_map == libc::MAP_FAILED {
			return std::ptr::null_mut();
		}

		(*rx).mask = (*xsk).config.rx_size - 1;
		(*rx).size = (*xsk).config.rx_size;
		(*rx).producer = (rx_map as *mut u8).offset(off.rx.producer as isize) as *mut u32;
		(*rx).consumer = (rx_map as *mut u8).offset(off.rx.consumer as isize) as *mut u32;
		(*rx).flags = (rx_map as *mut u8).offset(off.rx.flags as isize) as *mut u32;
		(*rx).ring = (rx_map as *mut u8).offset(off.rx.desc as isize) as *mut c_void;
		(*rx).cached_prod = *(*rx).producer;
		(*rx).cached_cons = *(*rx).consumer;
	}
	(*xsk).rx = rx;

	if !tx.is_null() {
		tx_map = libc::mmap(std::ptr::null_mut(), off.tx.desc as usize + (*xsk).config.tx_size as usize * std::mem::size_of::<xdp_desc>(), libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED | libc::MAP_POPULATE, (*xsk).fd, libc::XDP_PGOFF_TX_RING as i64);
		if tx_map == libc::MAP_FAILED {
			return std::ptr::null_mut();
		}

		(*tx).mask = (*xsk).config.tx_size - 1;
		(*tx).size = (*xsk).config.tx_size;
		(*tx).producer = (tx_map as *mut u8).offset(off.tx.producer as isize) as *mut u32;
		(*tx).consumer = (tx_map as *mut u8).offset(off.tx.consumer as isize) as *mut u32;
		(*tx).flags = (tx_map as *mut u8).offset(off.tx.flags as isize) as *mut u32;
		(*tx).ring = (tx_map as *mut u8).offset(off.tx.desc as isize) as *mut c_void;
		(*tx).cached_prod = *(*tx).producer;
		(*tx).cached_cons = *(*tx).consumer + (*xsk).config.tx_size;
	}
	(*xsk).tx = tx;

	sxdp.sxdp_family = libc::PF_XDP as u16;

	sxdp.sxdp_ifindex = (*ctx).ifindex as u32;
	sxdp.sxdp_queue_id = (*ctx).queue_id;

	if (*umem).refcount > 1 {
		sxdp.sxdp_flags |= libc::XDP_SHARED_UMEM;
		sxdp.sxdp_shared_umem_fd = (*umem).fd as u32;
	} else {
		sxdp.sxdp_flags = (*xsk).config.bind_flags;
	}

	err = libc::bind((*xsk).fd, &sxdp as *const sockaddr_xdp as *const sockaddr, std::mem::size_of::<sockaddr_xdp>() as u32);

	if err != 0 {
		return std::ptr::null_mut();
	}

	if ((*xsk).config.__bindgen_anon_1.libxdp_flags & XSK_LIBXDP_FLAGS__INHIBIT_PROG_LOAD) != 0 {
		err = __xsk_setup_xdp_prog(xsk, std::ptr::null_mut());
		if err != 0 {
			return std::ptr::null_mut();
		}
	}

	(*umem).fill_save = std::ptr::null_mut();
	(*umem).comp_save = std::ptr::null_mut();

	return xsk;
}




unsafe fn xsk_socket__create_shared(
	xsk_ptr: *mut *mut xsk_socket,
	ifname: *const i8,
	queue_id: u32,
	umem: *mut xsk_umem,
	rx: *mut xsk_ring_cons,
	tx: *mut xsk_ring_prod,
	fill: *mut xsk_ring_prod,
	comp: *mut xsk_ring_cons,
	usr_config: *const xsk_socket_config,
) -> i32 {
	let mut xsk: *mut xsk_socket;

	if xsk_ptr.is_null() {
		return -1;
	}

	let mut opts = xsk_socket_opts {
		sz: std::mem::size_of::<xsk_socket_opts>(),
		rx,
		tx,
		fill,
		comp,
		rx_size: 0,
		tx_size: 0,
		libxdp_flags: 0,
		xdp_flags: 0,
		bind_flags: 0,
	};

	if !usr_config.is_null() {
		opts.rx_size = (*usr_config).rx_size;
		opts.tx_size = (*usr_config).tx_size;
		opts.libxdp_flags = (*usr_config).__bindgen_anon_1.libxdp_flags;
		opts.xdp_flags = (*usr_config).xdp_flags;
		opts.bind_flags = (*usr_config).bind_flags;
	}

	xsk = xsk_socket__create_opts(ifname, queue_id, umem, &mut opts);
	if xsk.is_null() {
		return nix::errno::errno();
	}

	*xsk_ptr = xsk;
	return 0;
}

pub unsafe fn xsk_socket__create(
	xsk_ptr: *mut *mut xsk_socket,
	ifname: *const i8,
	queue_id: u32,
	umem: *mut xsk_umem,
	rx: *mut xsk_ring_cons,
	tx: *mut xsk_ring_prod,
	usr_config: *const xsk_socket_config,
) -> i32 {
	if umem.is_null() {
		return -1;
	}

	return xsk_socket__create_shared(xsk_ptr, ifname, queue_id, umem, rx, tx, (*umem).fill_save, (*umem).comp_save, usr_config);
}