use async_std::io::prelude::*;
use bluetooth_serial_port_async::{BtProtocol, BtSocket};
use std::time;

fn main() {
    async_std::task::block_on(run());
}

async fn run() {
    let devices = bluetooth_serial_port_async::scan_devices(time::Duration::from_secs(20)).unwrap();
    if devices.len() == 0 {
        panic!("No devices found");
    }

    println!("Found bluetooth devices {:?}", devices);

    // "device.addr" is the MAC address of the device
    let device = &devices[0];
    println!(
        "Connecting to `{}` ({})",
        device.name,
        device.addr.to_string()
    );

    // create and connect the RFCOMM socket
    let mut socket = BtSocket::new(BtProtocol::RFCOMM).unwrap();
    socket.connect(device.addr).unwrap();

    // BtSocket implements the `Read` and `Write` traits (they're blocking)
    let mut buffer = [0; 10];
    let mut stream = socket.get_stream();
    let num_bytes_read = stream.read(&mut buffer[..]).await.unwrap();
    let num_bytes_written = stream.write(&buffer[0..num_bytes_read]).await.unwrap();
    println!(
        "Read `{}` bytes, wrote `{}` bytes",
        num_bytes_read, num_bytes_written
    );
}
