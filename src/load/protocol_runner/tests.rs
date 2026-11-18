use super::*;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::{accept_async, tungstenite::Message};

type CapturedRequest = (String, axum::http::HeaderMap, Vec<u8>);

async fn http_fixture() -> (
    String,
    mpsc::UnboundedReceiver<CapturedRequest>,
    JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::unbounded_channel();
    let app = Router::new().fallback(move |request: Request<Body>| {
        let sender = sender.clone();
        async move {
            let (parts, body) = request.into_parts();
            let body = to_bytes(body, 1024).await.unwrap();
            let status = if parts.uri.path() == "/fail" {
                StatusCode::IM_A_TEAPOT
            } else {
                StatusCode::OK
            };
            sender
                .send((parts.method.to_string(), parts.headers, body.to_vec()))
                .unwrap();
            (status, "hello")
        }
    });
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, receiver, server)
}

#[tokio::test]
async fn http_steps_preserve_methods_headers_bodies_statuses_and_received_bytes() {
    let (url, mut received, server) = http_fixture().await;
    let scenario = ThorScenario::new("http", &url).with_header("X-Shared", "global");
    let collector = MetricsCollector::new();
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    ProtocolRunner::execute_step(
        &client,
        &ScenarioStep::Navigate { url: url.clone() },
        &scenario,
        &collector,
    )
    .await;
    let (method, headers, body) = received.recv().await.unwrap();
    assert_eq!(method, "GET");
    assert_eq!(headers["x-shared"], "global");
    assert!(body.is_empty());

    for (input, expected, path, payload) in [
        ("pOsT", "POST", "/", Some("payload")),
        ("put", "PUT", "/", None),
        ("delete", "DELETE", "/", None),
        ("patch", "PATCH", "/", None),
        ("head", "HEAD", "/", None),
        ("options", "OPTIONS", "/", None),
        ("TRACE", "GET", "/", None),
        ("get", "GET", "/fail", None),
    ] {
        let step = ScenarioStep::HttpRequest {
            method: input.into(),
            url: format!("{url}{path}"),
            headers: [("X-Shared".into(), "step".into())].into_iter().collect(),
            body: payload.map(str::to_owned),
        };
        ProtocolRunner::execute_step(&client, &step, &scenario, &collector).await;
        let (method, headers, body) = received.recv().await.unwrap();
        assert_eq!(method, expected);
        let shared: Vec<_> = headers
            .get_all("x-shared")
            .iter()
            .map(|h| h.to_str().unwrap())
            .collect();
        assert_eq!(shared, ["global", "step"]);
        assert_eq!(body, payload.unwrap_or("").as_bytes());
    }
    let metrics = collector.snapshot();
    assert_eq!(
        (
            metrics.total_requests,
            metrics.successful_requests,
            metrics.failed_requests,
            metrics.total_bytes
        ),
        (9, 8, 1, 40)
    );
    assert_eq!(metrics.status_codes.get(&200), Some(&8));
    assert_eq!(metrics.status_codes.get(&418), Some(&1));
    server.abort();
}

#[tokio::test]
async fn websocket_steps_count_sent_and_received_utf8_bytes_and_failed_handshakes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        for expected in ["hi", "雪"] {
            let text = socket.next().await.unwrap().unwrap().into_text().unwrap();
            assert_eq!(text, expected);
            socket
                .send(Message::Text(format!("echo:{text}").into()))
                .await
                .unwrap();
        }
    });
    let collector = MetricsCollector::new();
    let scenario = ThorScenario::new("websocket", &url);
    let step = ScenarioStep::WebSocketConnect {
        url,
        messages: vec!["hi".into(), "雪".into()],
    };
    tokio::time::timeout(
        Duration::from_secs(5),
        ProtocolRunner::execute_step(&Client::new(), &step, &scenario, &collector),
    )
    .await
    .unwrap();
    server.await.unwrap();
    let invalid = ScenarioStep::WebSocketConnect {
        url: "invalid websocket URL".into(),
        messages: vec![],
    };
    ProtocolRunner::execute_step(&Client::new(), &invalid, &scenario, &collector).await;
    let metrics = collector.snapshot();
    assert_eq!(
        (
            metrics.total_requests,
            metrics.successful_requests,
            metrics.failed_requests,
            metrics.total_bytes
        ),
        (2, 1, 1, 20)
    );
    assert_eq!(metrics.status_codes.get(&101), Some(&1));
    assert_eq!(metrics.status_codes.get(&0), Some(&1));
}

#[tokio::test]
async fn unsupported_steps_and_transport_errors_fail_while_wait_records_no_sample() {
    let scenario = ThorScenario::new("unsupported", "invalid URL");
    let collector = MetricsCollector::new();
    let client = Client::new();
    let steps = [
        ScenarioStep::Click {
            selector: "body".into(),
        },
        ScenarioStep::Fill {
            selector: "input".into(),
            value: "v".into(),
        },
        ScenarioStep::CustomScript {
            name: "test".into(),
            script: "1".into(),
        },
        ScenarioStep::Navigate {
            url: "invalid URL".into(),
        },
        ScenarioStep::HttpRequest {
            method: "GET".into(),
            url: "invalid URL".into(),
            headers: Default::default(),
            body: None,
        },
    ];
    for step in steps {
        ProtocolRunner::execute_step(&client, &step, &scenario, &collector).await;
    }
    let start = Instant::now();
    ProtocolRunner::execute_step(
        &client,
        &ScenarioStep::Wait { duration_ms: 5 },
        &scenario,
        &collector,
    )
    .await;
    assert!(start.elapsed() >= Duration::from_millis(5));
    let metrics = collector.snapshot();
    assert_eq!(
        (
            metrics.total_requests,
            metrics.successful_requests,
            metrics.failed_requests,
            metrics.total_bytes
        ),
        (5, 0, 5, 0)
    );
    assert_eq!(metrics.status_codes.get(&0), Some(&5));
}

#[tokio::test]
async fn virtual_users_honor_ramp_up_and_target_rate() {
    let (url, _received, server) = http_fixture().await;
    let mut scenario = ThorScenario::new("paced", &url)
        .with_duration(1)
        .with_vus(2)
        .add_step(ScenarioStep::Navigate { url: url.clone() });
    // Second user wakes after the run deadline; joining it must retain that delay.
    scenario.ramp_up_secs = 4;
    scenario.target_rps = Some(4);
    let metrics =
        tokio::time::timeout(Duration::from_secs(5), ProtocolRunner::new().run(&scenario))
            .await
            .unwrap()
            .unwrap();
    assert!(
        (1..=2).contains(&metrics.total_requests),
        "{} requests escaped pacing",
        metrics.total_requests
    );
    assert_eq!(metrics.failed_requests, 0);
    assert!(metrics.duration_secs >= 1.8);
    server.abort();
}

#[tokio::test]
async fn completed_runs_enforce_error_budgets() {
    let mut scenario = ThorScenario::new("budget", "unused")
        .with_duration(1)
        .add_step(ScenarioStep::Click {
            selector: "body".into(),
        })
        .add_step(ScenarioStep::Wait { duration_ms: 1000 });
    scenario.budgets.max_error_rate = Some(0.0);
    let error = ProtocolRunner::new().run(&scenario).await.unwrap_err();
    assert!(error.starts_with("performance budget violated:"));
}
