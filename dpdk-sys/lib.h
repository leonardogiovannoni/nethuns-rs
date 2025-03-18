#include <rte_l2tpv2.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <unistd.h>
#include <rte_eal.h>
#include <rte_ethdev.h>
#include <rte_mbuf.h>
#include <rte_ether.h>

const size_t RTE_ETHER_ADDR_SIZE = sizeof(struct rte_ether_addr);
const size_t RTE_ARP_IPV4_SIZE = sizeof(struct rte_arp_ipv4);
const size_t RTE_ARP_HDR_SIZE = sizeof(struct rte_arp_hdr);
const size_t RTE_L2TPV2_COMBINED_MSG_HDR_SIZE = sizeof(struct rte_l2tpv2_combined_msg_hdr);

// rte_ethdev

int rust_rte_gettid(void)
{
    return rte_gettid();
}

uint64_t rust_rte_eth_rss_hf_refine(uint64_t rss_hf)
{
    return rte_eth_rss_hf_refine(rss_hf);
}

uint16_t rust_rte_eth_rx_burst(uint16_t port_id, uint16_t queue_id, struct rte_mbuf **rx_pkts, const uint16_t nb_pkts)
{
    return rte_eth_rx_burst(port_id, queue_id, rx_pkts, nb_pkts);
}

int rust_rte_eth_rx_queue_count(uint16_t port_id, uint16_t queue_id)
{
    return rte_eth_rx_queue_count(port_id, queue_id);
}

int rust_rte_eth_rx_descriptor_status(uint16_t port_id, uint16_t queue_id, uint16_t offset)
{
    return rte_eth_rx_descriptor_status(port_id, queue_id, offset);
}

int rust_rte_eth_tx_descriptor_status(uint16_t port_id, uint16_t queue_id, uint16_t offset)
{
    return rte_eth_tx_descriptor_status(port_id, queue_id, offset);
}
uint16_t rust_rte_eth_tx_burst(uint16_t port_id, uint16_t queue_id, struct rte_mbuf **tx_pkts, uint16_t nb_pkts)
{
    return rte_eth_tx_burst(port_id, queue_id, tx_pkts, nb_pkts);
}

uint16_t rust_rte_eth_tx_prepare(uint16_t port_id, uint16_t queue_id, struct rte_mbuf **tx_pkts, uint16_t nb_pkts)
{
    return rte_eth_tx_prepare(port_id, queue_id, tx_pkts, nb_pkts);
}

uint16_t rust_rte_eth_tx_buffer_flush(uint16_t port_id, uint16_t queue_id, struct rte_eth_dev_tx_buffer *buffer)
{
    return rte_eth_tx_buffer_flush(port_id, queue_id, buffer);
}

uint16_t rust_rte_eth_recycle_mbufs(uint16_t rx_port_id, uint16_t rx_queue_id, uint16_t tx_port_id, uint16_t tx_queue_id, struct rte_eth_recycle_rxq_info *recycle_rxq_info)
{
    return rte_eth_recycle_mbufs(rx_port_id, rx_queue_id, tx_port_id, tx_queue_id, recycle_rxq_info);
}

int rust_rte_eth_tx_queue_count(uint16_t port_id, uint16_t queue_id)
{
    return rte_eth_tx_queue_count(port_id, queue_id);
}

// rte_mbuf

void rust_rte_mbuf_prefetch_part1(struct rte_mbuf *m)
{
    rte_mbuf_prefetch_part1(m);
}

void rust_rte_mbuf_prefetch_part2(struct rte_mbuf *m)
{
    rte_mbuf_prefetch_part2(m);
}

uint16_t
rust_rte_pktmbuf_priv_size(struct rte_mempool *mp)
{
    return rte_pktmbuf_priv_size(mp);
}

rte_iova_t
rust_rte_mbuf_iova_get(const struct rte_mbuf *m)
{
    return rte_mbuf_iova_get(m);
}

