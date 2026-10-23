use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{DefaultBodyLimit, Query, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use parking_lot::Mutex;
use serde_json::Value;
use subtle::ConstantTimeEq;
use tokio::sync::{broadcast, Semaphore};
use tokio_stream::StreamExt;

use crate::engine::engine_coordinator::EngineHandle;

use super::tools;

type Sessions = Arc<Mutex<HashMap<String, broadcast::Sender<Event>>>>;

#[derive(Clone)]
struct McpState {
    engine: Option<EngineHandle>,
    sessions: Sessions,
    bearer_token: String,
    work: Arc<Semaphore>,
}

struct SessionLease {
    id: String,
    sessions: Sessions,
}

impl Drop for SessionLease {
    fn drop(&mut self) {
        self.sessions.lock().remove(&self.id);
    }
}

async fn auth_middleware(
    State(state): State<McpState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Native/stdio clients send no Origin. Untrusted web content cannot become
    // a browser controller even if a credential leaks into page content.
    if headers.contains_key("origin") {
        return Err(StatusCode::FORBIDDEN);
    }
    let expected = format!("Bearer {}", state.bearer_token);
    let authorized = headers
        .get("authorization")
        .is_some_and(|header| bool::from(header.as_bytes().ct_eq(expected.as_bytes())));
    if !authorized {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(request).await)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn build_mcp_router(engine: Option<EngineHandle>, token: [u8; 32]) -> Router {
    let state = McpState {
        engine,
        bearer_token: hex(&token),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        work: Arc::new(Semaphore::new(32)),
    };
    Router::new()
        .route("/mcp", post(streamable_handler))
        .route("/mcp/sse", get(sse_handler))
        .route("/mcp/message", post(message_handler))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state)
}

pub async fn start_mcp_server(engine: EngineHandle) -> anyhow::Result<()> {
    let mut token = [0u8; 32];
    getrandom::fill(&mut token)
        .map_err(|error| anyhow::anyhow!("MCP credential generation: {error}"))?;
    let app = build_mcp_router(Some(engine), token);
    let addr = SocketAddr::from(([127, 0, 0, 1], 8765));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let directory = crate::paths::thor_data_dir().join("Mcp");
    let _connection = super::connection::publish(&directory, listener.local_addr()?, token)?;
    crate::thor_info!("thor::mcp", "Thor MCP listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn streamable_handler(
    State(state): State<McpState>,
    Json(payload): Json<Value>,
) -> Result<Response, StatusCode> {
    let _permit = state
        .work
        .try_acquire()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    Ok(
        match tools::handle_message(state.engine.as_ref(), payload).await {
            Some(response) => Json(response).into_response(),
            None => StatusCode::ACCEPTED.into_response(),
        },
    )
}

async fn sse_handler(
    State(state): State<McpState>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let id = hex(&bytes);
    let (sender, receiver) = broadcast::channel(64);
    {
        let mut sessions = state.sessions.lock();
        if sessions.len() >= 32 {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        sessions.insert(id.clone(), sender);
    }
    let endpoint = Event::default()
        .event("endpoint")
        .data(format!("/mcp/message?session_id={id}"));
    let lease = SessionLease {
        id,
        sessions: state.sessions,
    };
    // Lag terminates the stream so callers reconnect rather than silently
    // losing tool replies. Dropping a stream immediately revokes its endpoint.
    let stream = futures::stream::unfold((receiver, lease), |(mut receiver, lease)| async move {
        receiver
            .recv()
            .await
            .ok()
            .map(|event| (Ok(event), (receiver, lease)))
    });
    let initial = tokio_stream::iter([Ok(endpoint)]);
    Ok(Sse::new(initial.chain(stream)).keep_alive(KeepAlive::default()))
}

async fn message_handler(
    State(state): State<McpState>,
    Query(query): Query<HashMap<String, String>>,
    Json(payload): Json<Value>,
) -> Result<StatusCode, StatusCode> {
    let id = query.get("session_id").ok_or(StatusCode::BAD_REQUEST)?;
    let sender = state
        .sessions
        .lock()
        .get(id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    let permit = state
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    tokio::spawn(async move {
        let _permit = permit;
        if let Some(response) = tools::handle_message(state.engine.as_ref(), payload).await {
            let event = Event::default().event("message").data(response.to_string());
            let _ = sender.send(event);
        }
    });
    Ok(StatusCode::ACCEPTED)
}
