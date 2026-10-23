use axum::{body::Body, http::{Request, Response}, routing::any, Router};
use serde_json::{json, Value};
use thor::api::{Header, HttpService, RequestBody, RequestSpec, Variables};

async fn echo(request: Request<Body>) -> Response<Body> {
    let repeated: Vec<_> = request.headers().get_all("x-repeat").iter().map(|value| value.to_str().unwrap().to_string()).collect();
    let query = request.uri().query().unwrap_or_default().to_string();
    let body = axum::body::to_bytes(request.into_body(), 1024 * 1024).await.unwrap();
    Response::builder().header("set-cookie", "first=1; Path=/").header("set-cookie", "second=2; Path=/")
        .body(Body::from(json!({"query":query,"repeated":repeated,"body":String::from_utf8_lossy(&body)}).to_string())).unwrap()
}

async fn server() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/echo", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, Router::new().route("/echo", any(echo))).await.unwrap(); });
    (url, task)
}

#[tokio::test]
async fn repeated_headers_queries_and_form_bodies_survive_real_http_execution() {
    let (url, server) = server().await;
    let mut request = RequestSpec::new("POST", &url);
    request.headers = vec![Header::new("x-repeat", "one"), Header::new("x-repeat", "two")];
    request.query = vec![Header::new("tag", "a b"), Header::new("tag", "second")];
    request.body = RequestBody::Form { entries: vec![Header::new("name", "Alice & Bob")] };
    let response = HttpService::new().unwrap().send(&request, &Variables::default()).await.unwrap();
    let body: Value = serde_json::from_str(&response.body.data).unwrap();
    assert_eq!(body["repeated"], json!(["one", "two"]));
    assert_eq!(body["query"], "tag=a+b&tag=second");
    assert_eq!(body["body"], "name=Alice+%26+Bob");
    assert_eq!(response.headers.iter().filter(|header| header.name == "set-cookie").count(), 2);
    server.abort();
}

#[tokio::test]
async fn variables_obey_scopes_and_missing_values_fail_before_sending() {
    let (url, server) = server().await;
    let mut variables = Variables::default();
    variables.project.insert("value".into(), "project".into());
    variables.environment.insert("value".into(), "environment".into());
    variables.collection.insert("value".into(), "collection".into());
    variables.request.insert("value".into(), "request".into());
    variables.overrides.insert("value".into(), "run".into());
    let mut request = RequestSpec::new("POST", &url);
    request.body = RequestBody::Text { text: "{{value}}".into(), content_type: "text/plain".into() };
    let service = HttpService::new().unwrap();
    let response = service.send(&request, &variables).await.unwrap();
    let body: Value = serde_json::from_str(&response.body.data).unwrap();
    assert_eq!(body["body"], "run");
    request.body = RequestBody::Text { text: "{{missing}}".into(), content_type: "text/plain".into() };
    assert!(service.send(&request, &variables).await.unwrap_err().to_string().contains("missing"));
    server.abort();
}

#[tokio::test]
async fn response_limits_are_enforced_while_reading() {
    let (url, server) = server().await;
    let mut request = RequestSpec::new("GET", &url);
    request.options.response_limit_bytes = 8;
    let error = HttpService::new().unwrap().send(&request, &Variables::default()).await.unwrap_err();
    assert!(error.to_string().contains("size limit"));
    server.abort();
}

#[tokio::test]
async fn invalid_methods_never_fall_back_to_get() {
    let (url, server) = server().await;
    let request = RequestSpec::new("not a method", &url);
    assert!(HttpService::new().unwrap().send(&request, &Variables::default()).await.is_err());
    server.abort();
}

#[tokio::test]
async fn legacy_backend_rejects_invalid_methods_instead_of_sending_get() {
    let (url, server) = server().await;
    let request = thor::features::backend::ApiRequest { method: "not a method".into(), url, ..Default::default() };
    assert!(thor::features::backend::BackendEngine::new().send(&request).await.is_err());
    server.abort();
}

#[tokio::test]
async fn redirects_are_returned_without_forwarding_credentials() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let redirect = url.clone();
    let app = Router::new().route("/redirect", any(move || async move {
        Response::builder().status(302).header("location", format!("{redirect}/destination")).body(Body::empty()).unwrap()
    })).route("/destination", any(|| async { "credentials should never reach here" }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });
    let mut request = RequestSpec::new("GET", format!("{url}/redirect"));
    request.headers.push(Header::new("authorization", "Bearer secret"));
    let response = HttpService::new().unwrap().send(&request, &Variables::default()).await.unwrap();
    assert_eq!(response.status, 302);
    assert_eq!(response.body_bytes, 0);
    server.abort();
}

