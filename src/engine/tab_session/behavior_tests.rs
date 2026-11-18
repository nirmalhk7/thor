use super::*;
use crate::engine::chromium_runtime::test_peer::connected_runtime;

fn detached_session(runtime: Arc<ChromiumRuntime>, ephemeral: bool) -> TabSession {
    let mut identity = IdentityContext::new("identity", "profile", ephemeral);
    identity.set_cookie("session", "cookie");
    identity.set_local_storage("theme", "dark");
    identity.set_auth_token("api", "secret");
    TabSession {
        id: TabId(77),
        page: Arc::new(Mutex::new(None)),
        runtime,
        frozen: AtomicBool::new(true),
        lifecycle: Arc::new(RwLock::new(TabLifecycle::Discarded)),
        cached_title: Arc::new(Mutex::new(Some("cached".into()))),
        identity_context: Arc::new(Mutex::new(identity)),
        nav_history: Arc::new(Mutex::new(Vec::new())),
        scroll_position: Arc::new(Mutex::new((12.0, 34.0))),
        trace_buffer: Arc::new(Mutex::new(vec!["captured".into()])),
        last_active: Arc::new(Mutex::new(Instant::now())),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn closing_detached_tab_expires_and_clears_only_disposable_identity() {
    for ephemeral in [false, true] {
        for blocking in [false, true] {
            let (runtime, _peer) = connected_runtime().await;
            let session = detached_session(Arc::clone(&runtime), ephemeral);
            let lifecycle = Arc::clone(&session.lifecycle);
            let identity = Arc::clone(&session.identity_context);
            if blocking {
                tokio::task::spawn_blocking(move || session.close_blocking())
                    .await
                    .unwrap()
                    .unwrap();
            } else {
                session.close().await.unwrap();
            }
            assert_eq!(*lifecycle.read(), TabLifecycle::Expired);
            {
                let identity = identity.lock();
                assert_eq!(identity.cookies.is_empty(), ephemeral);
                assert_eq!(identity.local_storage.is_empty(), ephemeral);
                assert_eq!(identity.auth_tokens.is_empty(), ephemeral);
                assert!(identity.indexed_db_enabled);
                assert!(identity.service_workers_enabled);
                assert_eq!(identity.profile_id, "profile");
            }
            runtime.shutdown().await.unwrap();
        }
    }
}
