//! Thor Chromium runtime: discovery, launch, CDP connection, and sync-friendly handles.
//!
//! # Chromium discovery order
//! 1. `THOR_CHROMIUM_PATH` environment variable (must exist on disk)
//! 2. Common macOS / Windows / Linux Chrome & Chromium install paths
//! 3. PATH lookup (`google-chrome`, `chromium`, `chrome`, …)
//!
//! If none are found, callers receive [`ChromiumError::NotFound`].

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use anyhow::anyhow;
use chromiumoxide::browser::{Browser, BrowserConfig, BrowserConfigBuilder};
use chromiumoxide::cdp::browser_protocol::target::CreateTargetParams;
use chromiumoxide::Page;
use futures::StreamExt;
use parking_lot::Mutex as SyncMutex;
use thiserror::Error;
use tokio::runtime::{Handle, Runtime};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use super::tab_session::{TabId, TabSession};

/// Environment variable that overrides Chromium/Chrome discovery.
pub const THOR_CHROMIUM_PATH_ENV: &str = "THOR_CHROMIUM_PATH";

static NEXT_TAB_ID: AtomicU64 = AtomicU64::new(1);

/// Errors from Chromium discovery / launch / CDP wiring.
#[derive(Debug, Error)]
pub enum ChromiumError {
    #[error(
        "Chromium/Chrome not found. Set {THOR_CHROMIUM_PATH_ENV} or install Chrome/Chromium \
         (checked common macOS/Windows/Linux paths and PATH)"
    )]
    NotFound,

    #[error("browser config error: {0}")]
    Config(String),

    #[error(transparent)]
    Cdp(#[from] chromiumoxide::error::CdpError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type ChromiumResult<T> = std::result::Result<T, ChromiumError>;

/// Launch / connect options for [`ChromiumRuntime`].
#[derive(Debug, Clone)]
pub struct ChromiumRuntimeConfig {
    /// Explicit executable; when `None`, uses [`discover_chromium_path`].
    pub executable: Option<PathBuf>,
    /// Show a browser window (`true`) or run headless (`false`).
    pub headful: bool,
    /// Optional user-data directory (isolated profile).
    pub user_data_dir: Option<PathBuf>,
    /// Remote debugging port; `0` lets Chromium pick an ephemeral port.
    pub remote_debugging_port: u16,
    /// Disable the sandbox (needed in some CI / container environments).
    pub no_sandbox: bool,
    /// How many about:blank targets to keep warm for fast tab acquire.
    pub prewarm_pool_size: usize,
}

impl Default for ChromiumRuntimeConfig {
    fn default() -> Self {
        Self {
            executable: None,
            headful: true,
            user_data_dir: Some(default_thor_profile_dir()),
            remote_debugging_port: 0,
            no_sandbox: cfg!(target_os = "linux"),
            prewarm_pool_size: 2,
        }
    }
}

/// Isolated Chromium profile under the Thor data directory (branded path, not Chrome's).
pub fn default_thor_profile_dir() -> PathBuf {
    crate::paths::thor_chromium_profile_dir()
}

/// Resolve the Chromium/Chrome binary path.
pub fn discover_chromium_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(THOR_CHROMIUM_PATH_ENV) {
        let p = PathBuf::from(path);
        if p.is_file() || p.exists() {
            return Some(p);
        }
    }

    for candidate in common_chromium_candidates() {
        if candidate.is_file() || candidate.exists() {
            return Some(candidate);
        }
    }

    for name in [
        "google-chrome-stable",
        "google-chrome",
        "chromium",
        "chromium-browser",
        "chrome",
        "msedge",
        "microsoft-edge",
    ] {
        if let Some(p) = path_lookup(name) {
            return Some(p);
        }
    }

    None
}

