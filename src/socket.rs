use super::sdp::{QueryRFCOMMChannel, QueryRFCOMMChannelStatus};
use async_std::os::unix::net::UnixStream;
use std::os::unix::io::{FromRawFd, RawFd};

use mio::{unix::EventedFd, Poll, Ready};

use std::error::Error;
use std::mem;
use std::os::unix::net::UnixStream as StdUnixStream;
use std::str;
use std::time;

/// Create an error
pub fn create_error_from_errno(message: &str, errno: i32) -> BtError {
    let nix_error = nix::Error::from_errno(nix::errno::from_i32(errno));
    BtError::Errno(
        errno as u32,
        format!("{:}: {:}", message, nix_error.to_string()),
    )
}

/// Create error from last
pub fn create_error_from_last(message: &str) -> BtError {
    create_error_from_errno(message, nix::errno::errno())
}

const AF_BLUETOOTH: i32 = 31;

const BTPROTO_L2CAP: isize = 0;
const BTPROTO_HCI: isize = 1;
const BTPROTO_SCO: isize = 2;
const BTPROTO_RFCOMM: isize = 3;
const BTPROTO_BNEP: isize = 4;
const BTPROTO_CMTP: isize = 5;
const BTPROTO_HIDP: isize = 6;
const BTPROTO_AVDTP: isize = 7;

#[allow(dead_code)]
enum BtProtocolBlueZ {
    L2CAP = BTPROTO_L2CAP,
    HCI = BTPROTO_HCI,
    SCO = BTPROTO_SCO,
    RFCOMM = BTPROTO_RFCOMM,
    BNEP = BTPROTO_BNEP,
    CMTP = BTPROTO_CMTP,
    HIDP = BTPROTO_HIDP,
    AVDTP = BTPROTO_AVDTP,
}

#[repr(C)]
#[derive(Copy, Debug, Clone)]
struct sockaddr_rc {
    rc_family: libc::sa_family_t,
    rc_bdaddr: BtAddr,
    rc_channel: u8,
}

/// Linux (Bluez) socket, created with AF_BLUETOOTH
#[derive(Debug)]
pub struct BtSocket {
    /// lol
    stream: StdUnixStream,
    fd: i32,
}

impl BtSocket {
    pub fn new(proto: BtProtocol) -> Result<BtSocket, BtError> {
        match proto {
            BtProtocol::RFCOMM => {
                let fd = unsafe {
                    libc::socket(
                        AF_BLUETOOTH,
                        libc::SOCK_STREAM,
                        BtProtocolBlueZ::RFCOMM as i32,
                    )
                };
                if fd < 0 {
                    Err(create_error_from_last("Failed to create Bluetooth socket"))
                } else {
                    Ok(BtSocket {
                        stream: unsafe { StdUnixStream::from_raw_fd(fd) },
                        fd,
                    })
                }
            }
        }
    }
    /// Initiate connection
    pub fn connect(&mut self, addr: &BtAddr) -> Result<BtSocketConnect, BtError> {
        let addr = addr.convert_host_byteorder();

        // Create temporary `mio` event loop
        let evtloop = mio::Poll::new().unwrap();
        let token = mio::Token(0);
        let mut events = mio::Events::with_capacity(2);

        let mut connect = BtSocketConnect::new(self, addr);
        loop {
            match connect.advance()? {
                BtAsync::WaitFor(evented, interest) => {
                    let mut event_received = false;
                    while !event_received {
                        // Register this, single, event source
                        evtloop
                            .register(evented, token, interest, mio::PollOpt::oneshot())
                            .unwrap();

                        // Wait for it to transition to the requested state
                        evtloop.poll(&mut events, None).unwrap();

                        for event in events.iter() {
                            if event.token() == token {
                                event_received = true;
                                evtloop.deregister(evented).unwrap();
                            }
                        }
                    }
                }

                BtAsync::Done => {
                    return Ok(connect);
                }
            }
        }
    }

    pub fn get_fd(&self) -> i32 {
        self.fd
    }

    pub fn get_stream(&self) -> UnixStream {
        let stream: UnixStream = unsafe { UnixStream::from_raw_fd(self.fd) };
        stream
    }
}

impl From<nix::Error> for BtError {
    fn from(e: nix::Error) -> BtError {
        BtError::Errno(e.as_errno().map(|x| x as u32).unwrap_or(0), e.to_string())
    }
}

