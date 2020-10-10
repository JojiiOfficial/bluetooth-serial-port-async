/*
use crate::socket::*;
use std::{
    io::{Read, Write},
    str, time,
};

impl BtSocket {
    /// Create an (still) unconnected socket.
    pub fn new(protocol: BtProtocol) -> Result<BtSocket, BtError> {
        Ok(From::from(platform::BtSocket::new(protocol)?))
    }

    /// Connect to the RFCOMM service on remote device with address `addr`. Channel will be
    /// determined through SDP protocol.
    ///
    /// This function can block for some seconds.
    pub fn connect(&mut self, addr: BtAddr) -> Result<(), BtError> {
        // Create temporary `mio` event loop
        let evtloop = mio::Poll::new().unwrap();
        let token = mio::Token(0);
        let mut events = mio::Events::with_capacity(2);

        // Request a socket connection
        let mut connect = self.0.connect(addr);

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
                    return Ok(());
                }
            }
        }
    }

    /// Connect to the RFCOMM service on remote device with address `addr`. Channel will be
    /// determined through SDP protocol.
    ///
    /// This function will return immediately and can therefor not indicate most kinds of failures.
    /// Once the connection actually has been established or an error has been determined the socket
    /// will become writable however. It is highly recommended to combine this call with the usage
    /// of `mio` (or some higher level event loop) to get proper non-blocking behaviour.
    pub fn connect_async(&mut self, addr: BtAddr) -> BtSocketConnect {
        BtSocketConnect(self.0.connect(addr))
    }
}

impl From<platform::BtSocket> for BtSocket {
    fn from(socket: platform::BtSocket) -> BtSocket {
        BtSocket(socket)
    }
}

impl mio::Evented for BtSocket {
    fn register(
        &self,
        poll: &mio::Poll,
        token: mio::Token,
        interest: mio::Ready,
        opts: mio::PollOpt,
    ) -> std::io::Result<()> {
        self.0.register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &mio::Poll,
        token: mio::Token,
        interest: mio::Ready,
        opts: mio::PollOpt,
    ) -> std::io::Result<()> {
        self.0.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> std::io::Result<()> {
        self.0.deregister(poll)
    }
}

/*
impl Read for BtSocket {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for BtSocket {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}
*/


/// Manages the bluetooth connection process when used from an asynchronous client.
#[derive(Debug)]
pub struct BtSocketConnect<'a>(platform::BtSocketConnect<'a>);

impl<'a> BtSocketConnect<'a> {
    /// Advance the connection process to the next state
    ///
    /// Usage: When receiving a new `BtSocketConnect` instance call this function to get the
    /// connection process started, then wait for the condition requested in `BtAsync` to apply
    /// (by polling for it in a `mio.Poll` instance in general). Once the condition is met, invoke
    /// this function again to advance to the next connect step. Repeat this process until you reach
    /// `BtAsync::Done`, then discard this object and enjoy your established connection.
    pub fn advance(&mut self) -> Result<BtAsync, BtError> {
        self.0.advance()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test()]
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

    #[test()]
    fn btaddr_to_string() {
        assert_eq!(BtAddr::any().to_string(), "00:00:00:00:00:00");
        assert_eq!(BtAddr([1, 2, 3, 4, 5, 6]).to_string(), "01:02:03:04:05:06");
    }

    #[test()]
    fn btaddr_roundtrips_to_from_str() {
        let addr = BtAddr([0, 22, 4, 1, 33, 192]);
        let addr_string = "00:ff:ee:ee:dd:12";

        assert_eq!(addr, BtAddr::from_str(&addr.to_string()).unwrap());
        assert!(
            addr_string.eq_ignore_ascii_case(&BtAddr::from_str(addr_string).unwrap().to_string())
        );
    }

    #[cfg(not(feature = "test_without_hardware"))]
    #[test()]
    fn creates_rfcomm_socket() {
        BtSocket::new(BtProtocol::RFCOMM).unwrap();
    }

    #[cfg(not(feature = "test_without_hardware"))]
    #[test()]
    fn scans_devices() {
        scan_devices(time::Duration::from_secs(20)).unwrap();
    }
}
*/
