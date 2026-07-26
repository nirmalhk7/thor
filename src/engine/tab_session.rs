//! One CDP page/target per Thor tab.
//! Supports 3-Level Isolation Hierarchy (Profile -> Identity Context -> Tab)
//! and 4-Stage Tab Lifecycle (Active -> Frozen -> Discarded -> Expired).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use chromiumoxide::cdp::browser_protocol::network::ClearBrowserCookiesParams;
use chromiumoxide::cdp::browser_protocol::page::{
    BringToFrontParams, GetNavigationHistoryParams, NavigateToHistoryEntryParams,
    StopLoadingParams,
};
use chromiumoxide::cdp::browser_protocol::storage::ClearDataForOriginParams;
use chromiumoxide::cdp::browser_protocol::target::{
    ActivateTargetParams, OpenDevToolsParams, TargetId,
};
use chromiumoxide::Page;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::chromium_runtime::{block_on_handle, ChromiumError, ChromiumResult, ChromiumRuntime};
use super::freezing::{CdpPageFreezer, ProcessFreezer};

/// Opaque tab identifier assigned by [`ChromiumRuntime`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId(pub u64);

/// 4-Stage Tab Lifecycle:
/// 1. Active: foreground or recently active, full CDP execution.
/// 2. Frozen: background tab after 15m idle, stopped/paused via CDP.
/// 3. Discarded: background tab after 1h idle, CDP target detached & memory released while retaining session state.
/// 4. Expired: background tab after 24h idle, tab closed and history wiped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabLifecycle {
    Active,
    Frozen,
    Discarded,
    Expired,
}

/// Level 2 Isolation: Identity Context
/// Holds cookies, localStorage, IndexedDB settings, Service Worker settings, and Auth tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityContext {
    pub id: String,
    pub profile_id: String,
    pub is_ephemeral: bool,
    pub cookies: HashMap<String, String>,
    pub local_storage: HashMap<String, String>,
    pub indexed_db_enabled: bool,
    pub service_workers_enabled: bool,
    pub auth_tokens: HashMap<String, String>,
}

impl IdentityContext {
    pub fn new(id: &str, profile_id: &str, is_ephemeral: bool) -> Self {
        Self {
            id: id.to_string(),
            profile_id: profile_id.to_string(),
            is_ephemeral,
            cookies: HashMap::new(),
            local_storage: HashMap::new(),
            indexed_db_enabled: true,
            service_workers_enabled: true,
            auth_tokens: HashMap::new(),
        }
    }

    pub fn ephemeral(profile_id: &str) -> Self {
        static NEXT_EPHEMERAL_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let counter = NEXT_EPHEMERAL_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self::new(&format!("ephemeral-{nanos}-{counter}"), profile_id, true)
    }

    pub fn set_cookie(&mut self, key: &str, val: &str) {
        self.cookies.insert(key.to_string(), val.to_string());
    }

    pub fn get_cookie(&self, key: &str) -> Option<&String> {
        self.cookies.get(key)
    }

    pub fn set_local_storage(&mut self, key: &str, val: &str) {
        self.local_storage.insert(key.to_string(), val.to_string());
    }

    pub fn get_local_storage(&self, key: &str) -> Option<&String> {
        self.local_storage.get(key)
    }

    pub fn set_auth_token(&mut self, realm: &str, token: &str) {
        self.auth_tokens.insert(realm.to_string(), token.to_string());
    }

    pub fn get_auth_token(&self, realm: &str) -> Option<&String> {
        self.auth_tokens.get(realm)
    }
}

/// CDP-backed tab session: navigate, history, JS eval, freeze/discard lifecycle.
pub struct TabSession {
    id: TabId,
    page: Arc<Mutex<Option<Page>>>,
    runtime: Arc<ChromiumRuntime>,
    frozen: AtomicBool,
    lifecycle: Arc<RwLock<TabLifecycle>>,
    cached_title: Arc<Mutex<Option<String>>>,
    // Level 2 Isolation: Identity Context
    identity_context: Arc<Mutex<IdentityContext>>,
    // Level 3 Isolation: Tab-scoped state
    nav_history: Arc<Mutex<Vec<String>>>,
    scroll_position: Arc<Mutex<(f64, f64)>>,
    trace_buffer: Arc<Mutex<Vec<String>>>,
    last_active: Arc<Mutex<Instant>>,
}

