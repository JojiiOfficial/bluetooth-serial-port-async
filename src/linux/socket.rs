use super::sdp::{QueryRFCOMMChannel, QueryRFCOMMChannelStatus};
use crate::bluetooth::{BtAddr, BtAsync, BtError, BtProtocol};
use async_std::os::unix::net::UnixStream;
use mio::{unix::EventedFd, Poll, Ready};

use std::os::unix::io::{FromRawFd, RawFd};
use std::os::unix::net::UnixStream as StdUnixStream;

use std::{
    io::{Read, Write},
    mem,
};

pub fn create_error_from_errno(message: &str, errno: i32) -> BtError {
    let nix_error = nix::Error::from_errno(nix::errno::from_i32(errno));
    BtError::Errno(
        errno as u32,
        format!("{:}: {:}", message, nix_error.to_string()),
    )
}

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
    pub stream: StdUnixStream,
    pub fd: i32,
}

impl BtSocket {
    /// Create an (still) unconnected socket, like `crate::BtSocket`
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
    pub fn connect(&mut self, addr: BtAddr) -> BtSocketConnect {
        let addr = addr.convert_host_byteorder();

        BtSocketConnect::new(self, addr)
    }

    pub fn get_fd(&self) -> i32 {
        self.fd
    }

    pub fn get_stream_std(&self) -> StdUnixStream {
        let stream: StdUnixStream = unsafe { StdUnixStream::from_raw_fd(self.fd) };
        stream
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

impl Read for BtSocket {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buf)
    }
}

impl Write for BtSocket {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
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
