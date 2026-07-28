use std::sync::Arc;
use std::net::SocketAddr;
use axum::{
    routing::{get, post},
    Router,
    response::sse::{Event, Sse},
    extract::State,
    Json,
};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use serde_json::Value;

use crate::engine::ChromiumRuntime;
use super::tools;

#[derive(Clone)]
pub struct McpState {
    pub runtime: Option<Arc<ChromiumRuntime>>,
    pub tx: broadcast::Sender<Event>,
}

pub fn build_mcp_router(runtime: Option<Arc<ChromiumRuntime>>) -> Router {
    let (tx, _rx) = broadcast::channel(100);
    let state = McpState {
        runtime,
        tx,
    };

    Router::new()
        .route("/sse", get(sse_handler))
        .route("/message", post(message_handler))
        .with_state(state)
}

pub async fn start_mcp_server(runtime: Arc<ChromiumRuntime>) {
    let app = build_mcp_router(Some(runtime));

    tokio::spawn(async move {
        let addr = SocketAddr::from(([127, 0, 0, 1], 8765));
        println!("⚡ [Thor MCP] Server listening on http://{}", addr);
        if let Ok(listener) = tokio::net::TcpListener::bind(addr).await {
            let _ = axum::serve(listener, app).await;
        } else {
            eprintln!("⚡ [Thor MCP] Failed to bind to {}", addr);
        }
    });
}

async fn sse_handler(State(state): State<McpState>) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(|res| {
            match res {
                Ok(e) => Some(Ok(e)),
                Err(_) => None,
            }
        });

    let endpoint_event = Event::default()
        .event("endpoint")
        .data("/message");
    
    let init_stream = tokio_stream::iter(vec![Ok(endpoint_event)]);
    
    Sse::new(init_stream.chain(stream))
}

async fn message_handler(
    State(state): State<McpState>,
    Json(payload): Json<Value>,
) {
    let runtime = state.runtime.clone();
    let tx = state.tx.clone();
    
    tokio::spawn(async move {
        let response = tools::handle_message(runtime.as_deref(), payload).await;
        if let Some(res) = response {
            let msg = Event::default().event("message").data(serde_json::to_string(&res).unwrap());
            let _ = tx.send(msg);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_mcp_sse_endpoint() {
        let app = build_mcp_router(None);

        let response = app
            .oneshot(Request::builder().uri("/sse").body(axum::body::Body::empty()).unwrap())
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/event-stream"
        );
    }

    #[tokio::test]
    async fn test_mcp_message_endpoint() {
        let app = build_mcp_router(None);

        let req_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/message")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(req_body.to_string()))
                    .unwrap(),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }
}
