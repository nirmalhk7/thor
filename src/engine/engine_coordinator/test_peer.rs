use super::*;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::{accept_async, tungstenite::Message};

pub(super) struct Peer(JoinHandle<()>);

impl Drop for Peer {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub(super) async fn engine(reject_create: bool) -> (BrowserEngine, EngineHandle, Peer) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        while let Some(Ok(Message::Text(text))) = socket.next().await {
            let command: Value = serde_json::from_str(&text).unwrap();
            let response = match command["method"].as_str().unwrap() {
                "Target.setDiscoverTargets" | "Browser.close" => {
                    json!({"id":command["id"],"result":{}})
                }
                "Target.createTarget" if reject_create => {
                    json!({"id":command["id"],"error":{"code":-32000,"message":"create refused by test peer"}})
                }
                "Target.createTarget" => continue,
                other => panic!("unexpected CDP command: {other}"),
            };
            if socket
                .send(Message::Text(response.to_string().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });
    let runtime = ChromiumRuntime::connect(format!("ws://{address}"))
        .await
        .unwrap();
    let (engine, handle) = BrowserEngine::new(runtime);
    (engine, handle, Peer(server))
}

pub(super) fn add_tab(engine: &mut BrowserEngine, id: u64) -> mpsc::Receiver<TabCommand> {
    let (sender, receiver) = mpsc::channel(8);
    engine.tabs.insert(
        TabId(id),
        TabRecord {
            command_tx: sender,
            last_user_interaction: None,
            generation: 1,
            restore: None,
        },
    );
    receiver
}
