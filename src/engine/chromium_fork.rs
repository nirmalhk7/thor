use std::process::Child;

pub struct ChromiumFork {
    process: Option<Child>,
    ipc_channel: String,
}

impl ChromiumFork {
    pub fn new(ipc_channel: String) -> Self {
        Self {
            process: None,
            ipc_channel,
        }
    }

    pub fn attach_prewarmed(&mut self, child: Child) {
        self.process = Some(child);
        println!("Attached to pre-warmed Chromium instance on IPC channel: {}", self.ipc_channel);
    }
    
    pub fn terminate(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

impl Drop for ChromiumFork {
    fn drop(&mut self) {
        self.terminate();
    }
}
