use super::{
    Arc, AtomicBool, BTreeMap, BackendEngine, Duration, Mutex, Ordering, SseWsLabReport,
    StreamLabEvent, TcpListener,
};
use super::{Read, Write};

impl BackendEngine {
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
                Some(h) => (Some(h.port), true, h.counter.load(Ordering::SeqCst)),
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
        super::super::loopback::validate_advertise_url(advertise_url, "SSE/WS lab")?;
        super::super::loopback::restart_counted(
            &mut self.sse_lab.lock(),
            port,
            events,
            sse_lab_loop,
        )
    }

    pub fn stop_sse_ws_server_lab(&self) {
        if let Some(h) = self.sse_lab.lock().take() {
            h.running.store(false, Ordering::SeqCst);
        }
    }

    pub fn sse_ws_server_lab_port(&self) -> Option<u16> {
        self.sse_lab.lock().as_ref().map(|h| h.port)
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
