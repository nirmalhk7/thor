use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::timeout;

use super::chromium_runtime::ChromiumRuntime;
use super::tab_session::{lifecycle_for_idle, HistoryEntry, TabId, TabLifecycle, TabSession};
use crate::app_state::AppState;
use crate::engine::profiles::PrivacyStatus;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Tab not found")]
    TabNotFound,
    #[error("Identity not found")]
    IdentityNotFound,
    #[error("Default identity cannot be destroyed")]
    DefaultIdentityProtected,
    #[error("Identity context is not owned by Thor")]
    IdentityNotOwned,
    #[error("Busy")]
    Busy,
    #[error("Timeout")]
    Timeout,
    #[error("CDP rejected")]
    CdpRejected,
    #[error("Runtime unavailable")]
    RuntimeUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentityContextId(pub u64);

impl From<u64> for IdentityContextId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone)]
pub struct IdentityRecord {
    pub id: IdentityContextId,
    /// User-visible Thor identity label. Chromium BrowserContext IDs remain
    /// opaque implementation details and are never used as the label.
    pub name: String,
    pub browser_context_id: Option<String>,
    pub retention: IdentityRetention,
    pub owned_by_thor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisposableTab {
    pub tab_id: TabId,
    pub identity_id: IdentityContextId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityRetention {
    UntilExplicitDestroy,
    UntilLastTabCloses,
}

#[derive(Debug, Default)]
pub struct EvidenceStore {}

struct TabRecord {
    command_tx: mpsc::Sender<TabCommand>,
    session: Arc<TabSession>,
    identity_id: IdentityContextId,
    #[allow(dead_code)]
    last_user_interaction: Option<std::time::Instant>,
    #[allow(dead_code)]
    generation: u64,
    #[allow(dead_code)]
    restore: Option<crate::engine::freezing::RestoreSnapshot>,
}

#[derive(Debug, Clone, Default)]
pub struct BrowserSnapshot {
    pub active_tabs: usize,
    pub active_tab: Option<TabId>,
    pub runtime_status: String,
    pub tabs: Vec<TabId>,
    pub identities: Vec<IdentityContextId>,
    pub identity_labels: Vec<(IdentityContextId, String)>,
    pub mcp_connected: bool,
    pub load: usize,
    pub privacy_status: Option<PrivacyStatus>,
}

pub enum EngineCommand {
    NewIdentity {
        name: Option<String>,
        reply: oneshot::Sender<Result<IdentityContextId, EngineError>>,
    },
    DestroyIdentity {
        identity_id: IdentityContextId,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    CreateDisposableTab {
        url: String,
        reply: oneshot::Sender<Result<DisposableTab, EngineError>>,
    },
    CreateTab {
        identity_id: IdentityContextId,
        url: String,
        reply: oneshot::Sender<Result<TabId, EngineError>>,
    },
    CloseTab {
        tab_id: TabId,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    ActivateTab {
        tab_id: TabId,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Navigate {
        tab_id: TabId,
        url: String,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Back {
        tab_id: TabId,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Forward {
        tab_id: TabId,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Reload {
        tab_id: TabId,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Evaluate {
        tab_id: TabId,
        generation: u64,
        script: String,
        reply: oneshot::Sender<Result<String, EngineError>>,
    },
    CaptureScreenshot {
        tab_id: TabId,
        options: ScreenshotOptions,
        reply: oneshot::Sender<Result<Vec<u8>, EngineError>>,
    },
    TogglePictureInPicture {
        tab_id: TabId,
        reply: oneshot::Sender<Result<crate::engine::browser_features::PipResult, EngineError>>,
    },
    Freeze {
        tab_id: TabId,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Discard {
        tab_id: TabId,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    MergeTabs {
        source: TabId,
        destination: TabId,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Shutdown {
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
}

#[derive(Debug, Clone)]
pub struct ScreenshotOptions {
    pub format: String,
    pub quality: u32,
}

enum TabCommand {
    Navigate {
        url: String,
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Activate {
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Back {
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Forward {
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Reload {
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Freeze {
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Discard {
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Close {
        reply: oneshot::Sender<Result<(), EngineError>>,
    },
    Evaluate {
        script: String,
        reply: oneshot::Sender<Result<String, EngineError>>,
    },
    CaptureScreenshot {
        options: ScreenshotOptions,
        reply: oneshot::Sender<Result<Vec<u8>, EngineError>>,
    },
    TogglePictureInPicture {
        reply: oneshot::Sender<Result<crate::engine::browser_features::PipResult, EngineError>>,
    },
    GetHistory {
        reply: oneshot::Sender<Vec<HistoryEntry>>,
    },
    MergeHistory {
        source_history: Vec<HistoryEntry>,
        reply: oneshot::Sender<()>,
    },
    TickLifecycle {
        reply: oneshot::Sender<Result<TabLifecycle, EngineError>>,
    },
}

const DEADLINE_STATE: Duration = Duration::from_secs(5);
const DEADLINE_SCREENSHOT: Duration = Duration::from_secs(15);
const DEADLINE_NAV: Duration = Duration::from_secs(30);
const DEADLINE_CREATE_TAB: Duration = Duration::from_secs(45);
const DEADLINE_LAUNCH_SHUTDOWN: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct EngineHandle {
    pub cmd_tx: mpsc::Sender<EngineCommand>,
    pub snapshot_rx: watch::Receiver<BrowserSnapshot>,
}

impl EngineHandle {
    pub fn send(&self, command: EngineCommand) -> Result<(), EngineError> {
        self.cmd_tx.try_send(command).map_err(|_| EngineError::Busy)
    }

    pub fn active_tab(&self) -> Option<TabId> {
        self.snapshot_rx.borrow().active_tab
    }

    pub async fn new_identity(&self) -> Result<IdentityContextId, EngineError> {
        self.new_identity_named(None).await
    }

    pub async fn new_identity_named(
        &self,
        name: Option<String>,
    ) -> Result<IdentityContextId, EngineError> {
        let (reply, response) = oneshot::channel();
        self.cmd_tx
            .send(EngineCommand::NewIdentity { name, reply })
            .await
            .map_err(|_| EngineError::RuntimeUnavailable)?;
        match timeout(DEADLINE_LAUNCH_SHUTDOWN, response).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(EngineError::RuntimeUnavailable),
            Err(_) => Err(EngineError::Timeout),
        }
    }

    pub async fn destroy_identity(
        &self,
        identity_id: IdentityContextId,
    ) -> Result<(), EngineError> {
        let (reply, response) = oneshot::channel();
        self.cmd_tx
            .send(EngineCommand::DestroyIdentity { identity_id, reply })
            .await
            .map_err(|_| EngineError::RuntimeUnavailable)?;
        match timeout(DEADLINE_LAUNCH_SHUTDOWN, response).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(EngineError::RuntimeUnavailable),
            Err(_) => Err(EngineError::Timeout),
        }
    }

    /// Create a tab in a fresh BrowserContext owned by Thor. Closing the last
    /// tab in that context disposes the complete identity context.
    pub async fn create_disposable_tab(&self, url: String) -> Result<DisposableTab, EngineError> {
        let (reply, response) = oneshot::channel();
        self.cmd_tx
            .send(EngineCommand::CreateDisposableTab { url, reply })
            .await
            .map_err(|_| EngineError::RuntimeUnavailable)?;
        match timeout(DEADLINE_CREATE_TAB, response).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(EngineError::RuntimeUnavailable),
            Err(_) => Err(EngineError::Timeout),
        }
    }
}

fn normalize_identity_name(name: Option<String>, id: u64) -> String {
    let Some(name) = name else {
        return format!("Identity {id}");
    };
    let normalized = name.trim();
    if normalized.is_empty() {
        format!("Identity {id}")
    } else {
        normalized.chars().take(80).collect()
    }
}

pub struct BrowserEngine {
    runtime: Arc<ChromiumRuntime>,
    tabs: HashMap<TabId, TabRecord>,
    identities: HashMap<IdentityContextId, IdentityRecord>,
    active_tab: Option<TabId>,
    next_identity_id: u64,
    #[allow(dead_code)]
    evidence_store: EvidenceStore,
    snapshot_tx: watch::Sender<BrowserSnapshot>,
    cmd_rx: mpsc::Receiver<EngineCommand>,
}

impl BrowserEngine {
    pub fn new(runtime: Arc<ChromiumRuntime>) -> (Self, EngineHandle) {
        let (cmd_tx, cmd_rx) = mpsc::channel(256);
        let (snapshot_tx, snapshot_rx) = watch::channel(BrowserSnapshot::default());
        let mut identities = HashMap::new();
        identities.insert(
            IdentityContextId(0),
            IdentityRecord {
                id: IdentityContextId(0),
                name: "Default".into(),
                browser_context_id: None,
                retention: IdentityRetention::UntilExplicitDestroy,
                owned_by_thor: false,
            },
        );
        (
            Self {
                runtime,
                tabs: HashMap::new(),
                identities,
                active_tab: None,
                next_identity_id: 1,
                evidence_store: EvidenceStore::default(),
                snapshot_tx,
                cmd_rx,
            },
            EngineHandle {
                cmd_tx,
                snapshot_rx,
            },
        )
    }

    /// Adopt the existing page targets exposed by Thor's self-CDP endpoint.
    pub async fn attach_existing(
        runtime: Arc<ChromiumRuntime>,
    ) -> Result<(Self, EngineHandle), EngineError> {
        let sessions = runtime
            .existing_tabs()
            .await
            .map_err(|_| EngineError::RuntimeUnavailable)?;
        let (mut engine, handle) = Self::new(runtime);
        for session in sessions {
            let id = session.id();
            let identity_id = engine.identity_id_for_session(&session);
            engine
                .tabs
                .insert(id, Self::spawn_tab_worker(session, identity_id));
            engine.active_tab = Some(id);
        }
        engine.publish_snapshot();
        Ok((engine, handle))
    }

    pub async fn run(mut self) {
        let mut tick = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                command = self.cmd_rx.recv() => {
                    match command {
                        Some(command) => {
                            self.handle_command(command).await;
                            self.publish_snapshot();
                        }
                        None => {
                            self.dispose_owned_ephemeral_identities().await;
                            break;
                        }
                    }
                }
                _ = tick.tick() => {
                    self.adopt_existing_tabs().await;
                    self.tick_lifecycles().await;
                    self.retry_unused_identity_cleanup().await;
                    self.publish_snapshot();
                }
            }
        }
    }

    fn publish_snapshot(&self) {
        let mut tabs: Vec<_> = self.tabs.keys().copied().collect();
        tabs.sort_by_key(|tab| tab.0);
        let identity_isolated = self
            .active_tab
            .and_then(|tab_id| self.tabs.get(&tab_id))
            .is_some_and(|record| record.identity_id.0 != 0);
        let privacy_status = AppState::global()
            .profiles()
            .get_profile()
            // The sidecar cannot prove which Chromium patchset a generic CDP
            // connection is running, so native defaults remain unverified.
            .privacy_status(false, identity_isolated);
        let mut identity_labels: Vec<_> = self
            .identities
            .values()
            .map(|identity| (identity.id.clone(), identity.name.clone()))
            .collect();
        identity_labels.sort_by_key(|(id, _)| id.0);
        let _ = self.snapshot_tx.send(BrowserSnapshot {
            active_tabs: tabs.len(),
            active_tab: self.active_tab,
            runtime_status: "running".into(),
            tabs,
            identities: self.identities.keys().cloned().collect(),
            identity_labels,
            mcp_connected: false,
            load: self.tabs.len(),
            privacy_status: Some(privacy_status),
        });
    }

    fn allocate_identity_id(&mut self) -> IdentityContextId {
        loop {
            if self.next_identity_id == 0 {
                self.next_identity_id = 1;
            }
            let id = IdentityContextId(self.next_identity_id);
            self.next_identity_id = self.next_identity_id.checked_add(1).unwrap_or(1);
            if !self.identities.contains_key(&id) {
                return id;
            }
        }
    }

    fn identity_id_for_session(&mut self, session: &TabSession) -> IdentityContextId {
        let context = session.identity_context();
        if !context.is_ephemeral {
            return IdentityContextId(0);
        }
        if let Some(record) = self
            .identities
            .values()
            .find(|record| record.browser_context_id.as_deref() == Some(context.id.as_str()))
        {
            return record.id.clone();
        }

        let id = self.allocate_identity_id();
        self.identities.insert(
            id.clone(),
            IdentityRecord {
                id: id.clone(),
                name: format!("Recovered identity {}", id.0),
                browser_context_id: Some(context.id),
                retention: IdentityRetention::UntilExplicitDestroy,
                // Recovered contexts may belong to another CDP client. Track
                // them, but never destroy them without ownership evidence.
                owned_by_thor: false,
            },
        );
        id
    }

    async fn tick_lifecycles(&mut self) {
        let mut replies = Vec::with_capacity(self.tabs.len());
        for (&tab_id, record) in &self.tabs {
            let (reply, response) = oneshot::channel();
            if record
                .command_tx
                .send(TabCommand::TickLifecycle { reply })
                .await
                .is_ok()
            {
                replies.push((tab_id, response));
            }
        }

        let mut expired = Vec::new();
        for (tab_id, response) in replies {
            if matches!(
                timeout(DEADLINE_LAUNCH_SHUTDOWN, response).await,
                Ok(Ok(Ok(TabLifecycle::Expired)))
            ) {
                expired.push(tab_id);
            }
        }

        for tab_id in expired {
            if let Some(record) = self.tabs.remove(&tab_id) {
                let identity_id = record.identity_id;
                if self.active_tab == Some(tab_id) {
                    self.active_tab = self.tabs.keys().next().copied();
                }
                self.dispose_unused_identity(&identity_id).await;
            }
        }
    }

    fn spawn_tab_worker(session: TabSession, identity_id: IdentityContextId) -> TabRecord {
        let session = Arc::new(session);
        let worker_session = Arc::clone(&session);
        let (command_tx, mut command_rx) = mpsc::channel(64);
        tokio::spawn(async move {
            if AppState::global()
                .profiles()
                .get_profile()
                .randomize_fingerprints
            {
                let _ = crate::engine::fingerprinting::FingerprintProtection::install(
                    worker_session.as_ref(),
                )
                .await;
            }
            while let Some(command) = command_rx.recv().await {
                match command {
                    TabCommand::Navigate { url, reply } => {
                        let result =
                            match timeout(DEADLINE_NAV, worker_session.navigate(&url)).await {
                                Ok(Ok(())) => Ok(()),
                                Ok(Err(_)) => Err(EngineError::CdpRejected),
                                Err(_) => Err(EngineError::Timeout),
                            };
                        let _ = reply.send(result);
                    }
                    TabCommand::Activate { reply } => {
                        let _ = reply.send(
                            worker_session
                                .bring_to_front_blocking()
                                .map_err(|_| EngineError::CdpRejected),
                        );
                    }
                    TabCommand::Back { reply } => {
                        let _ =
                            reply.send(match timeout(DEADLINE_NAV, worker_session.back()).await {
                                Ok(Ok(())) => Ok(()),
                                Ok(Err(_)) => Err(EngineError::CdpRejected),
                                Err(_) => Err(EngineError::Timeout),
                            });
                    }
                    TabCommand::Forward { reply } => {
                        let _ = reply.send(
                            match timeout(DEADLINE_NAV, worker_session.forward()).await {
                                Ok(Ok(())) => Ok(()),
                                Ok(Err(_)) => Err(EngineError::CdpRejected),
                                Err(_) => Err(EngineError::Timeout),
                            },
                        );
                    }
                    TabCommand::Reload { reply } => {
                        let _ = reply.send(
                            match timeout(DEADLINE_NAV, worker_session.reload()).await {
                                Ok(Ok(())) => Ok(()),
                                Ok(Err(_)) => Err(EngineError::CdpRejected),
                                Err(_) => Err(EngineError::Timeout),
                            },
                        );
                    }
                    TabCommand::Freeze { reply } => {
                        let _ = reply.send(
                            match timeout(DEADLINE_STATE, worker_session.freeze(false)).await {
                                Ok(Ok(())) => Ok(()),
                                Ok(Err(_)) => Err(EngineError::CdpRejected),
                                Err(_) => Err(EngineError::Timeout),
                            },
                        );
                    }
                    TabCommand::Discard { reply } => {
                        let _ = reply.send(
                            worker_session
                                .discard_blocking()
                                .map_err(|_| EngineError::CdpRejected),
                        );
                    }
                    TabCommand::Close { reply } => {
                        let result =
                            match timeout(DEADLINE_LAUNCH_SHUTDOWN, worker_session.close()).await {
                                Ok(Ok(())) => Ok(()),
                                Ok(Err(_)) => Err(EngineError::CdpRejected),
                                Err(_) => Err(EngineError::Timeout),
                            };
                        let _ = reply.send(result);
                        break;
                    }
                    TabCommand::Evaluate { script, reply } => {
                        let result = match timeout(
                            DEADLINE_STATE,
                            worker_session.evaluate_js(&script),
                        )
                        .await
                        {
                            Ok(Ok(value)) => Ok(value.to_string()),
                            Ok(Err(_)) => Err(EngineError::CdpRejected),
                            Err(_) => Err(EngineError::Timeout),
                        };
                        let _ = reply.send(result);
                    }
                    TabCommand::CaptureScreenshot { options, reply } => {
                        if options.format != "png" {
                            let _ = reply.send(Err(EngineError::CdpRejected));
                            continue;
                        }
                        let result =
                            match timeout(DEADLINE_SCREENSHOT, worker_session.capture_screenshot())
                                .await
                            {
                                Ok(Ok(bytes)) if !bytes.is_empty() => Ok(bytes),
                                Ok(_) => Err(EngineError::CdpRejected),
                                Err(_) => Err(EngineError::Timeout),
                            };
                        let _ = reply.send(result);
                    }
                    TabCommand::TogglePictureInPicture { reply } => {
                        let result =
                            crate::engine::browser_features::PictureInPicture::toggle_video(
                                worker_session.as_ref(),
                            )
                            .map_err(|_| EngineError::CdpRejected);
                        let _ = reply.send(result);
                    }
                    TabCommand::GetHistory { reply } => {
                        let _ = reply.send(worker_session.nav_history());
                    }
                    TabCommand::MergeHistory {
                        source_history,
                        reply,
                    } => {
                        worker_session.append_history(source_history);
                        let _ = reply.send(());
                    }
                    TabCommand::TickLifecycle { reply } => {
                        let current = worker_session.lifecycle();
                        if current == TabLifecycle::Active
                            && worker_session.is_visible().await.unwrap_or(false)
                        {
                            worker_session.touch();
                            let _ = reply.send(Ok(TabLifecycle::Active));
                            continue;
                        }

                        let next =
                            lifecycle_for_idle(current, worker_session.last_active().elapsed());
                        let result = match next {
                            TabLifecycle::Active if current == TabLifecycle::Active => {
                                Ok(TabLifecycle::Active)
                            }
                            TabLifecycle::Frozen if current != TabLifecycle::Frozen => {
                                match timeout(DEADLINE_STATE, worker_session.freeze(false)).await {
                                    Ok(Ok(())) => Ok(TabLifecycle::Frozen),
                                    Ok(Err(_)) => Err(EngineError::CdpRejected),
                                    Err(_) => Err(EngineError::Timeout),
                                }
                            }
                            TabLifecycle::Discarded if current != TabLifecycle::Discarded => {
                                worker_session
                                    .discard_blocking()
                                    .map(|()| TabLifecycle::Discarded)
                                    .map_err(|_| EngineError::CdpRejected)
                            }
                            TabLifecycle::Expired => {
                                match timeout(DEADLINE_LAUNCH_SHUTDOWN, worker_session.close())
                                    .await
                                {
                                    Ok(Ok(())) => Ok(TabLifecycle::Expired),
                                    Ok(Err(_)) => Err(EngineError::CdpRejected),
                                    Err(_) => Err(EngineError::Timeout),
                                }
                            }
                            stage => Ok(stage),
                        };
                        let should_stop = matches!(result, Ok(TabLifecycle::Expired));
                        let _ = reply.send(result);
                        if should_stop {
                            break;
                        }
                    }
                }
            }
        });
        TabRecord {
            command_tx,
            session,
            identity_id,
            last_user_interaction: Some(std::time::Instant::now()),
            generation: 1,
            restore: None,
        }
    }

    /// Keep Labs synchronized with tabs created or closed through Chromium's
    /// native browser chrome, not only tabs created by MCP.
    async fn adopt_existing_tabs(&mut self) {
        let Ok(sessions) = self.runtime.existing_tabs().await else {
            return;
        };
        let live_targets: std::collections::HashSet<String> =
            sessions.iter().filter_map(Self::target_key).collect();
        let closed_identities: std::collections::HashSet<IdentityContextId> = self
            .tabs
            .values()
            .filter(|record| {
                Self::target_key(record.session.as_ref())
                    .is_some_and(|target| !live_targets.contains(&target))
            })
            .map(|record| record.identity_id.clone())
            .collect();
        self.tabs.retain(|_, record| {
            Self::target_key(record.session.as_ref())
                .map(|target| live_targets.contains(&target))
                .unwrap_or_else(|| record.session.lifecycle() == TabLifecycle::Discarded)
        });
        for session in sessions {
            let target_key = Self::target_key(&session);
            if target_key.as_ref().is_some_and(|target| {
                self.tabs.values().any(|record| {
                    Self::target_key(record.session.as_ref()).as_ref() == Some(target)
                })
            }) {
                continue;
            }
            let id = session.id();
            let identity_id = self.identity_id_for_session(&session);
            self.tabs
                .insert(id, Self::spawn_tab_worker(session, identity_id));
            if self.active_tab.is_none() {
                self.active_tab = Some(id);
            }
        }
        for identity_id in closed_identities {
            self.dispose_unused_identity(&identity_id).await;
        }
        if self
            .active_tab
            .is_some_and(|active_tab| !self.tabs.contains_key(&active_tab))
        {
            self.active_tab = self.tabs.keys().next().copied();
        }
    }

    async fn dispose_unused_identity(&mut self, identity_id: &IdentityContextId) {
        if identity_id.0 == 0
            || self
                .tabs
                .values()
                .any(|tab| &tab.identity_id == identity_id)
        {
            return;
        }
        let context_id = self.identities.get(identity_id).and_then(|identity| {
            (identity.owned_by_thor && identity.retention == IdentityRetention::UntilLastTabCloses)
                .then(|| identity.browser_context_id.clone())
                .flatten()
        });
        if let Some(context_id) = context_id {
            if self
                .runtime
                .dispose_identity_context(&context_id)
                .await
                .is_ok()
            {
                self.identities.remove(identity_id);
            }
        }
    }

    /// Retry failed final-tab disposal on every coordinator tick. A transient
    /// CDP failure must not leave a Thor-owned ephemeral context alive for the
    /// rest of the browser session.
    async fn retry_unused_identity_cleanup(&mut self) {
        let unused: Vec<_> = self
            .identities
            .values()
            .filter(|identity| {
                identity.owned_by_thor
                    && identity.retention == IdentityRetention::UntilLastTabCloses
                    && !self.tabs.values().any(|tab| tab.identity_id == identity.id)
            })
            .map(|identity| identity.id.clone())
            .collect();
        for identity_id in unused {
            self.dispose_unused_identity(&identity_id).await;
        }
    }

    /// Labs owns disposable contexts it creates. If its coordinator detaches,
    /// dispose those contexts instead of abandoning privacy-sensitive storage.
    async fn dispose_owned_ephemeral_identities(&mut self) {
        let ephemeral: Vec<_> = self
            .identities
            .values()
            .filter(|identity| {
                identity.owned_by_thor
                    && identity.retention == IdentityRetention::UntilLastTabCloses
            })
            .filter_map(|identity| {
                identity
                    .browser_context_id
                    .clone()
                    .map(|context_id| (identity.id.clone(), context_id))
            })
            .collect();

        for (identity_id, context_id) in ephemeral {
            if matches!(
                timeout(
                    DEADLINE_LAUNCH_SHUTDOWN,
                    self.runtime.dispose_identity_context(&context_id)
                )
                .await,
                Ok(Ok(()))
            ) {
                self.tabs
                    .retain(|_, record| record.identity_id != identity_id);
                self.identities.remove(&identity_id);
            }
        }
        if self
            .active_tab
            .is_some_and(|active_tab| !self.tabs.contains_key(&active_tab))
        {
            self.active_tab = self.tabs.keys().next().copied();
        }
    }

    fn target_key(session: &TabSession) -> Option<String> {
        session.target_id().map(|target| target.inner().clone())
    }

    async fn create_identity_record(
        &mut self,
        requested_id: Option<IdentityContextId>,
        retention: IdentityRetention,
        requested_name: Option<String>,
    ) -> Result<IdentityContextId, EngineError> {
        let id = requested_id.unwrap_or_else(|| self.allocate_identity_id());
        self.next_identity_id = self.next_identity_id.max(id.0.saturating_add(1));
        if self.identities.contains_key(&id) {
            return Ok(id);
        }
        let context_id = match timeout(
            DEADLINE_LAUNCH_SHUTDOWN,
            self.runtime.create_identity_context(),
        )
        .await
        {
            Ok(Ok(context_id)) => context_id,
            Ok(Err(_)) => return Err(EngineError::CdpRejected),
            Err(_) => return Err(EngineError::Timeout),
        };
        self.identities.insert(
            id.clone(),
            IdentityRecord {
                id: id.clone(),
                name: normalize_identity_name(requested_name, id.0),
                browser_context_id: Some(context_id),
                retention,
                owned_by_thor: true,
            },
        );
        Ok(id)
    }

    async fn create_tab_in_identity(
        &mut self,
        identity_id: IdentityContextId,
        url: String,
    ) -> Result<TabId, EngineError> {
        let browser_context_id = if identity_id.0 == 0 {
            None
        } else {
            Some(
                self.identities
                    .get(&identity_id)
                    .and_then(|record| record.browser_context_id.clone())
                    .ok_or(EngineError::IdentityNotFound)?,
            )
        };

        // Start on about:blank so the tab worker can install its
        // document-start protection before the requested site loads.
        let session = match timeout(
            DEADLINE_LAUNCH_SHUTDOWN,
            self.runtime
                .new_tab_in_context(Some("about:blank"), browser_context_id.as_deref()),
        )
        .await
        {
            Ok(Ok(session)) => session,
            Ok(Err(_)) => {
                self.dispose_unused_identity(&identity_id).await;
                return Err(EngineError::RuntimeUnavailable);
            }
            Err(_) => {
                self.dispose_unused_identity(&identity_id).await;
                return Err(EngineError::Timeout);
            }
        };

        let id = session.id();
        let record = Self::spawn_tab_worker(session, identity_id.clone());
        let (navigate_tx, navigate_rx) = oneshot::channel();
        if record
            .command_tx
            .send(TabCommand::Navigate {
                url,
                reply: navigate_tx,
            })
            .await
            .is_err()
        {
            let _ = Self::close_record(&record).await;
            self.dispose_unused_identity(&identity_id).await;
            return Err(EngineError::RuntimeUnavailable);
        }

        self.tabs.insert(id, record);
        self.active_tab = Some(id);
        let result = match timeout(DEADLINE_NAV, navigate_rx).await {
            Ok(Ok(Ok(()))) => Ok(id),
            Ok(Ok(Err(err))) => Err(err),
            Ok(Err(_)) => Err(EngineError::RuntimeUnavailable),
            Err(_) => Err(EngineError::Timeout),
        };
        if result.is_err() {
            if let Some(record) = self.tabs.remove(&id) {
                let _ = Self::close_record(&record).await;
            }
            self.dispose_unused_identity(&identity_id).await;
            if self.active_tab == Some(id) {
                self.active_tab = self.tabs.keys().next().copied();
            }
        }
        result
    }

    async fn destroy_identity(
        &mut self,
        identity_id: &IdentityContextId,
    ) -> Result<(), EngineError> {
        if identity_id.0 == 0 {
            return Err(EngineError::DefaultIdentityProtected);
        }
        let identity = self
            .identities
            .get(identity_id)
            .cloned()
            .ok_or(EngineError::IdentityNotFound)?;
        if !identity.owned_by_thor {
            return Err(EngineError::IdentityNotOwned);
        }
        let context_id = identity
            .browser_context_id
            .ok_or(EngineError::IdentityNotFound)?;

        // Close every bound WebContents before disposing the BrowserContext.
        // Dropping only the sidecar records leaves native tabs alive and can
        // make Browser.disposeBrowserContext race storage/network teardown.
        let bound_tabs: Vec<TabId> = self
            .tabs
            .iter()
            .filter_map(|(&tab_id, record)| (&record.identity_id == identity_id).then_some(tab_id))
            .collect();
        for tab_id in bound_tabs {
            if let Some(record) = self.tabs.remove(&tab_id) {
                if Self::close_record(&record).await.is_err() {
                    self.tabs.insert(tab_id, record);
                    if self.active_tab.is_none() {
                        self.active_tab = Some(tab_id);
                    }
                    return Err(EngineError::CdpRejected);
                }
                if self.active_tab == Some(tab_id) {
                    self.active_tab = None;
                }
            }
        }

        match timeout(
            DEADLINE_LAUNCH_SHUTDOWN,
            self.runtime.dispose_identity_context(&context_id),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return Err(EngineError::CdpRejected),
            Err(_) => return Err(EngineError::Timeout),
        }

        self.identities.remove(identity_id);
        if self.active_tab.is_none() {
            self.active_tab = self.tabs.keys().next().copied();
        }
        Ok(())
    }

    async fn close_record(record: &TabRecord) -> Result<(), EngineError> {
        let (close_tx, close_rx) = oneshot::channel();
        if record
            .command_tx
            .send(TabCommand::Close { reply: close_tx })
            .await
            .is_err()
        {
            return Err(EngineError::RuntimeUnavailable);
        }
        match timeout(DEADLINE_LAUNCH_SHUTDOWN, close_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(EngineError::RuntimeUnavailable),
            Err(_) => Err(EngineError::Timeout),
        }
    }

    async fn send_tab(&self, tab_id: TabId, command: TabCommand) {
        if let Some(record) = self.tabs.get(&tab_id) {
            let _ = record.command_tx.send(command).await;
        } else {
            match command {
                TabCommand::Back { reply }
                | TabCommand::Forward { reply }
                | TabCommand::Reload { reply }
                | TabCommand::Freeze { reply }
                | TabCommand::Discard { reply } => {
                    let _ = reply.send(Err(EngineError::TabNotFound));
                }
                _ => unreachable!("only simple tab commands use send_tab"),
            }
        }
    }

    async fn handle_command(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::NewIdentity { name, reply } => {
                let result = self
                    .create_identity_record(None, IdentityRetention::UntilExplicitDestroy, name)
                    .await;
                let _ = reply.send(result);
            }
            EngineCommand::DestroyIdentity { identity_id, reply } => {
                let result = self.destroy_identity(&identity_id).await;
                let _ = reply.send(result);
            }
            EngineCommand::CreateDisposableTab { url, reply } => {
                let result = match self
                    .create_identity_record(None, IdentityRetention::UntilLastTabCloses, None)
                    .await
                {
                    Ok(identity_id) => self
                        .create_tab_in_identity(identity_id.clone(), url)
                        .await
                        .map(|tab_id| DisposableTab {
                            tab_id,
                            identity_id,
                        }),
                    Err(error) => Err(error),
                };
                let _ = reply.send(result);
            }
            EngineCommand::CreateTab {
                identity_id,
                url,
                reply,
            } => {
                let identity_id = if identity_id.0 == 0
                    && AppState::global()
                        .profiles()
                        .get_profile()
                        .privacy
                        .require_isolated_identity
                {
                    match self
                        .create_identity_record(None, IdentityRetention::UntilLastTabCloses, None)
                        .await
                    {
                        Ok(identity_id) => identity_id,
                        Err(error) => {
                            let _ = reply.send(Err(error));
                            return;
                        }
                    }
                } else {
                    identity_id
                };
                if identity_id.0 != 0 && !self.identities.contains_key(&identity_id) {
                    if let Err(error) = self
                        .create_identity_record(
                            Some(identity_id.clone()),
                            IdentityRetention::UntilLastTabCloses,
                            None,
                        )
                        .await
                    {
                        let _ = reply.send(Err(error));
                        return;
                    }
                }
                let result = self.create_tab_in_identity(identity_id, url).await;
                let _ = reply.send(result);
            }
            EngineCommand::CloseTab { tab_id, reply } => {
                if let Some(record) = self.tabs.remove(&tab_id) {
                    if self.active_tab == Some(tab_id) {
                        self.active_tab = self.tabs.keys().next().copied();
                    }
                    let identity_id = record.identity_id.clone();
                    let close_result = Self::close_record(&record).await;
                    self.dispose_unused_identity(&identity_id).await;
                    let _ = reply.send(close_result);
                } else {
                    let _ = reply.send(Err(EngineError::TabNotFound));
                }
            }
            EngineCommand::ActivateTab { tab_id, reply } => {
                if let Some(record) = self.tabs.get(&tab_id) {
                    self.active_tab = Some(tab_id);
                    let _ = record.command_tx.send(TabCommand::Activate { reply }).await;
                } else {
                    let _ = reply.send(Err(EngineError::TabNotFound));
                }
            }
            EngineCommand::Navigate { tab_id, url, reply } => {
                if let Some(record) = self.tabs.get(&tab_id) {
                    self.active_tab = Some(tab_id);
                    let _ = record
                        .command_tx
                        .send(TabCommand::Navigate { url, reply })
                        .await;
                } else {
                    let _ = reply.send(Err(EngineError::TabNotFound));
                }
            }
            EngineCommand::Back { tab_id, reply } => {
                self.send_tab(tab_id, TabCommand::Back { reply }).await
            }
            EngineCommand::Forward { tab_id, reply } => {
                self.send_tab(tab_id, TabCommand::Forward { reply }).await
            }
            EngineCommand::Reload { tab_id, reply } => {
                self.send_tab(tab_id, TabCommand::Reload { reply }).await
            }
            EngineCommand::Evaluate {
                tab_id,
                script,
                reply,
                ..
            } => {
                if let Some(record) = self.tabs.get(&tab_id) {
                    let _ = record
                        .command_tx
                        .send(TabCommand::Evaluate { script, reply })
                        .await;
                } else {
                    let _ = reply.send(Err(EngineError::TabNotFound));
                }
            }
            EngineCommand::CaptureScreenshot {
                tab_id,
                options,
                reply,
            } => {
                if let Some(record) = self.tabs.get(&tab_id) {
                    let _ = record
                        .command_tx
                        .send(TabCommand::CaptureScreenshot { options, reply })
                        .await;
                } else {
                    let _ = reply.send(Err(EngineError::TabNotFound));
                }
            }
            EngineCommand::TogglePictureInPicture { tab_id, reply } => {
                if let Some(record) = self.tabs.get(&tab_id) {
                    let _ = record
                        .command_tx
                        .send(TabCommand::TogglePictureInPicture { reply })
                        .await;
                } else {
                    let _ = reply.send(Err(EngineError::TabNotFound));
                }
            }
            EngineCommand::Freeze { tab_id, reply } => {
                self.send_tab(tab_id, TabCommand::Freeze { reply }).await
            }
            EngineCommand::Discard { tab_id, reply } => {
                self.send_tab(tab_id, TabCommand::Discard { reply }).await
            }
            EngineCommand::MergeTabs {
                source,
                destination,
                reply,
            } => {
                let source_tx = self
                    .tabs
                    .get(&source)
                    .map(|record| record.command_tx.clone());
                let destination_tx = self
                    .tabs
                    .get(&destination)
                    .map(|record| record.command_tx.clone());
                if let (Some(source_tx), Some(destination_tx)) = (source_tx, destination_tx) {
                    let (history_tx, history_rx) = oneshot::channel();
                    if source_tx
                        .send(TabCommand::GetHistory { reply: history_tx })
                        .await
                        .is_ok()
                    {
                        if let Ok(history) = history_rx.await {
                            let (merge_tx, merge_rx) = oneshot::channel();
                            if destination_tx
                                .send(TabCommand::MergeHistory {
                                    source_history: history,
                                    reply: merge_tx,
                                })
                                .await
                                .is_ok()
                            {
                                let _ = merge_rx.await;
                                let (close_tx, close_rx) = oneshot::channel();
                                let _ = source_tx.send(TabCommand::Close { reply: close_tx }).await;
                                let _ = close_rx.await;
                                if let Some(record) = self.tabs.remove(&source) {
                                    self.dispose_unused_identity(&record.identity_id).await;
                                }
                                if self.active_tab == Some(source) {
                                    self.active_tab = Some(destination);
                                }
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
                    Ok(Ok(())) => {
                        let _ = reply.send(Ok(()));
                    }
                    Ok(Err(_)) => {
                        let _ = reply.send(Err(EngineError::CdpRejected));
                    }
                    Err(_) => {
                        let _ = reply.send(Err(EngineError::Timeout));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::chromium_runtime::ChromiumRuntimeConfig;

    #[test]
    fn identity_names_are_bounded_and_have_stable_fallbacks() {
        assert_eq!(normalize_identity_name(None, 7), "Identity 7");
        assert_eq!(normalize_identity_name(Some("  ".into()), 7), "Identity 7");
        assert_eq!(normalize_identity_name(Some("  Work  ".into()), 7), "Work");
        assert_eq!(normalize_identity_name(Some("x".repeat(100)), 7).len(), 80);
    }

    #[tokio::test]
    #[ignore = "requires a live Thor-owned Chromium build"]
    async fn coordinator_create_close_tab() {
        let runtime = ChromiumRuntime::launch(ChromiumRuntimeConfig {
            headful: false,
            ..Default::default()
        })
        .await
        .unwrap();
        let (engine, handle) = BrowserEngine::new(runtime);
        tokio::spawn(engine.run());

        let (create_tx, create_rx) = oneshot::channel();
        handle
            .cmd_tx
            .send(EngineCommand::CreateTab {
                identity_id: IdentityContextId(0),
                url: "about:blank".into(),
                reply: create_tx,
            })
            .await
            .unwrap();
        let id = create_rx.await.unwrap().unwrap();

        let (close_tx, close_rx) = oneshot::channel();
        handle
            .cmd_tx
            .send(EngineCommand::CloseTab {
                tab_id: id,
                reply: close_tx,
            })
            .await
            .unwrap();
        close_rx.await.unwrap().unwrap();
    }

    #[tokio::test]
    #[ignore = "requires a live Thor-owned Chromium build"]
    async fn disposable_tab_owns_and_releases_identity() {
        let runtime = ChromiumRuntime::launch(ChromiumRuntimeConfig {
            headful: false,
            ..Default::default()
        })
        .await
        .unwrap();
        let (engine, mut handle) = BrowserEngine::new(runtime);
        tokio::spawn(engine.run());

        let disposable = handle
            .create_disposable_tab("about:blank".into())
            .await
            .unwrap();
        assert_ne!(disposable.identity_id, IdentityContextId(0));

        let (close_tx, close_rx) = oneshot::channel();
        handle
            .cmd_tx
            .send(EngineCommand::CloseTab {
                tab_id: disposable.tab_id,
                reply: close_tx,
            })
            .await
            .unwrap();
        close_rx.await.unwrap().unwrap();
        handle.snapshot_rx.changed().await.unwrap();
        assert!(!handle
            .snapshot_rx
            .borrow()
            .identities
            .contains(&disposable.identity_id));
    }
}
