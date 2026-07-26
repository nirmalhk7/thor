use std::sync::{Arc, Barrier};
use std::thread;
use std::net::TcpListener;
use thor::AppState;

#[test]
fn test_app_state_stress() {
    let state = AppState::global();
    let num_threads = 100;
    let barrier = Arc::new(Barrier::new(num_threads));

    let mut handles = vec![];

    // Dummy server to not fail locust startup connection entirely
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{addr}/");

    for i in 0..num_threads {
        let b = barrier.clone();
        let u = url.clone();
        handles.push(thread::spawn(move || {
            b.wait();
            for j in 0..100 {
                // Toggle profiles
                state.toggle_profile();

                // Toggle security shield
                state.security().set_strip_tracking(j % 2 == 0);
                state.security().set_inject_headers(j % 2 != 0);

                // Start/stop locust
                if j % 10 == 0 {
                    let _ = state.locust_start(&u, 1, 0, 1);
                } else if j % 10 == 5 {
                    let _ = state.locust_stop();
                }

                // Check getters
                let _ = state.security().strip_tracking_enabled();
                let _ = state.security().inject_headers_enabled();
                let _ = state.locust_is_running();
                let _ = state.locust_snapshot();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}
