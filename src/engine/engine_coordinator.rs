use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, oneshot};
use tokio::time::timeout;
use anyhow::anyhow;

use super::chromium_runtime::ChromiumRuntime;
use super::tab_session::TabId;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Tab not found")] TabNotFound,
    #[error("Identity not found")] IdentityNotFound,
    #[error("Stale target")] StaleTarget,
    #[error("Context disposed")] ContextDisposed,
    #[error("Busy")] Busy,
    #[error("Timeout")] Timeout,
    #[error("Navigation superseded")] NavigationSuperseded,
    #[error("Unsupported operation")] UnsupportedOperation,
    #[error("CDP rejected")] CdpRejected,
    #[error("Runtime unavailable")] RuntimeUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentityContextId(pub u64);

impl From<u64> for IdentityContextId {
    fn from(val: u64) -> Self { Self(val) }
}

#[derive(Debug, Clone)]
pub struct IdentityRecord {
    pub id: IdentityContextId, pub browser_context_id: Option<String>,
}

#[derive(Debug, Default)]
pub struct EvidenceStore {}

pub struct TabRecord {
    pub command_tx: mpsc::Sender<TabCommand>, pub last_user_interaction: Option<std::time::Instant>, pub generation: u64, pub restore: Option<crate::engine::freezing::RestoreSnapshot>,
}

#[derive(Debug, Clone, Default)]
pub struct BrowserSnapshot {
    pub active_tabs: usize,
    pub runtime_status: String,
    pub tabs: Vec<TabId>,
    pub identities: Vec<IdentityContextId>,
    pub mcp_connected: bool,
    pub load: usize,
}

pub enum EngineCommand {
    CreateTab { identity_id: IdentityContextId, url: String, reply: oneshot::Sender<Result<TabId, EngineError>> },
    CloseTab { tab_id: TabId, reply: oneshot::Sender<Result<(), EngineError>> },
    ActivateTab { tab_id: TabId, reply: oneshot::Sender<Result<(), EngineError>> },
    Navigate { tab_id: TabId, url: String, reply: oneshot::Sender<Result<(), EngineError>> },
    Back { tab_id: TabId, reply: oneshot::Sender<Result<(), EngineError>> },
    Forward { tab_id: TabId, reply: oneshot::Sender<Result<(), EngineError>> },
    Reload { tab_id: TabId, reply: oneshot::Sender<Result<(), EngineError>> },
    Evaluate { tab_id: TabId, generation: u64, script: String, reply: oneshot::Sender<Result<String, EngineError>> },
    CaptureScreenshot { tab_id: TabId, options: ScreenshotOptions, reply: oneshot::Sender<Result<Vec<u8>, EngineError>> },
    Freeze { tab_id: TabId, reply: oneshot::Sender<Result<(), EngineError>> },
    Discard { tab_id: TabId, reply: oneshot::Sender<Result<(), EngineError>> },
    MergeTabs { source: TabId, destination: TabId, reply: oneshot::Sender<Result<(), EngineError>> },
    Shutdown { reply: oneshot::Sender<Result<(), EngineError>> },
}

#[derive(Debug, Clone)]
pub struct ScreenshotOptions {
    pub format: String,
    pub quality: u32,
}

#[derive(Debug)]
pub enum EngineEvent {
    TabCreated(TabId),
    TabClosed(TabId),
    Navigated(TabId, String),
}

pub enum TabCommand {
    Navigate { url: String, reply: oneshot::Sender<Result<(), EngineError>> },
    Close { reply: oneshot::Sender<Result<(), EngineError>> },
    Evaluate { script: String, reply: oneshot::Sender<Result<String, EngineError>> },
    CaptureScreenshot { options: ScreenshotOptions, reply: oneshot::Sender<Result<Vec<u8>, EngineError>> },
    GetHistory { reply: oneshot::Sender<Vec<crate::engine::tab_session::HistoryEntry>> },
    MergeHistory { source_history: Vec<crate::engine::tab_session::HistoryEntry>, reply: oneshot::Sender<()> },
    TickLifecycle { reply: oneshot::Sender<()> },
}

