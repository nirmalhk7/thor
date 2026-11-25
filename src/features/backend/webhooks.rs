use super::super::loopback::read_http_request;
use super::{
    now_ms, AppState, Arc, AtomicBool, BackendEngine, Duration, HashMap, Ordering, TcpListener,
    WebhookEvent, WebhookSigInput, WebhookSigReport, Write,
};

impl BackendEngine {
    // 4b. Live Webhook Sink -------------------------------------------------
    /// Start a real localhost HTTP listener that records every inbound request
    /// as a [`WebhookEvent`]. Pass `0` to bind an ephemeral port. Returns the
    /// bound port. Any previous sink is stopped first.
    pub fn start_webhook_sink(&self, port: u16) -> Result<u16, String> {
        super::super::loopback::restart_simple(
            &mut self.webhook_sink.lock(),
            port,
            webhook_sink_loop,
        )
    }

    pub fn stop_webhook_sink(&self) {
        if let Some(h) = self.webhook_sink.lock().take() {
            h.running.store(false, Ordering::SeqCst);
        }
    }

    pub fn webhook_sink_port(&self) -> Option<u16> {
        self.webhook_sink.lock().as_ref().map(|h| h.port)
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
            || provided
                .strip_prefix("sha256=")
                .map(|s| s == hex.as_str())
                .unwrap_or(false)
            || provided
                .strip_prefix("v1=")
                .map(|s| s == hex.as_str())
                .unwrap_or(false);
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
}
/// Minimal blocking HTTP sink loop: records every inbound request as a
/// [`WebhookEvent`] on the process-wide backend engine and replies `200 OK`.
fn webhook_sink_loop(listener: TcpListener, running: Arc<AtomicBool>) {
    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let _ = stream.set_nonblocking(false);
                if let Some(event) = read_http_request(&mut stream)
                    .as_deref()
                    .and_then(parse_http_request)
                {
                    AppState::global()
                        .features()
                        .backend()
                        .record_webhook(event);
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