impl mio::Evented for BtSocket {
    fn register(
        &self,
        poll: &Poll,
        token: mio::Token,
        interest: Ready,
        opts: mio::PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.get_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: mio::Token,
        interest: Ready,
        opts: mio::PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.get_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> std::io::Result<()> {
        EventedFd(&self.get_fd()).deregister(poll)
    }
}

#[derive(Debug)]
enum BtSocketConnectState {
    SDPSearch,
    Connect,
    Done,
}

/// Manages the bluetooth connection process when used from an asynchronous client.
#[derive(Debug)]
pub struct BtSocketConnect<'a> {
    addr: BtAddr,
    pollfd: RawFd,
    state: BtSocketConnectState,
    socket: &'a mut BtSocket,
    query: QueryRFCOMMChannel,
}
impl<'a> BtSocketConnect<'a> {
    fn new(socket: &'a mut BtSocket, addr: BtAddr) -> Self {
        BtSocketConnect {
            addr,
            pollfd: 0,
            query: QueryRFCOMMChannel::new(addr),
            socket,
            state: BtSocketConnectState::SDPSearch,
        }
    }
    /// Advance the connection process to the next state
    pub fn advance(&mut self) -> Result<BtAsync, BtError> {
        match self.state {
            BtSocketConnectState::SDPSearch => {
                match self.query.advance()? {
                    // Forward SDP's pleas for another round
                    QueryRFCOMMChannelStatus::WaitReadable(fd) => {
                        self.pollfd = fd;
                        Ok(BtAsync::WaitFor(self, Ready::readable()))
                    }

                    QueryRFCOMMChannelStatus::WaitWritable(fd) => {
                        self.pollfd = fd;
                        Ok(BtAsync::WaitFor(self, Ready::writable()))
                    }

                    // Received channel number, start actual connection
                    QueryRFCOMMChannelStatus::Done(channel) => {
                        let full_address = sockaddr_rc {
                            rc_family: AF_BLUETOOTH as u16,
                            rc_bdaddr: self.addr,
                            rc_channel: channel,
                        };

                        self.pollfd = self.socket.get_fd();

                        if unsafe {
                            libc::connect(
                                self.pollfd,
                                &full_address as *const sockaddr_rc as *const libc::sockaddr,
                                mem::size_of::<sockaddr_rc>() as u32,
                            )
                        } < 0
                        {
                            Err(create_error_from_last(
                                "Failed to connect() to target device",
                            ))
                        } else {
                            self.state = BtSocketConnectState::Connect;
                            Ok(BtAsync::WaitFor(self, Ready::writable()))
                        }
                    }
                }
            }

            BtSocketConnectState::Connect => {
                // First check if socket is actually connected using `getpeername()`
                let mut full_address = sockaddr_rc {
                    rc_family: AF_BLUETOOTH as u16,
                    rc_bdaddr: BtAddr::any(),
                    rc_channel: 0,
                };
                let mut socklen = mem::size_of::<sockaddr_rc>() as libc::socklen_t;
                if unsafe {
                    libc::getpeername(
                        self.pollfd,
                        &mut full_address as *mut sockaddr_rc as *mut libc::sockaddr,
                        &mut socklen,
                    )
                } < 0
                {
                    if nix::errno::Errno::last() == nix::errno::Errno::ENOTCONN {
                        // Connection has failed – obtain actual error code using `read()`
                        let mut buf = [0u8; 1];
                        nix::unistd::read(self.pollfd, &mut buf).unwrap_err();
                        Err(create_error_from_last(
                            "Failed to connect() to target device",
                        ))
                    } else {
                        // Some unexpected error
                        Err(create_error_from_last("getpeername() failed"))
                    }
                } else {
                    self.state = BtSocketConnectState::Done;
                    Ok(BtAsync::Done)
                }
            }

            BtSocketConnectState::Done => {
                panic!("Trying advance `BtSocketConnect` from `Done` state");
            }
        }
    }
}

impl<'a> mio::Evented for BtSocketConnect<'a> {
    fn register(
        &self,
        poll: &Poll,
        token: mio::Token,
        interest: Ready,
        opts: mio::PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.pollfd).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: mio::Token,
        interest: Ready,
        opts: mio::PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.pollfd).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> std::io::Result<()> {
        EventedFd(&self.pollfd).deregister(poll)
    }
}

/// A 6-byte long MAC address.
#[repr(C, packed)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BtAddr(pub [u8; 6]);

impl std::fmt::Debug for BtAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

impl BtAddr {
    /// Returns the MAC address `00:00:00:00:00:00`
    pub fn any() -> BtAddr {
        BtAddr([0, 0, 0, 0, 0, 0])
    }

    /// Linux lower-layers actually hold the address in native byte-order
    /// althrough they are always displayed in network byte-order
    #[doc(hidden)]
    #[inline(always)]
    #[cfg(target_endian = "little")]
    pub fn convert_host_byteorder(mut self) -> BtAddr {
        {
            let (value_1, value_2) = (&mut self.0).split_at_mut(3);
            std::mem::swap(&mut value_1[0], &mut value_2[2]);
            std::mem::swap(&mut value_1[1], &mut value_2[1]);
            std::mem::swap(&mut value_1[2], &mut value_2[0]);
        }

        self
    }

    #[doc(hidden)]
    #[inline(always)]
    #[cfg(target_endian = "big")]
    pub fn convert_host_byteorder(self) -> BtAddr {
        // Public address structure contents are always big-endian
        self
    }
}

