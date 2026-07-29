use thor::daemon::prewarm::PrewarmDaemon;
use thor::engine::chromium_runtime::{ChromiumRuntimeConfig, ChromiumRuntime};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;

#[test]
fn test_exhaustion() {
    let daemon = PrewarmDaemon::new(1);
    
    // Create an instance by manually starting it
    let result = daemon.warm_thor_chromium(false);
    if result.is_err() {
        println!("Error warming up: {:?}", result.err());
        return;
    }
    
    // Acquire a tab
    let tab1 = daemon.acquire_tab().unwrap();
    println!("Acquired tab 1");
    
    let tab2 = daemon.acquire_tab().unwrap();
    println!("Acquired tab 2");
    
    let tab3 = daemon.acquire_tab().unwrap();
    println!("Acquired tab 3");
}
