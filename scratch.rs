use thor::engine::chromium_runtime::ChromiumRuntime;

async fn test(rt: std::sync::Arc<ChromiumRuntime>) {
    let mut browser = rt.browser().lock().await;
    let ctx = browser.create_browser_context().await.unwrap();
    let id = ctx.id().to_string(); // test if this exists
}
