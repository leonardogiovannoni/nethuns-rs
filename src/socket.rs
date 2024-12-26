use crate::{netmap::PkthdrTrait, types::SocketOptions};

struct NethunsRing;

/*
struct nethuns_socket_base
{
    struct nethuns_socket_options opt;
    struct nethuns_ring           tx_ring;
    struct nethuns_ring           rx_ring;
    char                         *devname;
    int                           queue;
    int 		                  ifindex;

    nethuns_filter_t              filter;
    void *                        filter_ctx;
};

*/

enum Filter<P: PkthdrTrait> {
    Closure(Box<dyn Fn(&P, &[u8]) -> bool>),
    Function(fn(&P, &[u8]) -> bool),
}

struct InnerSocker<P: PkthdrTrait> {
    opt: SocketOptions,
    tx_ring: NethunsRing,
    rx_ring: NethunsRing,
    devname: String,
    queue: i32,
    ifindex: i32,
    filter: Filter<P>,
}
