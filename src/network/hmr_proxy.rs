use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub struct HmrProxy {
    is_sleeping: Arc<Mutex<bool>>,
    buffer: Arc<Mutex<VecDeque<String>>>,
}

impl HmrProxy {
    pub fn new() -> Self {
        Self {
            is_sleeping: Arc::new(Mutex::new(false)),
            buffer: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn on_tab_sleep(&self) {
        let mut sleeping = self.is_sleeping.lock().unwrap();
        *sleeping = true;
        println!("Tab is sleeping. HMR Proxy started buffering.");
    }

    pub fn on_tab_thaw(&self) {
        let mut sleeping = self.is_sleeping.lock().unwrap();
        *sleeping = false;
        println!("Tab thawed. Flushing HMR Proxy buffer...");
        self.flush_buffer();
    }

    pub fn receive_payload(&self, payload: String) {
        let sleeping = self.is_sleeping.lock().unwrap();
        if *sleeping {
            let mut buf = self.buffer.lock().unwrap();
            buf.push_back(payload.clone());
            println!("Buffered HMR payload: {}", payload);
        } else {
            println!("Forwarding HMR payload immediately: {}", payload);
        }
    }

    fn flush_buffer(&self) {
        let mut buf = self.buffer.lock().unwrap();
        while let Some(payload) = buf.pop_front() {
            println!("Flushed HMR payload: {}", payload);
        }
    }
}
