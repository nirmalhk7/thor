use super::*;

impl TabSession {
    /// Discard the tab: detach CDP page target & release memory while retaining tab metadata.
    pub fn discard_blocking(&self) -> ChromiumResult<()> {
        let stage = self.lifecycle();
        if stage == TabLifecycle::Discarded || stage == TabLifecycle::Expired {
            return Ok(());
        }

        let tab_key = format!("tab-{}", self.id.0);
        let t0 = Instant::now();
        let page_opt = self.page.lock().take();
        if let Some(page) = page_opt {
            let handle = self.runtime.handle().clone();
            let _ = block_on_handle(&handle, async move {
                page.close().await?;
                Ok::<(), ChromiumError>(())
            });
        }
        let discard_ms = t0.elapsed().as_secs_f64() * 1000.0;
        crate::app_state::AppState::global()
            .features()
            .perf()
            .note_discard(&tab_key, discard_ms);

        *self.lifecycle.write() = TabLifecycle::Discarded;
        self.frozen.store(true, Ordering::Relaxed);
        self.push_trace("Tab discarded (CDP target detached & memory released)");
        crate::thor_info!(
            "thor::tab_session",
            "⚡ [Thor TabSession] Tab {} discarded (memory released)",
            self.id.0
        );
        Ok(())
    }

    /// Restore a discarded tab back to Active state by launching a new CDP page target.
    pub fn restore_blocking(&self) -> ChromiumResult<()> {
        if self.lifecycle() != TabLifecycle::Discarded {
            return Ok(());
        }

        let tab_key = format!("tab-{}", self.id.0);
        let t0 = Instant::now();
        let last_url = self
            .nav_history
            .lock()
            .last()
            .map(|e| e.url.clone())
            .unwrap_or_else(|| "https://www.google.com".to_string());

        let restored_session = self.runtime.new_tab_blocking(Some(&last_url))?;
        if let Some(new_page) = restored_session.page() {
            *self.page.lock() = Some(new_page);
        }
        let restore_ms = t0.elapsed().as_secs_f64() * 1000.0;
        crate::app_state::AppState::global()
            .features()
            .perf()
            .note_restore(&tab_key, restore_ms);

        *self.lifecycle.write() = TabLifecycle::Active;
        self.frozen.store(false, Ordering::Relaxed);
        self.touch();
        self.push_trace("Tab restored from discarded state");
        crate::thor_info!(
            "thor::tab_session",
            "⚡ [Thor TabSession] Tab {} restored to Active at {last_url}",
            self.id.0
        );
        Ok(())
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
        let tab_key = format!("tab-{}", self.id.0);
        let page_opt = self.page.lock().clone();
        if let Some(page) = page_opt {
            if let Ok(ms) = CdpPageFreezer::freeze_page(&page).await {
                crate::app_state::AppState::global()
                    .features()
                    .perf()
                    .note_soft_freeze(&tab_key, ms);
            }
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
        let tab_key = format!("tab-{}", self.id.0);
        let result: ChromiumResult<Option<f64>> =
            block_on_handle(self.runtime.handle(), async move {
                let mut freeze_ms = None;
                if let Some(page) = page_opt {
                    if let Ok(ms) = CdpPageFreezer::freeze_page(&page).await {
                        freeze_ms = Some(ms);
                    }
                }
                if also_os_suspend {
                    if let Some(pid) = pid {
                        ProcessFreezer::freeze_process(pid)
                            .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))?;
                    }
                }
                Ok(freeze_ms)
            });
        match result {
            Ok(freeze_ms) => {
                if let Some(ms) = freeze_ms {
                    crate::app_state::AppState::global()
                        .features()
                        .perf()
                        .note_soft_freeze(&tab_key, ms);
                }
                self.frozen.store(true, Ordering::Relaxed);
                *self.lifecycle.write() = TabLifecycle::Frozen;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub async fn unfreeze(&self, also_os_resume: bool) -> ChromiumResult<()> {
        let tab_key = format!("tab-{}", self.id.0);
        if also_os_resume {
            if let Some(pid) = self.runtime.child_pid() {
                ProcessFreezer::unfreeze_process(pid)
                    .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))?;
            }
        }
        let page_opt = self.page.lock().clone();
        if let Some(page) = page_opt {
            if let Ok(ms) = CdpPageFreezer::unfreeze_page(&page).await {
                crate::app_state::AppState::global()
                    .features()
                    .perf()
                    .note_soft_unfreeze(&tab_key, ms);
            }
        }
        self.frozen.store(false, Ordering::Relaxed);
        *self.lifecycle.write() = TabLifecycle::Active;
        Ok(())
    }

    pub fn unfreeze_blocking(&self, also_os_resume: bool) -> ChromiumResult<()> {
        let page_opt = self.page.lock().clone();
        let pid = self.runtime.child_pid();
        let tab_key = format!("tab-{}", self.id.0);
        let result: ChromiumResult<Option<f64>> =
            block_on_handle(self.runtime.handle(), async move {
                if also_os_resume {
                    if let Some(pid) = pid {
                        ProcessFreezer::unfreeze_process(pid)
                            .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))?;
                    }
                }
                let mut unfreeze_ms = None;
                if let Some(page) = page_opt {
                    if let Ok(ms) = CdpPageFreezer::unfreeze_page(&page).await {
                        unfreeze_ms = Some(ms);
                    }
                }
                Ok(unfreeze_ms)
            });
        match result {
            Ok(unfreeze_ms) => {
                if let Some(ms) = unfreeze_ms {
                    crate::app_state::AppState::global()
                        .features()
                        .perf()
                        .note_soft_unfreeze(&tab_key, ms);
                }
                self.frozen.store(false, Ordering::Relaxed);
                *self.lifecycle.write() = TabLifecycle::Active;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}