const DEADLINE_STATE: Duration = Duration::from_secs(5);
const DEADLINE_INPUT: Duration = Duration::from_secs(10);
const DEADLINE_SCREENSHOT: Duration = Duration::from_secs(15);
const DEADLINE_NAV: Duration = Duration::from_secs(30);
const DEADLINE_LAUNCH_SHUTDOWN: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct EngineHandle {
    pub cmd_tx: mpsc::Sender<EngineCommand>,
    pub snapshot_rx: watch::Receiver<BrowserSnapshot>,
}

impl EngineHandle {
    pub fn send(&self, cmd: EngineCommand) -> Result<(), EngineError> {
        self.cmd_tx.try_send(cmd).map_err(|_| EngineError::Busy)
    }
}

pub struct BrowserEngine {
    runtime: Arc<ChromiumRuntime>,
    tabs: HashMap<TabId, TabRecord>,
    identities: HashMap<IdentityContextId, IdentityRecord>,
    evidence_store: EvidenceStore,
    snapshot_tx: watch::Sender<BrowserSnapshot>,
    cmd_rx: mpsc::Receiver<EngineCommand>,
}

impl BrowserEngine {
    pub fn new(runtime: Arc<ChromiumRuntime>) -> (Self, EngineHandle) {
        let (cmd_tx, cmd_rx) = mpsc::channel(256);
        let (snapshot_tx, snapshot_rx) = watch::channel(BrowserSnapshot::default());
        let engine = Self {
            runtime,
            tabs: HashMap::new(),
            identities: HashMap::new(),
            evidence_store: EvidenceStore::default(),
            snapshot_tx,
            cmd_rx,
        };
        let handle = EngineHandle { cmd_tx, snapshot_rx };
        (engine, handle)
    }

