
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(non_upper_case_globals)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(improper_ctypes)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub const NIOCTXSYNC: u64 = uapi::_IO('i' as _, 148_u64);
/// Sync rx queues
pub const NIOCRXSYNC: u64 = uapi::_IO('i' as _, 149_u64);



//include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
//
////! Constants for the bindings with the netmap C library.
////!
////!
////! # NIOCTXSYNC and NIOCRXSYNC
////! The ioctl commands to sync TX/RX netmap rings.
////!
////! NIOCTXSYNC, NIOCRXSYNC synchronize tx or rx queues,
////! whose identity is set in NETMAP_REQ_REGISTER through nr_ringid.
////! These are non blocking and take no argument.
//
///// Sync tx queues
//pub const NIOCTXSYNC: u64 = uapi::_IO('i' as _, 148_u64);
///// Sync rx queues
//pub const NIOCRXSYNC: u64 = uapi::_IO('i' as _, 149_u64);
