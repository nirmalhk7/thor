use super::test_peer::connected_runtime;
use super::*;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connected_runtime_drives_cdp_and_preserves_remote_ownership() {
    let (runtime, mut peer) = connected_runtime().await;
    assert_eq!(runtime.executable_path(), Path::new("<connected>"));
    assert_eq!(runtime.child_pid(), None);
    assert!(runtime._runtime_keep_alive.is_none());
    assert_eq!(runtime.prewarm_target.load(Ordering::Relaxed), 0);
    assert!(runtime.prewarm.lock().is_empty());
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        runtime.new_tab(Some("https://example.test/雪")),
    )
    .await
    .expect("CDP handler must drive replies");
    assert!(result.is_err());
    let discover = peer.commands.recv().await.unwrap();
    assert_eq!(discover["method"], "Target.setDiscoverTargets");
    let create = peer.commands.recv().await.unwrap();
    assert_eq!(create["method"], "Target.createTarget");
    assert_eq!(create["params"]["url"], "https://example.test/雪");
    let handle = runtime.handle().clone();
    let answer = tokio::task::spawn_blocking(move || block_on_handle(&handle, async { 42 }))
        .await
        .unwrap();
    assert_eq!(answer, 42);
    tokio::time::timeout(Duration::from_secs(2), runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        peer.commands.recv().await.unwrap()["method"],
        "Browser.close"
    );
}
