use super::MetricsCollector;
use std::time::Instant;

pub(super) async fn send_http(
    req: reqwest::RequestBuilder,
    start: Instant,
    collector: &MetricsCollector,
) {
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let bytes = resp.bytes().await.map(|b| b.len() as u64).unwrap_or(0);
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            collector.record_sample(status, latency, bytes, status < 400);
        }
        Err(_) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            collector.record_sample(0, latency, 0, false);
        }
    }
}

pub(super) async fn execute_websocket(
    url: &str,
    messages: &[String],
    collector: &MetricsCollector,
) {
    let start = Instant::now();

    // We use connect_async to handle actual WS connection
    if let Ok((mut ws_stream, resp)) = tokio_tungstenite::connect_async(url.to_owned()).await {
        let mut payload_bytes = 0;
        use futures::{SinkExt, StreamExt};
        for msg in messages {
            #[allow(clippy::redundant_pattern_matching)]
            if let Ok(_) = ws_stream
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    msg.clone().into(),
                ))
                .await
            {
                payload_bytes += msg.len() as u64;
                if let Some(Ok(m)) = ws_stream.next().await {
                    payload_bytes += m.into_data().len() as u64;
                }
            }
        }
        let _ = ws_stream.close(None).await;
        let status = resp.status().as_u16();
        let latency = start.elapsed().as_secs_f64() * 1000.0;
        collector.record_sample(status, latency, payload_bytes, status < 500);
    } else {
        let latency = start.elapsed().as_secs_f64() * 1000.0;
        collector.record_sample(0, latency, 0, false);
    }
}
