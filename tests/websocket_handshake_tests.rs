use std::time::Duration;

use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, connect_async, tungstenite};

#[test]
fn rfc6455_accept_key_matches_the_protocol_example() {
    assert_eq!(
        tungstenite::handshake::derive_accept_key(b"dGhlIHNhbXBsZSBub25jZQ=="),
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
}

#[tokio::test]
async fn local_handshake_preserves_bidirectional_text_and_binary_frames() {
    let exchange = async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            assert_eq!(
                socket.next().await.unwrap().unwrap(),
                tungstenite::Message::Text("Thor handshake".into())
            );
            socket
                .send(tungstenite::Message::Binary(vec![0, 1, 127, 255].into()))
                .await
                .unwrap();
        });
        let (mut client, response) = connect_async(format!("ws://{address}/")).await.unwrap();
        assert_eq!(response.status().as_u16(), 101);
        client
            .send(tungstenite::Message::Text("Thor handshake".into()))
            .await
            .unwrap();
        assert_eq!(
            client.next().await.unwrap().unwrap(),
            tungstenite::Message::Binary(vec![0, 1, 127, 255].into())
        );
        server.await.unwrap();
    };
    tokio::time::timeout(Duration::from_secs(10), exchange)
        .await
        .expect("local WebSocket exchange timed out");
}
