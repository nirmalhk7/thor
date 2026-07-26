//! Real CDP client wrapper for console / DevTools helpers.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use chromiumoxide::cdp::js_protocol::runtime::EventConsoleApiCalled;
use futures::StreamExt;
use parking_lot::Mutex;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

use crate::engine::chromium_runtime::{ChromiumError, ChromiumResult, ChromiumRuntime};
use crate::engine::tab_session::TabSession;

/// Represents a console.log (and friends) event from the JS runtime via CDP.
#[derive(Debug, Clone)]
pub struct ConsoleLogEvent {
    pub timestamp: u64,
    pub level: String,
    pub message: String,
}

/// Native DevTools panel consuming CDP console events + inspector URL helpers.
pub struct CdpIntegration {
    pub is_connected: bool,
    logs: Arc<Mutex<Vec<ConsoleLogEvent>>>,
    log_tx: Option<UnboundedSender<ConsoleLogEvent>>,
    log_rx: Option<UnboundedReceiver<ConsoleLogEvent>>,
    listener_task: Option<JoinHandle<()>>,
    ws_url: Option<String>,
}

impl CdpIntegration {
    pub fn new() -> Self {
        Self {
            is_connected: false,
            logs: Arc::new(Mutex::new(Vec::new())),
            log_tx: None,
            log_rx: None,
            listener_task: None,
            ws_url: None,
        }
    }

    /// Attach to a runtime's websocket URL (for DevTools frontend links).
    pub fn attach_runtime(&mut self, runtime: &ChromiumRuntime) {
        self.ws_url = Some(runtime.websocket_url().to_string());
        self.is_connected = true;
    }

