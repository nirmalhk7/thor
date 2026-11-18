use super::ChromiumRuntime;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::{accept_async, tungstenite::Message};

pub(in crate::engine) struct Peer {
    task: JoinHandle<()>,
    pub commands: mpsc::UnboundedReceiver<Value>,
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(in crate::engine) async fn connected_runtime() -> (Arc<ChromiumRuntime>, Peer) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, commands) = mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        let stream = listener.accept().await.unwrap().0;
        let mut socket = accept_async(stream).await.unwrap();
        while let Some(message) = socket.next().await {
            let Ok(Message::Text(text)) = message else {
                break;
            };
            let command: Value = serde_json::from_str(&text).unwrap();
            let method = command["method"].as_str().unwrap();
            let response = if method == "Target.createTarget" {
                json!({"id":command["id"],"error":{"code":-32000,"message":"creation rejected"}})
            } else {
                assert!(["Target.setDiscoverTargets", "Browser.close"].contains(&method));
                json!({"id":command["id"],"result":{}})
            };
            sender.send(command).unwrap();
            if socket
                .send(Message::Text(response.to_string().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });
    let url = format!("ws://{address}");
    let runtime = ChromiumRuntime::connect(&url).await.unwrap();
    assert_eq!(runtime.websocket_url(), url);
    (runtime, Peer { task, commands })
}