#[tokio::test]
async fn binary_response_round_trips_without_lossy_decoding() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/", any(|| async { vec![0_u8, 255, 128, 65] }))).await.unwrap();
    });
    let response = HttpService::new().unwrap().send(&RequestSpec::new("GET", url), &Variables::default()).await.unwrap();
    assert_eq!(response.body.encoding, "base64");
    assert_eq!(response.body.data, "AP+AQQ==");
    assert_eq!(response.body_bytes, 4);
    server.abort();
}

#[tokio::test]
async fn json_variables_preserve_json_types_and_escaping() {
    let (url, server) = server().await;
    let mut request = RequestSpec::new("POST", url);
    request.body = RequestBody::Json { value: json!({"name":"{{name}}", "number":7, "nested":["{{name}}"]}) };
    let mut variables = Variables::default();
    variables.environment.insert("name".into(), "a\"b\\c".into());
    let response = HttpService::new().unwrap().send(&request, &variables).await.unwrap();
    let echo: Value = serde_json::from_str(&response.body.data).unwrap();
    let body: Value = serde_json::from_str(echo["body"].as_str().unwrap()).unwrap();
    assert_eq!(body, json!({"name":"a\"b\\c", "number":7, "nested":["a\"b\\c"]}));
    server.abort();
}

#[tokio::test]
async fn cancellation_interrupts_a_request_waiting_for_headers() {
    use std::time::Duration;
    use thor::api::CancellationToken;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let cancellation = CancellationToken::new();
    let signal = cancellation.clone();
    let app = Router::new().route("/", any(move || {
        let signal = signal.clone();
        async move { signal.cancel(); tokio::time::sleep(Duration::from_secs(10)).await; "late" }
    }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });
    let service = HttpService::new().unwrap();
    let request = RequestSpec::new("GET", url);
    let result = tokio::time::timeout(Duration::from_secs(1), service.send_cancellable(&request, &Variables::default(), &cancellation)).await.unwrap();
    assert!(matches!(result, Err(thor::api::ApiError::Cancelled)));
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocking_compatibility_executor_can_be_called_from_an_async_caller() {
    let (url, server) = server().await;
    let request = thor::features::backend::ApiRequest { method: "GET".into(), url, ..Default::default() };
    let response = thor::features::backend::BackendEngine::new().send_blocking(&request).unwrap();
    assert_eq!(response.status, 200);
    server.abort();
}

#[tokio::test]
async fn caller_content_type_is_not_duplicated_by_body_defaults() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let app = Router::new().route("/", any(|request: Request<Body>| async move {
            let values: Vec<_> = request.headers().get_all("content-type").iter().map(|value| value.to_str().unwrap()).collect();
            serde_json::to_string(&values).unwrap()
        }));
        axum::serve(listener, app).await.unwrap();
    });
    let mut request = RequestSpec::new("POST", url);
    request.headers.push(Header::new("Content-Type", "application/problem+json"));
    request.body = RequestBody::Text { text: "{}".into(), content_type: "text/plain".into() };
    let response = HttpService::new().unwrap().send(&request, &Variables::default()).await.unwrap();
    assert_eq!(response.body.data, "[\"application/problem+json\"]");
    server.abort();
}

#[tokio::test]
async fn timeout_covers_body_streaming_and_transport_errors_do_not_expose_urls() {
    use futures::StreamExt;
    use std::time::Duration;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/?key=super-secret-value", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let app = Router::new().route("/", any(|| async {
            let stream = futures::stream::iter([0, 1]).then(|index| async move {
                if index == 1 { tokio::time::sleep(Duration::from_secs(10)).await; }
                Ok::<_, std::io::Error>("body")
            });
            Body::from_stream(stream)
        }));
        axum::serve(listener, app).await.unwrap();
    });
    let mut request = RequestSpec::new("GET", url);
    request.options.timeout_ms = 50;
    let result = HttpService::new().unwrap().send(&request, &Variables::default()).await;
    let error = result.unwrap_err();
    assert!(matches!(error, thor::api::ApiError::Transport(_)));
    assert!(!error.to_string().contains("super-secret-value"));
    server.abort();
}
