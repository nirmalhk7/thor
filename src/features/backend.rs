//! Backend specialist features (12 + 12 wave-2).
//!
//! API workbench, auth vault, contract validation, webhook sink, IDOR matrix,
//! request chaining, gRPC/GraphQL inspection, trace adjacency, mock/stub server,
//! chaos injection, idempotency auditing and DB adjacency. Logic is real Rust:
//! JSON schema validation, deterministic chaos, SQL/GraphQL parsing, and graph
//! analysis — no simulated randomness.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::app_state::AppState;
use crate::net::tamper::{InterceptedRequest, InterceptedResponse};

// ---------------------------------------------------------------------------
// Shared request/response model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiRequest {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: String,
    pub latency_ms: f64,
}

// ---------------------------------------------------------------------------
// 2. Auth Vault
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthScheme {
    Bearer { token: String },
    Basic { username: String, password: String },
    ApiKey { header: String, key: String },
    /// Shared secret for webhook HMAC verification (WebhookSignatureVault).
    WebhookSecret { secret: String },
}

// ---------------------------------------------------------------------------
// 5. IDOR Matrix
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdorProbe {
    pub identity: String,
    pub resource: String,
    /// Whether this identity is the legitimate owner of the resource.
    pub is_owner: bool,
    pub status: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdorFinding {
    pub identity: String,
    pub resource: String,
    pub status: u16,
    pub severity: String,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// 6. Request Chaining
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainStep {
    pub name: String,
    pub request: ApiRequest,
    /// Variable name -> dot path into this step's JSON response to capture.
    #[serde(default)]
    pub captures: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// 7. gRPC / GraphQL
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQlAnalysis {
    pub operation_type: String,
    pub operation_name: Option<String>,
    pub top_level_fields: Vec<String>,
    pub max_depth: usize,
    pub is_introspection: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcCall {
    pub package: String,
    pub service: String,
    pub method: String,
}

// ---------------------------------------------------------------------------
// 8. Trace Adjacency
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub span_id: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    pub service: String,
    pub name: String,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceGraph {
    /// service -> downstream services it calls.
    pub edges: BTreeMap<String, Vec<String>>,
    pub critical_path: Vec<String>,
    pub critical_path_ms: f64,
}

// ---------------------------------------------------------------------------
// 9. Mock / Stub
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockRoute {
    pub method: String,
    /// Path pattern with `:param` placeholders, e.g. `/users/:id`.
    pub path_pattern: String,
    pub status: u16,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockMatch {
    pub status: u16,
    pub body: String,
    pub path_params: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// 10. Chaos Injection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosRule {
    pub url_pattern: String,
    /// Injected latency in ms (0 = none).
    #[serde(default)]
    pub added_latency_ms: u64,
    /// Fault probability in [0,1], applied deterministically per request id.
    #[serde(default)]
    pub error_rate: f64,
    #[serde(default)]
    pub error_status: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosOutcome {
    pub injected_fault: bool,
    pub added_latency_ms: u64,
    pub forced_status: Option<u16>,
}

// ---------------------------------------------------------------------------
// 11. Idempotency Auditor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyEvent {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyFinding {
    pub url: String,
    pub method: String,
    pub severity: String,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// 12. DB Adjacency
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbReport {
    /// table -> tables it is joined with.
    pub adjacency: BTreeMap<String, Vec<String>>,
    /// Normalized query template -> execution count (>1 with high count = N+1).
    pub n_plus_one: Vec<QueryCount>,
    pub total_queries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCount {
    pub template: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub received_ms: u64,
}

/// Combined IDOR/BOLA matrix result from sending one request across contexts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdorMatrixReport {
    pub probes: Vec<IdorProbe>,
    pub findings: Vec<IdorFinding>,
}

/// Per-step result of an executed request chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainStepResult {
    pub name: String,
    pub status: u16,
    pub latency_ms: f64,
    pub captured: Vec<String>,
}

/// End-to-end request-chain execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainRunResult {
    pub steps: Vec<ChainStepResult>,
    pub vars: HashMap<String, String>,
    pub ok: bool,
}

/// A single cache access correlated to a DB table (for cache adjacency).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheOp {
    pub key: String,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub hit: bool,
}

/// DB analysis with cache-key ↔ table adjacency layered alongside.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCacheReport {
    pub db: DbReport,
    /// cache key -> tables accessed alongside it.
    pub cache_adjacency: BTreeMap<String, Vec<String>>,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

/// Live localhost webhook sink listener handle.
struct WebhookSinkHandle {
    port: u16,
    running: Arc<AtomicBool>,
}

struct LiveStubHandle {
    port: u16,
    running: Arc<AtomicBool>,
    hit_count: Arc<std::sync::atomic::AtomicU64>,
    #[allow(dead_code)]
    routes: Arc<Mutex<Vec<StubRouteSpec>>>,
}

struct SseLabHandle {
    port: u16,
    running: Arc<AtomicBool>,
    event_count: Arc<std::sync::atomic::AtomicU64>,
    #[allow(dead_code)]
    events: Arc<Mutex<Vec<StreamLabEvent>>>,
}

/// Backend specialist engine.
pub struct BackendEngine {
    http: std::sync::OnceLock<Result<crate::api::HttpService, String>>,
    // 1. API workbench history
    history: Mutex<Vec<(ApiRequest, ApiResponse)>>,
    // 2. Auth vault (keyed by context_id, multi-context)
    vault: Mutex<HashMap<String, AuthScheme>>,
    // 4. Webhook sink
    webhooks: Mutex<Vec<WebhookEvent>>,
    webhook_sink: Mutex<Option<WebhookSinkHandle>>,
    // 9. Mock routes
    mocks: Mutex<Vec<MockRoute>>,
    // 10. Chaos rules
    chaos: Mutex<Vec<ChaosRule>>,
    live_stub: Mutex<Option<LiveStubHandle>>,
    sse_lab: Mutex<Option<SseLabHandle>>,
}

impl Default for BackendEngine {
    fn default() -> Self {
        Self::new()
    }
}


// ===========================================================================
// Wave-2 backend features (12)
// ===========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookSigInput {
    pub payload: String,
    pub secret: String,
    pub provided_signature: String,
    #[serde(default = "default_algo")]
    pub algorithm: String,
    #[serde(default)]
    pub timestamp: Option<i64>,
    #[serde(default)]
    pub now: Option<i64>,
    #[serde(default = "default_tol")]
    pub tolerance_secs: i64,
}

fn default_algo() -> String { "sha256".into() }
fn default_tol() -> i64 { 300 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookSigReport {
    pub valid: bool,
    pub expected_prefix: String,
    pub timestamp_ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitSample {
    pub status: u16,
    pub retry_after_secs: Option<f64>,
    pub remaining: Option<i64>,
    pub limit: Option<i64>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitReport {
    pub limited_count: usize,
    pub inferred_limit: Option<i64>,
    pub avg_retry_after: Option<f64>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSlice {
    pub page: u32,
    pub page_size: u32,
    pub items: usize,
    pub total: Option<u64>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationReport {
    pub pages: usize,
    pub issues: Vec<String>,
    pub estimated_total: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSemanticsEntry {
    pub url: String,
    pub method: String,
    pub cache_control: String,
    pub etag: Option<String>,
    pub vary: Option<String>,
    pub status: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSemanticsReport {
    pub findings: Vec<String>,
    pub cacheable: usize,
    pub non_cacheable: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipartPart {
    pub name: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipartReport {
    pub parts: usize,
    pub total_bytes: usize,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamLabEvent {
    pub kind: String,
    pub payload: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseWsLabReport {
    pub event_count: usize,
    pub kinds: BTreeMap<String, usize>,
    pub max_gap_ms: u64,
    pub issues: Vec<String>,
    #[serde(default)]
    pub bound_port: Option<u16>,
    #[serde(default)]
    pub live: bool,
    #[serde(default)]
    pub live_event_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiPath {
    pub path: String,
    pub method: String,
    pub operation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiDiffReport {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
    pub breaking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActualInteraction {
    pub method: String,
    pub path: String,
    pub status: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiTrafficReport {
    pub matched: usize,
    pub unmatched: Vec<String>,
    pub undeclared_methods: Vec<String>,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OauthFlowStep {
    pub step: String,
    pub url: String,
    pub params: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OauthFlowReport {
    pub steps: usize,
    pub issues: Vec<String>,
    pub has_pkce: bool,
    pub has_state: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueHop {
    pub from: String,
    pub to: String,
    pub latency_ms: f64,
    pub topic: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueAdjacencyReport {
    pub nodes: Vec<String>,
    pub edges: Vec<(String, String)>,
    pub hottest_topic: Option<String>,
    pub max_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcStreamFrame {
    pub stream_id: u32,
    pub flags: String,
    pub message_bytes: usize,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcStreamReport {
    pub frames: usize,
    pub total_bytes: usize,
    pub streams: usize,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StubRouteSpec {
    pub method: String,
    pub path: String,
    pub status: u16,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveStubReport {
    pub routes: usize,
    pub collisions: Vec<String>,
    pub open_paths: Vec<String>,
    #[serde(default)]
    pub bound_port: Option<u16>,
    #[serde(default)]
    pub live: bool,
    #[serde(default)]
    pub hit_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderContract {
    pub required: Vec<String>,
    pub forbidden: Vec<String>,
    pub patterns: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderFuzzCase {
    pub mutation: String,
    pub headers: BTreeMap<String, String>,
    /// HTTP status when `execute` ran this case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    /// Transport / consent error when execute failed for this case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderFuzzReport {
    pub cases: Vec<HeaderFuzzCase>,
    /// True when cases were sent via consent-gated `send_blocking`.
    #[serde(default)]
    pub executed: bool,
}

impl BackendEngine {
    pub fn new() -> Self {
        Self {
            http: std::sync::OnceLock::new(),
            history: Mutex::new(Vec::new()),
            vault: Mutex::new(HashMap::new()),
            webhooks: Mutex::new(Vec::new()),
            webhook_sink: Mutex::new(None),
            mocks: Mutex::new(Vec::new()),
            chaos: Mutex::new(Vec::new()),
            live_stub: Mutex::new(None),
            sse_lab: Mutex::new(None),
        }
    }

    // 1. API Workbench ------------------------------------------------------
    /// Resolve `{{var}}` templates in URL/headers/body against a variable map.
    pub fn build_request(&self, mut req: ApiRequest, vars: &HashMap<String, String>) -> ApiRequest {
        req.url = substitute(&req.url, vars);
        req.body = req.body.map(|b| substitute(&b, vars));
        req.headers = req
            .headers
            .into_iter()
            .map(|(k, v)| (k, substitute(&v, vars)))
            .collect();
        req
    }

    /// Convert a workbench request to a tamper-engine [`InterceptedRequest`].
    pub fn to_intercepted(&self, req: &ApiRequest, id: &str) -> InterceptedRequest {
        InterceptedRequest {
            id: id.to_string(),
            timestamp: now_ms(),
            method: req.method.clone(),
            url: req.url.clone(),
            headers: req.headers.clone(),
            body: req.body.clone(),
            is_modified: false,
        }
    }

    /// Send a request for real over HTTP and record it in the workbench history.
    ///
    /// Any registered chaos rule matching the URL is applied first: injected
    /// latency is slept and a forced fault short-circuits the real request with a
    /// synthetic error response (deterministic per request id — no fake RNG).
    pub fn send_blocking(&self, req: &ApiRequest) -> Result<ApiResponse, String> {
        let request_id = format!("{} {}", req.method, req.url);
        let chaos = self.apply_chaos(&req.url, &request_id);
        if chaos.added_latency_ms > 0 {
            std::thread::sleep(Duration::from_millis(chaos.added_latency_ms));
        }
        if let Some(status) = chaos.forced_status {
            let resp = ApiResponse {
                status,
                headers: HashMap::new(),
                body: "{\"thor_chaos\":\"injected fault\"}".to_string(),
                latency_ms: chaos.added_latency_ms as f64,
            };
            self.history.lock().push((req.clone(), resp.clone()));
            return Ok(resp);
        }
        let resp = crate::api::blocking::run(self.send(req))??;
        self.history.lock().push((req.clone(), resp.clone()));
        Ok(resp)
    }

    pub async fn send(&self, req: &ApiRequest) -> Result<ApiResponse, String> {
        let mut request = crate::api::RequestSpec::new(&req.method, &req.url);
        request.headers = req.headers.iter()
            .map(|(name, value)| crate::api::Header::new(name, value)).collect();
        if let Some(text) = &req.body {
            let content_type = req.headers.iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                .map(|(_, value)| value.clone()).unwrap_or_else(|| "text/plain".into());
            request.body = crate::api::RequestBody::Text { text: text.clone(), content_type };
        }
        let response = self.http_service()?.send(&request, &crate::api::Variables::default())
            .await.map_err(|error| error.to_string())?;
        if response.body.encoding != "utf8" {
            return Err("Binary response requires the API workbench binary viewer".into());
        }
        Ok(ApiResponse {
            status: response.status,
            headers: response.headers.into_iter().map(|header| (header.name, header.value)).collect(),
            body: response.body.data,
            latency_ms: response.total_ms,
        })
    }

    /// Shared connection pool used by feature execution and the API workbench.
    pub fn http_service(&self) -> Result<&crate::api::HttpService, String> {
        self.http.get_or_init(|| crate::api::HttpService::new().map_err(|error| error.to_string()))
            .as_ref().map_err(Clone::clone)
    }

    pub fn history(&self) -> Vec<(ApiRequest, ApiResponse)> {
        self.history.lock().clone()
    }

    // 2. Auth Vault ---------------------------------------------------------
    pub fn store_auth(&self, name: &str, scheme: AuthScheme) {
        self.vault.lock().insert(name.to_string(), scheme);
    }

    pub fn auth_header(&self, name: &str) -> Option<(String, String)> {
        let scheme = self.vault.lock().get(name)?.clone();
        Some(match scheme {
            AuthScheme::Bearer { token } => ("Authorization".into(), format!("Bearer {token}")),
            AuthScheme::Basic { username, password } => {
                let enc = base64_encode(format!("{username}:{password}").as_bytes());
                ("Authorization".into(), format!("Basic {enc}"))
            }
            AuthScheme::ApiKey { header, key } => (header, key),
            AuthScheme::WebhookSecret { secret } => ("X-Thor-Webhook-Secret".into(), secret),
        })
    }

    /// Resolve a stored webhook HMAC secret from the auth vault (by context name).
    pub fn webhook_secret(&self, name: &str) -> Option<String> {
        match self.vault.lock().get(name)? {
            AuthScheme::WebhookSecret { secret } => Some(secret.clone()),
            _ => None,
        }
    }

    pub fn apply_auth(&self, name: &str, req: &mut ApiRequest) -> bool {
        if let Some((k, v)) = self.auth_header(name) {
            req.headers.insert(k, v);
            true
        } else {
            false
        }
    }

    /// All stored auth context ids (sorted), for the multi-context IDOR matrix.
    pub fn auth_contexts(&self) -> Vec<String> {
        let mut v: Vec<String> = self.vault.lock().keys().cloned().collect();
        v.sort();
        v
    }

    /// Actually send `req` once per auth-vault `context_id`, then classify the
    /// resulting status matrix for IDOR/BOLA. `owner_context` (if any) is the
    /// legitimate owner of the resource; every other context receiving 2xx is a
    /// broken-object-level-authorization finding.
    pub fn run_idor_matrix(
        &self,
        req: &ApiRequest,
        contexts: &[String],
        owner_context: Option<&str>,
    ) -> IdorMatrixReport {
        let mut probes = Vec::new();
        for ctx in contexts {
            let mut r = req.clone();
            let _ = self.apply_auth(ctx, &mut r);
            let status = self.send_blocking(&r).map(|resp| resp.status).unwrap_or(0);
            let is_owner = owner_context.map(|o| o == ctx).unwrap_or(false);
            probes.push(IdorProbe {
                identity: ctx.clone(),
                resource: req.url.clone(),
                is_owner,
                status,
            });
        }
        let findings = self.analyze_idor(&probes);
        IdorMatrixReport { probes, findings }
    }

    /// Execute a request chain end-to-end: resolve variables, send each step for
    /// real, capture response fields into the variable map, and thread them into
    /// downstream steps.
    pub fn run_chain(&self, steps: &[ChainStep]) -> Result<ChainRunResult, String> {
        self.resolve_chain(steps)?;
        let mut vars: HashMap<String, String> = HashMap::new();
        let mut results = Vec::new();
        let mut ok = true;
        for step in steps {
            let req = self.build_request(step.request.clone(), &vars);
            let resp = self.send_blocking(&req)?;
            let mut captured = Vec::new();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body) {
                self.apply_captures(step, &json, &mut vars);
                captured = step.captures.keys().cloned().collect();
            }
            if !(200..300).contains(&resp.status) {
                ok = false;
            }
            results.push(ChainStepResult {
                name: step.name.clone(),
                status: resp.status,
                latency_ms: resp.latency_ms,
                captured,
            });
        }
        Ok(ChainRunResult {
            steps: results,
            vars,
            ok,
        })
    }

    /// Convert recorded tamper-engine traffic history into replayable mock routes
    /// (one per request/response pair). Returns the number of mocks added.
    pub fn mock_from_tamper_history(
        &self,
        history: &[(InterceptedRequest, Option<InterceptedResponse>)],
    ) -> usize {
        let mut added = 0;
        for (req, resp) in history {
            if let Some(r) = resp {
                self.add_mock(MockRoute {
                    method: req.method.clone(),
                    path_pattern: url_path(&req.url),
                    status: r.status_code,
                    body: r.body.clone().unwrap_or_default(),
                });
                added += 1;
            }
        }
        added
    }

    // 4b. Live Webhook Sink -------------------------------------------------
    /// Start a real localhost HTTP listener that records every inbound request
    /// as a [`WebhookEvent`]. Pass `0` to bind an ephemeral port. Returns the
    /// bound port. Any previous sink is stopped first.
    pub fn start_webhook_sink(&self, port: u16) -> Result<u16, String> {
        let mut slot = self.webhook_sink.lock();
        if let Some(h) = slot.take() {
            h.running.store(false, Ordering::SeqCst);
        }
        let listener =
            TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("bind failed: {e}"))?;
        let bound = listener
            .local_addr()
            .map_err(|e| e.to_string())?
            .port();
        listener
            .set_nonblocking(true)
            .map_err(|e| e.to_string())?;
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();
        std::thread::spawn(move || webhook_sink_loop(listener, flag));
        *slot = Some(WebhookSinkHandle {
            port: bound,
            running,
        });
        Ok(bound)
    }

    pub fn stop_webhook_sink(&self) {
        if let Some(h) = self.webhook_sink.lock().take() {
            h.running.store(false, Ordering::SeqCst);
        }
    }

    pub fn webhook_sink_port(&self) -> Option<u16> {
        self.webhook_sink.lock().as_ref().map(|h| h.port)
    }

    // 3. Contract Validation ------------------------------------------------
    /// Validate `value` against a minimal JSON schema (`type`, `required`,
    /// `properties`, `items`). Returns a list of human-readable violations.
    pub fn validate_contract(
        &self,
        value: &serde_json::Value,
        schema: &serde_json::Value,
    ) -> Vec<String> {
        let mut errs = Vec::new();
        validate_json(value, schema, "$", &mut errs);
        errs
    }

    // 4. Webhook Sink -------------------------------------------------------
    pub fn record_webhook(&self, event: WebhookEvent) {
        self.webhooks.lock().push(event);
    }

    pub fn webhooks(&self) -> Vec<WebhookEvent> {
        self.webhooks.lock().clone()
    }

    pub fn webhooks_matching(&self, path_contains: &str) -> Vec<WebhookEvent> {
        self.webhooks
            .lock()
            .iter()
            .filter(|w| w.path.contains(path_contains))
            .cloned()
            .collect()
    }

    pub fn clear_webhooks(&self) {
        self.webhooks.lock().clear();
    }

    // 5. IDOR Matrix --------------------------------------------------------
    pub fn analyze_idor(&self, probes: &[IdorProbe]) -> Vec<IdorFinding> {
        let mut findings = Vec::new();
        for p in probes {
            // A non-owner receiving a success status indicates broken object-level
            // authorization (IDOR / BOLA).
            if !p.is_owner && (200..300).contains(&p.status) {
                findings.push(IdorFinding {
                    identity: p.identity.clone(),
                    resource: p.resource.clone(),
                    status: p.status,
                    severity: "critical".into(),
                    detail: "non-owner identity received a 2xx on another user's resource".into(),
                });
            }
            // An owner being denied their own resource is a false-positive authz bug.
            if p.is_owner && (p.status == 401 || p.status == 403) {
                findings.push(IdorFinding {
                    identity: p.identity.clone(),
                    resource: p.resource.clone(),
                    status: p.status,
                    severity: "medium".into(),
                    detail: "owner denied access to their own resource".into(),
                });
            }
        }
        findings
    }

    // 6. Request Chaining ---------------------------------------------------
    /// Statically resolve which variables each step captures and validate that
    /// every `{{var}}` referenced downstream is produced by an earlier step.
    pub fn resolve_chain(&self, steps: &[ChainStep]) -> Result<Vec<String>, String> {
        let mut available: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut order = Vec::new();
        for step in steps {
            for var in referenced_vars(&step.request.url).into_iter().chain(
                step.request
                    .body
                    .as_deref()
                    .map(referenced_vars)
                    .unwrap_or_default(),
            ) {
                if !available.contains(&var) {
                    return Err(format!(
                        "step '{}' references '{{{{{}}}}}' before it is captured",
                        step.name, var
                    ));
                }
            }
            for key in step.captures.keys() {
                available.insert(key.clone());
            }
            order.push(step.name.clone());
        }
        Ok(order)
    }

    /// Apply captured variables from a JSON response into the variable map.
    pub fn apply_captures(
        &self,
        step: &ChainStep,
        response_json: &serde_json::Value,
        vars: &mut HashMap<String, String>,
    ) {
        for (var, path) in &step.captures {
            if let Some(v) = json_path(response_json, path) {
                let s = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                vars.insert(var.clone(), s);
            }
        }
    }

    // 7. gRPC / GraphQL -----------------------------------------------------
    pub fn analyze_graphql(&self, query: &str) -> GraphQlAnalysis {
        let trimmed = query.trim();
        let operation_type = if trimmed.starts_with("mutation") {
            "mutation"
        } else if trimmed.starts_with("subscription") {
            "subscription"
        } else {
            "query"
        };
        // Operation name follows the keyword.
        let operation_name = trimmed
            .split_whitespace()
            .nth(1)
            .filter(|s| *s != "{" && !s.starts_with('{'))
            .map(|s| s.trim_end_matches('{').trim_end_matches('(').to_string())
            .filter(|s| !s.is_empty());

        let is_introspection = trimmed.contains("__schema") || trimmed.contains("__type");

        // Brace-depth walk to compute max nesting and top-level fields.
        let mut depth: i32 = 0;
        let mut max_depth = 0i32;
        let mut top_level = Vec::new();
        let mut token = String::new();
        for ch in trimmed.chars() {
            match ch {
                '{' => {
                    let t = token.trim().to_string();
                    if depth == 1 && !t.is_empty() {
                        top_level.push(last_ident(&t));
                    }
                    depth += 1;
                    max_depth = max_depth.max(depth);
                    token.clear();
                }
                '}' => {
                    depth -= 1;
                    token.clear();
                }
                '\n' | ',' => {
                    let t = token.trim().to_string();
                    if depth == 1 && !t.is_empty() {
                        top_level.push(last_ident(&t));
                    }
                    token.clear();
                }
                _ => token.push(ch),
            }
        }
        top_level.retain(|f| !f.is_empty() && !f.starts_with(operation_type));
        top_level.dedup();
        GraphQlAnalysis {
            operation_type: operation_type.to_string(),
            operation_name,
            top_level_fields: top_level,
            max_depth: max_depth.max(0) as usize,
            is_introspection,
        }
    }

    pub fn parse_grpc_path(&self, path: &str) -> Option<GrpcCall> {
        let clean = path.trim_start_matches('/');
        let (svc_full, method) = clean.split_once('/')?;
        let (package, service) = match svc_full.rsplit_once('.') {
            Some((p, s)) => (p.to_string(), s.to_string()),
            None => (String::new(), svc_full.to_string()),
        };
        if method.is_empty() || service.is_empty() {
            return None;
        }
        Some(GrpcCall {
            package,
            service,
            method: method.to_string(),
        })
    }

    // 8. Trace Adjacency ----------------------------------------------------
    pub fn build_trace_graph(&self, spans: &[Span]) -> TraceGraph {
        let by_id: HashMap<&str, &Span> = spans.iter().map(|s| (s.span_id.as_str(), s)).collect();
        let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut children: HashMap<&str, Vec<&Span>> = HashMap::new();
        for s in spans {
            if let Some(pid) = &s.parent_id {
                if let Some(parent) = by_id.get(pid.as_str()) {
                    if parent.service != s.service {
                        let e = edges.entry(parent.service.clone()).or_default();
                        if !e.contains(&s.service) {
                            e.push(s.service.clone());
                        }
                    }
                    children.entry(pid.as_str()).or_default().push(s);
                }
            }
        }
        // Critical path = longest cumulative duration root->leaf.
        let roots: Vec<&Span> = spans
            .iter()
            .filter(|s| {
                s.parent_id
                    .as_ref()
                    .map(|p| !by_id.contains_key(p.as_str()))
                    .unwrap_or(true)
            })
            .collect();
        let mut best_path = Vec::new();
        let mut best_ms = 0.0;
        for root in roots {
            let mut path = Vec::new();
            let ms = Self::longest_path(root, &children, &mut path);
            if ms > best_ms {
                best_ms = ms;
                best_path = path;
            }
        }
        TraceGraph {
            edges,
            critical_path: best_path,
            critical_path_ms: best_ms,
        }
    }

    fn longest_path(
        span: &Span,
        children: &HashMap<&str, Vec<&Span>>,
        path: &mut Vec<String>,
    ) -> f64 {
        path.push(format!("{}:{}", span.service, span.name));
        let mut best_child_ms = 0.0;
        let mut best_child_path: Vec<String> = Vec::new();
        if let Some(kids) = children.get(span.span_id.as_str()) {
            for k in kids {
                let mut cp = Vec::new();
                let ms = Self::longest_path(k, children, &mut cp);
                if ms > best_child_ms {
                    best_child_ms = ms;
                    best_child_path = cp;
                }
            }
        }
        path.extend(best_child_path);
        span.duration_ms + best_child_ms
    }

    // 9. Mock / Stub --------------------------------------------------------
    pub fn add_mock(&self, route: MockRoute) {
        self.mocks.lock().push(route);
    }

    pub fn clear_mocks(&self) {
        self.mocks.lock().clear();
    }

    pub fn match_mock(&self, method: &str, path: &str) -> Option<MockMatch> {
        for route in self.mocks.lock().iter() {
            if !route.method.eq_ignore_ascii_case(method) {
                continue;
            }
            if let Some(params) = match_path(&route.path_pattern, path) {
                return Some(MockMatch {
                    status: route.status,
                    body: route.body.clone(),
                    path_params: params,
                });
            }
        }
        None
    }

    // 10. Chaos Injection ---------------------------------------------------
    pub fn add_chaos_rule(&self, rule: ChaosRule) {
        self.chaos.lock().push(rule);
    }

    pub fn clear_chaos(&self) {
        self.chaos.lock().clear();
    }

    /// Decide chaos outcome deterministically from `request_id` (stable hash),
    /// so replays reproduce the exact same fault pattern (no fake RNG).
    pub fn apply_chaos(&self, url: &str, request_id: &str) -> ChaosOutcome {
        let mut outcome = ChaosOutcome {
            injected_fault: false,
            added_latency_ms: 0,
            forced_status: None,
        };
        for rule in self.chaos.lock().iter() {
            if !url.contains(&rule.url_pattern) {
                continue;
            }
            outcome.added_latency_ms += rule.added_latency_ms;
            if rule.error_rate > 0.0 {
                let bucket = (fnv1a(request_id) % 10_000) as f64 / 10_000.0;
                if bucket < rule.error_rate {
                    outcome.injected_fault = true;
                    outcome.forced_status = Some(if rule.error_status > 0 {
                        rule.error_status
                    } else {
                        503
                    });
                }
            }
        }
        outcome
    }

    // 11. Idempotency Auditor ----------------------------------------------
    pub fn audit_idempotency(&self, events: &[IdempotencyEvent]) -> Vec<IdempotencyFinding> {
        let mut findings = Vec::new();
        let mut seen_keys: HashMap<String, usize> = HashMap::new();
        let mut unsafe_no_key: HashMap<(String, String), usize> = HashMap::new();
        for e in events {
            let method = e.method.to_uppercase();
            let is_unsafe = matches!(method.as_str(), "POST" | "PATCH");
            match &e.idempotency_key {
                Some(k) => {
                    *seen_keys.entry(k.clone()).or_default() += 1;
                }
                None if is_unsafe => {
                    *unsafe_no_key
                        .entry((method.clone(), e.url.clone()))
                        .or_default() += 1;
                }
                None => {}
            }
        }
        for ((method, url), count) in unsafe_no_key {
            if count > 1 {
                findings.push(IdempotencyFinding {
                    url,
                    method,
                    severity: "high".into(),
                    detail: format!(
                        "{count} non-idempotent requests without an Idempotency-Key (retry unsafe)"
                    ),
                });
            }
        }
        for (key, count) in seen_keys {
            if count > 1 {
                findings.push(IdempotencyFinding {
                    url: format!("key={key}"),
                    method: "*".into(),
                    severity: "info".into(),
                    detail: format!("idempotency key reused {count}x (expected for safe retries)"),
                });
            }
        }
        findings
    }

    // 12. DB Adjacency ------------------------------------------------------
    pub fn analyze_db(&self, queries: &[String]) -> DbReport {
        let mut adjacency: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut templates: HashMap<String, usize> = HashMap::new();
        for q in queries {
            let tables = extract_tables(q);
            for i in 0..tables.len() {
                for j in 0..tables.len() {
                    if i != j {
                        let e = adjacency.entry(tables[i].clone()).or_default();
                        if !e.contains(&tables[j]) {
                            e.push(tables[j].clone());
                        }
                    }
                }
            }
            *templates.entry(normalize_sql(q)).or_default() += 1;
        }
        let mut n_plus_one: Vec<QueryCount> = templates
            .into_iter()
            .filter(|(_, c)| *c >= 3)
            .map(|(template, count)| QueryCount { template, count })
            .collect();
        n_plus_one.sort_by(|a, b| b.count.cmp(&a.count));
        DbReport {
            adjacency,
            n_plus_one,
            total_queries: queries.len(),
        }
    }

    /// DB adjacency analysis with a cache-key ↔ table adjacency graph layered
    /// alongside, so cache access can be reasoned about next to the queries it
    /// fronts (hit/miss ratio + which tables each key shields).
    pub fn analyze_db_cache(&self, queries: &[String], cache_ops: &[CacheOp]) -> DbCacheReport {
        let db = self.analyze_db(queries);
        let mut cache_adjacency: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut hits = 0usize;
        let mut misses = 0usize;
        for op in cache_ops {
            if op.hit {
                hits += 1;
            } else {
                misses += 1;
            }
            if let Some(table) = &op.table {
                let e = cache_adjacency.entry(op.key.clone()).or_default();
                if !e.contains(table) {
                    e.push(table.clone());
                }
            }
        }
        DbCacheReport {
            db,
            cache_adjacency,
            cache_hits: hits,
            cache_misses: misses,
        }
    }

    // ---- Wave-2 backend ---------------------------------------------------

    pub fn webhook_signature_vault(&self, input: &WebhookSigInput) -> WebhookSigReport {
        let mut secret = input.secret.clone();
        // Prefer vault-stored WebhookSecret when `secret` looks like a context name.
        if secret.is_empty() || (!secret.contains(' ') && secret.len() < 64) {
            if let Some(stored) = self.webhook_secret(&input.secret) {
                secret = stored;
            }
        }
        let algo = input.algorithm.to_lowercase();
        let signed = if let Some(ts) = input.timestamp {
            format!("{}.{}", ts, input.payload)
        } else {
            input.payload.clone()
        };
        let mac = crate::features::security::hmac_sha256(secret.as_bytes(), signed.as_bytes());
        let hex = crate::features::security::hex_encode(&mac);
        let expected = if algo.contains("sha256") || algo == "hmac" {
            format!("sha256={hex}")
        } else {
            format!("{algo}={hex}")
        };
        let provided = input.provided_signature.trim().to_lowercase();
        let expected_l = expected.to_lowercase();
        let valid = provided == expected_l
            || provided == hex
            || provided.strip_prefix("sha256=").map(|s| s == hex.as_str()).unwrap_or(false)
            || provided.strip_prefix("v1=").map(|s| s == hex.as_str()).unwrap_or(false);
        let mut timestamp_ok = true;
        if let (Some(ts), Some(now)) = (input.timestamp, input.now) {
            timestamp_ok = (now - ts).abs() <= input.tolerance_secs;
        }
        WebhookSigReport {
            valid: valid && timestamp_ok,
            expected_prefix: expected.chars().take(20).collect(),
            timestamp_ok,
            detail: if valid && timestamp_ok {
                "hmac-sha256 signature ok".into()
            } else if !timestamp_ok {
                "timestamp outside tolerance".into()
            } else {
                "signature mismatch".into()
            },
        }
    }

    pub fn rate_limit_probe(&self, samples: &[RateLimitSample]) -> RateLimitReport {
        let limited: Vec<_> = samples.iter().filter(|s| s.status == 429 || s.remaining == Some(0)).collect();
        let inferred_limit = samples.iter().filter_map(|s| s.limit).max();
        let retries: Vec<f64> = limited.iter().filter_map(|s| s.retry_after_secs).collect();
        let avg_retry_after = if retries.is_empty() {
            None
        } else {
            Some(retries.iter().sum::<f64>() / retries.len() as f64)
        };
        let mut issues = Vec::new();
        if !limited.is_empty() && retries.is_empty() {
            issues.push("429/exhausted without Retry-After".into());
        }
        for s in samples {
            if let (Some(rem), Some(lim)) = (s.remaining, s.limit) {
                if rem > lim {
                    issues.push(format!("remaining {rem} exceeds limit {lim}"));
                }
            }
        }
        RateLimitReport {
            limited_count: limited.len(),
            inferred_limit,
            avg_retry_after,
            issues,
        }
    }

    /// Hit `url` repeatedly via reqwest; consent required for public targets.
    pub fn rate_limit_probe_http(
        &self,
        url: &str,
        count: usize,
        lab_consent: bool,
    ) -> Result<RateLimitReport, String> {
        crate::net::consent::require_lab_consent(url, lab_consent)?;
        let n = count.clamp(1, 50);
        let mut samples = Vec::with_capacity(n);
        for i in 0..n {
            let resp = self.send_blocking(&ApiRequest {
                method: "GET".into(),
                url: url.to_string(),
                headers: HashMap::new(),
                body: None,
            })?;
            let remaining = resp
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("x-ratelimit-remaining"))
                .and_then(|(_, v)| v.parse().ok());
            let limit = resp
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("x-ratelimit-limit"))
                .and_then(|(_, v)| v.parse().ok());
            let retry_after_secs = resp
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))
                .and_then(|(_, v)| v.parse().ok());
            samples.push(RateLimitSample {
                status: resp.status,
                retry_after_secs,
                remaining,
                limit,
                timestamp_ms: i as u64,
            });
        }
        Ok(self.rate_limit_probe(&samples))
    }

    pub fn pagination_auditor(&self, pages: &[PageSlice]) -> PaginationReport {
        let mut issues = Vec::new();
        let mut estimated_total = None;
        for (i, p) in pages.iter().enumerate() {
            if p.page_size == 0 {
                issues.push(format!("page {i}: zero page_size"));
            }
            if p.items > p.page_size as usize {
                issues.push(format!(
                    "page {}: items {} > page_size {}",
                    p.page, p.items, p.page_size
                ));
            }
            if let Some(t) = p.total {
                estimated_total = Some(t);
            }
            if i + 1 < pages.len() {
                let n = &pages[i + 1];
                if p.next_cursor.is_none() && n.page != p.page + 1 && p.page_size > 0
                    && n.page != p.page + 1 {
                        issues.push(format!(
                            "non-sequential page jump {} -> {}",
                            p.page, n.page
                        ));
                    }
            }
        }
        if let Some(last) = pages.last() {
            if last.items == last.page_size as usize && last.next_cursor.is_none() && last.total.is_none()
            {
                issues.push("last page full-sized without next cursor/total — may be truncated".into());
            }
        }
        PaginationReport {
            pages: pages.len(),
            issues,
            estimated_total,
        }
    }

    /// Fetch paginated URLs via reqwest and analyze JSON arrays / `items` fields.
    pub fn pagination_auditor_http(
        &self,
        urls: &[String],
        page_size_hint: u32,
        lab_consent: bool,
    ) -> Result<PaginationReport, String> {
        for u in urls {
            crate::net::consent::require_lab_consent(u, lab_consent)?;
        }
        let mut pages = Vec::new();
        for (i, url) in urls.iter().enumerate() {
            let resp = self.send_blocking(&ApiRequest {
                method: "GET".into(),
                url: url.clone(),
                headers: HashMap::new(),
                body: None,
            })?;
            let items = count_page_items(&resp.body);
            let total = serde_json::from_str::<serde_json::Value>(&resp.body)
                .ok()
                .and_then(|v| {
                    v.get("total")
                        .or_else(|| v.get("count"))
                        .and_then(|t| t.as_u64())
                });
            let next_cursor = serde_json::from_str::<serde_json::Value>(&resp.body)
                .ok()
                .and_then(|v| {
                    v.get("next")
                        .or_else(|| v.get("next_cursor"))
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                });
            pages.push(PageSlice {
                page: (i as u32) + 1,
                page_size: page_size_hint.max(1),
                items,
                total,
                next_cursor,
            });
        }
        Ok(self.pagination_auditor(&pages))
    }

    pub fn cache_semantics_lens(&self, entries: &[CacheSemanticsEntry]) -> CacheSemanticsReport {
        let mut findings = Vec::new();
        let mut cacheable = 0usize;
        let mut non_cacheable = 0usize;
        for e in entries {
            let cc = e.cache_control.to_lowercase();
            let no_store = cc.contains("no-store") || cc.contains("private");
            let method_ok = matches!(e.method.to_uppercase().as_str(), "GET" | "HEAD");
            if no_store || !method_ok || e.status >= 400 {
                non_cacheable += 1;
                if method_ok && e.status < 400 && cc.is_empty() {
                    findings.push(format!("{} missing Cache-Control", e.url));
                }
            } else {
                cacheable += 1;
                if e.etag.is_none() && !cc.contains("max-age") {
                    findings.push(format!("{} cacheable without ETag or max-age", e.url));
                }
            }
            if let Some(v) = &e.vary {
                if v.to_lowercase().contains("*") {
                    findings.push(format!("{} Vary: * disables shared caches", e.url));
                }
            }
        }
        CacheSemanticsReport {
            findings,
            cacheable,
            non_cacheable,
        }
    }

    pub fn multipart_workbench(&self, parts: &[MultipartPart]) -> MultipartReport {
        let mut issues = Vec::new();
        let total_bytes: usize = parts.iter().map(|p| p.size).sum();
        let mut names = BTreeMap::<String, usize>::new();
        for p in parts {
            *names.entry(p.name.clone()).or_default() += 1;
            if p.filename.is_some() && p.content_type.is_none() {
                issues.push(format!("part '{}' file missing content-type", p.name));
            }
            if p.size > 10_000_000 {
                issues.push(format!("part '{}' exceeds 10MB ({})", p.name, p.size));
            }
            if p.name.is_empty() {
                issues.push("unnamed multipart part".into());
            }
        }
        for (n, c) in names {
            if c > 1 {
                issues.push(format!("duplicate part name '{n}' x{c}"));
            }
        }
        MultipartReport {
            parts: parts.len(),
            total_bytes,
            issues,
        }
    }

    pub fn sse_ws_server_lab(&self, events: &[StreamLabEvent]) -> SseWsLabReport {
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        let mut max_gap = 0u64;
        let mut issues = Vec::new();
        for e in events {
            *kinds.entry(e.kind.clone()).or_default() += 1;
            if e.payload.is_empty() {
                issues.push(format!("empty payload for kind {}", e.kind));
            }
        }
        for w in events.windows(2) {
            let gap = w[1].timestamp_ms.saturating_sub(w[0].timestamp_ms);
            max_gap = max_gap.max(gap);
        }
        if max_gap > 30_000 {
            issues.push(format!("max inter-event gap {max_gap}ms (>30s)"));
        }
        let (bound_port, live, live_event_count) = {
            let slot = self.sse_lab.lock();
            match slot.as_ref() {
                Some(h) => (
                    Some(h.port),
                    true,
                    h.event_count.load(Ordering::SeqCst),
                ),
                None => (None, false, 0),
            }
        };
        SseWsLabReport {
            event_count: events.len(),
            kinds,
            max_gap_ms: max_gap,
            issues,
            bound_port,
            live,
            live_event_count,
        }
    }

    /// Start SSE lab on 127.0.0.1 only. Non-local advertise URLs are refused.
    pub fn start_sse_ws_server_lab(
        &self,
        port: u16,
        events: Vec<StreamLabEvent>,
        advertise_url: Option<&str>,
    ) -> Result<u16, String> {
        if let Some(url) = advertise_url {
            if !crate::net::consent::is_private_or_local_url(url) {
                return Err(format!(
                    "SSE/WS lab refuses non-local advertise URL: {url}"
                ));
            }
        }
        let mut slot = self.sse_lab.lock();
        if let Some(h) = slot.take() {
            h.running.store(false, Ordering::SeqCst);
        }
        let listener =
            TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("bind failed: {e}"))?;
        let bound = listener
            .local_addr()
            .map_err(|e| e.to_string())?
            .port();
        listener
            .set_nonblocking(true)
            .map_err(|e| e.to_string())?;
        let running = Arc::new(AtomicBool::new(true));
        let event_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let events = Arc::new(Mutex::new(events));
        let flag = running.clone();
        let ec = event_count.clone();
        let ev = events.clone();
        std::thread::spawn(move || sse_lab_loop(listener, flag, ec, ev));
        *slot = Some(SseLabHandle {
            port: bound,
            running,
            event_count,
            events,
        });
        Ok(bound)
    }

    pub fn stop_sse_ws_server_lab(&self) {
        if let Some(h) = self.sse_lab.lock().take() {
            h.running.store(false, Ordering::SeqCst);
        }
    }

    pub fn sse_ws_server_lab_port(&self) -> Option<u16> {
        self.sse_lab.lock().as_ref().map(|h| h.port)
    }

    pub fn openapi_diff_gate(
        &self,
        before: &[OpenApiPath],
        after: &[OpenApiPath],
    ) -> OpenApiDiffReport {
        let key = |p: &OpenApiPath| format!("{} {}", p.method.to_uppercase(), p.path);
        let bset: BTreeMap<String, &OpenApiPath> = before.iter().map(|p| (key(p), p)).collect();
        let aset: BTreeMap<String, &OpenApiPath> = after.iter().map(|p| (key(p), p)).collect();
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();
        for (k, p) in &aset {
            if !bset.contains_key(k) {
                added.push(k.clone());
            } else if bset[k].operation_id != p.operation_id {
                changed.push(format!("{k} operationId drift"));
            }
        }
        for k in bset.keys() {
            if !aset.contains_key(k) {
                removed.push(k.clone());
            }
        }
        let breaking = !removed.is_empty();
        OpenApiDiffReport {
            added,
            removed,
            changed,
            breaking,
        }
    }

    /// Parse OpenAPI 3.x / Swagger 2 JSON into path+method rows.
    pub fn parse_openapi_paths(doc: &serde_json::Value) -> Vec<OpenApiPath> {
        let mut out = Vec::new();
        let paths = match doc.get("paths").and_then(|p| p.as_object()) {
            Some(p) => p,
            None => return out,
        };
        for (path, item) in paths {
            let Some(obj) = item.as_object() else { continue };
            for (method, op) in obj {
                let m = method.to_lowercase();
                if !matches!(
                    m.as_str(),
                    "get" | "post" | "put" | "patch" | "delete" | "head" | "options" | "trace"
                ) {
                    continue;
                }
                let operation_id = op
                    .get("operationId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                out.push(OpenApiPath {
                    path: path.clone(),
                    method: m.to_uppercase(),
                    operation_id,
                });
            }
        }
        out
    }

    /// Validate captured HTTP interactions against declared OpenAPI paths.
    pub fn openapi_validate_traffic(
        &self,
        paths: &[OpenApiPath],
        interactions: &[ActualInteraction],
    ) -> OpenApiTrafficReport {
        let declared: BTreeMap<String, &OpenApiPath> = paths
            .iter()
            .map(|p| (format!("{} {}", p.method.to_uppercase(), normalize_api_path(&p.path)), p))
            .collect();
        let mut matched = 0usize;
        let mut unmatched = Vec::new();
        let mut undeclared_methods = Vec::new();
        for i in interactions {
            let path = normalize_api_path(&i.path);
            let key = format!("{} {}", i.method.to_uppercase(), path);
            if declared.contains_key(&key) {
                matched += 1;
            } else {
                // Same path, different method?
                let path_exists = paths.iter().any(|p| normalize_api_path(&p.path) == path);
                if path_exists {
                    undeclared_methods.push(key.clone());
                }
                unmatched.push(key);
            }
        }
        let ok = unmatched.is_empty();
        OpenApiTrafficReport {
            matched,
            unmatched,
            undeclared_methods,
            ok,
        }
    }

    pub fn oauth_flow_capture(&self, steps: &[OauthFlowStep]) -> OauthFlowReport {
        let mut issues = Vec::new();
        let mut has_pkce = false;
        let mut has_state = false;
        for s in steps {
            if s.params.contains_key("code_challenge") || s.params.contains_key("code_verifier") {
                has_pkce = true;
            }
            if s.params.contains_key("state") {
                has_state = true;
            }
            if s.step == "authorize" && !s.params.contains_key("redirect_uri") {
                issues.push("authorize missing redirect_uri".into());
            }
            if s.step == "token" && s.params.get("grant_type").map(|g| g.as_str()) == Some("authorization_code")
                && !s.params.contains_key("code")
            {
                issues.push("token exchange missing code".into());
            }
            if s.url.contains("http://") && !s.url.contains("localhost") && !s.url.contains("127.0.0.1") {
                issues.push(format!("insecure http endpoint: {}", s.url));
            }
        }
        if !has_state {
            issues.push("flow missing CSRF state parameter".into());
        }
        if !has_pkce {
            issues.push("flow missing PKCE".into());
        }
        OauthFlowReport {
            steps: steps.len(),
            issues,
            has_pkce,
            has_state,
        }
    }

    pub fn queue_adjacency(&self, hops: &[QueueHop]) -> QueueAdjacencyReport {
        let mut nodes = BTreeMap::<String, ()>::new();
        let mut edges = Vec::new();
        let mut topic_hits: BTreeMap<String, usize> = BTreeMap::new();
        let mut max_latency = 0.0f64;
        for h in hops {
            nodes.insert(h.from.clone(), ());
            nodes.insert(h.to.clone(), ());
            edges.push((h.from.clone(), h.to.clone()));
            *topic_hits.entry(h.topic.clone()).or_default() += 1;
            max_latency = max_latency.max(h.latency_ms);
        }
        let hottest_topic = topic_hits
            .into_iter()
            .max_by_key(|(_, c)| *c)
            .map(|(t, _)| t);
        QueueAdjacencyReport {
            nodes: nodes.into_keys().collect(),
            edges,
            hottest_topic,
            max_latency_ms: max_latency,
        }
    }

    pub fn grpc_stream_workbench(&self, frames: &[GrpcStreamFrame]) -> GrpcStreamReport {
        let mut streams = BTreeMap::<u32, usize>::new();
        let mut total_bytes = 0usize;
        let mut issues = Vec::new();
        for f in frames {
            *streams.entry(f.stream_id).or_default() += 1;
            total_bytes += f.message_bytes;
            if f.flags.contains("END") && f.message_bytes == 0 && streams[&f.stream_id] == 1 {
                issues.push(format!("stream {} ended with empty sole frame", f.stream_id));
            }
        }
        for w in frames.windows(2) {
            if w[0].stream_id == w[1].stream_id
                && w[1].timestamp_ms < w[0].timestamp_ms
            {
                issues.push(format!("out-of-order timestamps on stream {}", w[0].stream_id));
            }
        }
        GrpcStreamReport {
            frames: frames.len(),
            total_bytes,
            streams: streams.len(),
            issues,
        }
    }

    pub fn live_stub_server(&self, routes: &[StubRouteSpec]) -> LiveStubReport {
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        let mut collisions = Vec::new();
        let mut open_paths = Vec::new();
        for r in routes {
            let k = format!("{} {}", r.method.to_uppercase(), r.path);
            let c = seen.entry(k.clone()).or_insert(0);
            *c += 1;
            if *c == 2 {
                collisions.push(k.clone());
            }
            if r.path.contains("*") || r.path.contains("{") {
                open_paths.push(k);
            }
        }
        let (bound_port, live, hit_count) = {
            let slot = self.live_stub.lock();
            match slot.as_ref() {
                Some(h) => (Some(h.port), true, h.hit_count.load(Ordering::SeqCst)),
                None => (None, false, 0),
            }
        };
        LiveStubReport {
            routes: routes.len(),
            collisions,
            open_paths,
            bound_port,
            live,
            hit_count,
        }
    }

    /// Bind a stub HTTP server on 127.0.0.1. Refuses non-local advertise URLs.
    pub fn start_live_stub_server(
        &self,
        port: u16,
        routes: Vec<StubRouteSpec>,
        advertise_url: Option<&str>,
    ) -> Result<u16, String> {
        if let Some(url) = advertise_url {
            if !crate::net::consent::is_private_or_local_url(url) {
                return Err(format!(
                    "live stub refuses non-local advertise URL: {url}"
                ));
            }
        }
        let mut slot = self.live_stub.lock();
        if let Some(h) = slot.take() {
            h.running.store(false, Ordering::SeqCst);
        }
        let listener =
            TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("bind failed: {e}"))?;
        let bound = listener
            .local_addr()
            .map_err(|e| e.to_string())?
            .port();
        listener
            .set_nonblocking(true)
            .map_err(|e| e.to_string())?;
        let running = Arc::new(AtomicBool::new(true));
        let hit_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let routes = Arc::new(Mutex::new(routes));
        let flag = running.clone();
        let hits = hit_count.clone();
        let rts = routes.clone();
        std::thread::spawn(move || live_stub_loop(listener, flag, hits, rts));
        *slot = Some(LiveStubHandle {
            port: bound,
            running,
            hit_count,
            routes,
        });
        Ok(bound)
    }

    pub fn stop_live_stub_server(&self) {
        if let Some(h) = self.live_stub.lock().take() {
            h.running.store(false, Ordering::SeqCst);
        }
    }

    pub fn live_stub_server_port(&self) -> Option<u16> {
        self.live_stub.lock().as_ref().map(|h| h.port)
    }

    pub fn header_contract_fuzzer(
        &self,
        base: &BTreeMap<String, String>,
        contract: &HeaderContract,
    ) -> HeaderFuzzReport {
        self.header_contract_fuzzer_gated(base, contract, None, false, false)
            .expect("non-execute header fuzz cannot fail")
    }

    /// Generate header mutation cases; when `execute` + `url`, consent-gated
    /// `send_blocking` records status per case.
    pub fn header_contract_fuzzer_gated(
        &self,
        base: &BTreeMap<String, String>,
        contract: &HeaderContract,
        url: Option<&str>,
        execute: bool,
        lab_consent: bool,
    ) -> Result<HeaderFuzzReport, String> {
        let mut cases = Vec::new();
        for reqh in &contract.required {
            let mut h = base.clone();
            h.remove(reqh);
            cases.push(HeaderFuzzCase {
                mutation: format!("drop:{reqh}"),
                headers: h,
                status: None,
                error: None,
            });
        }
        for forb in &contract.forbidden {
            let mut h = base.clone();
            h.insert(forb.clone(), "1".into());
            cases.push(HeaderFuzzCase {
                mutation: format!("inject:{forb}"),
                headers: h,
                status: None,
                error: None,
            });
        }
        for name in contract.patterns.keys() {
            let mut h = base.clone();
            h.insert(name.clone(), "!!!invalid!!!".into());
            cases.push(HeaderFuzzCase {
                mutation: format!("break-pattern:{name}"),
                headers: h,
                status: None,
                error: None,
            });
        }
        if let Some(ct) = base.get("content-type").cloned() {
            let mut h = base.clone();
            h.insert("Content-Type".into(), ct);
            h.remove("content-type");
            cases.push(HeaderFuzzCase {
                mutation: "case-smash:content-type".into(),
                headers: h,
                status: None,
                error: None,
            });
        }

        if !execute {
            return Ok(HeaderFuzzReport {
                cases,
                executed: false,
            });
        }
        let url = url
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .ok_or_else(|| "header-contract-fuzzer execute requires url".to_string())?;
        crate::net::consent::require_lab_consent(url, lab_consent)?;
        for case in &mut cases {
            let mut headers = HashMap::new();
            for (k, v) in &case.headers {
                headers.insert(k.clone(), v.clone());
            }
            let req = ApiRequest {
                method: "GET".into(),
                url: url.to_string(),
                headers,
                body: None,
            };
            match self.send_blocking(&req) {
                Ok(resp) => {
                    case.status = Some(resp.status);
                    case.error = None;
                }
                Err(e) => {
                    case.status = None;
                    case.error = Some(e);
                }
            }
        }
        Ok(HeaderFuzzReport {
            cases,
            executed: true,
        })
    }

}

/// Extract the path (no scheme/host/query) from a URL, defaulting to `/`.
fn url_path(url: &str) -> String {
    let after = url.split("://").nth(1).unwrap_or(url);
    let path = after.split('/').skip(1).collect::<Vec<_>>().join("/");
    let path = path.split('?').next().unwrap_or("");
    format!("/{path}")
}

/// Minimal blocking HTTP sink loop: records every inbound request as a
/// [`WebhookEvent`] on the process-wide backend engine and replies `200 OK`.
fn webhook_sink_loop(listener: TcpListener, running: Arc<AtomicBool>) {
    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let _ = stream.set_nonblocking(false);
                if let Some(event) = read_webhook_request(&mut stream)
                    .as_deref()
                    .and_then(parse_http_request)
                {
                    AppState::global().features().backend().record_webhook(event);
                }
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                );
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => break,
        }
    }
}

fn read_webhook_request(stream: &mut TcpStream) -> Option<String> {
    const MAX_REQUEST_BYTES: usize = 16 * 1024;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    let mut request = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let count = match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        };
        if request.len() + count > MAX_REQUEST_BYTES {
            return None;
        }
        request.extend_from_slice(&chunk[..count]);
        let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
            .map(|(_, value)| value.trim().parse::<usize>())
            .transpose()
            .ok()?
            .unwrap_or(0);
        let expected = header_end.checked_add(4)?.checked_add(content_length)?;
        if expected > MAX_REQUEST_BYTES {
            return None;
        }
        if request.len() >= expected {
            request.truncate(expected);
            return Some(String::from_utf8_lossy(&request).into_owned());
        }
    }
}

fn live_stub_loop(
    listener: TcpListener,
    running: Arc<AtomicBool>,
    hits: Arc<std::sync::atomic::AtomicU64>,
    routes: Arc<Mutex<Vec<StubRouteSpec>>>,
) {
    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                hits.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]);
                let (method, path) = parse_request_line(&raw);
                let route = {
                    let rts = routes.lock();
                    rts.iter()
                        .find(|r| {
                            r.method.eq_ignore_ascii_case(&method)
                                && (r.path == path
                                    || r.path == "*"
                                    || path_matches_template(&r.path, &path))
                        })
                        .cloned()
                };
                let (status, body) = match route {
                    Some(r) => (r.status, r.body),
                    None => (404, "{\"error\":\"not found\"}".into()),
                };
                let resp = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => break,
        }
    }
}

fn sse_lab_loop(
    listener: TcpListener,
    running: Arc<AtomicBool>,
    event_count: Arc<std::sync::atomic::AtomicU64>,
    events: Arc<Mutex<Vec<StreamLabEvent>>>,
) {
    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let snapshot = events.lock().clone();
                let mut body = String::from("retry: 1000\n\n");
                for e in &snapshot {
                    body.push_str(&format!("event: {}\ndata: {}\n\n", e.kind, e.payload));
                    event_count.fetch_add(1, Ordering::SeqCst);
                }
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => break,
        }
    }
}

fn parse_request_line(raw: &str) -> (String, String) {
    let line = raw.lines().next().unwrap_or("");
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let path = parts
        .next()
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/")
        .to_string();
    (method, path)
}

fn path_matches_template(template: &str, path: &str) -> bool {
    let t: Vec<&str> = template.trim_matches('/').split('/').collect();
    let p: Vec<&str> = path.trim_matches('/').split('/').collect();
    if t.len() != p.len() {
        return false;
    }
    t.iter().zip(p.iter()).all(|(a, b)| {
        a.starts_with('{') && a.ends_with('}') || *a == "*" || *a == *b
    })
}

fn count_page_items(body: &str) -> usize {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else {
        return 0;
    };
    if let Some(arr) = v.as_array() {
        return arr.len();
    }
    for key in ["items", "data", "results", "records"] {
        if let Some(arr) = v.get(key).and_then(|x| x.as_array()) {
            return arr.len();
        }
    }
    0
}

fn normalize_api_path(path: &str) -> String {
    let p = path.split('?').next().unwrap_or(path);
    if p.starts_with('/') {
        p.to_string()
    } else {
        format!("/{p}")
    }
}

/// Parse a raw HTTP/1.1 request into a [`WebhookEvent`].
fn parse_http_request(raw: &str) -> Option<WebhookEvent> {
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw, ""));
    let mut lines = head.lines();
    let request_line = lines.next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next().unwrap_or("/").to_string();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    Some(WebhookEvent {
        method,
        path,
        headers,
        body: body.to_string(),
        received_ms: now_ms(),
    })
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

fn substitute(input: &str, vars: &HashMap<String, String>) -> String {
    let mut out = input.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{k}}}}}"), v);
    }
    out
}

fn referenced_vars(input: &str) -> Vec<String> {
    let mut vars = Vec::new();
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        if let Some(end) = rest[start + 2..].find("}}") {
            let name = rest[start + 2..start + 2 + end].trim().to_string();
            if !name.is_empty() {
                vars.push(name);
            }
            rest = &rest[start + 2 + end + 2..];
        } else {
            break;
        }
    }
    vars
}

