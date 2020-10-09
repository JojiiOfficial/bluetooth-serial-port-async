# bluetooth-serial-port [![Build Status](https://travis-ci.org/Dushistov/bluetooth-serial-port.svg?branch=master)](https://travis-ci.org/Dushistov/bluetooth-serial-port) [![Build status](https://ci.appveyor.com/api/projects/status/uyg280ku24iau8g3/branch/master?svg=true)](https://ci.appveyor.com/project/Dushistov/bluetooth-serial-port/branch/master)

Rust library for interacting with the Bluetooth stack via RFCOMM channels.

This library currently only works on Linux/BlueZ. You can find it on
[crates.io](https://crates.io/crates/bluetooth-serial-port).

Cargo.toml:

```toml
[dependencies]
bluetooth-serial-port = "0.5.1"
```

Important functions:

```rust
bluetooth_serial_port::scan_devices()
BtSocket::new()
BtSocket::connect()
BtSocket::connect_async()
BtSocket::read()
BtSocket::write()

impl mio::Evented for BtSocket { ... } // for async IO with mio
```

[Click here](examples/example.rs) for full example.

## API Reference

[API reference documentation is on docs.rs](https://docs.rs/bluetooth-serial-port)