/// Candidate install locations (platform-specific). Documented fallback list.
pub fn common_chromium_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();

    #[cfg(target_os = "macos")]
    {
        out.extend(
            [
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Chromium.app/Contents/MacOS/Chromium",
                "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            ]
            .into_iter()
            .map(PathBuf::from),
        );
        if let Ok(home) = std::env::var("HOME") {
            out.push(PathBuf::from(format!(
                "{home}/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
            )));
            out.push(PathBuf::from(format!(
                "{home}/Applications/Chromium.app/Contents/MacOS/Chromium"
            )));
        }
    }

    #[cfg(target_os = "windows")]
    {
        let prefixes = [
            std::env::var("PROGRAMFILES").ok(),
            std::env::var("PROGRAMFILES(X86)").ok(),
            std::env::var("LOCALAPPDATA").ok(),
        ];
        let rels = [
            r"Google\Chrome\Application\chrome.exe",
            r"Chromium\Application\chrome.exe",
            r"Microsoft\Edge\Application\msedge.exe",
        ];
        for prefix in prefixes.into_iter().flatten() {
            for rel in rels {
                out.push(PathBuf::from(&prefix).join(rel));
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        out.extend(
            [
                "/usr/bin/google-chrome-stable",
                "/usr/bin/google-chrome",
                "/usr/bin/chromium",
                "/usr/bin/chromium-browser",
                "/snap/bin/chromium",
                "/opt/google/chrome/chrome",
            ]
            .into_iter()
            .map(PathBuf::from),
        );
    }

    out
}

fn path_lookup(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn build_browser_config(cfg: &ChromiumRuntimeConfig) -> ChromiumResult<(BrowserConfig, PathBuf)> {
    let executable = match &cfg.executable {
        Some(p) => p.clone(),
        None => discover_chromium_path().ok_or(ChromiumError::NotFound)?,
    };

    let mut builder: BrowserConfigBuilder = BrowserConfig::builder()
        .chrome_executable(&executable)
        .port(cfg.remote_debugging_port)
        .viewport(None);

    if cfg.headful {
        builder = builder.with_head();
    } else {
        builder = builder.new_headless_mode();
    }
    if cfg.no_sandbox {
        builder = builder.no_sandbox();
    }
    if let Some(dir) = &cfg.user_data_dir {
        builder = builder.user_data_dir(dir);
    }

    let built = builder.build().map_err(ChromiumError::Config)?;
    Ok((built, executable))
}

/// Owns a connected Chromium process + CDP websocket, with a tokio [`Handle`]
/// so the shell (main thread) can drive tabs via blocking wrappers.
pub struct ChromiumRuntime {
    browser: Arc<Mutex<Browser>>,
    handle: Handle,
    /// Keeps the dedicated runtime alive when we spawned one ourselves.
    _runtime_keep_alive: Option<Arc<Runtime>>,
    executable: PathBuf,
    ws_url: String,
    child_pid: Option<u32>,
    handler_task: SyncMutex<Option<JoinHandle<()>>>,
    prewarm: Arc<SyncMutex<Vec<Page>>>,
    prewarm_target: AtomicUsize,
}

impl ChromiumRuntime {
    /// Launch Chromium on the current tokio runtime (must be called from async context).
    pub async fn launch(config: ChromiumRuntimeConfig) -> ChromiumResult<Arc<Self>> {
        let handle = Handle::current();
        Self::launch_with_handle(config, handle, None).await
    }

    /// Launch on a dedicated multi-thread runtime (safe from the GUI main thread).
    pub fn launch_blocking(config: ChromiumRuntimeConfig) -> ChromiumResult<Arc<Self>> {
        let rt = Runtime::new().map_err(|e| ChromiumError::Other(anyhow!(e)))?;
        let handle = rt.handle().clone();
        let rt = Arc::new(rt);
        let rt_keep = Arc::clone(&rt);
        handle.block_on(Self::launch_with_handle(config, handle.clone(), Some(rt_keep)))
    }

    /// Connect to an already-running browser via HTTP or WebSocket debug URL.
    pub async fn connect(debug_url: impl Into<String>) -> ChromiumResult<Arc<Self>> {
        let handle = Handle::current();
        let (browser, handler) = Browser::connect(debug_url.into()).await?;
        Self::from_browser(
            browser,
            handler,
            handle,
            None,
            PathBuf::from("<connected>"),
            0,
        )
        .await
    }

    async fn launch_with_handle(
        config: ChromiumRuntimeConfig,
        handle: Handle,
        runtime_keep_alive: Option<Arc<Runtime>>,
    ) -> ChromiumResult<Arc<Self>> {
        let (browser_config, executable) = build_browser_config(&config)?;
        let (browser, handler) = Browser::launch(browser_config).await?;
        let runtime = Self::from_browser(
            browser,
            handler,
            handle,
            runtime_keep_alive,
            executable,
            config.prewarm_pool_size,
        )
        .await?;
        runtime.replenish_prewarm().await?;
        Ok(runtime)
    }

    async fn from_browser(
        mut browser: Browser,
        mut handler: chromiumoxide::Handler,
        handle: Handle,
        runtime_keep_alive: Option<Arc<Runtime>>,
        executable: PathBuf,
        prewarm_pool_size: usize,
    ) -> ChromiumResult<Arc<Self>> {
        let ws_url = browser.websocket_address().clone();
        let child_pid = browser.get_mut_child().and_then(|c| c.inner.id());

        let handler_task = handle.spawn(async move {
            while let Some(event) = handler.next().await {
                if event.is_err() {
                    break;
                }
            }
        });

        Ok(Arc::new(Self {
            browser: Arc::new(Mutex::new(browser)),
            handle,
            _runtime_keep_alive: runtime_keep_alive,
            executable,
            ws_url,
            child_pid,
            handler_task: SyncMutex::new(Some(handler_task)),
            prewarm: Arc::new(SyncMutex::new(Vec::new())),
            prewarm_target: AtomicUsize::new(prewarm_pool_size),
        }))
    }

    /// Path of the launched executable (or `"<connected>"` when attached remotely).
    pub fn executable_path(&self) -> &Path {
        &self.executable
    }

    /// CDP WebSocket debugger URL.
    pub fn websocket_url(&self) -> &str {
        &self.ws_url
    }

    /// OS pid of the Chromium process when we spawned it.
    pub fn child_pid(&self) -> Option<u32> {
        self.child_pid
    }

    /// Tokio handle driving CDP I/O — use for `spawn` / blocking bridges.
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Run an async CDP future from a sync context (shell main thread).
    pub fn block_on<F, T>(&self, fut: F) -> T
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        block_on_handle(&self.handle, fut)
    }

    /// Create a new tab (CDP target). Prefer [`Self::acquire_tab`] when prewarm is enabled.
    pub async fn new_tab(self: &Arc<Self>, url: Option<&str>) -> ChromiumResult<TabSession> {
        let start = url.unwrap_or("about:blank");
        let page = {
            let browser = self.browser.lock().await;
            browser.new_page(start).await?
        };
        Ok(TabSession::from_page(
            next_tab_id(),
            page,
            Arc::clone(self),
        ))
    }

    /// Blocking [`Self::new_tab`].
    pub fn new_tab_blocking(self: &Arc<Self>, url: Option<&str>) -> ChromiumResult<TabSession> {
        let url = url.map(str::to_owned);
        let this = Arc::clone(self);
        self.block_on(async move { this.new_tab(url.as_deref()).await })
    }

    /// Pop a pre-warmed about:blank page, or create one if the pool is empty.
    pub async fn acquire_tab(self: &Arc<Self>) -> ChromiumResult<TabSession> {
        let page = self.prewarm.lock().pop();
        let page = match page {
            Some(p) => p,
            None => {
                let browser = self.browser.lock().await;
                let params = CreateTargetParams::builder()
                    .url("about:blank")
                    .build()
                    .map_err(ChromiumError::Config)?;
                browser.new_page(params).await?
            }
        };
        let this = Arc::clone(self);
        this.handle.spawn({
            let this = Arc::clone(&this);
            async move {
                let _ = this.replenish_prewarm().await;
            }
        });
        Ok(TabSession::from_page(next_tab_id(), page, this))
    }

    /// Blocking [`Self::acquire_tab`].
    pub fn acquire_tab_blocking(self: &Arc<Self>) -> ChromiumResult<TabSession> {
        let this = Arc::clone(self);
        self.block_on(async move { this.acquire_tab().await })
    }

    /// Navigate an existing session (convenience for shell).
    pub async fn navigate(self: &Arc<Self>, tab: &TabSession, url: &str) -> ChromiumResult<()> {
        let _ = self;
        tab.navigate(url).await
    }

    /// Close a tab session.
    pub async fn close_tab(self: &Arc<Self>, tab: TabSession) -> ChromiumResult<()> {
        let _ = self;
        tab.close().await
    }

    pub fn close_tab_blocking(self: &Arc<Self>, tab: TabSession) -> ChromiumResult<()> {
        tab.close_blocking()
    }

    async fn replenish_prewarm(self: &Arc<Self>) -> ChromiumResult<()> {
        let target = self.prewarm_target.load(Ordering::Relaxed);
        loop {
            let len = self.prewarm.lock().len();
            if len >= target {
                break;
            }
            let page = {
                let browser = self.browser.lock().await;
                browser.new_page("about:blank").await?
            };
            self.prewarm.lock().push(page);
        }
        Ok(())
    }

    /// Gracefully close the browser and cancel the handler task.
    pub async fn shutdown(self: Arc<Self>) -> ChromiumResult<()> {
        self.prewarm.lock().clear();
        {
            let mut browser = self.browser.lock().await;
            let _ = browser.close().await;
            let _ = browser.wait().await;
        }
        if let Some(task) = self.handler_task.lock().take() {
            task.abort();
        }
        Ok(())
    }

    pub fn shutdown_blocking(self: Arc<Self>) -> ChromiumResult<()> {
        let handle = self.handle.clone();
        block_on_handle(&handle, async move { self.shutdown().await })
    }

    /// Shared browser lock for advanced CDP callers (DevTools integration).
    pub fn browser(&self) -> Arc<Mutex<Browser>> {
        Arc::clone(&self.browser)
    }
}

fn next_tab_id() -> TabId {
    TabId(NEXT_TAB_ID.fetch_add(1, Ordering::Relaxed))
}

/// Run `fut` on `handle` from sync code without nested `block_on` panics.
pub fn block_on_handle<F, T>(handle: &Handle, fut: F) -> T
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    if Handle::try_current().is_ok() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        handle.spawn(async move {
            let _ = tx.send(fut.await);
        });
        rx.recv()
            .expect("chromium runtime dropped before future completed")
    } else {
        handle.block_on(fut)
    }
}

