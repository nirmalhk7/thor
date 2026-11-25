use super::super::loopback::read_http_head;
use super::{
    now_ms, AppState, Arc, AtomicBool, Duration, OobInteraction, Ordering, SecurityEngine,
    TcpListener, Write,
};

impl SecurityEngine {
    // 12. OOB Listener ------------------------------------------------------
    /// Allocate a unique out-of-band correlation token (subdomain) for blind
    /// SSRF/RCE detection.
    pub fn new_oob_token(&self) -> String {
        let mut c = self.oob_counter.lock();
        *c += 1;
        format!("t{:x}{:x}.{}", now_ms(), *c, self.oob_domain)
    }

    pub fn record_oob(&self, interaction: OobInteraction) {
        self.oob_interactions.lock().push(interaction);
    }

    pub fn oob_interactions(&self, token: &str) -> Vec<OobInteraction> {
        self.oob_interactions
            .lock()
            .iter()
            .filter(|i| i.token == token)
            .cloned()
            .collect()
    }

    pub fn all_oob_interactions(&self) -> Vec<OobInteraction> {
        self.oob_interactions.lock().clone()
    }

    /// Start a real localhost HTTP loopback listener for out-of-band interaction
    /// detection (blind SSRF/RCE). Callbacks are correlated to the OOB token that
    /// appears in the request path or `Host` header. Pass `0` for an ephemeral
    /// port; returns the bound port.
    pub fn start_oob_listener(&self, port: u16) -> Result<u16, String> {
        super::super::loopback::restart_simple(&mut self.oob_sink.lock(), port, oob_listener_loop)
    }

    pub fn stop_oob_listener(&self) {
        if let Some(h) = self.oob_sink.lock().take() {
            h.running.store(false, Ordering::SeqCst);
        }
    }

    pub fn oob_listener_port(&self) -> Option<u16> {
        self.oob_sink.lock().as_ref().map(|h| h.port)
    }
}
/// Minimal OOB loopback listener loop: records each callback as an
/// [`OobInteraction`] correlated by the token found in the path or Host header.
fn oob_listener_loop(listener: TcpListener, running: Arc<AtomicBool>) {
    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, addr)) => {
                let _ = stream.set_nonblocking(false);
                let Some(raw) = read_http_head(&mut stream) else {
                    continue;
                };
                let (token, path) = oob_token_from_request(&raw);
                AppState::global()
                    .features()
                    .security()
                    .record_oob(OobInteraction {
                        token,
                        protocol: "http".into(),
                        source_ip: addr.ip().to_string(),
                        received_ms: now_ms(),
                        detail: format!("HTTP callback {path}"),
                    });
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

/// Extract an OOB correlation token from a raw HTTP request: prefer the first
/// path segment, fall back to the leftmost Host label.
fn oob_token_from_request(raw: &str) -> (String, String) {
    let (head, _) = raw.split_once("\r\n\r\n").unwrap_or((raw, ""));
    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or("");
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string();
    let first_seg = path
        .trim_start_matches('/')
        .split(['/', '?'])
        .next()
        .unwrap_or("");
    if !first_seg.is_empty() {
        return (first_seg.to_string(), path);
    }
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("host") {
                let host = v.trim().split(':').next().unwrap_or("");
                let label = host.split('.').next().unwrap_or("");
                if !label.is_empty() {
                    return (label.to_string(), path);
                }
            }
        }
    }
    ("unknown".to_string(), path)
}