void rust_rte_mbuf_iova_set(struct rte_mbuf *m, rte_iova_t iova)
{
    rte_mbuf_iova_set(m, iova);
}

rte_iova_t
rust_rte_mbuf_data_iova(const struct rte_mbuf *m)
{
    return rte_mbuf_data_iova(m);
}

rte_iova_t
rust_rte_mbuf_data_iova_default(const struct rte_mbuf *m)
{
    return rte_mbuf_data_iova_default(m);
}

struct rte_mbuf *
rust_rte_mbuf_from_indirect(struct rte_mbuf *mi)
{
    return rte_mbuf_from_indirect(mi);
}

char *
rust_rte_mbuf_buf_addr(struct rte_mbuf *mb, struct rte_mempool *mp)
{
    return rte_mbuf_buf_addr(mb, mp);
}

char *
rust_rte_mbuf_data_addr_default(struct rte_mbuf *mb)
{
    return rte_mbuf_data_addr_default(mb);
}

char *
rust_rte_mbuf_to_baddr(struct rte_mbuf *md)
{
    return rte_mbuf_to_baddr(md);
}

void *
rust_rte_mbuf_to_priv(struct rte_mbuf *m)
{
    return rte_mbuf_to_priv(m);
}

uint32_t
rust_rte_pktmbuf_priv_flags(struct rte_mempool *mp)
{
    return rte_pktmbuf_priv_flags(mp);
}

uint16_t
rust_rte_mbuf_refcnt_read(const struct rte_mbuf *m)
{
    return rte_mbuf_refcnt_read(m);
}

void rust_rte_mbuf_refcnt_set(struct rte_mbuf *m, uint16_t new_value)
{
    rte_mbuf_refcnt_set(m, new_value);
}

uint16_t
rust___rte_mbuf_refcnt_update(struct rte_mbuf *m, int16_t value)
{
    return __rte_mbuf_refcnt_update(m, value);
}

uint16_t
rust_rte_mbuf_refcnt_update(struct rte_mbuf *m, int16_t value)
{
    return rte_mbuf_refcnt_update(m, value);
}

uint16_t
rust_rte_mbuf_ext_refcnt_read(const struct rte_mbuf_ext_shared_info *shinfo)
{
    return rte_mbuf_ext_refcnt_read(shinfo);
}

void rust_rte_mbuf_ext_refcnt_set(struct rte_mbuf_ext_shared_info *shinfo, uint16_t new_value)
{
    rte_mbuf_ext_refcnt_set(shinfo, new_value);
}

uint16_t
rust_rte_mbuf_ext_refcnt_update(struct rte_mbuf_ext_shared_info *shinfo, int16_t value)
{
    return rte_mbuf_ext_refcnt_update(shinfo, value);
}

void rust___rte_mbuf_raw_sanity_check(const struct rte_mbuf *m)
{
    __rte_mbuf_raw_sanity_check(m);
}

struct rte_mbuf *
rust_rte_mbuf_raw_alloc(struct rte_mempool *mp)
{
    return rte_mbuf_raw_alloc(mp);
}

void rust_rte_mbuf_raw_free(struct rte_mbuf *m)
{
    rte_mbuf_raw_free(m);
}

struct rte_mbuf *
rust_rte_pktmbuf_alloc(struct rte_mempool *mp)
{
    return rte_pktmbuf_alloc(mp);
}

int rust_rte_pktmbuf_alloc_bulk(struct rte_mempool *pool, struct rte_mbuf **mbufs, unsigned count)
{
    return rte_pktmbuf_alloc_bulk(pool, mbufs, count);
}

struct rte_mbuf_ext_shared_info *
rust_rte_pktmbuf_ext_shinfo_init_helper(void *buf_addr, uint16_t *buf_len,
                                        rte_mbuf_extbuf_free_callback_t free_cb,
                                        void *fcb_opaque)
{
    return rte_pktmbuf_ext_shinfo_init_helper(buf_addr, buf_len, free_cb, fcb_opaque);
}

