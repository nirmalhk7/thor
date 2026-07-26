//! Pre-warm pool for Thor Chromium targets / pages.

use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::time::sleep;

use crate::engine::chromium_runtime::{
    discover_chromium_path, ChromiumError, ChromiumResult, ChromiumRuntime, ChromiumRuntimeConfig,
};
use crate::engine::tab_session::TabSession;

/// Keeps N ready Chromium pages (and optionally OS processes) for fast new-tab acquire.
pub struct PrewarmDaemon {
    pool_size: usize,
    /// Legacy OS-level process pool (discovery-aware).
    instances: Arc<Mutex<Vec<Child>>>,
    /// Preferred CDP-backed runtime + page pool.
    runtime: Arc<Mutex<Option<Arc<ChromiumRuntime>>>>,
}

impl PrewarmDaemon {
    pub fn new(pool_size: usize) -> Self {
        Self {
            pool_size,
            instances: Arc::new(Mutex::new(Vec::with_capacity(pool_size))),
            runtime: Arc::new(Mutex::new(None)),
        }
    }

    /// Discover Chromium and launch a [`ChromiumRuntime`] with a warm page pool.
    pub fn warm_thor_chromium(&self, headful: bool) -> ChromiumResult<Arc<ChromiumRuntime>> {
        let executable = discover_chromium_path().ok_or(ChromiumError::NotFound)?;
        let config = ChromiumRuntimeConfig {
            executable: Some(executable),
            headful,
            prewarm_pool_size: self.pool_size.max(1),
            no_sandbox: cfg!(target_os = "linux"),
            ..Default::default()
        };
        let runtime = ChromiumRuntime::launch_blocking(config)?;
        *self.runtime.lock() = Some(Arc::clone(&runtime));
        Ok(runtime)
    }

    /// Async loop that replenishes the legacy OS process pool.
    pub async fn start(&self) {
        loop {
            self.replenish_process_pool();
            sleep(Duration::from_millis(500)).await;
        }
    }

    /// Acquire a pre-warmed CDP tab when a runtime is available.
    pub fn acquire_tab(&self) -> ChromiumResult<TabSession> {
        let runtime = self
            .runtime
            .lock()
            .clone()
            .ok_or_else(|| ChromiumError::Other(anyhow::anyhow!("prewarm runtime not started; call warm_thor_chromium first")))?;
        runtime.acquire_tab_blocking()
    }

    pub fn runtime(&self) -> Option<Arc<ChromiumRuntime>> {
        self.runtime.lock().clone()
    }

    fn replenish_process_pool(&self) {
        let mut instances = self.instances.lock();

        instances.retain_mut(|child| match child.try_wait() {
            Ok(Some(_)) => false,
            Ok(None) => true,
            Err(_) => false,
        });

        while instances.len() < self.pool_size {
            match self.spawn_instance() {
                Ok(child) => instances.push(child),
                Err(_) => break,
            }
        }
    }

    fn spawn_instance(&self) -> std::io::Result<Child> {
        let cmd = discover_chromium_path().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Chromium not found (set THOR_CHROMIUM_PATH)",
            )
        })?;

        Command::new(cmd)
            .args([
                "--headless=new",
                "--disable-gpu",
                "--remote-debugging-port=0",
                "--no-first-run",
                "--no-default-browser-check",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    }

    /// Pop a legacy OS Chromium child (for [`crate::engine::chromium_fork::ChromiumFork::attach_prewarmed`]).
    pub fn acquire_instance(&self) -> Option<Child> {
        self.instances.lock().pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_starts_empty() {
        let d = PrewarmDaemon::new(2);
        assert!(d.runtime().is_none());
        assert!(d.acquire_instance().is_none());
    }
}
