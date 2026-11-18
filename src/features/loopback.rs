use parking_lot::Mutex;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

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
