use thor::engine::chromium_runtime::{ChromiumRuntimeConfig, ChromiumRuntime};
use std::time::Duration;

#[tokio::test]
async fn test_runtime_prewarm_exhaustion() {
    let cfg = ChromiumRuntimeConfig {
        prewarm_pool_size: 1,
        no_sandbox: true,
        headful: false,
        ..Default::default()
    };
    
    // Some CI might not have Chromium, so we skip if not found
    let rt = match ChromiumRuntime::launch(cfg).await {
        Ok(rt) => rt,
        Err(_) => return,
    };
    
    // Pop the only prewarmed tab
    let tab1 = rt.acquire_tab().await.expect("tab1");
    // This should fallback to creating a new one directly and spawn replenishment
    let tab2 = rt.acquire_tab().await.expect("tab2");
    
    // Wait for replenishment
    tokio::time::sleep(Duration::from_millis(1000)).await;
    
    let tab3 = rt.acquire_tab().await.expect("tab3");
    
    rt.close_tab(tab1).await.unwrap();
    rt.close_tab(tab2).await.unwrap();
    rt.close_tab(tab3).await.unwrap();
    rt.shutdown().await.unwrap();
}
