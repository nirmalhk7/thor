//! Chromium process fork / attach helper for Thor.
//!
//! Prefer [`crate::engine::chromium_runtime::ChromiumRuntime`] for CDP-backed
//! tabs. This type retains a lightweight process handle for prewarm attach and
//! discovery-aware launches.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

use super::chromium_runtime::{
    discover_chromium_path, ChromiumError, ChromiumResult, ChromiumRuntime, ChromiumRuntimeConfig,
};

/// Holds a Chromium OS process (legacy attach path) and/or a [`ChromiumRuntime`].
pub struct ChromiumFork {
    process: Option<Child>,
    ipc_channel: String,
    executable: Option<PathBuf>,
    runtime: Option<Arc<ChromiumRuntime>>,
}

impl ChromiumFork {
    pub fn new(ipc_channel: String) -> Self {
        Self {
            process: None,
            ipc_channel,
            executable: discover_chromium_path().map(|c| c.executable_path().to_path_buf()),
            runtime: None,
        }
    }

    /// Create a fork handle using explicit discovery.
    pub fn with_discovered(ipc_channel: String) -> ChromiumResult<Self> {
        let executable = discover_chromium_path().ok_or(ChromiumError::NotFound)?.executable_path().to_path_buf();
        Ok(Self {
            process: None,
            ipc_channel,
            executable: Some(executable),
            runtime: None,
        })
    }

    pub fn ipc_channel(&self) -> &str {
        &self.ipc_channel
    }

    pub fn executable(&self) -> Option<&PathBuf> {
        self.executable.as_ref()
    }

    pub fn runtime(&self) -> Option<&Arc<ChromiumRuntime>> {
        self.runtime.as_ref()
    }

    /// Attach a pre-warmed OS child (from [`crate::daemon::prewarm::PrewarmDaemon`]).
    pub fn attach_prewarmed(&mut self, child: Child) {
        self.process = Some(child);
        println!(
            "Attached to pre-warmed Chromium instance on IPC channel: {}",
            self.ipc_channel
        );
    }

    /// Launch via chromiumoxide and store the runtime (preferred path).
    pub fn launch_runtime(&mut self, config: ChromiumRuntimeConfig) -> ChromiumResult<Arc<ChromiumRuntime>> {
        let mut config = config;
        if config.executable.is_none() {
            config.executable = self.executable.clone();
        }
        let runtime = ChromiumRuntime::launch_blocking(config)?;
        self.executable = Some(runtime.executable_path().to_path_buf());
        self.runtime = Some(Arc::clone(&runtime));
        Ok(runtime)
    }

    /// Spawn a bare Chromium with remote debugging (no CDP client yet).
    pub fn spawn_debug_process(&mut self, port: u16) -> ChromiumResult<()> {
        let exe = self
            .executable
            .clone()
            .or_else(|| discover_chromium_path().map(|c| c.executable_path().to_path_buf()))
            .ok_or(ChromiumError::NotFound)?;

        let child = Command::new(&exe)
            .args([
                format!("--remote-debugging-port={port}"),
                "--no-first-run".into(),
                "--no-default-browser-check".into(),
                "--disable-background-networking".into(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| ChromiumError::Other(e.into()))?;

        self.process = Some(child);
        self.executable = Some(exe);
        Ok(())
    }

    pub fn child_pid(&self) -> Option<u32> {
        self.process
            .as_ref()
            .map(|c| c.id())
            .or_else(|| self.runtime.as_ref().and_then(|r| r.child_pid()))
    }

    pub fn terminate(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            let _ = runtime.shutdown_blocking();
        }
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

impl Drop for ChromiumFork {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_records_ipc_and_optional_discovery() {
        let fork = ChromiumFork::new("thor-ipc-test".into());
        assert_eq!(fork.ipc_channel(), "thor-ipc-test");
        // executable may or may not be present depending on the machine
        let _ = fork.executable();
    }
}