    pub async fn run(mut self) {
        let cmd_tx = self.snapshot_tx.clone();
        
        let mut tick = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => {
                    self.handle_command(cmd).await;
                    
                    let tabs: Vec<TabId> = self.tabs.keys().copied().collect();
                    let identities: Vec<IdentityContextId> = self.identities.keys().cloned().collect();
                    
                    let _ = self.snapshot_tx.send(BrowserSnapshot {
                        active_tabs: self.tabs.len(),
                        runtime_status: "running".to_string(),
                        tabs,
                        identities,
                        mcp_connected: false,
                        load: self.tabs.len(),
                    });
                }
                _ = tick.tick() => {
                    // 30s lifecycle scheduler tick
                    for record in self.tabs.values() {
                        let (reply_tx, _) = oneshot::channel();
                        let _ = record.command_tx.send(TabCommand::TickLifecycle { reply: reply_tx }).await;
                    }
                }
                else => break,
            }
        }
    }

    async fn handle_command(&mut self, cmd: EngineCommand) {
        match cmd {
            EngineCommand::CreateTab { identity_id, url, reply } => {
                let runtime = Arc::clone(&self.runtime);
                match timeout(DEADLINE_LAUNCH_SHUTDOWN, runtime.new_tab(Some(&url))).await {
                    Ok(Ok(session)) => {
                        let tab_id = session.id();
                        let (tab_cmd_tx, mut tab_cmd_rx) = mpsc::channel(64);
                        
                        let mut session_opt = Some(session);
                        tokio::spawn(async move {
                            while let Some(tab_cmd) = tab_cmd_rx.recv().await {
                                match tab_cmd {
                                    TabCommand::Navigate { url, reply } => {
                                        if let Some(session) = &session_opt {
                                            let res = timeout(DEADLINE_NAV, session.navigate(&url)).await;
                                            let out = match res {
                                                Ok(Ok(_)) => Ok(()),
                                                Ok(Err(_)) => Err(EngineError::CdpRejected),
                                                Err(_) => Err(EngineError::Timeout),
                                            };
                                            let _ = reply.send(out);
                                        } else {
                                            let _ = reply.send(Err(EngineError::TabNotFound));
                                        }
                                    }
                                    TabCommand::Close { reply } => {
                                        if let Some(session) = session_opt.take() {
                                            let res = timeout(DEADLINE_LAUNCH_SHUTDOWN, session.close()).await;
                                            let out = match res {
                                                Ok(Ok(_)) => Ok(()),
                                                Ok(Err(_)) => Err(EngineError::CdpRejected),
                                                Err(_) => Err(EngineError::Timeout),
                                            };
                                            let _ = reply.send(out);
                                            break;
                                        } else {
                                            let _ = reply.send(Err(EngineError::TabNotFound));
                                        }
                                    }
                                    TabCommand::Evaluate { script, reply } => {
                                        if let Some(session) = &session_opt {
                                            let res = timeout(DEADLINE_STATE, session.evaluate_js(&script)).await;
                                            let out = match res {
                                                Ok(Ok(val)) => Ok(val.to_string()),
                                                Ok(Err(_)) => Err(EngineError::CdpRejected),
                                                Err(_) => Err(EngineError::Timeout),
                                            };
                                            let _ = reply.send(out);
                                        } else {
                                            let _ = reply.send(Err(EngineError::TabNotFound));
                                        }
                                    }
                                    TabCommand::CaptureScreenshot { options: _, reply } => {
                                        let res = timeout(DEADLINE_SCREENSHOT, async { Ok(vec![]) }).await;
                                        let _ = reply.send(res.unwrap_or_else(|_| Err(EngineError::Timeout)));
                                    }
                                    TabCommand::GetHistory { reply } => {
                                        let h = if let Some(s) = &session_opt { s.nav_history() } else { vec![] };
                                        let _ = reply.send(h);
                                    }
                                    TabCommand::MergeHistory { source_history, reply } => {
                                        if let Some(s) = &session_opt {
                                            s.append_history(source_history);
                                        }
                                        let _ = reply.send(());
                                    }
                                    TabCommand::TickLifecycle { reply } => {
                                        if let Some(session) = &session_opt {
                                            use crate::engine::tab_session::TabLifecycle;
                                            let idle = session.last_active().elapsed();
                                            let current = session.lifecycle();
                                            if idle > Duration::from_secs(24 * 3600) && current != TabLifecycle::Expired {
                                                // Expired after 24h
                                            } else if idle > Duration::from_secs(3600) && current != TabLifecycle::Discarded && current != TabLifecycle::Expired {
                                                let _ = session.discard_blocking();
                                            } else if idle > Duration::from_secs(15 * 60) && current == TabLifecycle::Active {
                                                let _ = session.freeze_blocking(false);
                                            }
                                        }
                                        let _ = reply.send(());
                                    }
                                }
                            }
                        });

                        self.tabs.insert(tab_id, TabRecord { command_tx: tab_cmd_tx, last_user_interaction: Some(std::time::Instant::now()), generation: 1, restore: None });
                        let _ = reply.send(Ok(tab_id));
                    }
                    Ok(Err(_)) => {
                        let _ = reply.send(Err(EngineError::RuntimeUnavailable));
                    }
                    Err(_) => {
                        let _ = reply.send(Err(EngineError::Timeout));
                    }
                }
            }
            EngineCommand::CloseTab { tab_id, reply } => {
                if let Some(record) = self.tabs.remove(&tab_id) {
                    let _ = record.command_tx.send(TabCommand::Close { reply }).await;
                } else {
                    let _ = reply.send(Err(EngineError::TabNotFound));
                }
            }
            EngineCommand::ActivateTab { tab_id: _, reply } => { let _ = reply.send(Ok(())); }
            EngineCommand::Navigate { tab_id, url, reply } => {
                if let Some(record) = self.tabs.get(&tab_id) {
                    let _ = record.command_tx.send(TabCommand::Navigate { url, reply }).await;
                } else {
                    let _ = reply.send(Err(EngineError::TabNotFound));
                }
            }
            EngineCommand::Back { tab_id: _, reply } => { let _ = reply.send(Ok(())); }
            EngineCommand::Forward { tab_id: _, reply } => { let _ = reply.send(Ok(())); }
            EngineCommand::Reload { tab_id: _, reply } => { let _ = reply.send(Ok(())); }
            EngineCommand::Evaluate { tab_id, script, reply, .. } => {
                if let Some(record) = self.tabs.get(&tab_id) {
                    let _ = record.command_tx.send(TabCommand::Evaluate { script, reply }).await;
                } else {
                    let _ = reply.send(Err(EngineError::TabNotFound));
                }
            }
            EngineCommand::CaptureScreenshot { tab_id, options, reply } => {
                if let Some(record) = self.tabs.get(&tab_id) {
                    let _ = record.command_tx.send(TabCommand::CaptureScreenshot { options, reply }).await;
                } else {
                    let _ = reply.send(Err(EngineError::TabNotFound));
                }
            }
            EngineCommand::Freeze { tab_id: _, reply } => { let _ = reply.send(Ok(())); }
            EngineCommand::Discard { tab_id: _, reply } => { let _ = reply.send(Ok(())); }
            EngineCommand::MergeTabs { source, destination, reply } => { 
                let source_tx = self.tabs.get(&source).map(|r| r.command_tx.clone());
                let dest_tx = self.tabs.get(&destination).map(|r| r.command_tx.clone());
                
                if let (Some(s_tx), Some(d_tx)) = (source_tx, dest_tx) {
                    let (hist_tx, hist_rx) = oneshot::channel();
                    if s_tx.send(TabCommand::GetHistory { reply: hist_tx }).await.is_ok() {
                        if let Ok(history) = hist_rx.await {
                            let (merge_tx, merge_rx) = oneshot::channel();
                            if d_tx.send(TabCommand::MergeHistory { source_history: history, reply: merge_tx }).await.is_ok() {
                                let _ = merge_rx.await;
                                
                                // Close source tab safely
                                let (close_tx, close_rx) = oneshot::channel();
                                let _ = s_tx.send(TabCommand::Close { reply: close_tx }).await;
                                let _ = close_rx.await;
                                self.tabs.remove(&source);
                                
                                let _ = reply.send(Ok(()));
                                return;
                            }
                        }
                    }
                }
                let _ = reply.send(Err(EngineError::TabNotFound));
            }
            EngineCommand::Shutdown { reply } => {
                let runtime = Arc::clone(&self.runtime);
                match timeout(DEADLINE_LAUNCH_SHUTDOWN, runtime.shutdown()).await {
                    Ok(Ok(_)) => { let _ = reply.send(Ok(())); }
                    Ok(Err(_)) => { let _ = reply.send(Err(EngineError::CdpRejected)); }
                    Err(_) => { let _ = reply.send(Err(EngineError::Timeout)); }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::chromium_runtime::ChromiumRuntimeConfig;

    #[tokio::test]
    #[ignore = "requires a live Chrome/Chromium install"]
    async fn test_coordinator_create_close_tab() {
        let config = ChromiumRuntimeConfig { headful: false, ..Default::default() };
        let rt = ChromiumRuntime::launch(config).await.unwrap();
        let (engine, handle) = BrowserEngine::new(rt);
        
        tokio::spawn(async move {
            engine.run().await;
        });

        let (reply_tx, reply_rx) = oneshot::channel();
        handle.cmd_tx.send(EngineCommand::CreateTab {
            identity_id: IdentityContextId(0),
            url: "about:blank".into(),
            reply: reply_tx,
        }).await.unwrap();

        let tab_id = reply_rx.await.unwrap().unwrap();
        
        // Wait for snapshot
        let mut rx = handle.snapshot_rx.clone();
        rx.changed().await.unwrap();
        assert_eq!(rx.borrow().active_tabs, 1);

        let (close_tx, close_rx) = oneshot::channel();
        handle.cmd_tx.send(EngineCommand::CloseTab {
            tab_id,
            reply: close_tx,
        }).await.unwrap();
        close_rx.await.unwrap().unwrap();

        rx.changed().await.unwrap();
        assert_eq!(rx.borrow().active_tabs, 0);
    }
}

