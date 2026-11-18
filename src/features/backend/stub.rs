use super::{
    path_matches_template, Arc, AtomicBool, BTreeMap, BackendEngine, Duration, LiveStubReport,
    Mutex, Ordering, StubRouteSpec, TcpListener,
};
use super::{Read, Write};

impl BackendEngine {
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
                Some(h) => (Some(h.port), true, h.counter.load(Ordering::SeqCst)),
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
        super::super::loopback::validate_advertise_url(advertise_url, "live stub")?;
        super::super::loopback::restart_counted(
            &mut self.live_stub.lock(),
            port,
            routes,
            live_stub_loop,
        )
    }

    pub fn stop_live_stub_server(&self) {
        if let Some(h) = self.live_stub.lock().take() {
            h.running.store(false, Ordering::SeqCst);
        }
    }

    pub fn live_stub_server_port(&self) -> Option<u16> {
        self.live_stub.lock().as_ref().map(|h| h.port)
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