void rust_rte_pktmbuf_attach_extbuf(struct rte_mbuf *m, void *buf_addr, rte_iova_t buf_iova,
                                    uint16_t buf_len, struct rte_mbuf_ext_shared_info *shinfo)
{
    rte_pktmbuf_attach_extbuf(m, buf_addr, buf_iova, buf_len, shinfo);
}

void rust_rte_mbuf_dynfield_copy(struct rte_mbuf *mdst, const struct rte_mbuf *msrc)
{
    rte_mbuf_dynfield_copy(mdst, msrc);
}

void rust___rte_pktmbuf_copy_hdr(struct rte_mbuf *mdst, const struct rte_mbuf *msrc)
{
    __rte_pktmbuf_copy_hdr(mdst, msrc);
}

void rust_rte_pktmbuf_attach(struct rte_mbuf *mi, struct rte_mbuf *m)
{
    rte_pktmbuf_attach(mi, m);
}

void rust___rte_pktmbuf_free_extbuf(struct rte_mbuf *m)
{
    __rte_pktmbuf_free_extbuf(m);
}

void rust___rte_pktmbuf_free_direct(struct rte_mbuf *m)
{
    __rte_pktmbuf_free_direct(m);
}

void rust_rte_pktmbuf_detach(struct rte_mbuf *m)
{
    rte_pktmbuf_detach(m);
}

int rust___rte_pktmbuf_pinned_extbuf_decref(struct rte_mbuf *m)
{
    return __rte_pktmbuf_pinned_extbuf_decref(m);
}

struct rte_mbuf *
rust_rte_pktmbuf_prefree_seg(struct rte_mbuf *m)
{
    return rte_pktmbuf_prefree_seg(m);
}

void rust_rte_pktmbuf_free_seg(struct rte_mbuf *m)
{
    rte_pktmbuf_free_seg(m);
}

void rust_rte_pktmbuf_free(struct rte_mbuf *m)
{
    rte_pktmbuf_free(m);
}

void rust_rte_pktmbuf_refcnt_update(struct rte_mbuf *m, int16_t v)
{
    rte_pktmbuf_refcnt_update(m, v);
}

uint16_t
rust_rte_pktmbuf_headroom(const struct rte_mbuf *m)
{
    return rte_pktmbuf_headroom(m);
}

uint16_t
rust_rte_pktmbuf_tailroom(const struct rte_mbuf *m)
{
    return rte_pktmbuf_tailroom(m);
}

struct rte_mbuf *
rust_rte_pktmbuf_lastseg(struct rte_mbuf *m)
{
    return rte_pktmbuf_lastseg(m);
}

char *
rust_rte_pktmbuf_prepend(struct rte_mbuf *m, uint16_t len)
{
    return rte_pktmbuf_prepend(m, len);
}

char *
rust_rte_pktmbuf_append(struct rte_mbuf *m, uint16_t len)
{
    return rte_pktmbuf_append(m, len);
}

char *
rust_rte_pktmbuf_adj(struct rte_mbuf *m, uint16_t len)
{
    return rte_pktmbuf_adj(m, len);
}

int rust_rte_pktmbuf_trim(struct rte_mbuf *m, uint16_t len)
{
    return rte_pktmbuf_trim(m, len);
}

int rust_rte_pktmbuf_is_contiguous(const struct rte_mbuf *m)
{
    return rte_pktmbuf_is_contiguous(m);
}

const void *
rust_rte_pktmbuf_read(const struct rte_mbuf *m, uint32_t off, uint32_t len, void *buf)
{
    return rte_pktmbuf_read(m, off, len, buf);
}

int rust_rte_pktmbuf_chain(struct rte_mbuf *head, struct rte_mbuf *tail)
{
    return rte_pktmbuf_chain(head, tail);
}

