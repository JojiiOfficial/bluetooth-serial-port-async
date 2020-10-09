use super::{ffi::*, socket::create_error_from_last};

use crate::bluetooth::{BtAddr, BtDevice, BtError};

use libc::close;
use std::{
    ffi::CStr,
    mem,
    os::raw::*,
    os::unix::{
        io::{AsRawFd, FromRawFd, IntoRawFd},
        net::UnixStream,
    },
    ptr, time, vec,
};

#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct InquiryInfo {
    pub bdaddr: BtAddr,
    pub pscan_rep_mode: uint8_t,
    pub pscan_period_mode: uint8_t,
    pub pscan_mode: uint8_t,
    pub dev_class: [uint8_t; 3usize],
    pub clock_offset: uint16_t,
}

impl Default for InquiryInfo {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

const IREQ_CACHE_FLUSH: c_long = 1;

// BlueZ funcitons
#[cfg(target_os = "linux")]
#[link(name = "bluetooth")]
extern "C" {
    fn hci_get_route(addr: *mut BtAddr) -> c_int /* device_id */;
    fn hci_open_dev(device_id: c_int) -> c_int /* socket to local bluetooth adapter */;

    // The inquiry last at most for "1.28 * timout" seconds
    fn hci_inquiry(
        device_id: c_int,
        timeout: c_int,
        max_rsp: c_int,
        lap: *const u8,
        inquiry_info: *mut *mut InquiryInfo,
        flags: c_long,
    ) -> c_int;

    fn hci_read_remote_name(
        socket: c_int,
        addr: *const BtAddr,
        max_len: c_int,
        name: *mut c_char,
        timeout_ms: c_int,
    ) -> c_int;
}

pub fn scan_devices(timeout: time::Duration) -> Result<Vec<BtDevice>, BtError> {
    let device_id = unsafe { hci_get_route(ptr::null_mut()) };
    if device_id < 0 {
        return Err(create_error_from_last(
            "hci_get_route(): No local bluetooth adapter found",
        ));
    }

    let local_socket = unsafe { hci_open_dev(device_id) };
    if local_socket < 0 {
        return Err(create_error_from_last(
            "hci_open_dev(): Opening local bluetooth adapter failed",
        ));
    }

    let local_socket = unsafe { UnixStream::from_raw_fd(local_socket) };

    let mut inquiry_infos = vec::from_elem(InquiryInfo::default(), 256);

    let timeout_secs = timeout.as_secs();
    let max_secs = u64::from(u32::max_value());
    let timeout_secs = if timeout_secs > max_secs {
        return Err(BtError::Desc(format!(
            "Timeout value too big {} > {}",
            timeout_secs, max_secs
        )));
    } else {
        timeout_secs as u32
    };

    let timeout = ((f64::from(timeout_secs) + f64::from(timeout.subsec_nanos()) / 1_000_000_000.)
        / 1.28)
        .round();
    let timeout = timeout.min(f64::from(c_int::max_value())).max(1.) as c_int;
    let flags = IREQ_CACHE_FLUSH;

    let mut inquiry_info = inquiry_infos.as_mut_ptr();
    let number_responses = unsafe {
        hci_inquiry(
            device_id,
            timeout,
            inquiry_infos.len() as c_int,
            ptr::null(),
            &mut inquiry_info,
            flags,
        )
    };
    if number_responses < 0 {
        return Err(create_error_from_last(
            "hci_inquiry(): Scanning remote bluetooth devices failed",
        ));
    }

    inquiry_infos.truncate(number_responses as usize);

    let mut devices = Vec::with_capacity(inquiry_infos.len());
    for inquiry_info in &inquiry_infos {
        let mut cname = [0; 256];
        let name = if unsafe {
            hci_read_remote_name(
                local_socket.as_raw_fd(),
                &inquiry_info.bdaddr,
                cname.len() as c_int,
                &mut cname[0],
                0,
            )
        } < 0
        {
            "[unknown]".to_string()
        } else {
            unsafe { CStr::from_ptr(&cname[0]) }
                .to_string_lossy()
                .into_owned()
        };

        devices.push(BtDevice {
            name,
            addr: inquiry_info.bdaddr.convert_host_byteorder(),
        })
    }

    let local_socket = local_socket.into_raw_fd();
    if unsafe { close(local_socket) } < 0 {
        return Err(create_error_from_last("close()"));
    }

    Ok(devices)
}