impl ToString for BtAddr {
    /// Converts `BtAddr` to a string of the format `XX:XX:XX:XX:XX:XX`.
    fn to_string(&self) -> String {
        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

impl str::FromStr for BtAddr {
    type Err = ();
    /// Converts a string of the format `XX:XX:XX:XX:XX:XX` to a `BtAddr`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let splits_iter = s.split(':');
        let mut addr = BtAddr::any();
        let mut i = 0;
        for split_str in splits_iter {
            if i == 6 || split_str.len() != 2 {
                return Err(());
            } // only 6 values (0 <= i <= 5) are allowed
            let high = (split_str.as_bytes()[0] as char).to_digit(16).ok_or(())?;
            let low = (split_str.as_bytes()[1] as char).to_digit(16).ok_or(())?;
            addr.0[i] = (high * 16 + low) as u8;
            i += 1;
        }
        if i != 6 {
            return Err(());
        }
        Ok(addr)
    }
}

/// What needs to happen to advance to the next state an asynchronous process
#[allow(missing_debug_implementations)] // `&mio::Evented` doesn't do `Debug`
pub enum BtAsync<'a> {
    /// Caller needs to wait for the given `Evented` object to reach the given `Ready` state
    WaitFor(&'a dyn mio::Evented, Ready),

    /// Asynchronous transaction has completed
    Done,
}

/// Represents an error which occurred in this library.
#[derive(Debug)]
pub enum BtError {
    /// No specific information is known.
    Unknown,

    /// On Unix platforms: the error code and an explanation for this error code.
    Errno(u32, String),

    /// This error only has a description.
    Desc(String),

    /// `std::io::Error`
    IoError(std::io::Error),
}

impl std::fmt::Display for BtError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:}", std::error::Error::to_string(self))
    }
}

impl Error for BtError {
    /*
    fn description(&self) -> &str {
        match self {
            BtError::Unknown => "Unknown Bluetooth Error",
            BtError::Errno(_, ref message) => message.as_str(),
            BtError::Desc(ref message) => message.as_str(),
            BtError::IoError(ref err) => err.,
        }
    }
    */
}

impl From<std::io::Error> for BtError {
    fn from(error: std::io::Error) -> Self {
        BtError::IoError(error)
    }
}

/// The Bluetooth protocol you can use with this libary.
///
/// Will probably be always `RFCOMM`.
#[derive(Clone, Copy, Debug)]
pub enum BtProtocol {
    // L2CAP = BTPROTO_L2CAP,
    // HCI = BTPROTO_HCI,
    // SCO = BTPROTO_SCO,
    // BNEP = BTPROTO_BNEP,
    // CMTP = BTPROTO_CMTP,
    // HIDP = BTPROTO_HIDP,
    // AVDTP = BTPROTO_AVDTP
    /// Serial RFCOMM connection to a bluetooth device.
    RFCOMM, // = BTPROTO_RFCOMM
}

/// A device with its a name and address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtDevice {
    /// The name of the device.
    pub name: String,

    /// The MAC address of the device.
    pub addr: BtAddr,
}

impl BtDevice {
    /// Create a new `BtDevice` manually from a name and addr.
    pub fn new(name: String, addr: BtAddr) -> BtDevice {
        BtDevice { name, addr }
    }
}

/// Finds a vector of Bluetooth devices in range.
///
/// This function blocks for some seconds.
pub fn scan_devices(timeout: time::Duration) -> Result<Vec<BtDevice>, BtError> {
    crate::scan_devices(timeout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn btaddr_from_string() {
        match BtAddr::from_str("00:00:00:00:00:00") {
            Ok(addr) => assert_eq!(addr, BtAddr([0u8; 6])),
            Err(_) => panic!(""),
        }

        let fail_strings = [
            "addr : String",
            "00:00:00:00:00",
            "00:00:00:00:00:00:00",
            "-00:00:00:00:00:00",
            "0G:00:00:00:00:00",
        ];
        for &s in &fail_strings {
            match BtAddr::from_str(s) {
                Ok(_) => panic!("Somehow managed to parse \"{}\" as an address?!", s),
                Err(_) => (),
            }
        }
    }

    #[test]
    fn btaddr_to_string() {
        assert_eq!(BtAddr::any().to_string(), "00:00:00:00:00:00");
        assert_eq!(BtAddr([1, 2, 3, 4, 5, 6]).to_string(), "01:02:03:04:05:06");
    }

    #[test]
    fn btaddr_roundtrips_to_from_str() {
        let addr = BtAddr([0, 22, 4, 1, 33, 192]);
        let addr_string = "00:ff:ee:ee:dd:12";

        assert_eq!(addr, BtAddr::from_str(&addr.to_string()).unwrap());
        assert!(
            addr_string.eq_ignore_ascii_case(&BtAddr::from_str(addr_string).unwrap().to_string())
        );
    }

    #[cfg(not(feature = "test_without_hardware"))]
    #[test]
    fn creates_rfcomm_socket() {
        BtSocket::new(BtProtocol::RFCOMM).unwrap();
    }

    #[cfg(not(feature = "test_without_hardware"))]
    #[test]
    fn scans_devices() {
        scan_devices(time::Duration::from_secs(20)).unwrap();
    }
}
