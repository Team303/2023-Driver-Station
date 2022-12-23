use std::time::Duration;

use ansi_term::Colour;
use rand::Rng;
use serde::Deserialize;

use futures::{future::select, pin_mut, SinkExt};
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures_util::{future::try_join_all, StreamExt};

use tokio_tungstenite::{connect_async, tungstenite::Message};

use serialport::{available_ports, SerialPortType};

use usb_proto::{ProtoReadable, ProtoWriteable, ProxyPacket};

#[derive(Deserialize, Clone)]
struct ProxyConfig {
    url: String,
    serial_port: String,
    serial_baud: u32,
}

#[tokio::main]
async fn main() -> ! {
    // Parse configuration
    let config = match std::fs::read_to_string("./proxy.config.json") {
        Ok(config_contents) => {
            match serde_json::from_str::<ProxyConfig>(config_contents.as_str()) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("Could not parse config file!");
                    eprintln!("{}", e);
                    std::process::exit(1)
                }
            }
        }
        Err(_) => ProxyConfig {
            url: String::from("ws://127.0.0.1:5810/nt/usb-proxy"),
            serial_port: String::from(if cfg!(target_os = "windows") {
                "COM3"
            } else {
                "/dev/ttyUSB0"
            }),
            serial_baud: 115_200,
        },
    };

    // Create a full duplex channel between the two main async tasks
    let (usb_tx, usb_rx) = futures_channel::mpsc::unbounded();
    let (ws_tx, ws_rx) = futures_channel::mpsc::unbounded();

    // Spawn the async tasks
    let ws_future = tokio::spawn(create_ws_client(config.clone(), ws_tx, usb_rx));
    let usb_future = tokio::spawn(create_usb_master(config, usb_tx, ws_rx));

    // Run both tasks concurrently
    try_join_all(vec![ws_future, usb_future]).await.unwrap();

    loop {}
}

