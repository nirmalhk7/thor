//! Backend specialist features (12 + 12 wave-2).
//!
//! API workbench, auth vault, contract validation, webhook sink, IDOR matrix,
//! request chaining, gRPC/GraphQL inspection, trace adjacency, mock/stub server,
//! chaos injection, idempotency auditing and DB adjacency. Logic is real Rust:
//! JSON schema validation, deterministic chaos, SQL/GraphQL parsing, and graph
//! analysis — no simulated randomness.

mod auth;
mod cache;
mod chain;
mod chaos;
mod contracts;
mod database;
mod headers;
mod mocks;
mod multipart;
mod openapi;
mod pagination;
mod protocols;
mod rate_limits;
mod requests;
mod stream_lab;
mod stub;
mod tracing;
mod webhooks;
mod workflows;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::net::TcpListener;
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
    Bearer {
        token: String,
    },
    Basic {
        username: String,
        password: String,
    },
    ApiKey {
        header: String,
        key: String,
    },
    /// Shared secret for webhook HMAC verification (WebhookSignatureVault).
    WebhookSecret {
        secret: String,
    },
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

use super::loopback::{CountedHandle, LoopbackHandle};
type WebhookSinkHandle = LoopbackHandle;
type LiveStubHandle = CountedHandle;
type SseLabHandle = CountedHandle;

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

fn default_algo() -> String {
    "sha256".into()
}
fn default_tol() -> i64 {
    300
}

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
}

fn path_matches_template(template: &str, path: &str) -> bool {
    let t: Vec<&str> = template.trim_matches('/').split('/').collect();
    let p: Vec<&str> = path.trim_matches('/').split('/').collect();
    if t.len() != p.len() {
        return false;
    }
    t.iter()
        .zip(p.iter())
        .all(|(a, b)| a.starts_with('{') && a.ends_with('}') || *a == "*" || *a == *b)
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
        let before = vec![OpenApiPath {
            path: "/a".into(),
            method: "GET".into(),
            operation_id: Some("getA".into()),
        }];
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
            page: 1,
            page_size: 10,
            items: 10,
            total: None,
            next_cursor: None,
        }]);
        assert!(!p.issues.is_empty());
    }
}

#[cfg(test)]
mod behavior_tests;