/// Dot-path lookup into JSON supporting `a.b.0.c` (numeric array indices).
pub fn json_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut cur = value;
    for seg in path.split('.').filter(|s| !s.is_empty()) {
        cur = match cur {
            serde_json::Value::Object(o) => o.get(seg)?,
            serde_json::Value::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

fn validate_json(
    value: &serde_json::Value,
    schema: &serde_json::Value,
    path: &str,
    errs: &mut Vec<String>,
) {
    let expected_type = schema.get("type").and_then(|t| t.as_str());
    if let Some(t) = expected_type {
        let actual = json_type_name(value);
        let ok = match t {
            "integer" => value.is_i64() || value.is_u64(),
            "number" => value.is_number(),
            other => other == actual,
        };
        if !ok {
            errs.push(format!("{path}: expected {t}, got {actual}"));
            return;
        }
    }
    match value {
        serde_json::Value::Object(obj) => {
            if let Some(req) = schema.get("required").and_then(|r| r.as_array()) {
                for r in req {
                    if let Some(key) = r.as_str() {
                        if !obj.contains_key(key) {
                            errs.push(format!("{path}: missing required property '{key}'"));
                        }
                    }
                }
            }
            if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
                for (key, sub_schema) in props {
                    if let Some(v) = obj.get(key) {
                        validate_json(v, sub_schema, &format!("{path}.{key}"), errs);
                    }
                }
            }
        }
        serde_json::Value::Array(arr) => {
            if let Some(items) = schema.get("items") {
                for (i, v) in arr.iter().enumerate() {
                    validate_json(v, items, &format!("{path}[{i}]"), errs);
                }
            }
        }
        _ => {}
    }
}

fn json_type_name(v: &serde_json::Value) -> &'static str {
    use serde_json::Value;
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn last_ident(s: &str) -> String {
    let s = s.trim().trim_end_matches('(');
    s.split(|c: char| c.is_whitespace() || c == ':' || c == '(')
        .filter(|t| !t.is_empty())
        .next_back()
        .unwrap_or("")
        .to_string()
}

/// Match `/users/:id` style pattern to a concrete path, returning captured params.
fn match_path(pattern: &str, path: &str) -> Option<HashMap<String, String>> {
    let p_path = path.split('?').next().unwrap_or(path);
    let pat: Vec<&str> = pattern.trim_matches('/').split('/').collect();
    let seg: Vec<&str> = p_path.trim_matches('/').split('/').collect();
    if pat.len() != seg.len() {
        return None;
    }
    let mut params = HashMap::new();
    for (p, s) in pat.iter().zip(seg.iter()) {
        if let Some(name) = p.strip_prefix(':') {
            params.insert(name.to_string(), s.to_string());
        } else if p != s {
            return None;
        }
    }
    Some(params)
}

/// Normalize a SQL string into a template by stripping literals & whitespace,
/// so repeated identical-shape queries collapse (used for N+1 detection).
fn normalize_sql(sql: &str) -> String {
    let lowered = sql.to_lowercase();
    let mut out = String::with_capacity(lowered.len());
    let mut chars = lowered.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                for n in chars.by_ref() {
                    if n == '\'' {
                        break;
                    }
                }
                out.push('?');
            }
            '0'..='9' => {
                out.push('?');
                while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    chars.next();
                }
            }
            c if c.is_whitespace() => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            c => out.push(c),
        }
    }
    out.trim().to_string()
}

