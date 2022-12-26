use std::time::Duration;

use ansi_term::Colour;

use futures::{future::select, pin_mut, SinkExt};
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures_util::{future::try_join_all, StreamExt};


use usb_proto::{ProxyPacket, ProtoReadable, ProtoWriteable};


#[tokio::main]
async fn main() {
     // Create a full duplex channel between the two main async tasks
     let (usb_tx_to_nt, nt_rx_from_usb) = futures_channel::mpsc::unbounded();
     let (nt_tx_to_usb, usb_rx_from_nt) = futures_channel::mpsc::unbounded();
 
     // Spawn the async tasks
    //  let ws_future = tokio::spawn(create_ws_client(config.clone(), ws_tx, usb_rx));
     let usb_future = tokio::spawn(create_usb_slave(usb_tx_to_nt, usb_rx_from_nt));
 
     // Run both tasks concurrently
     try_join_all(vec![/* ws_future, */ usb_future]).await.unwrap();
 
     panic!("unreachable");
}

/// This creates a loop which never ends. It
async fn create_usb_slave(
    tx_to_nt: UnboundedSender<ProxyPacket>,
    mut rx_from_nt: UnboundedReceiver<ProxyPacket>,
) -> ! {
    // Loop continuously while no ports are found or an error condition is met, to always try reconnecting
    loop {
        // Bind to serial device on USB C port
        let Ok(port) = serialport::new("/dev/ttyGS0", 115_200)
            .timeout(Duration::from_secs(60 * 60))
            .open() else {
                eprintln!(
                    "{} {}",
                    Colour::Red.paint("Failed to open serial port `/dev/ttyGS0` at 115,200 baud."),
                    Colour::White.dimmed().paint("Trying again in 5 seconds...")
                );
                continue;
            };

        println!(
            "{}",
            Colour::Green.paint(
                "USB Serial connection with port `/dev/ttyGS0` has been established successfully",
            )
        );

        let (Ok(mut reader), Ok(mut writer)) = (port.try_clone(), port.try_clone()) else {
            eprintln!(
                "{} {}",
                Colour::Red.paint("Failed to clone serial port for reading and writing."),
                Colour::White.dimmed().paint("Trying again in 5 seconds...")
            );
            continue;
        };

        let mut tx_to_nt = tx_to_nt.clone();

        // Read packets from usb serial and send them to the nt client
        let usb_to_nt = tokio::task::spawn_blocking(move || {
            loop {
                // If there was a reading error, break and retry the connection
                let Ok(num_bytes) = reader.bytes_to_read() else {
                    eprintln!(
                        "{} {}",
                        Colour::Red.paint("Failed to read bytes from serial."),
                        Colour::White.dimmed().paint("Trying again in 5 seconds...")
                    );
                    break
                 };

                // If there are no bytes to read, continue
                if num_bytes == 0 {
                    continue;
                }

                // Read a packet from the stream
                let packet = match reader.read_packet() {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("{:?}", e);
                        eprintln!(
                            "{} {}",
                            Colour::Red.paint("Failed to read and decode packet from stream."),
                            Colour::White.dimmed().paint("Trying again in 5 seconds...")
                        );
                        break;
                    }
                };

                // Send the packet to the ws client to be sent over the network
                tx_to_nt.unbounded_send(packet).unwrap();
            }
        });

        let nt_to_usb = async {
            loop {
                // Get the next packet from the ws client
                let ws_packet = rx_from_nt.next().await;

                // If no packet is available, keep looping until one is
                let Some(packet) = ws_packet else {
                    continue;
                };

                // Write the packet to the stream
                let Ok(_) = writer.write_packet(packet) else {
                    eprintln!(
                        "{} {}",
                        Colour::Red.paint("Failed to encode and write packet to stream."),
                        Colour::White.dimmed().paint("Trying again in 5 seconds...")
                    );
                    break
                };
            }
        };

        // Run both concurrently, and retry on any errors
        pin_mut!(nt_to_usb, usb_to_nt);
        select(nt_to_usb, usb_to_nt).await;

        // Wait the 5 seconds between retry attempts for futures
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