uint64_t
rust_rte_mbuf_tx_offload(uint64_t il2, uint64_t il3, uint64_t il4, uint64_t tso,
                         uint64_t ol3, uint64_t ol2, uint64_t unused)
{
    return rte_mbuf_tx_offload(il2, il3, il4, tso, ol3, ol2, unused);
}

int rust_rte_validate_tx_offload(const struct rte_mbuf *m)
{
    return rte_validate_tx_offload(m);
}

int rust_rte_pktmbuf_linearize(struct rte_mbuf *mbuf)
{
    return rte_pktmbuf_linearize(mbuf);
}

uint32_t
rust_rte_mbuf_sched_queue_get(const struct rte_mbuf *m)
{
    return rte_mbuf_sched_queue_get(m);
}

uint8_t
rust_rte_mbuf_sched_traffic_class_get(const struct rte_mbuf *m)
{
    return rte_mbuf_sched_traffic_class_get(m);
}

uint8_t
rust_rte_mbuf_sched_color_get(const struct rte_mbuf *m)
{
    return rte_mbuf_sched_color_get(m);
}

void rust_rte_mbuf_sched_get(const struct rte_mbuf *m, uint32_t *queue_id,
                             uint8_t *traffic_class, uint8_t *color)
{
    rte_mbuf_sched_get(m, queue_id, traffic_class, color);
}

void rust_rte_mbuf_sched_queue_set(struct rte_mbuf *m, uint32_t queue_id)
{
    rte_mbuf_sched_queue_set(m, queue_id);
}

void rust_rte_mbuf_sched_traffic_class_set(struct rte_mbuf *m, uint8_t traffic_class)
{
    rte_mbuf_sched_traffic_class_set(m, traffic_class);
}

void rust_rte_mbuf_sched_color_set(struct rte_mbuf *m, uint8_t color)
{
    rte_mbuf_sched_color_set(m, color);
}

void rust_rte_mbuf_sched_set(struct rte_mbuf *m, uint32_t queue_id,
                             uint8_t traffic_class, uint8_t color)
{
    rte_mbuf_sched_set(m, queue_id, traffic_class, color);
}

// rte_ether

int rust_rte_is_same_ether_addr(const struct rte_ether_addr *ea1,
                                const struct rte_ether_addr *ea2)
{
    return rte_is_same_ether_addr(ea1, ea2);
}

int rust_rte_is_zero_ether_addr(const struct rte_ether_addr *ea)
{
    return rte_is_zero_ether_addr(ea);
}

int rust_rte_is_unicast_ether_addr(const struct rte_ether_addr *ea)
{
    return rte_is_unicast_ether_addr(ea);
}

int rust_rte_is_multicast_ether_addr(const struct rte_ether_addr *ea)
{
    return rte_is_multicast_ether_addr(ea);
}

int rust_rte_is_broadcast_ether_addr(const struct rte_ether_addr *ea)
{
    return rte_is_broadcast_ether_addr(ea);
}

int rust_rte_is_universal_ether_addr(const struct rte_ether_addr *ea)
{
    return rte_is_universal_ether_addr(ea);
}

int rust_rte_is_local_admin_ether_addr(const struct rte_ether_addr *ea)
{
    return rte_is_local_admin_ether_addr(ea);
}

int rust_rte_is_valid_assigned_ether_addr(const struct rte_ether_addr *ea)
{
    return rte_is_valid_assigned_ether_addr(ea);
}

void rust_rte_ether_addr_copy(const struct rte_ether_addr *ea_from,
                              struct rte_ether_addr *ea_to)
{
    rte_ether_addr_copy(ea_from, ea_to);
}

int rust_rte_vlan_strip(struct rte_mbuf *m)
{
    return rte_vlan_strip(m);
}

int rust_rte_vlan_insert(struct rte_mbuf **m)
{
    return rte_vlan_insert(m);
}

void *rust_rte_pktmbuf_mtod(struct rte_mbuf *m)
{
    return rte_pktmbuf_mtod(m, void *);
}
