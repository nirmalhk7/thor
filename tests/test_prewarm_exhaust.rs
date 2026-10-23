use thor::daemon::prewarm::PrewarmDaemon;

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
    let _tab1 = daemon.acquire_tab().unwrap();
    println!("Acquired tab 1");
    
    let _tab2 = daemon.acquire_tab().unwrap();
    println!("Acquired tab 2");
    
    let _tab3 = daemon.acquire_tab().unwrap();
    println!("Acquired tab 3");
}