fn extract_tables(sql: &str) -> Vec<String> {
    let lowered = sql.to_lowercase();
    let tokens: Vec<&str> = lowered.split_whitespace().collect();
    let mut tables = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        if (*t == "from" || *t == "join" || *t == "into" || *t == "update") && i + 1 < tokens.len()
        {
            let name = tokens[i + 1].trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
            if !name.is_empty() && name != "select" {
                tables.push(name.to_string());
            }
        }
    }
    tables.dedup();
    tables
}

/// FNV-1a 64-bit hash for deterministic chaos bucketing.
fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(CHARS[((n >> 18) & 63) as usize] as char);
        out.push(CHARS[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_auth_vault_basic_and_bearer() {
        let e = BackendEngine::new();
        e.store_auth(
            "api",
            AuthScheme::Bearer {
                token: "abc".into(),
            },
        );
        assert_eq!(
            e.auth_header("api"),
            Some(("Authorization".into(), "Bearer abc".into()))
        );
        e.store_auth(
            "basic",
            AuthScheme::Basic {
                username: "user".into(),
                password: "pass".into(),
            },
        );
        let (_, v) = e.auth_header("basic").unwrap();
        assert_eq!(v, "Basic dXNlcjpwYXNz");
    }

    #[test]
    fn test_contract_validation() {
        let e = BackendEngine::new();
        let schema = json!({
            "type": "object",
            "required": ["id", "name"],
            "properties": { "id": {"type": "integer"}, "name": {"type": "string"} }
        });
        assert!(e
            .validate_contract(&json!({"id": 1, "name": "x"}), &schema)
            .is_empty());
        let errs = e.validate_contract(&json!({"id": "oops"}), &schema);
        assert!(errs.iter().any(|e| e.contains("expected integer")));
        assert!(errs
            .iter()
            .any(|e| e.contains("missing required property 'name'")));
    }

    #[test]
    fn test_idor_matrix() {
        let e = BackendEngine::new();
        let probes = vec![
            IdorProbe {
                identity: "attacker".into(),
                resource: "/u/2".into(),
                is_owner: false,
                status: 200,
            },
            IdorProbe {
                identity: "owner".into(),
                resource: "/u/1".into(),
                is_owner: true,
                status: 200,
            },
        ];
        let f = e.analyze_idor(&probes);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, "critical");
    }

    #[test]
    fn test_request_chaining_resolution() {
        let e = BackendEngine::new();
        let mut login = ChainStep {
            name: "login".into(),
            request: ApiRequest {
                method: "POST".into(),
                url: "/login".into(),
                ..Default::default()
            },
            captures: HashMap::new(),
        };
        login.captures.insert("token".into(), "data.token".into());
        let use_step = ChainStep {
            name: "me".into(),
            request: ApiRequest {
                method: "GET".into(),
                url: "/me?t={{token}}".into(),
                ..Default::default()
            },
            captures: HashMap::new(),
        };
        assert!(e.resolve_chain(&[login.clone(), use_step.clone()]).is_ok());
        // Wrong order fails.
        assert!(e.resolve_chain(&[use_step, login]).is_err());
    }

    #[test]
    fn test_apply_captures() {
        let e = BackendEngine::new();
        let mut step = ChainStep {
            name: "login".into(),
            request: ApiRequest::default(),
            captures: HashMap::new(),
        };
        step.captures.insert("token".into(), "data.token".into());
        let mut vars = HashMap::new();
        e.apply_captures(&step, &json!({"data": {"token": "xyz"}}), &mut vars);
        assert_eq!(vars.get("token").map(|s| s.as_str()), Some("xyz"));
    }

    #[test]
    fn test_graphql_analysis() {
        let e = BackendEngine::new();
        let q = "query GetUser { user { id name posts { title } } }";
        let a = e.analyze_graphql(q);
        assert_eq!(a.operation_type, "query");
        assert_eq!(a.operation_name.as_deref(), Some("GetUser"));
        assert!(a.top_level_fields.contains(&"user".to_string()));
        assert!(a.max_depth >= 3);
        assert!(!a.is_introspection);

        let intro = e.analyze_graphql("{ __schema { types { name } } }");
        assert!(intro.is_introspection);
    }

    #[test]
    fn test_grpc_path() {
        let e = BackendEngine::new();
        let call = e.parse_grpc_path("/thor.load.LoadService/Run").unwrap();
        assert_eq!(call.package, "thor.load");
        assert_eq!(call.service, "LoadService");
        assert_eq!(call.method, "Run");
    }

    #[test]
    fn test_trace_graph_critical_path() {
        let e = BackendEngine::new();
        let spans = vec![
            Span {
                span_id: "1".into(),
                parent_id: None,
                service: "gateway".into(),
                name: "GET /".into(),
                duration_ms: 10.0,
            },
            Span {
                span_id: "2".into(),
                parent_id: Some("1".into()),
                service: "api".into(),
                name: "handle".into(),
                duration_ms: 50.0,
            },
            Span {
                span_id: "3".into(),
                parent_id: Some("2".into()),
                service: "db".into(),
                name: "query".into(),
                duration_ms: 40.0,
            },
            Span {
                span_id: "4".into(),
                parent_id: Some("1".into()),
                service: "cache".into(),
                name: "get".into(),
                duration_ms: 5.0,
            },
        ];
        let g = e.build_trace_graph(&spans);
        assert!(g.edges.get("gateway").unwrap().contains(&"api".to_string()));
        assert!((g.critical_path_ms - 100.0).abs() < 1e-6);
        assert_eq!(g.critical_path.len(), 3);
    }

    #[test]
    fn test_mock_matching_with_params() {
        let e = BackendEngine::new();
        e.add_mock(MockRoute {
            method: "GET".into(),
            path_pattern: "/users/:id".into(),
            status: 200,
            body: "{}".into(),
        });
        let m = e.match_mock("GET", "/users/42").unwrap();
        assert_eq!(m.status, 200);
        assert_eq!(m.path_params.get("id").map(|s| s.as_str()), Some("42"));
        assert!(e.match_mock("GET", "/orders/1").is_none());
    }

    #[test]
    fn test_chaos_is_deterministic() {
        let e = BackendEngine::new();
        e.add_chaos_rule(ChaosRule {
            url_pattern: "api".into(),
            added_latency_ms: 100,
            error_rate: 1.0,
            error_status: 500,
        });
        let o1 = e.apply_chaos("https://api.x/y", "req-1");
        let o2 = e.apply_chaos("https://api.x/y", "req-1");
        assert_eq!(o1.forced_status, Some(500));
        assert_eq!(o1.added_latency_ms, 100);
        assert_eq!(o1.injected_fault, o2.injected_fault);
        assert!(e.apply_chaos("https://other.x", "req-1").added_latency_ms == 0);
    }

    #[test]
    fn test_idempotency_audit() {
        let e = BackendEngine::new();
        let events = vec![
            IdempotencyEvent {
                method: "POST".into(),
                url: "/pay".into(),
                idempotency_key: None,
            },
            IdempotencyEvent {
                method: "POST".into(),
                url: "/pay".into(),
                idempotency_key: None,
            },
        ];
        let f = e.audit_idempotency(&events);
        assert!(f.iter().any(|x| x.severity == "high"));
    }

    #[test]
    fn test_db_adjacency_and_n_plus_one() {
        let e = BackendEngine::new();
        let mut queries =
            vec!["SELECT * FROM users u JOIN orders o ON u.id = o.user_id".to_string()];
        for i in 0..5 {
            queries.push(format!("SELECT * FROM orders WHERE user_id = {i}"));
        }
        let r = e.analyze_db(&queries);
        assert!(r
            .adjacency
            .get("users")
            .unwrap()
            .contains(&"orders".to_string()));
        assert!(r.n_plus_one.iter().any(|q| q.count >= 5));
    }

    #[test]
    fn test_webhook_sig_and_rate_limit() {
        let e = BackendEngine::new();
        // Known HMAC-SHA256 vector: key="key", msg="The quick brown fox jumps over the lazy dog"
        // → f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8
        let secret = "key";
        let payload = "The quick brown fox jumps over the lazy dog";
        let expected_hex = "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8";
        e.store_auth(
            "wh",
            AuthScheme::WebhookSecret {
                secret: secret.into(),
            },
        );
        let r = e.webhook_signature_vault(&WebhookSigInput {
            payload: payload.into(),
            secret: "wh".into(), // resolve from vault
            provided_signature: format!("sha256={expected_hex}"),
            algorithm: "sha256".into(),
            timestamp: None,
            now: None,
            tolerance_secs: 300,
        });
        assert!(r.valid, "{}", r.detail);
        let rl = e.rate_limit_probe(&[RateLimitSample {
            status: 429,
            retry_after_secs: Some(1.5),
            remaining: Some(0),
            limit: Some(100),
            timestamp_ms: 1,
        }]);
        assert_eq!(rl.limited_count, 1);
    }

    #[test]
    fn test_openapi_parse_and_validate_traffic() {
        let e = BackendEngine::new();
        let doc = serde_json::json!({
            "openapi": "3.0.0",
            "paths": {
                "/users": {
                    "get": { "operationId": "listUsers" },
                    "post": { "operationId": "createUser" }
                }
            }
        });
        let paths = BackendEngine::parse_openapi_paths(&doc);
        assert_eq!(paths.len(), 2);
        let traffic = e.openapi_validate_traffic(
            &paths,
            &[
                ActualInteraction {
                    method: "GET".into(),
                    path: "/users".into(),
                    status: 200,
                },
                ActualInteraction {
                    method: "DELETE".into(),
                    path: "/users".into(),
                    status: 204,
                },
            ],
        );
        assert_eq!(traffic.matched, 1);
        assert!(!traffic.ok);
        assert!(!traffic.undeclared_methods.is_empty());
    }

    #[test]
    fn test_live_stub_and_sse_loopback() {
        let e = BackendEngine::new();
        assert!(e
            .start_live_stub_server(
                0,
                vec![StubRouteSpec {
                    method: "GET".into(),
                    path: "/ping".into(),
                    status: 200,
                    body: "{\"ok\":true}".into(),
                }],
                Some("https://evil.example.com/"),
            )
            .is_err());
        let port = e
            .start_live_stub_server(
                0,
                vec![StubRouteSpec {
                    method: "GET".into(),
                    path: "/ping".into(),
                    status: 200,
                    body: "{\"ok\":true}".into(),
                }],
                Some("http://127.0.0.1/"),
            )
            .expect("bind stub");
        let url = format!("http://127.0.0.1:{port}/ping");
        let resp = e
            .send_blocking(&ApiRequest {
                method: "GET".into(),
                url,
                headers: HashMap::new(),
                body: None,
            })
            .expect("reqwest");
        assert_eq!(resp.status, 200);
        assert!(resp.body.contains("ok"));
        let report = e.live_stub_server(&[]);
        assert!(report.live);
        assert_eq!(report.bound_port, Some(port));
        assert!(report.hit_count >= 1);
        e.stop_live_stub_server();

        let sse_port = e
            .start_sse_ws_server_lab(
                0,
                vec![StreamLabEvent {
                    kind: "message".into(),
                    payload: "hello".into(),
                    timestamp_ms: 1,
                }],
                None,
            )
            .expect("sse");
        let sse_url = format!("http://127.0.0.1:{sse_port}/events");
        let sse = e
            .send_blocking(&ApiRequest {
                method: "GET".into(),
                url: sse_url,
                headers: HashMap::new(),
                body: None,
            })
            .expect("sse get");
        assert_eq!(sse.status, 200);
        assert!(sse.body.contains("hello") || sse.body.contains("event:"));
        let srep = e.sse_ws_server_lab(&[]);
        assert!(srep.live);
        assert_eq!(srep.bound_port, Some(sse_port));
        e.stop_sse_ws_server_lab();
    }

    #[test]
    fn test_openapi_diff_breaking() {
        let e = BackendEngine::new();
        let before = vec![OpenApiPath { path: "/a".into(), method: "GET".into(), operation_id: Some("getA".into()) }];
        let after = vec![];
        let d = e.openapi_diff_gate(&before, &after);
        assert!(d.breaking);
        assert_eq!(d.removed.len(), 1);
    }

    #[test]
    fn test_oauth_and_pagination() {
        let e = BackendEngine::new();
        let o = e.oauth_flow_capture(&[OauthFlowStep {
            step: "authorize".into(),
            url: "https://idp/auth".into(),
            params: BTreeMap::new(),
        }]);
        assert!(!o.has_pkce);
        assert!(!o.issues.is_empty());
        let p = e.pagination_auditor(&[PageSlice {
            page: 1, page_size: 10, items: 10, total: None, next_cursor: None,
        }]);
        assert!(!p.issues.is_empty());
    }

}
