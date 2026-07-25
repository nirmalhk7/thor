use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

pub struct PrewarmDaemon {
    pool_size: usize,
    instances: Arc<Mutex<Vec<Child>>>,
}

impl PrewarmDaemon {
    pub fn new(pool_size: usize) -> Self {
        Self {
            pool_size,
            instances: Arc::new(Mutex::new(Vec::with_capacity(pool_size))),
        }
    }

    pub async fn start(&self) {
        loop {
            self.replenish_pool();
            sleep(Duration::from_millis(500)).await;
        }
    }

    fn replenish_pool(&self) {
        let mut instances = self.instances.lock().unwrap();
        
        // Clean up dead instances
        instances.retain_mut(|child| {
            match child.try_wait() {
                Ok(Some(_)) => false,
                Ok(None) => true,
                Err(_) => false,
            }
        });

        // Spawn new instances if needed
        while instances.len() < self.pool_size {
            if let Ok(child) = self.spawn_instance() {
                instances.push(child);
            } else {
                break;
            }
        }
    }

    fn spawn_instance(&self) -> std::io::Result<Child> {
        #[cfg(target_os = "macos")]
        let cmd = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
        #[cfg(target_os = "windows")]
        let cmd = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
        #[cfg(target_os = "linux")]
        let cmd = "google-chrome";

        Command::new(cmd)
            .args(&[
                "--headless",
                "--disable-gpu",
                "--remote-debugging-port=0",
                "--no-sandbox",
            ])
            .spawn()
    }

    pub fn acquire_instance(&self) -> Option<Child> {
        let mut instances = self.instances.lock().unwrap();
        instances.pop()
    }
}
