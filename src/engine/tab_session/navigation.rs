use super::*;

impl TabSession {
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
        block_on_handle(self.runtime.handle(), async move {
            history_step(&page, -1).await
        })
    }

    /// Go forward in history when possible.
    pub async fn forward(&self) -> ChromiumResult<()> {
        self.navigate_history(1).await
    }

    pub fn forward_blocking(&self) -> ChromiumResult<()> {
        let page = self.ensure_active_page()?;
        self.touch();
        block_on_handle(self.runtime.handle(), async move {
            history_step(&page, 1).await
        })
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

    async fn navigate_history(&self, delta: i64) -> ChromiumResult<()> {
        let page = self.ensure_active_page()?;
        history_step(&page, delta).await?;
        self.refresh_title_cache().await;
        Ok(())
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
