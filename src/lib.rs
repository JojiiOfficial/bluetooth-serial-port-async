//! Interact with Bluetooth devices via RFCOMM channels.
#![deny(
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_numeric_casts,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

mod ffi;

mod hci;
mod sdp;
pub mod socket;

pub use self::{
    hci::scan_devices,
    socket::{BtAddr, BtProtocol, BtSocket, BtSocketConnect},
};
