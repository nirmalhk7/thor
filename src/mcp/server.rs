use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::sse::{Event, Sse},
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Mutex};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};

use super::tools;
use crate::engine::engine_coordinator::EngineHandle;

const MAX_MCP_SESSIONS: usize = 128;
const MCP_SESSION_IDLE_TTL: Duration = Duration::from_secs(5 * 60);

#[derive(Clone)]
pub struct McpSession {
    pub client_id: String,
    pub tx: broadcast::Sender<Event>,
    last_used: Instant,
}

#[derive(Clone)]
pub struct McpState {
    pub engine: Option<EngineHandle>,
    pub sessions: Arc<Mutex<HashMap<String, McpSession>>>,
    pub bearer_token: String,
    pub expected_authority: Option<String>,
}

struct SessionStream<S> {
    inner: Pin<Box<S>>,
    sessions: Arc<Mutex<HashMap<String, McpSession>>>,
    client_id: String,
}

impl<S> SessionStream<S> {
    fn new(inner: S, sessions: Arc<Mutex<HashMap<String, McpSession>>>, client_id: String) -> Self {
        Self {
            inner: Box::pin(inner),
            sessions,
            client_id,
        }
    }
}

impl<S: Stream> Stream for SessionStream<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().inner.as_mut().poll_next(context)
    }
}

impl<S> Drop for SessionStream<S> {
    fn drop(&mut self) {
        if let Ok(mut sessions) = self.sessions.try_lock() {
            sessions.remove(&self.client_id);
            return;
        }

        let sessions = Arc::clone(&self.sessions);
        let client_id = self.client_id.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                sessions.lock().await.remove(&client_id);
            });
        }
    }
}

fn prune_expired_sessions(sessions: &mut HashMap<String, McpSession>, now: Instant) {
    sessions.retain(|_, session| now.duration_since(session.last_used) < MCP_SESSION_IDLE_TTL);
}

fn make_room_for_session(sessions: &mut HashMap<String, McpSession>, now: Instant) {
    prune_expired_sessions(sessions, now);
    while sessions.len() >= MAX_MCP_SESSIONS {
        let Some(oldest) = sessions
            .iter()
            .min_by_key(|(_, session)| session.last_used)
            .map(|(client_id, _)| client_id.clone())
        else {
            break;
        };
        sessions.remove(&oldest);
    }
}

fn spawn_session_expiry(
    sessions: Arc<Mutex<HashMap<String, McpSession>>>,
    client_id: String,
    idle_ttl: Duration,
) {
    tokio::spawn(async move {
        loop {
            let remaining = {
                let mut sessions = sessions.lock().await;
                let Some(session) = sessions.get(&client_id) else {
                    return;
                };
                let idle = session.last_used.elapsed();
                if idle >= idle_ttl {
                    sessions.remove(&client_id);
                    return;
                }
                idle_ttl - idle
            };
            tokio::time::sleep(remaining).await;
        }
    });
}

