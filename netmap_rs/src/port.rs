use std::ffi::CStr;
use std::ffi::CString;

use anyhow::Result;
use anyhow::bail;
use netmap_sys::nmport_close;
use netmap_sys::nmport_d;
use netmap_sys::nmport_open;
use netmap_sys::nmport_open_desc;
use netmap_sys::nmport_prepare;

use crate::context::Receiver;
use crate::context::Transmitter;
/*
pub struct Port {
    nmport_d: *mut nmport_d,
}

impl Port {
    pub fn chan(&mut self) -> (Transmitter<'_>, Receiver<'_>) {
        let me: *mut Self = self;
        let tx = Transmitter::new(me);
        let rx = Receiver::new(me);
        (tx, rx)
    }

    pub fn open(portspec: &str) -> Result<Self> {
        // TODO: portspec should be owned?
        let portspec = CString::new(portspec)?;
        let nmport_d = unsafe { nmport_open(portspec.as_ptr()) };
        if nmport_d.is_null() {
            bail!("Failed to prepare port");
        } else {
            Ok(Self { nmport_d })
        }
    }

    pub fn interface(&self) -> Interface<'_> {
        unsafe {
            Interface {
                inner: (*self.nmport_d).nifp,
                _phantom: std::marker::PhantomData,
            }
        }
    }

    pub fn rx_sync(&self) {
        if unsafe {
            libc::ioctl((*self.nmport_d).fd, netmap_sys::constants::NIOCRXSYNC)
        } < 0 {
            panic!("Failed to sync RX");
        }
    }

    pub fn tx_sync(&self) {
        if unsafe {
            libc::ioctl((*self.nmport_d).fd, netmap_sys::constants::NIOCTXSYNC)
        } < 0 {
            panic!("Failed to sync TX");
        }
    }
}

impl Drop for Port {
    fn drop(&mut self) {
        unsafe { nmport_close(self.nmport_d) };
    }
}
*/
// unsafe impl Send for NmPortDescriptor {}
