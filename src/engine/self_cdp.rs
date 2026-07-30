//! Attach Rust Labs to a running Thor Chromium instance via self-CDP.

use std::sync::Arc;

use super::chromium_runtime::{ChromiumResult, ChromiumRuntime};

/// Connect to Thor's own browser DevTools endpoint (`http://host:port` or `ws://…`).
pub async fn connect_runtime(debug_url: impl Into<String>) -> ChromiumResult<Arc<ChromiumRuntime>> {
    ChromiumRuntime::connect(debug_url.into()).await
}

/// Blocking connect for CLI / native-messaging hosts.
pub fn connect_runtime_blocking(
    debug_url: impl Into<String>,
) -> ChromiumResult<Arc<ChromiumRuntime>> {
    let url = debug_url.into();
    let rt = tokio::runtime::Runtime::new().map_err(|e| {
        super::chromium_runtime::ChromiumError::Other(anyhow::anyhow!(e))
    })?;
    rt.block_on(connect_runtime(url))
}
