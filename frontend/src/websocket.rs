use futures::{
    channel::mpsc::{Receiver, Sender},
    SinkExt, StreamExt,
};
use gloo_console::error;
use gloo_net::websocket::{self, futures::WebSocket};
use wasm_bindgen_futures::spawn_local;

use crate::types::{Action, MailMessageMetadata};

pub struct WebsocketService {
    pub sender: Sender<Action>,
    pub receiver: Receiver<MailMessageMetadata>,
}

impl WebsocketService {
    pub fn new() -> Self {
        // convert http URL to wesocket URL
        let mut location = web_sys::window()
            .unwrap()
            .location()
            .origin()
            .unwrap()
            .replace("http://", "ws://")
            .replace("https://", "wss://");
        location.push_str("/ws");
        let ws = WebSocket::open(&location).unwrap();

        let (mut write, mut read) = ws.split();
        let (ws_sender, mut ws_receiver) = futures::channel::mpsc::channel::<Action>(32);
        let (mut message_sender, message_receiver) =
            futures::channel::mpsc::channel::<MailMessageMetadata>(32);

        // forward messages over websocket to server
        spawn_local(async move {
            while let Some(a) = ws_receiver.next().await {
                match serde_json_wasm::to_string(&a) {
                    Ok(json_action) => {
                        if write
                            .send(websocket::Message::Text(json_action))
                            .await
                            .is_err()
                        {
                            error!("Error sending action over websocket");
                        }
                    }
                    _ => error!("Error formatting action to json"),
                }
            }
        });

        // retrieve and parse messages from incoming websocket
        spawn_local(async move {
            while let Some(msg) = read.next().await {
                if let Ok(websocket::Message::Text(data)) = msg {
                    if let Ok(message) = serde_json_wasm::from_str::<MailMessageMetadata>(&data) {
                        if message_sender.send(message).await.is_err() {
                            error!("Error queuing message");
                        }
                    }
                }
            }
        });

        Self {
            sender: ws_sender,
            receiver: message_receiver,
        }
    }
}
