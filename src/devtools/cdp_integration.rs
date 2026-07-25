use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// Represents a console.log event from the JS runtime
#[derive(Debug, Clone)]
pub struct ConsoleLogEvent {
    pub timestamp: u64,
    pub level: String,
    pub message: String,
}

/// Native DevTools panel consuming standard CDP + SPSC Fast Path console.log log stream.
pub struct CdpIntegration {
    pub is_connected: bool,
    log_receiver: Option<Receiver<ConsoleLogEvent>>,
    logs: Vec<ConsoleLogEvent>,
}

impl CdpIntegration {
    pub fn new() -> Self {
        Self {
            is_connected: false,
            log_receiver: None,
            logs: Vec::new(),
        }
    }

    pub fn connect(&mut self) -> Sender<ConsoleLogEvent> {
        self.is_connected = true;
        let (tx, rx) = mpsc::channel();
        self.log_receiver = Some(rx);
        
        tx
    }

    pub fn poll_logs(&mut self) {
        if let Some(rx) = &self.log_receiver {
            while let Ok(log) = rx.try_recv() {
                self.logs.push(log);
            }
        }
    }

    pub fn get_recent_logs(&self, count: usize) -> Vec<ConsoleLogEvent> {
        let skip = if self.logs.len() > count {
            self.logs.len() - count
        } else {
            0
        };
        self.logs.iter().skip(skip).cloned().collect()
    }
}

impl Default for CdpIntegration {
    fn default() -> Self {
        Self::new()
    }
}