/// Spawn a background thread that keeps a tokio runtime alive and returns its handle.
pub fn spawn_engine_runtime() -> (Handle, oneshot::Sender<()>) {
    let (tx_handle, rx_handle) = std::sync::mpsc::sync_channel(1);
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    thread::spawn(move || {
        let rt = Runtime::new().expect("engine tokio runtime");
        let handle = rt.handle().clone();
        let _ = tx_handle.send(handle);
        rt.block_on(async move {
            let _ = shutdown_rx.await;
        });
    });
    let handle = rx_handle.recv().expect("engine runtime thread");
    (handle, shutdown_tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn discovery_respects_env_override() {
        let dir = std::env::temp_dir().join(format!(
            "thor-chromium-discovery-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let fake = dir.join("fake-chromium");
        std::fs::write(&fake, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&fake).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&fake, perms).unwrap();
        }

        std::env::set_var(THOR_CHROMIUM_PATH_ENV, &fake);
        let found = discover_chromium_path();
        std::env::remove_var(THOR_CHROMIUM_PATH_ENV);
        let _ = std::fs::remove_file(&fake);
        let _ = std::fs::remove_dir(&dir);

        assert_eq!(found.as_deref(), Some(fake.as_path()));
    }

    #[test]
    fn common_candidates_non_empty_on_supported_os() {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            assert!(!common_chromium_candidates().is_empty());
        }
    }

    #[test]
    fn runtime_config_defaults() {
        let cfg = ChromiumRuntimeConfig::default();
        assert!(cfg.headful);
        assert_eq!(cfg.remote_debugging_port, 0);
        assert_eq!(cfg.prewarm_pool_size, 2);
    }

    #[test]
    #[ignore = "requires a live Chrome/Chromium install"]
    fn live_launch_smoke() {
        let path = discover_chromium_path().expect("Chrome not installed");
        let cfg = ChromiumRuntimeConfig {
            executable: Some(path),
            headful: false,
            prewarm_pool_size: 1,
            no_sandbox: true,
            ..Default::default()
        };
        let rt = ChromiumRuntime::launch_blocking(cfg).expect("launch");
        let tab = rt.acquire_tab_blocking().expect("tab");
        tab.navigate_blocking("about:blank").expect("nav");
        let _ = tab.url_blocking();
        rt.close_tab_blocking(tab).expect("close tab");
        Arc::clone(&rt).shutdown_blocking().expect("shutdown");
        let _ = Duration::from_millis(1);
    }

    #[test]
    fn test_concurrent_tab_id_generation() {
        use std::thread;
        use std::sync::{Arc, Mutex};
        
        let thread_count = 10;
        let iters_per_thread = 100_000; // 1,000,000 total IDs
        let mut handles = vec![];
        let results = Arc::new(Mutex::new(Vec::with_capacity(thread_count * iters_per_thread)));
        
        for _ in 0..thread_count {
            let results_clone = Arc::clone(&results);
            handles.push(thread::spawn(move || {
                let mut local_ids = Vec::with_capacity(iters_per_thread);
                for _ in 0..iters_per_thread {
                    local_ids.push(crate::engine::chromium_runtime::next_tab_id().0);
                }
                results_clone.lock().unwrap().extend(local_ids);
            }));
        }
        
        for h in handles {
            h.join().unwrap();
        }
        
        let all_ids = results.lock().unwrap();
        assert_eq!(all_ids.len(), thread_count * iters_per_thread, "Should generate 1,000,000 IDs");
        
        // Use a boolean array for fast unique check if IDs are sequential, 
        // or a HashSet since memory is fine (1M elements in HashSet is ~16MB).
        // A vec with sort and dedup is faster.
        let mut sorted_ids = all_ids.clone();
        sorted_ids.sort_unstable();
        let original_len = sorted_ids.len();
        sorted_ids.dedup();
        let dedup_len = sorted_ids.len();
        
        assert_eq!(original_len, dedup_len, "Found {} collisions in Tab ID generation!", original_len - dedup_len);
    }
}
