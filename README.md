## bluetooth-serial-port-async

Rust library for interacting with the Bluetooth stack via RFCOMM channels.

This library only works on Linux/BlueZ. You can find it on [crates.io](https://crates.io/crates/bluetooth-serial-port-async).

You can use async_std or tokio to read and write async.

## Dependencies

The build machine requires `libbluetooth-dev`. On Ubuntu/Debian you can install it with:

```
apt install libbluetooth-dev
```

The execution machine requires `libbluetooth`. On Ubuntu/Debian you can install it with:

```
apt install libbluetooth
```

## Cargo.toml

For async_std:

```toml
[dependencies]
bluetooth-serial-port-async = "0.6.3"
```

For tokio

```toml
[dependencies]
bluetooth-serial-port-async = { version = "0.6.3", features = "tokio", default-features = false }
```

## Important functions

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
