use std::sync::Arc;
use std::net::SocketAddr;
use std::collections::HashMap;
use axum::{
    routing::{get, post},
    Router,
    response::sse::{Event, Sse},
    extract::{State, Request},
    http::{StatusCode, HeaderMap},
    Json,
    middleware::{self, Next},
};
use tokio::sync::{broadcast, Mutex};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use serde_json::Value;

use crate::engine::engine_coordinator::EngineHandle;
use super::tools;

#[derive(Clone)]
pub struct McpSession {
    pub client_id: String,
    pub tx: broadcast::Sender<Event>,
}

#[derive(Clone)]
pub struct McpState {
    pub engine: Option<EngineHandle>,
    pub sessions: Arc<Mutex<HashMap<String, McpSession>>>,
    pub bearer_token: String,
}

async fn auth_middleware(
    State(state): State<McpState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str == format!("Bearer {}", state.bearer_token) {
                return Ok(next.run(request).await);
            }
        }
    }
    Err(StatusCode::UNAUTHORIZED)
}

pub fn build_mcp_router(engine: Option<EngineHandle>, token: [u8; 32]) -> Router {
    let mut token_hex = String::with_capacity(64);
    for b in token {
        token_hex.push_str(&format!("{:02x}", b));
    }
    let state = McpState {
        engine,
        sessions: Arc::new(Mutex::new(HashMap::new())),
        bearer_token: token_hex,
    };

    Router::new()
        .route("/mcp/sse", get(sse_handler))
        .route("/mcp/message", post(message_handler))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state)
}

pub async fn start_mcp_server(engine: EngineHandle) {
    // Basic random token generation without rand crate
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let mut token = [0u8; 32];
    for i in 0..32 { token[i] = ((timestamp >> (i % 8) * 8) & 0xff) as u8; }
    
    let app = build_mcp_router(Some(engine), token);

    tokio::spawn(async move {
        let addr = SocketAddr::from(([127, 0, 0, 1], 8765));
        crate::thor_info!("thor::mcp", "⚡ [Thor MCP] Server listening on http://{}", addr);
        if let Ok(listener) = tokio::net::TcpListener::bind(addr).await {
            let _ = axum::serve(listener, app).await;
        }
    });
}

async fn sse_handler(State(state): State<McpState>) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = broadcast::channel(100);
    let client_id = format!("session_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
    
    state.sessions.lock().await.insert(client_id.clone(), McpSession {
        client_id: client_id.clone(),
        tx: tx.clone(),
    });

    let stream = BroadcastStream::new(rx)
        .filter_map(|res| {
            match res {
                Ok(e) => Some(Ok(e)),
                Err(_) => None,
            }
        });

    let endpoint_event = Event::default()
        .event("endpoint")
        .data(format!("/mcp/message?session_id={}", client_id));
    
    let init_stream = tokio_stream::iter(vec![Ok(endpoint_event)]);
    
    Sse::new(init_stream.chain(stream))
}

async fn message_handler(
    State(state): State<McpState>,
    axum::extract::Query(query): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<Value>,
) {
    if let Some(session_id) = query.get("session_id") {
        let tx = {
            let sessions = state.sessions.lock().await;
            if let Some(session) = sessions.get(session_id) {
                Some(session.tx.clone())
            } else { None }
        };
        
        if let Some(tx) = tx {
            let engine = state.engine.clone();
            tokio::spawn(async move {
                let response = tools::handle_message(engine.as_ref(), payload).await;
                if let Some(res) = response {
                    let msg = Event::default().event("message").data(serde_json::to_string(&res).unwrap());
                    let _ = tx.send(msg);
                }
            });
        }
    }
}