    /// Subscribe to `Runtime.consoleAPICalled` on a tab's page.
    pub async fn attach_tab_console(&mut self, tab: &TabSession) -> ChromiumResult<()> {
        self.ws_url = Some(tab.runtime().websocket_url().to_string());
        self.is_connected = true;

        let (tx, rx) = unbounded_channel();
        self.log_tx = Some(tx.clone());
        self.log_rx = Some(rx);

        let page = tab.page().ok_or_else(|| ChromiumError::Config("Page not initialized".to_string()))?;
        let mut events = page
            .event_listener::<EventConsoleApiCalled>()
            .await?;

        let logs = Arc::clone(&self.logs);
        let handle = tab.runtime().handle().spawn(async move {
            while let Some(event) = events.next().await {
                let level = format!("{:?}", event.r#type).to_lowercase();
                let message = event
                    .args
                    .iter()
                    .filter_map(|arg| {
                        arg.value
                            .as_ref()
                            .map(|v| v.to_string())
                            .or_else(|| arg.description.clone())
                            .or_else(|| arg.unserializable_value.as_ref().map(|u| format!("{u:?}")))
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let entry = ConsoleLogEvent {
                    timestamp: now_ms(),
                    level,
                    message,
                };
                logs.lock().push(entry.clone());
                let _ = tx.send(entry);
            }
        });
        self.listener_task = Some(handle);
        Ok(())
    }

    /// Blocking wrapper for [`Self::attach_tab_console`].
    pub fn attach_tab_console_blocking(&mut self, tab: &TabSession) -> ChromiumResult<()> {
        // We need &mut self across block_on — clone needed fields carefully.
        let page = match tab.page() {
            Some(p) => p,
            None => return Err(ChromiumError::Config("Page not initialized".to_string())),
        };
        let handle = tab.runtime().handle().clone();
        let ws = tab.runtime().websocket_url().to_string();
        let logs = Arc::clone(&self.logs);

        let (tx, rx) = unbounded_channel();
        let tx_bg = tx.clone();

        let task = handle.spawn(async move {
            let mut events = match page.event_listener::<EventConsoleApiCalled>().await {
                Ok(e) => e,
                Err(_) => return,
            };
            while let Some(event) = events.next().await {
                let level = format!("{:?}", event.r#type).to_lowercase();
                let message = event
                    .args
                    .iter()
                    .filter_map(|arg| {
                        arg.value
                            .as_ref()
                            .map(|v| v.to_string())
                            .or_else(|| arg.description.clone())
                            .or_else(|| arg.unserializable_value.as_ref().map(|u| format!("{u:?}")))
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let entry = ConsoleLogEvent {
                    timestamp: now_ms(),
                    level,
                    message,
                };
                logs.lock().push(entry.clone());
                let _ = tx_bg.send(entry);
            }
        });

        self.ws_url = Some(ws);
        self.is_connected = true;
        self.log_tx = Some(tx);
        self.log_rx = Some(rx);
        self.listener_task = Some(task);
        Ok(())
    }

    /// Legacy mpsc-style inject path (kept for tests / non-CDP producers).
    pub fn connect_local_feed(&mut self) -> UnboundedSender<ConsoleLogEvent> {
        self.is_connected = true;
        let (tx, rx) = unbounded_channel();
        self.log_tx = Some(tx.clone());
        self.log_rx = Some(rx);
        tx
    }

    /// Drain the CDP / local feed into the ring buffer.
    pub fn poll_logs(&mut self) {
        if let Some(rx) = self.log_rx.as_mut() {
            while let Ok(log) = rx.try_recv() {
                self.logs.lock().push(log);
            }
        }
    }

    pub fn get_recent_logs(&self, count: usize) -> Vec<ConsoleLogEvent> {
        let logs = self.logs.lock();
        let skip = if logs.len() > count {
            logs.len() - count
        } else {
            0
        };
        logs.iter().skip(skip).cloned().collect()
    }

    /// Build a Chrome DevTools frontend URL for the attached websocket, when possible.
    ///
    /// Format: `devtools://devtools/bundled/inspector.html?ws=<host>/<path>`
    pub fn open_devtools_url(&self) -> Option<String> {
        let ws = self.ws_url.as_ref()?;
        let stripped = ws
            .strip_prefix("ws://")
            .or_else(|| ws.strip_prefix("wss://"))?;
        Some(format!(
            "devtools://devtools/bundled/inspector.html?ws={stripped}"
        ))
    }

    /// HTTP endpoint often used to list targets: derive from ws URL host.
    pub fn remote_debugging_http_base(&self) -> Option<String> {
        let ws = self.ws_url.as_ref()?;
        // ws://127.0.0.1:9222/devtools/browser/... -> http://127.0.0.1:9222
        let rest = ws
            .strip_prefix("ws://")
            .or_else(|| ws.strip_prefix("wss://"))?;
        let host_port = rest.split('/').next()?;
        let scheme = if ws.starts_with("wss://") {
            "https"
        } else {
            "http"
        };
        Some(format!("{scheme}://{host_port}"))
    }
}

impl Default for CdpIntegration {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CdpIntegration {
    fn drop(&mut self) {
        if let Some(task) = self.listener_task.take() {
            task.abort();
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_feed_and_poll() {
        let mut cdp = CdpIntegration::new();
        let tx = cdp.connect_local_feed();
        tx.send(ConsoleLogEvent {
            timestamp: 1,
            level: "log".into(),
            message: "hi".into(),
        })
        .unwrap();
        cdp.poll_logs();
        let recent = cdp.get_recent_logs(10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].message, "hi");
    }

    #[test]
    fn devtools_url_from_ws() {
        let mut cdp = CdpIntegration::new();
        cdp.ws_url = Some("ws://127.0.0.1:9222/devtools/browser/abc".into());
        let url = cdp.open_devtools_url().unwrap();
        assert!(url.contains("devtools://devtools/bundled/inspector.html?ws=127.0.0.1:9222"));
        assert_eq!(
            cdp.remote_debugging_http_base().as_deref(),
            Some("http://127.0.0.1:9222")
        );
    }
}
