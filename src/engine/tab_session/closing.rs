use super::*;

impl TabSession {
    /// Expire tab state before attempting remote cleanup.
    fn expire_identity(&self) -> bool {
        *self.lifecycle.write() = TabLifecycle::Expired;
        let is_ephemeral = self.is_ephemeral_context();
        if is_ephemeral {
            self.identity_context.lock().cookies.clear();
            self.identity_context.lock().local_storage.clear();
            self.identity_context.lock().auth_tokens.clear();
        }
        is_ephemeral
    }

    /// Close the CDP target and expire session.
    pub async fn close(self) -> ChromiumResult<()> {
        let is_ephemeral = self.expire_identity();
        let page_opt = self.page.lock().take();
        if let Some(page) = page_opt {
            close_page(page, is_ephemeral).await?;
        }
        Ok(())
    }

    pub fn close_blocking(self) -> ChromiumResult<()> {
        let is_ephemeral = self.expire_identity();
        let page_opt = self.page.lock().take();
        if let Some(page) = page_opt {
            let handle = self.runtime.handle().clone();
            block_on_handle(&handle, async move { close_page(page, is_ephemeral).await })
        } else {
            Ok(())
        }
    }
}

async fn close_page(page: Page, is_ephemeral: bool) -> ChromiumResult<()> {
    if is_ephemeral {
        clear_page_data(&page).await;
    }
    page.close().await?;
    Ok(())
}

async fn clear_page_data(page: &Page) {
    let Ok(Some(url)) = page.url().await else {
        return;
    };
    if url.is_empty() {
        return;
    }
    let origin = url.split('/').take(3).collect::<Vec<&str>>().join("/");
    if !origin.is_empty() {
        let _ = page
            .execute(ClearDataForOriginParams::new(origin, "all".to_string()))
            .await;
    }
}