impl TabSession {
    pub(crate) fn from_page(id: TabId, page: Page, runtime: Arc<ChromiumRuntime>) -> Self {
        let identity_context = IdentityContext::new(&format!("ctx_{}", id.0), "default_profile", false);
        Self::from_page_with_context(id, page, runtime, identity_context)
    }

    pub(crate) fn from_page_with_context(
        id: TabId,
        page: Page,
        runtime: Arc<ChromiumRuntime>,
        identity_context: IdentityContext,
    ) -> Self {
        Self {
            id,
            page: Arc::new(Mutex::new(Some(page))),
            runtime,
            frozen: AtomicBool::new(false),
            lifecycle: Arc::new(RwLock::new(TabLifecycle::Active)),
            cached_title: Arc::new(Mutex::new(None)),
            identity_context: Arc::new(Mutex::new(identity_context)),
            nav_history: Arc::new(Mutex::new(Vec::new())),
            scroll_position: Arc::new(Mutex::new((0.0, 0.0))),
            trace_buffer: Arc::new(Mutex::new(Vec::new())),
            last_active: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub fn id(&self) -> TabId {
        self.id
    }

    pub fn target_id(&self) -> Option<TargetId> {
        self.page.lock().as_ref().map(|p| p.target_id().clone())
    }

    pub fn page(&self) -> Option<Page> {
        self.page.lock().clone()
    }

    pub fn runtime(&self) -> &Arc<ChromiumRuntime> {
        &self.runtime
    }

    pub fn lifecycle(&self) -> TabLifecycle {
        *self.lifecycle.read()
    }

    pub fn set_lifecycle(&self, stage: TabLifecycle) {
        *self.lifecycle.write() = stage;
        if stage == TabLifecycle::Frozen {
            self.frozen.store(true, Ordering::Relaxed);
        } else if stage == TabLifecycle::Active {
            self.frozen.store(false, Ordering::Relaxed);
        }
    }

    pub fn identity_context(&self) -> IdentityContext {
        self.identity_context.lock().clone()
    }

    pub fn update_identity_context<F: FnOnce(&mut IdentityContext)>(&self, f: F) {
        f(&mut self.identity_context.lock());
    }

    pub fn is_ephemeral_context(&self) -> bool {
        self.identity_context.lock().is_ephemeral
    }

    pub fn nav_history(&self) -> Vec<String> {
        self.nav_history.lock().clone()
    }

    pub fn push_nav_history(&self, url: &str) {
        let mut hist = self.nav_history.lock();
        if hist.last().map(|s| s.as_str()) != Some(url) {
            hist.push(url.to_string());
        }
    }

    pub fn scroll_position(&self) -> (f64, f64) {
        *self.scroll_position.lock()
    }

    pub fn set_scroll_position(&self, x: f64, y: f64) {
        *self.scroll_position.lock() = (x, y);
    }

    pub fn trace_buffer(&self) -> Vec<String> {
        self.trace_buffer.lock().clone()
    }

    pub fn push_trace(&self, entry: &str) {
        self.trace_buffer.lock().push(entry.to_string());
    }

    pub fn last_active(&self) -> Instant {
        *self.last_active.lock()
    }

    pub fn touch(&self) {
        *self.last_active.lock() = Instant::now();
    }

    /// Discard the tab: detach CDP page target & release memory while retaining tab metadata.
    pub fn discard_blocking(&self) -> ChromiumResult<()> {
        let stage = self.lifecycle();
        if stage == TabLifecycle::Discarded || stage == TabLifecycle::Expired {
            return Ok(());
        }

        let page_opt = self.page.lock().take();
        if let Some(page) = page_opt {
            let handle = self.runtime.handle().clone();
            let _ = block_on_handle(&handle, async move {
                page.close().await?;
                Ok::<(), ChromiumError>(())
            });
        }

        *self.lifecycle.write() = TabLifecycle::Discarded;
        self.frozen.store(true, Ordering::Relaxed);
        self.push_trace("Tab discarded (CDP target detached & memory released)");
        println!("⚡ [Thor TabSession] Tab {} discarded (memory released)", self.id.0);
        Ok(())
    }

    /// Restore a discarded tab back to Active state by launching a new CDP page target.
    pub fn restore_blocking(&self) -> ChromiumResult<()> {
        if self.lifecycle() != TabLifecycle::Discarded {
            return Ok(());
        }

        let last_url = self
            .nav_history
            .lock()
            .last()
            .cloned()
            .unwrap_or_else(|| "https://www.google.com".to_string());

        let restored_session = self.runtime.new_tab_blocking(Some(&last_url))?;
        if let Some(new_page) = restored_session.page() {
            *self.page.lock() = Some(new_page);
        }
        *self.lifecycle.write() = TabLifecycle::Active;
        self.frozen.store(false, Ordering::Relaxed);
        self.touch();
        self.push_trace("Tab restored from discarded state");
        println!("⚡ [Thor TabSession] Tab {} restored to Active at {last_url}", self.id.0);
        Ok(())
    }

    /// Ensure active page target is restored if discarded.
    fn ensure_active_page(&self) -> ChromiumResult<Page> {
        if self.lifecycle() == TabLifecycle::Discarded {
            self.restore_blocking()?;
        }
        self.page
            .lock()
            .clone()
            .ok_or_else(|| ChromiumError::Other(anyhow::anyhow!("No active CDP page target for tab")))
    }

    /// Navigate to `url` and wait for the load to settle when possible.
    pub async fn navigate(&self, url: &str) -> ChromiumResult<()> {
        let page = self.ensure_active_page()?;
        self.touch();
        self.push_nav_history(url);
        self.push_trace(&format!("Navigate: {url}"));
        page.goto(url).await?;
        let _ = page.wait_for_navigation().await;
        self.refresh_title_cache().await;
        Ok(())
    }

    pub fn navigate_blocking(&self, url: &str) -> ChromiumResult<()> {
        let page = self.ensure_active_page()?;
        self.touch();
        self.push_nav_history(url);
        self.push_trace(&format!("Navigate: {url}"));
        let url = url.to_owned();
        let title_slot = Arc::clone(&self.cached_title);
        block_on_handle(self.runtime.handle(), async move {
            page.goto(&url).await?;
            let _ = page.wait_for_navigation().await;
            if let Ok(t) = page.evaluate("document.title").await {
                if let Ok(s) = t.into_value::<String>() {
                    *title_slot.lock() = Some(s);
                }
            }
            Ok(())
        })
    }

    /// Go back in history when possible.
    pub async fn back(&self) -> ChromiumResult<()> {
        self.navigate_history(-1).await
    }

    pub fn back_blocking(&self) -> ChromiumResult<()> {
        let page = self.ensure_active_page()?;
        self.touch();
        block_on_handle(self.runtime.handle(), async move { history_step(&page, -1).await })
    }

    /// Go forward in history when possible.
    pub async fn forward(&self) -> ChromiumResult<()> {
        self.navigate_history(1).await
    }

    pub fn forward_blocking(&self) -> ChromiumResult<()> {
        let page = self.ensure_active_page()?;
        self.touch();
        block_on_handle(self.runtime.handle(), async move { history_step(&page, 1).await })
    }

    pub async fn reload(&self) -> ChromiumResult<()> {
        let page = self.ensure_active_page()?;
        self.touch();
        page.reload().await?;
        let _ = page.wait_for_navigation().await;
        self.refresh_title_cache().await;
        Ok(())
    }

    pub fn reload_blocking(&self) -> ChromiumResult<()> {
        let page = self.ensure_active_page()?;
        self.touch();
        let title_slot = Arc::clone(&self.cached_title);
        block_on_handle(self.runtime.handle(), async move {
            page.reload().await?;
            let _ = page.wait_for_navigation().await;
            if let Ok(t) = page.evaluate("document.title").await {
                if let Ok(s) = t.into_value::<String>() {
                    *title_slot.lock() = Some(s);
                }
            }
            Ok(())
        })
    }

    /// Stop in-flight navigations / loads (`Page.stopLoading`).
    pub async fn stop(&self) -> ChromiumResult<()> {
        let page = self.ensure_active_page()?;
        page.execute(StopLoadingParams::default()).await?;
        Ok(())
    }

    pub fn stop_blocking(&self) -> ChromiumResult<()> {
        let page = self.ensure_active_page()?;
        block_on_handle(self.runtime.handle(), async move {
            page.execute(StopLoadingParams::default()).await?;
            Ok(())
        })
    }

    /// Evaluate a JS expression and return the JSON value.
    pub async fn evaluate_js(&self, expression: &str) -> ChromiumResult<Value> {
        let page = self.ensure_active_page()?;
        let result = page.evaluate(expression).await?;
        let value = result
            .into_value::<Value>()
            .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))?;
        Ok(value)
    }