/// Creates the WS half of the proxy
///
/// TODO:
///     - Removed hardcoded connection url
///     - Add better error handling
async fn create_ws_client(
    config: ProxyConfig,
    tx: UnboundedSender<ProxyPacket>,
    mut rx: UnboundedReceiver<ProxyPacket>,
) {
    let url = &config.url;

    loop {
        // Generate a random 16 bytes and base64 them to create our unique connection key
        let ws_key = base64::encode(rand::thread_rng().gen::<[u8; 16]>());

        // Create the raw HTTP request to initiate the WS connection
        let req = http::Request::builder()
            .method("GET")
            .uri(url)
            .header("Sec-WebSocket-Key", ws_key)
            .header("Sec-WebSocket-Protocol", "networktables.first.wpi.edu")
            .header("Sec-WebSocket-Version", "13")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Host", url)
            .body(())
            .expect("Could not create http request");

        // Connect to the NT4 WS server
        let (ws_stream, _) = match connect_async(req).await {
            Ok(ws) => ws,
            Err(e) => {
                eprintln!("{:?}", e);
                eprintln!(
                    "{} {}",
                    Colour::Red.paint("Error connectiing to NT4 WS server."),
                    Colour::Black.paint("Trying again in 5 seconds...")
                );
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        println!(
            "{}",
            Colour::Green.paint("WebSocket handshake has been successfully completed")
        );

        // Split the WS stream into a read stream and a write stream
        let (mut write, mut read) = ws_stream.split();

        // Forward all WS messages over to the USB port
        let ws_to_usb = async {
            loop {
                // Get the message from the ws stream
                let message = read.next().await;

                // If no message is available, keep looping until one is
                let Some(message) = message else {
                    continue;
                };

                let message = match message {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("{:?}", e);
                        eprintln!(
                            "{} {}",
                            Colour::Red.paint("Failed read ws message from stream."),
                            Colour::White.dimmed().paint("Trying again in 5 seconds...")
                        );
                        break;
                    }
                };

                match message {
                    Message::Text(string) => tx
                        .unbounded_send(ProxyPacket::Text(string.clone()))
                        .unwrap(),
                    Message::Binary(data) => tx.unbounded_send(ProxyPacket::Binary(data)).unwrap(),
                    Message::Close(_) => tx.unbounded_send(ProxyPacket::Close).unwrap(),
                    _ => eprintln!("Unimplemented message type: {:?}", message),
                };
            }
        };

        // Forward messages from the USB port to the WS connection
        let usb_to_ws = async {
            loop {
                // Get the next packet from the usb client
                let usb_packet = rx.next().await;

                // If no packet is available, keep looping until one is
                let Some(packet) = usb_packet else {
                    continue;
                };

                let ws_message = packet.into_message();

                // Write the packet to the stream
                let Ok(_) = write.send(ws_message).await else {
                    eprintln!(
                        "{} {}",
                        Colour::Red.paint("Failed to send ws message over write stream."),
                        Colour::White.dimmed().paint("Trying again in 5 seconds...")
                    );
                    break
                };
            }
        };

        // Run both concurrently
        pin_mut!(ws_to_usb, usb_to_ws);
        select(ws_to_usb, usb_to_ws).await;

        // Wait the 5 seconds between retry attempts for futures
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// This creates a loop which never ends. It
async fn create_usb_master(
    config: ProxyConfig,
    tx: UnboundedSender<ProxyPacket>,
    mut rx: UnboundedReceiver<ProxyPacket>,
) {
    // Loop continuously while no ports are found or an error condition is met, to always try reconnecting
    loop {
        // Try to enumerate the available ports (if failed, try again)
        let ports = match available_ports() {
            Ok(ports) => ports,
            Err(e) => {
                eprintln!("{:?}", e);
                eprintln!(
                    "{} {}",
                    Colour::Red.paint("Error enumerating serial ports."),
                    Colour::Black.paint("Trying again in 5 seconds...")
                );
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // If no ports were found, try again
        if ports.len() == 0 {
            eprintln!(
                "{} {}",
                Colour::Red.paint("No ports found."),
                Colour::White.dimmed().paint("Trying again in 5 seconds...")
            );
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        };

        // Try and get the port that matches the configured value
        let port = ports.iter().find(|p| {
            matches!(p.port_type, SerialPortType::UsbPort(_)) && p.port_name == config.serial_port
        });

        // If the configured port was not found, try again
        let Some(port) = port else {
            // Print out the port information
            for p in &ports {
                match ports.len() {
                    1 => println!("Found 1 port:"),
                    n => println!("Found {} ports:", n),
                };

                println!("  {}", p.port_name);
                match &p.port_type {
                    SerialPortType::UsbPort(info) => {
                        println!("    Type: USB");
                        println!(
                            "    Manufacturer: {}",
                            info.manufacturer.as_ref().map_or("", String::as_str)
                        );
                        println!(
                            "    Product: {}",
                            info.product.as_ref().map_or("", String::as_str)
                        );
                    }
                    SerialPortType::BluetoothPort => {
                        println!("    Type: Bluetooth");
                    }
                    SerialPortType::PciPort => {
                        println!("    Type: PCI");
                    }
                    SerialPortType::Unknown => {
                        println!("    Type: Unknown");
                    }
                }
            }

            eprintln!(
                "{} {}",
                Colour::Red.paint(format!("Configured port `{}` not found.", config.serial_port)),
                Colour::White.dimmed().paint("Trying again in 5 seconds...")
            );
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        };

        let Ok(port) = serialport::new(port.port_name.as_str(), config.serial_baud)
            .timeout(Duration::from_millis(50))
            .open() else {
                eprintln!(
                    "{} {}",
                    Colour::Red.paint(format!("Failed to open serial port `{}` at {} baud.", config.serial_port, config.serial_baud)),
                    Colour::White.dimmed().paint("Trying again in 5 seconds...")
                );
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            };

        println!(
            "{}",
            Colour::Green.paint(format!(
                "USB Serial connection with port `{}` has been established successfully",
                config.serial_port
            ))
        );

        let (Ok(mut reader), Ok(mut writer)) = (port.try_clone(), port.try_clone()) else {
            eprintln!(
                "{} {}",
                Colour::Red.paint("Failed to clone serial port for reading and writing."),
                Colour::White.dimmed().paint("Trying again in 5 seconds...")
            );
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        };

        // Read packets from usb serial and send them to the ws client
        let usb_to_ws = async {
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
                let Ok(packet) = reader.read_packet() else {
                    eprintln!(
                        "{} {}",
                        Colour::Red.paint("Failed to read and decode packet from stream."),
                        Colour::White.dimmed().paint("Trying again in 5 seconds...")
                    );
                    break
                };

                // Send the packet to the ws client to be sent over the network
                tx.unbounded_send(packet).unwrap();
            }
        };

        let ws_to_usb = async {
            loop {
                // Get the next packet from the ws client
                let ws_packet = rx.next().await;

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
        pin_mut!(ws_to_usb, usb_to_ws);
        select(ws_to_usb, usb_to_ws).await;

        // Wait the 5 seconds between retry attempts for futures
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// Trait to allow ProxyPackets to be converted to tungstenite Messages
pub trait IntoMessage: Sized {
    fn into_message(self) -> Message;
}

impl IntoMessage for ProxyPacket {
    fn into_message(self) -> Message {
        match self {
            ProxyPacket::Text(string) => Message::text(string),
            ProxyPacket::Binary(data) => Message::binary(data),
            ProxyPacket::Close => Message::Close(None),
        }
    }
}
