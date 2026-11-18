use thor::daemon::prewarm::PrewarmDaemon;

#[test]
#[ignore = "Requires an owned Chromium executable configured with THOR_CHROMIUM_PATH"]
fn test_exhaustion() {
    let daemon = PrewarmDaemon::new(1);

    // Create an instance by manually starting it
    daemon
        .warm_thor_chromium(false)
        .expect("owned Chromium runtime must start for this live test");

    // Acquire a tab
    let _tab1 = daemon.acquire_tab().unwrap();
    println!("Acquired tab 1");

    let _tab2 = daemon.acquire_tab().unwrap();
    println!("Acquired tab 2");

    let _tab3 = daemon.acquire_tab().unwrap();
    println!("Acquired tab 3");
}