    pub fn evaluate_js_blocking(&self, expression: &str) -> ChromiumResult<Value> {
        let page = self.ensure_active_page()?;
        let expression = expression.to_owned();
        block_on_handle(self.runtime.handle(), async move {
            let result = page.evaluate(expression).await?;
            result
                .into_value::<Value>()
                .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))
        })
    }

    pub async fn title(&self) -> ChromiumResult<String> {
        let page = self.ensure_active_page()?;
        let result = page.evaluate("document.title").await?;
        let title = result.into_value::<String>().unwrap_or_default();
        *self.cached_title.lock() = Some(title.clone());
        Ok(title)
    }

    pub fn title_blocking(&self) -> ChromiumResult<String> {
        let page = self.ensure_active_page()?;
        let title_slot = Arc::clone(&self.cached_title);
        block_on_handle(self.runtime.handle(), async move {
            let result = page.evaluate("document.title").await?;
            let title = result.into_value::<String>().unwrap_or_default();
            *title_slot.lock() = Some(title.clone());
            Ok(title)
        })
    }

    /// Cached title from the last navigate/reload/title call (no CDP round-trip).
    pub fn cached_title(&self) -> Option<String> {
        self.cached_title.lock().clone()
    }

    pub async fn url(&self) -> ChromiumResult<Option<String>> {
        if let Ok(page) = self.ensure_active_page() {
            Ok(page.url().await?)
        } else {
            Ok(self.nav_history.lock().last().cloned())
        }
    }

    pub fn url_blocking(&self) -> ChromiumResult<Option<String>> {
        if self.lifecycle() == TabLifecycle::Discarded {
            return Ok(self.nav_history.lock().last().cloned());
        }
        let page = self.ensure_active_page()?;
        block_on_handle(self.runtime.handle(), async move { Ok(page.url().await?) })
    }

    /// Activate this tab in the Chromium window (bring to front).
    pub fn bring_to_front_blocking(&self) -> ChromiumResult<()> {
        let page = self.ensure_active_page()?;
        self.touch();
        *self.lifecycle.write() = TabLifecycle::Active;
        self.frozen.store(false, Ordering::Relaxed);
        let target_id = page.target_id().clone();
        let browser = self.runtime.browser();
        block_on_handle(self.runtime.handle(), async move {
            {
                let browser = browser.lock().await;
                let _ = browser.execute(ActivateTargetParams::new(target_id)).await;
            }
            page.execute(BringToFrontParams::default()).await?;
            Ok(())
        })
    }

    /// Open Chromium DevTools for this target.
    pub fn open_devtools_blocking(&self) -> ChromiumResult<()> {
        let page = self.ensure_active_page()?;
        let target_id = page.target_id().clone();
        let browser = self.runtime.browser();
        block_on_handle(self.runtime.handle(), async move {
            let browser = browser.lock().await;
            browser
                .execute(OpenDevToolsParams::new(target_id))
                .await?;
            Ok(())
        })
    }

    /// Soft-freeze background tab via CDP (never SIGSTOP — one shared browser process).
    pub fn soft_freeze_blocking(&self) -> ChromiumResult<()> {
        self.freeze_blocking(false)
    }

    pub fn soft_unfreeze_blocking(&self) -> ChromiumResult<()> {
        self.unfreeze_blocking(false)
    }

    /// Freeze the tab via CDP page lifecycle when available; optionally SIGSTOP the browser pid.
    pub async fn freeze(&self, also_os_suspend: bool) -> ChromiumResult<()> {
        let page_opt = self.page.lock().clone();
        if let Some(page) = page_opt {
            let _ = CdpPageFreezer::freeze_page(&page).await;
        }
        if also_os_suspend {
            if let Some(pid) = self.runtime.child_pid() {
                ProcessFreezer::freeze_process(pid)
                    .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))?;
            }
        }
        self.frozen.store(true, Ordering::Relaxed);
        *self.lifecycle.write() = TabLifecycle::Frozen;
        Ok(())
    }

    pub fn freeze_blocking(&self, also_os_suspend: bool) -> ChromiumResult<()> {
        let page_opt = self.page.lock().clone();
        let pid = self.runtime.child_pid();
        let result: ChromiumResult<()> = block_on_handle(self.runtime.handle(), async move {
            if let Some(page) = page_opt {
                let _ = CdpPageFreezer::freeze_page(&page).await;
            }
            if also_os_suspend {
                if let Some(pid) = pid {
                    ProcessFreezer::freeze_process(pid)
                        .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))?;
                }
            }
            Ok(())
        });
        if result.is_ok() {
            self.frozen.store(true, Ordering::Relaxed);
            *self.lifecycle.write() = TabLifecycle::Frozen;
        }
        result
    }

    pub async fn unfreeze(&self, also_os_resume: bool) -> ChromiumResult<()> {
        if also_os_resume {
            if let Some(pid) = self.runtime.child_pid() {
                ProcessFreezer::unfreeze_process(pid)
                    .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))?;
            }
        }
        let page_opt = self.page.lock().clone();
        if let Some(page) = page_opt {
            let _ = CdpPageFreezer::unfreeze_page(&page).await;
        }
        self.frozen.store(false, Ordering::Relaxed);
        *self.lifecycle.write() = TabLifecycle::Active;
        Ok(())
    }

    pub fn unfreeze_blocking(&self, also_os_resume: bool) -> ChromiumResult<()> {
        let page_opt = self.page.lock().clone();
        let pid = self.runtime.child_pid();
        let result: ChromiumResult<()> = block_on_handle(self.runtime.handle(), async move {
            if also_os_resume {
                if let Some(pid) = pid {
                    ProcessFreezer::unfreeze_process(pid)
                        .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))?;
                }
            }
            if let Some(page) = page_opt {
                let _ = CdpPageFreezer::unfreeze_page(&page).await;
            }
            Ok(())
        });
        if result.is_ok() {
            self.frozen.store(false, Ordering::Relaxed);
            *self.lifecycle.write() = TabLifecycle::Active;
        }
        result
    }

    /// Close the CDP target and expire session.
    pub async fn close(self) -> ChromiumResult<()> {
        *self.lifecycle.write() = TabLifecycle::Expired;
        let is_ephemeral = self.is_ephemeral_context();
        if is_ephemeral {
            self.identity_context.lock().cookies.clear();
            self.identity_context.lock().local_storage.clear();
            self.identity_context.lock().auth_tokens.clear();
        }
        let page_opt = self.page.lock().take();
        if let Some(page) = page_opt {
            if is_ephemeral {
                if let Ok(Some(url)) = page.url().await {
                    if !url.is_empty() {
                        let origin = url.split('/').take(3).collect::<Vec<&str>>().join("/");
                        if !origin.is_empty() {
                            let _ = page
                                .execute(ClearDataForOriginParams::new(origin, "all".to_string()))
                                .await;
                        }
                    }
                }
            }
            page.close().await?;
        }
        Ok(())
    }

    pub fn close_blocking(self) -> ChromiumResult<()> {
        *self.lifecycle.write() = TabLifecycle::Expired;
        let is_ephemeral = self.is_ephemeral_context();
        if is_ephemeral {
            self.identity_context.lock().cookies.clear();
            self.identity_context.lock().local_storage.clear();
            self.identity_context.lock().auth_tokens.clear();
        }
        let page_opt = self.page.lock().take();
        if let Some(page) = page_opt {
            let handle = self.runtime.handle().clone();
            block_on_handle(&handle, async move {
                if is_ephemeral {
                    if let Ok(Some(url)) = page.url().await {
                        if !url.is_empty() {
                            let origin = url.split('/').take(3).collect::<Vec<&str>>().join("/");
                            if !origin.is_empty() {
                                let _ = page
                                    .execute(ClearDataForOriginParams::new(origin, "all".to_string()))
                                    .await;
                            }
                        }
                    }
                }
                page.close().await?;
                Ok(())
            })
        } else {
            Ok(())
        }
    }

    async fn navigate_history(&self, delta: i64) -> ChromiumResult<()> {
        let page = self.ensure_active_page()?;
        history_step(&page, delta).await?;
        self.refresh_title_cache().await;
        Ok(())
    }

    async fn refresh_title_cache(&self) {
        let page_opt = self.page.lock().clone();
        if let Some(page) = page_opt {
            if let Ok(result) = page.evaluate("document.title").await {
                if let Ok(title) = result.into_value::<String>() {
                    *self.cached_title.lock() = Some(title);
                }
            }
        }
    }
}

