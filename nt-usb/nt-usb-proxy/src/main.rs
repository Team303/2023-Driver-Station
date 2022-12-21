use rand::Rng;
use usb_proto::ProxyPacket;

use futures::{future::select, pin_mut};
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures_util::{future::try_join_all, StreamExt, TryStreamExt};

use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::main]
async fn main() {
    let (usb_tx, usb_rx) = futures_channel::mpsc::unbounded();
    let (ws_tx, ws_rx) = futures_channel::mpsc::unbounded();

    let ws_future = tokio::spawn(create_ws_client(ws_tx, usb_rx));
    let usb_future = tokio::spawn(create_usb_master(usb_tx, ws_rx));

    try_join_all(vec![ws_future, usb_future]).await.unwrap();
}

async fn create_ws_client(tx: UnboundedSender<ProxyPacket>, rx: UnboundedReceiver<ProxyPacket>) {
    const URL: &str = "ws://127.0.0.1:5810/nt/usb-proxy";

    let ws_key = base64::encode(rand::thread_rng().gen::<[u8; 16]>());

    let req = http::Request::builder()
        .method("GET")
        .uri(URL)
        .header("Sec-WebSocket-Key", ws_key)
        .header("Sec-WebSocket-Protocol", "networktables.first.wpi.edu")
        .header("Sec-WebSocket-Version", "13")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Host", URL)
        .body(())
        .expect("Could not create http request");

    let (ws_stream, _) = connect_async(req).await.expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");

    let (write, read) = ws_stream.split();

    // Forward WS messages over to the USB port
    let ws_to_usb = {
        read.try_for_each(|message| async {
            match message {
                Message::Text(string) => tx
                    .unbounded_send(ProxyPacket::Text(string.clone()))
                    .unwrap(),
                Message::Binary(data) => tx.unbounded_send(ProxyPacket::Binary(data)).unwrap(),
                Message::Close(_) => tx.unbounded_send(ProxyPacket::Close).unwrap(),
                _ => eprintln!("Unimplemented message type: {:?}", message),
            };

            Ok(())
        })
    };

    // Forward messages from the USB port to the WS connection
    let usb_to_ws = rx.map(|p| p.into_message()).map(Ok).forward(write);

    // Run both concurrently
    pin_mut!(ws_to_usb, usb_to_ws);
    select(ws_to_usb, usb_to_ws).await;
}

async fn create_usb_master(_tx: UnboundedSender<ProxyPacket>, _rx: UnboundedReceiver<ProxyPacket>) {}

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
