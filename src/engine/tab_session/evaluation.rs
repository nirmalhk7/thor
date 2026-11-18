use super::*;

impl TabSession {
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

    /// Capture a PNG screenshot via CDP `Page.captureScreenshot`.
    pub async fn capture_screenshot(&self) -> ChromiumResult<Vec<u8>> {
        let page = self.ensure_active_page()?;
        let params = chromiumoxide::page::ScreenshotParams::builder()
            .format(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png)
            .build();
        page.screenshot(params)
            .await
            .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))
    }

    pub fn capture_screenshot_blocking(&self) -> ChromiumResult<Vec<u8>> {
        let page = self.ensure_active_page()?;
        block_on_handle(self.runtime.handle(), async move {
            let params = chromiumoxide::page::ScreenshotParams::builder()
                .format(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png)
                .build();
            page.screenshot(params)
                .await
                .map_err(|e| ChromiumError::Other(anyhow::anyhow!(e)))
        })
    }

    /// Cloneable handle for process-wide active-tab evaluate (CDP bridge hook).
    pub fn eval_handle(&self) -> TabEvalHandle {
        TabEvalHandle {
            page: Arc::clone(&self.page),
            runtime: Arc::clone(&self.runtime),
        }
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
            Ok(self.nav_history.lock().last().map(|e| e.url.clone()))
        }
    }

    pub fn url_blocking(&self) -> ChromiumResult<Option<String>> {
        if self.lifecycle() == TabLifecycle::Discarded {
            return Ok(self.nav_history.lock().last().map(|e| e.url.clone()));
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
            browser.execute(OpenDevToolsParams::new(target_id)).await?;
            Ok(())
        })
    }
}
