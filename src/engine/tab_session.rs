//! One CDP page/target per Thor tab.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chromiumoxide::cdp::browser_protocol::page::{
    BringToFrontParams, GetNavigationHistoryParams, NavigateToHistoryEntryParams,
    StopLoadingParams,
};
use chromiumoxide::cdp::browser_protocol::target::{
    ActivateTargetParams, OpenDevToolsParams, TargetId,
};
use chromiumoxide::Page;
use parking_lot::Mutex;
use serde_json::Value;

use super::chromium_runtime::{block_on_handle, ChromiumError, ChromiumResult, ChromiumRuntime};
use super::freezing::{CdpPageFreezer, ProcessFreezer};

/// Opaque tab identifier assigned by [`ChromiumRuntime`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

/// Lifecycle / freeze state for a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabLifecycle {
    Active,
    Frozen,
}

/// CDP-backed tab session: navigate, history, JS eval, optional freeze.
pub struct TabSession {
    id: TabId,
    page: Page,
    runtime: Arc<ChromiumRuntime>,
    frozen: AtomicBool,
    /// Last known title cache (updated on demand).
    cached_title: Arc<Mutex<Option<String>>>,
}

impl TabSession {
    pub(crate) fn from_page(id: TabId, page: Page, runtime: Arc<ChromiumRuntime>) -> Self {
        Self {
            id,
            page,
            runtime,
            frozen: AtomicBool::new(false),
            cached_title: Arc::new(Mutex::new(None)),
        }
    }

    pub fn id(&self) -> TabId {
        self.id
    }

    pub fn target_id(&self) -> &TargetId {
        self.page.target_id()
    }

    pub fn page(&self) -> &Page {
        &self.page
    }

    pub fn runtime(&self) -> &Arc<ChromiumRuntime> {
        &self.runtime
    }

    pub fn lifecycle(&self) -> TabLifecycle {
        if self.frozen.load(Ordering::Relaxed) {
            TabLifecycle::Frozen
        } else {
            TabLifecycle::Active
        }
    }

    /// Navigate to `url` and wait for the load to settle when possible.
    pub async fn navigate(&self, url: &str) -> ChromiumResult<()> {
        self.page.goto(url).await?;
        let _ = self.page.wait_for_navigation().await;
        self.refresh_title_cache().await;
        Ok(())
    }

    pub fn navigate_blocking(&self, url: &str) -> ChromiumResult<()> {
        let url = url.to_owned();
        let page = self.page.clone();
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
        let page = self.page.clone();
        block_on_handle(self.runtime.handle(), async move { history_step(&page, -1).await })
    }

    /// Go forward in history when possible.
    pub async fn forward(&self) -> ChromiumResult<()> {
        self.navigate_history(1).await
    }

    pub fn forward_blocking(&self) -> ChromiumResult<()> {
        let page = self.page.clone();
        block_on_handle(self.runtime.handle(), async move { history_step(&page, 1).await })
    }

    pub async fn reload(&self) -> ChromiumResult<()> {
        self.page.reload().await?;
        let _ = self.page.wait_for_navigation().await;
        self.refresh_title_cache().await;
        Ok(())
    }

    pub fn reload_blocking(&self) -> ChromiumResult<()> {
        let page = self.page.clone();
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
        self.page.execute(StopLoadingParams::default()).await?;
        Ok(())
    }

    pub fn stop_blocking(&self) -> ChromiumResult<()> {
        let page = self.page.clone();
        block_on_handle(self.runtime.handle(), async move {
            page.execute(StopLoadingParams::default()).await?;
            Ok(())
        })
    }

    /// Evaluate a JS expression and return the JSON value.
    pub async fn evaluate_js(&self, expression: &str) -> ChromiumResult<Value> {
        let result = self.page.evaluate(expression).await?;
        let value = result
            .into_value::<Value>()
            .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))?;
        Ok(value)
    }

    pub fn evaluate_js_blocking(&self, expression: &str) -> ChromiumResult<Value> {
        let expression = expression.to_owned();
        let page = self.page.clone();
        block_on_handle(self.runtime.handle(), async move {
            let result = page.evaluate(expression).await?;
            result
                .into_value::<Value>()
                .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))
        })
    }

    pub async fn title(&self) -> ChromiumResult<String> {
        let result = self.page.evaluate("document.title").await?;
        let title = result.into_value::<String>().unwrap_or_default();
        *self.cached_title.lock() = Some(title.clone());
        Ok(title)
    }

    pub fn title_blocking(&self) -> ChromiumResult<String> {
        let page = self.page.clone();
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
        Ok(self.page.url().await?)
    }

    pub fn url_blocking(&self) -> ChromiumResult<Option<String>> {
        let page = self.page.clone();
        block_on_handle(self.runtime.handle(), async move { Ok(page.url().await?) })
    }

    /// Activate this tab in the Chromium window (bring to front).
    pub fn bring_to_front_blocking(&self) -> ChromiumResult<()> {
        let page = self.page.clone();
        let target_id = self.page.target_id().clone();
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
        let target_id = self.page.target_id().clone();
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
        let _ = CdpPageFreezer::freeze_page(&self.page).await;
        if also_os_suspend {
            if let Some(pid) = self.runtime.child_pid() {
                ProcessFreezer::freeze_process(pid)
                    .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))?;
            }
        }
        self.frozen.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn freeze_blocking(&self, also_os_suspend: bool) -> ChromiumResult<()> {
        let page = self.page.clone();
        let pid = self.runtime.child_pid();
        let result: ChromiumResult<()> = block_on_handle(self.runtime.handle(), async move {
            let _ = CdpPageFreezer::freeze_page(&page).await;
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
        let _ = CdpPageFreezer::unfreeze_page(&self.page).await;
        self.frozen.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub fn unfreeze_blocking(&self, also_os_resume: bool) -> ChromiumResult<()> {
        let page = self.page.clone();
        let pid = self.runtime.child_pid();
        let result: ChromiumResult<()> = block_on_handle(self.runtime.handle(), async move {
            if also_os_resume {
                if let Some(pid) = pid {
                    ProcessFreezer::unfreeze_process(pid)
                        .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))?;
                }
            }
            let _ = CdpPageFreezer::unfreeze_page(&page).await;
            Ok(())
        });
        if result.is_ok() {
            self.frozen.store(false, Ordering::Relaxed);
        }
        result
    }

    /// Close the CDP target.
    pub async fn close(self) -> ChromiumResult<()> {
        self.page.close().await?;
        Ok(())
    }

    pub fn close_blocking(self) -> ChromiumResult<()> {
        let page = self.page;
        let handle = self.runtime.handle().clone();
        block_on_handle(&handle, async move {
            page.close().await?;
            Ok(())
        })
    }

    async fn navigate_history(&self, delta: i64) -> ChromiumResult<()> {
        history_step(&self.page, delta).await?;
        self.refresh_title_cache().await;
        Ok(())
    }

    async fn refresh_title_cache(&self) {
        if let Ok(result) = self.page.evaluate("document.title").await {
            if let Ok(title) = result.into_value::<String>() {
                *self.cached_title.lock() = Some(title);
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
    fn tab_id_construction() {
        let id = TabId(42);
        assert_eq!(id.0, 42);
        assert_eq!(format!("{:?}", id), "TabId(42)");
    }

    #[test]
    fn lifecycle_enum_distinct() {
        assert_ne!(TabLifecycle::Active, TabLifecycle::Frozen);
    }
}
