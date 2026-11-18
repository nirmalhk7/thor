use super::*;
use axum::body::{to_bytes, Body};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::Router;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc;

struct Capture {
    method: String,
    headers: HeaderMap,
    body: Vec<u8>,
}

struct Peer {
    url: String,
    requests: mpsc::UnboundedReceiver<Capture>,
    task: tokio::task::JoinHandle<()>,
}

impl Peer {
    async fn start(responses: Vec<(u16, &'static str)>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/resource", listener.local_addr().unwrap());
        let (sender, requests) = mpsc::unbounded_channel();
        let responses = Arc::new(responses);
        let position = Arc::new(AtomicUsize::new(0));
        let app = Router::new().fallback(move |request: Request<Body>| {
            let output = sender.clone();
            let response = responses[position.fetch_add(1, Ordering::SeqCst)];
            async move {
                let (parts, body) = request.into_parts();
                let bytes = to_bytes(body, 4096).await.unwrap();
                output
                    .send(Capture {
                        method: parts.method.to_string(),
                        headers: parts.headers,
                        body: bytes.to_vec(),
                    })
                    .unwrap();
                (StatusCode::from_u16(response.0).unwrap(), response.1)
            }
        });
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            url,
            requests,
            task,
        }
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn request(url: &str) -> InterceptedRequest {
    InterceptedRequest {
        id: "captured".into(),
        timestamp: 42,
        method: "GET".into(),
        url: url.into(),
        headers: HashMap::from([
            ("aUtHoRiZaTiOn".into(), "Bearer base".into()),
            ("CoOkIe".into(), "session=base".into()),
            ("X-Shared".into(), "retained".into()),
        ]),
        body: None,
        is_modified: false,
    }
}

#[tokio::test]
async fn default_roles_replay_original_credentials_then_anonymous_and_cache_matrix() {
    let mut peer = Peer::start(vec![(200, "{}"), (200, "{}")]).await;
    let engine = IdorMatrixEngine::new();
    let matrix = engine.run_matrix(&request(&peer.url), vec![]).await;
    assert_eq!(matrix.target_url, peer.url);
    assert_eq!(matrix.http_method, "GET");
    assert_eq!(matrix.timestamp, 42);
    assert_eq!(matrix.vulnerability_count, 1);
    let names: Vec<_> = matrix
        .role_results
        .iter()
        .map(|role| role.role_name.as_str())
        .collect();
    assert_eq!(names, ["Original Context", "Anonymous"]);
    let original = peer.requests.recv().await.unwrap();
    let anonymous = peer.requests.recv().await.unwrap();
    assert_eq!(original.headers["authorization"], "Bearer base");
    assert_eq!(original.headers["cookie"], "session=base");
    assert!(!anonymous.headers.contains_key("authorization"));
    assert!(!anonymous.headers.contains_key("cookie"));
    assert_eq!(anonymous.headers["x-shared"], "retained");
    assert_eq!(
        serde_json::to_value(engine.get_last_matrix()).unwrap(),
        serde_json::to_value(Some(matrix)).unwrap()
    );
}

#[tokio::test]
async fn role_requests_preserve_method_fallback_duplicate_credentials_and_utf8_body() {
    let cases = [
        ("pOsT", "POST"),
        ("put", "PUT"),
        ("delete", "DELETE"),
        ("PATCH", "PATCH"),
        ("head", "GET"),
        ("OPTIONS", "GET"),
        ("get", "GET"),
    ];
    let mut peer = Peer::start(vec![(200, "ok"); cases.len()]).await;
    let engine = IdorMatrixEngine::new();
    for (input, expected) in cases {
        let mut base = request(&peer.url);
        base.method = input.into();
        base.body = Some("雪".into());
        let mut role = IdorRoleConfig::new("Admin")
            .with_auth("Bearer role")
            .with_cookie("session=role");
        role.custom_headers
            .insert("authorization".into(), "Bearer custom".into());
        let matrix = engine.run_matrix(&base, vec![role]).await;
        assert_eq!(matrix.http_method, input);
        assert_eq!(matrix.role_results[0].status_code, 200);
        let capture = peer.requests.recv().await.unwrap();
        assert_eq!(capture.method, expected);
        let auth: Vec<_> = capture
            .headers
            .get_all("authorization")
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect();
        assert_eq!(auth, ["Bearer role", "Bearer custom"]);
        assert_eq!(capture.headers["cookie"], "session=role");
        assert_eq!(capture.headers["x-shared"], "retained");
        assert_eq!(capture.body, "雪".as_bytes());
    }
}

#[tokio::test]
async fn classifications_preserve_role_substrings_status_boundaries_and_error_body_refusal() {
    let cases = [
        ("Admin", 200, "{}", false),
        ("SUPERUSER", 200, "雪", true),
        ("Anonymous", 299, "ok", true),
        ("User", 300, "ok", false),
        ("user", 200, "FORBIDDEN", false),
        ("Anonymous", 200, "ACCESS DENIED", false),
        ("User", 200, "UnAuthorized", false),
        ("User", 204, "", true),
        ("username", 403, "{}", false),
    ];
    let peer = Peer::start(
        cases
            .iter()
            .map(|(_, status, body, _)| (*status, *body))
            .collect(),
    )
    .await;
    let roles = cases
        .iter()
        .map(|(role, _, _, _)| IdorRoleConfig::new(*role))
        .collect();
    let engine = IdorMatrixEngine::new();
    let matrix = engine.run_matrix(&request(&peer.url), roles).await;
    assert_eq!(matrix.vulnerability_count, 3);
    for (result, (role, status, body, vulnerable)) in matrix.role_results.iter().zip(cases) {
        assert_eq!(result.role_name, role);
        assert_eq!(result.status_code, status);
        assert_eq!(result.content_length, body.len());
        assert_eq!(result.is_potential_vulnerability, vulnerable);
        if vulnerable {
            assert!(result
                .status_description
                .starts_with("⚠️ POTENTIAL IDOR: Status "));
        } else {
            assert_eq!(result.status_description, format!("Status {status}"));
        }
    }
    let failed = engine
        .run_matrix(&request(":bad URL"), vec![IdorRoleConfig::new("User")])
        .await;
    assert_eq!(failed.vulnerability_count, 0);
    assert_eq!(failed.role_results[0].status_code, 0);
    assert_eq!(failed.role_results[0].content_length, 0);
    assert!(failed.role_results[0]
        .status_description
        .starts_with("Error: "));
    assert_eq!(engine.get_last_matrix().unwrap().target_url, ":bad URL");
}
