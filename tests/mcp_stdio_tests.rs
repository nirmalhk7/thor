use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn stdio_bridge_forwards_requests_and_recovers_after_invalid_json() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = thor::mcp::server::build_mcp_router(None, [5; 32]);
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let directory = std::env::temp_dir().join(format!(
        "thor-stdio-test-{}-{}",
        std::process::id(),
        address.port()
    ));
    let credentials = thor::mcp::connection::publish(&directory, address, [5; 32]).unwrap();
    let info = thor::mcp::connection::load(&directory.join("connection.json")).unwrap();
    let (mut client, stream) = tokio::io::duplex(8192);
    let (read, write) = tokio::io::split(stream);
    let bridge = tokio::spawn(thor::mcp::stdio::bridge(info, BufReader::new(read), write));
    client
        .write_all(b"invalid json\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n")
        .await
        .unwrap();
    client.shutdown().await.unwrap();
    let mut output = String::new();
    client.read_to_string(&mut output).await.unwrap();
    bridge.await.unwrap().unwrap();
    let replies: Vec<serde_json::Value> = output
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0]["error"]["code"], -32700);
    assert_eq!(replies[1]["id"], 2);
    assert!(replies[1]["result"].is_object());
    drop(credentials);
    std::fs::remove_dir(directory).unwrap();
    server.abort();
}

#[tokio::test]
async fn stdio_bridge_rejects_oversized_input_before_sending_it() {
    let directory =
        std::env::temp_dir().join(format!("thor-stdio-limit-test-{}", std::process::id()));
    let credentials =
        thor::mcp::connection::publish(&directory, "127.0.0.1:1".parse().unwrap(), [1; 32])
            .unwrap();
    let info = thor::mcp::connection::load(&directory.join("connection.json")).unwrap();
    let input = vec![b'x'; 300_000];
    let result =
        thor::mcp::stdio::bridge(info, BufReader::new(input.as_slice()), tokio::io::sink()).await;
    assert!(result.unwrap_err().to_string().contains("size limit"));
    drop(credentials);
    std::fs::remove_dir(directory).unwrap();
}