async fn history_step(page: &Page, delta: i64) -> ChromiumResult<()> {
    let hist = page
        .execute(GetNavigationHistoryParams::default())
        .await?
        .result;
    let current = hist.current_index;
    let target = current + delta;
    if target < 0 || target as usize >= hist.entries.len() {
        return Ok(());
    }
    let entry_id = hist.entries[target as usize].id;
    page.execute(NavigateToHistoryEntryParams::new(entry_id))
        .await?;
    let _ = page.wait_for_navigation().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_id_construction() {
        let id = TabId(42);
        assert_eq!(id.0, 42);
        assert_eq!(format!("{:?}", id), "TabId(42)");
    }

    #[test]
    fn test_four_stage_lifecycle_enum() {
        assert_ne!(TabLifecycle::Active, TabLifecycle::Frozen);
        assert_ne!(TabLifecycle::Frozen, TabLifecycle::Discarded);
        assert_ne!(TabLifecycle::Discarded, TabLifecycle::Expired);
    }

    #[test]
    fn test_identity_context_ephemeral() {
        let ctx = IdentityContext::ephemeral("dev_host");
        assert!(ctx.is_ephemeral);
        assert_eq!(ctx.profile_id, "dev_host");
        assert!(ctx.id.starts_with("ephemeral-"));
    }

    #[test]
    fn test_identity_context_storage_and_tokens() {
        let mut ctx = IdentityContext::new("ctx_1", "dev_host", false);
        ctx.set_cookie("session_id", "secret_123");
        ctx.set_local_storage("theme", "dark");
        ctx.set_auth_token("api.dev", "Bearer token_xyz");

        assert_eq!(ctx.get_cookie("session_id").map(|s| s.as_str()), Some("secret_123"));
        assert_eq!(ctx.get_local_storage("theme").map(|s| s.as_str()), Some("dark"));
        assert_eq!(ctx.get_auth_token("api.dev").map(|s| s.as_str()), Some("Bearer token_xyz"));
    }

    #[test]
    fn test_ephemeral_identity_context_collision() {
        use std::thread;
        use std::collections::HashSet;
        use std::sync::{Arc, Mutex};
        
        let thread_count = 10;
        let iters = 1000;
        let mut handles = vec![];
        let results = Arc::new(Mutex::new(Vec::new()));
        
        for _ in 0..thread_count {
            let results_clone = Arc::clone(&results);
            handles.push(thread::spawn(move || {
                let mut local_ids = Vec::new();
                for _ in 0..iters {
                    let ctx = IdentityContext::ephemeral("dev_host");
                    local_ids.push(ctx.id);
                }
                results_clone.lock().unwrap().extend(local_ids);
            }));
        }
        
        for h in handles {
            h.join().unwrap();
        }
        
        let all_ids = results.lock().unwrap();
        let mut unique_ids = HashSet::new();
        let mut collisions = 0;
        for id in all_ids.iter() {
            if !unique_ids.insert(id.clone()) {
                collisions += 1;
            }
        }
        
        // Assert no collisions
        assert_eq!(collisions, 0, "Found {} collisions in ephemeral ID generation!", collisions);
    }
}