async fn auth_middleware(
    State(state): State<McpState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    if let Some(expected) = &state.expected_authority {
        if !request_source_allowed(&headers, expected) {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    if let Some(candidate) = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    {
        if constant_time_eq(candidate.as_bytes(), state.bearer_token.as_bytes()) {
            return Ok(next.run(request).await);
        }
    }
    Err(StatusCode::UNAUTHORIZED)
}

fn request_source_allowed(headers: &HeaderMap, expected_authority: &str) -> bool {
    let host_matches = headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == expected_authority);
    let origin_matches = headers
        .get("origin")
        .and_then(|value| value.to_str().ok())
        .map(|value| value == format!("http://{expected_authority}"))
        .unwrap_or(true);
    host_matches && origin_matches
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        let left_byte = left.get(index).copied().unwrap_or_default();
        let right_byte = right.get(index).copied().unwrap_or_default();
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

pub fn build_mcp_router(engine: Option<EngineHandle>, token: [u8; 32]) -> Router {
    build_mcp_router_for_authority(engine, token, None)
}

fn build_mcp_router_for_authority(
    engine: Option<EngineHandle>,
    token: [u8; 32],
    expected_authority: Option<String>,
) -> Router {
    let token_hex = token_hex(&token);
    let state = McpState {
        engine,
        sessions: Arc::new(Mutex::new(HashMap::new())),
        bearer_token: token_hex,
        expected_authority,
    };

    Router::new()
        .route("/mcp/sse", get(sse_handler))
        .route("/mcp/message", post(message_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state)
}

fn token_hex(token: &[u8; 32]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in token {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn generate_bearer_token() -> anyhow::Result<[u8; 32]> {
    let mut token = [0u8; 32];
    getrandom::getrandom(&mut token)
        .map_err(|error| anyhow::anyhow!("OS CSPRNG unavailable for MCP token: {error}"))?;
    Ok(token)
}

#[cfg(test)]
fn persist_bearer_token_to(token: &[u8; 32], path: &Path) -> anyhow::Result<()> {
    persist_owner_only_file(path, format!("{}\n", token_hex(token)).as_bytes())
}

fn persist_owner_only_file(path: &Path, expected: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("MCP token path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)?;
    let temp_name = format!(
        ".mcp-handoff.{}.{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let temp_path = parent.join(temp_name);
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(&temp_path)?;
    file.write_all(expected)?;
    file.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    drop(file);
    verify_owner_only_file(&temp_path, expected)?;

    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    if let Err(error) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error.into());
    }
    verify_owner_only_file(path, expected)
}

fn verify_owner_only_file(path: &Path, expected: &[u8]) -> anyhow::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        anyhow::bail!("MCP handoff is not a regular file: {}", path.display());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode() & 0o777;
        if mode != 0o600 {
            anyhow::bail!(
                "MCP handoff has insecure permissions {mode:o}, expected 600: {}",
                path.display()
            );
        }
    }
    let actual = std::fs::read(path)?;
    if !constant_time_eq(&actual, expected) {
        anyhow::bail!("MCP handoff readback failed: {}", path.display());
    }
    Ok(())
}

fn remove_handoff_if_unchanged(path: &Path, expected: &[u8]) {
    let unchanged = std::fs::symlink_metadata(path)
        .ok()
        .filter(|metadata| metadata.file_type().is_file() && !metadata.file_type().is_symlink())
        .and_then(|_| std::fs::read(path).ok())
        .is_some_and(|actual| constant_time_eq(&actual, expected));
    if unchanged {
        let _ = std::fs::remove_file(path);
    }
}

pub async fn start_mcp_server(engine: EngineHandle) {
    let data_dir = crate::paths::thor_data_dir();
    let token_path = data_dir.join("mcp-token");
    let endpoint_path = data_dir.join("mcp-endpoint");
    // Handoffs are per-process capabilities. Never leave stale discovery data
    // that looks live after startup fails.
    for path in [&token_path, &endpoint_path] {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                crate::thor_warn!(
                    "thor::mcp",
                    "MCP server disabled; stale handoff cleanup failed for {}: {error}",
                    path.display()
                );
                return;
            }
        }
    }

    let bind_addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = match tokio::net::TcpListener::bind(bind_addr).await {
        Ok(listener) => listener,
        Err(error) => {
            crate::thor_warn!("thor::mcp", "MCP server disabled; bind failed: {error}");
            return;
        }
    };
    let addr = match listener.local_addr() {
        Ok(addr) => addr,
        Err(error) => {
            crate::thor_warn!(
                "thor::mcp",
                "MCP server disabled; listener discovery failed: {error}"
            );
            return;
        }
    };
    let token = match generate_bearer_token() {
        Ok(token) => token,
        Err(error) => {
            crate::thor_warn!("thor::mcp", "MCP server disabled: {error}");
            return;
        }
    };
    let token_handoff = format!("{}\n", token_hex(&token));
    if let Err(error) = persist_owner_only_file(&token_path, token_handoff.as_bytes()) {
        crate::thor_warn!(
            "thor::mcp",
            "MCP server disabled; token handoff failed: {error}"
        );
        return;
    }
    let endpoint = format!("http://{addr}");
    let endpoint_handoff = format!("{endpoint}\n");
    if let Err(error) = persist_owner_only_file(&endpoint_path, endpoint_handoff.as_bytes()) {
        let _ = std::fs::remove_file(&token_path);
        crate::thor_warn!(
            "thor::mcp",
            "MCP server disabled; endpoint handoff failed: {error}"
        );
        return;
    }

    let app = build_mcp_router_for_authority(Some(engine), token, Some(addr.to_string()));

    tokio::spawn(async move {
        crate::thor_info!(
            "thor::mcp",
            "⚡ [Thor MCP] Server listening on {}; discovery files: {}, {}",
            endpoint,
            endpoint_path.display(),
            token_path.display(),
        );
        if let Err(error) = axum::serve(listener, app).await {
            crate::thor_warn!("thor::mcp", "MCP server stopped: {error}");
        }
        remove_handoff_if_unchanged(&endpoint_path, endpoint_handoff.as_bytes());
        remove_handoff_if_unchanged(&token_path, token_handoff.as_bytes());
    });
}

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn generated_tokens_are_nonzero_and_not_repeated() {
        let first = generate_bearer_token().unwrap();
        let second = generate_bearer_token().unwrap();
        assert_ne!(first, [0; 32]);
        assert_ne!(first, second);
        assert_eq!(token_hex(&first).len(), 64);
    }

    #[test]
    fn token_handoff_is_exact_regular_and_owner_only() {
        let token = [0x5au8; 32];
        let dir = std::env::temp_dir().join(format!(
            "thor-mcp-token-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("mcp-token");
        persist_bearer_token_to(&token, &path).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            format!("{}\n", token_hex(&token))
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn host_and_optional_origin_must_match_listener() {
        let mut headers = HeaderMap::new();
        headers.insert("host", "127.0.0.1:49152".parse().unwrap());
        assert!(request_source_allowed(&headers, "127.0.0.1:49152"));
        headers.insert("origin", "http://127.0.0.1:49152".parse().unwrap());
        assert!(request_source_allowed(&headers, "127.0.0.1:49152"));
        headers.insert("origin", "https://attacker.example".parse().unwrap());
        assert!(!request_source_allowed(&headers, "127.0.0.1:49152"));
        headers.insert("host", "attacker.example".parse().unwrap());
        assert!(!request_source_allowed(&headers, "127.0.0.1:49152"));
    }

    fn test_session(client_id: &str, last_used: Instant) -> McpSession {
        let (tx, _) = broadcast::channel(1);
        McpSession {
            client_id: client_id.to_string(),
            tx,
            last_used,
        }
    }

    #[test]
    fn session_cleanup_expires_idle_and_evicts_oldest_at_cap() {
        let now = Instant::now();
        let mut sessions = HashMap::new();
        sessions.insert(
            "expired".into(),
            test_session("expired", now - MCP_SESSION_IDLE_TTL),
        );
        for index in 0..MAX_MCP_SESSIONS {
            let client_id = format!("active-{index}");
            sessions.insert(
                client_id.clone(),
                test_session(&client_id, now - Duration::from_millis(index as u64)),
            );
        }

        make_room_for_session(&mut sessions, now);

        assert_eq!(sessions.len(), MAX_MCP_SESSIONS - 1);
        assert!(!sessions.contains_key("expired"));
        assert!(!sessions.contains_key(&format!("active-{}", MAX_MCP_SESSIONS - 1)));
    }

    #[test]
    fn ordinary_session_prune_preserves_active_sessions_at_cap() {
        let now = Instant::now();
        let mut sessions = HashMap::new();
        for index in 0..MAX_MCP_SESSIONS {
            let client_id = format!("active-{index}");
            sessions.insert(client_id.clone(), test_session(&client_id, now));
        }

        prune_expired_sessions(&mut sessions, now);

        assert_eq!(sessions.len(), MAX_MCP_SESSIONS);
    }

    #[tokio::test]
    async fn session_stream_drop_removes_disconnected_client() {
        let sessions = Arc::new(Mutex::new(HashMap::new()));
        sessions
            .lock()
            .await
            .insert("client".into(), test_session("client", Instant::now()));
        let stream = tokio_stream::empty::<Result<Event, std::convert::Infallible>>();
        drop(SessionStream::new(
            stream,
            Arc::clone(&sessions),
            "client".into(),
        ));

        assert!(!sessions.lock().await.contains_key("client"));
    }

    #[tokio::test]
    async fn idle_session_expires_without_followup_request() {
        let sessions = Arc::new(Mutex::new(HashMap::new()));
        sessions
            .lock()
            .await
            .insert("client".into(), test_session("client", Instant::now()));
        spawn_session_expiry(
            Arc::clone(&sessions),
            "client".into(),
            Duration::from_millis(10),
        );

        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(!sessions.lock().await.contains_key("client"));
    }
}

async fn sse_handler(
    State(state): State<McpState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = broadcast::channel(100);
    let client_id = format!(
        "session_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    {
        let now = Instant::now();
        let mut sessions = state.sessions.lock().await;
        make_room_for_session(&mut sessions, now);
        sessions.insert(
            client_id.clone(),
            McpSession {
                client_id: client_id.clone(),
                tx: tx.clone(),
                last_used: now,
            },
        );
    }
    spawn_session_expiry(
        Arc::clone(&state.sessions),
        client_id.clone(),
        MCP_SESSION_IDLE_TTL,
    );

    let stream = BroadcastStream::new(rx).filter_map(|res| match res {
        Ok(e) => Some(Ok(e)),
        Err(_) => None,
    });

    let endpoint_event = Event::default()
        .event("endpoint")
        .data(format!("/mcp/message?session_id={}", client_id));

    let init_stream = tokio_stream::iter(vec![Ok(endpoint_event)]);

    Sse::new(SessionStream::new(
        init_stream.chain(stream),
        Arc::clone(&state.sessions),
        client_id,
    ))
}

async fn message_handler(
    State(state): State<McpState>,
    axum::extract::Query(query): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<Value>,
) {
    if let Some(session_id) = query.get("session_id") {
        let tx = {
            let mut sessions = state.sessions.lock().await;
            prune_expired_sessions(&mut sessions, Instant::now());
            if let Some(session) = sessions.get_mut(session_id) {
                session.last_used = Instant::now();
                Some(session.tx.clone())
            } else {
                None
            }
        };

        if let Some(tx) = tx {
            let engine = state.engine.clone();
            let sessions = Arc::clone(&state.sessions);
            let session_id = session_id.clone();
            tokio::spawn(async move {
                let response = tools::handle_message(engine.as_ref(), payload).await;
                if let Some(res) = response {
                    let msg = Event::default()
                        .event("message")
                        .data(serde_json::to_string(&res).unwrap());
                    if tx.send(msg).is_err() {
                        sessions.lock().await.remove(&session_id);
                    }
                }
            });
        }
    }
}
