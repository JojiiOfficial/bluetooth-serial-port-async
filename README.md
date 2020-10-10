Rust library for interacting with the Bluetooth stack via RFCOMM channels.

This library only works on Linux/BlueZ. You can find it on

You can use async_std to read and write async

[crates.io](https://crates.io/crates/bluetooth-serial-port-async).

Cargo.toml:

```toml
[dependencies]
bluetooth-serial-port-async = "0.5.1"
```

Important functions:

```rust
bluetooth_serial_port::scan_devices()
BtSocket::new()
BtSocket::connect()
BtSocket::connect_async()
BtSocket::get_stream() // Use for read/write. Only call it once.

```

[Click here](examples/example.rs) for a full example.

## API Reference

[API reference documentation is on docs.rs](https://docs.rs/bluetooth-serial-port-async)
