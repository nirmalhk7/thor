use parking_lot::Mutex;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub(super) struct LoopbackHandle {
    pub(super) port: u16,
    pub(super) running: Arc<AtomicBool>,
}

pub(super) struct CountedHandle {
    pub(super) port: u16,
    pub(super) running: Arc<AtomicBool>,
    pub(super) counter: Arc<AtomicU64>,
}

pub(super) fn restart_simple(
    slot: &mut Option<LoopbackHandle>,
    port: u16,
    worker: impl FnOnce(TcpListener, Arc<AtomicBool>) + Send + 'static,
) -> Result<u16, String> {
    if let Some(handle) = slot.take() {
        handle.running.store(false, Ordering::SeqCst);
    }
    let (listener, bound) = bind(port)?;
    let running = Arc::new(AtomicBool::new(true));
    let flag = running.clone();
    std::thread::spawn(move || worker(listener, flag));
    *slot = Some(LoopbackHandle {
        port: bound,
        running,
    });
    Ok(bound)
}

pub(super) fn restart_counted<T: Send + 'static>(
    slot: &mut Option<CountedHandle>,
    port: u16,
    payload: T,
    worker: impl FnOnce(TcpListener, Arc<AtomicBool>, Arc<AtomicU64>, Arc<Mutex<T>>) + Send + 'static,
) -> Result<u16, String> {
    if let Some(handle) = slot.take() {
        handle.running.store(false, Ordering::SeqCst);
    }
    let (listener, bound) = bind(port)?;
    let running = Arc::new(AtomicBool::new(true));
    let counter = Arc::new(AtomicU64::new(0));
    let payload = Arc::new(Mutex::new(payload));
    let flag = running.clone();
    let count = counter.clone();
    std::thread::spawn(move || worker(listener, flag, count, payload));
    *slot = Some(CountedHandle {
        port: bound,
        running,
        counter,
    });
    Ok(bound)
}

fn bind(port: u16) -> Result<(TcpListener, u16), String> {
    let listener =
        TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("bind failed: {e}"))?;
    let bound = listener.local_addr().map_err(|e| e.to_string())?.port();
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    Ok((listener, bound))
}

pub(super) fn validate_advertise_url(url: Option<&str>, label: &str) -> Result<(), String> {
    if let Some(url) = url {
        if !crate::net::consent::is_private_or_local_url(url) {
            return Err(format!("{label} refuses non-local advertise URL: {url}"));
        }
    }
    Ok(())
}

const MAX_REQUEST_BYTES: usize = 16 * 1024;

pub(super) fn read_http_head(stream: &mut TcpStream) -> Option<String> {
    let (request, body_start) = read_until_headers_end(stream)?;
    Some(String::from_utf8_lossy(&request[..body_start]).into_owned())
}

pub(super) fn read_http_request(stream: &mut TcpStream) -> Option<String> {
    let (mut request, body_start) = read_until_headers_end(stream)?;
    let headers = String::from_utf8_lossy(&request[..body_start]);
    let content_length = headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.trim().parse::<usize>())
        .transpose()
        .ok()?
        .unwrap_or(0);
    let expected = body_start.checked_add(content_length)?;
    if expected > MAX_REQUEST_BYTES {
        return None;
    }

    let received = request.len().min(expected);
    request.resize(expected, 0);
    stream.read_exact(&mut request[received..]).ok()?;
    Some(String::from_utf8_lossy(&request).into_owned())
}

fn read_until_headers_end(stream: &mut TcpStream) -> Option<(Vec<u8>, usize)> {
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
        request.extend_from_slice(&chunk[..count]);
        if let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            let body_start = header_end + 4;
            return (body_start <= MAX_REQUEST_BYTES).then_some((request, body_start));
        }
        if request.len() >= MAX_REQUEST_BYTES {
            return None;
        }
    }
}
