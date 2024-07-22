//! Interact with Bluetooth devices via RFCOMM channels.
#![deny(
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_numeric_casts,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

mod bluetooth;
pub use crate::bluetooth::*;

#[cfg(all(feature = "async_std", feature = "tokio"))]
compile_error!("feature \"async_std\" and feature \"tokio\" cannot be enabled at the same time");

// ////////////////////////////////////
// Linux implementation of functions
#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
#[allow(unused_variables)] // TODO: remove warnings
mod windows;

mod platform {

    #[cfg(target_os = "linux")]
    pub use crate::linux::*;

    #[cfg(target_os = "windows")]
    pub use crate::windows::*;
}

/// OS-specific functionality
pub mod os {
    /// Linux-specific definitions
    #[cfg(target_os = "linux")]
    pub mod linux {
        pub use crate::linux::{BtSocket, BtSocketConnect};
    }
}
