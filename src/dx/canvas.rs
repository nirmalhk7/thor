use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Breakpoint {
    Mobile,
    Tablet,
    Laptop,
    Desktop,
}

#[derive(Debug, Clone)]
pub enum ViewportEvent {
    Scroll { x: f32, y: f32 },
    Click { x: f32, y: f32, element_id: Option<String> },
    FormInput { element_id: String, value: String },
}

/// Multi-viewport synchronized scroll/click/form event bus across 3-6 breakpoint panes.
pub struct CanvasEventBus {
    pub active_breakpoints: Vec<Breakpoint>,
    pub sync_enabled: bool,
}

impl CanvasEventBus {
    pub fn new() -> Self {
        Self {
            active_breakpoints: vec![
                Breakpoint::Mobile,
                Breakpoint::Tablet,
                Breakpoint::Laptop,
            ],
            sync_enabled: true,
        }
    }

    pub fn toggle_sync(&mut self) {
        self.sync_enabled = !self.sync_enabled;
    }

    pub fn set_breakpoints(&mut self, breakpoints: Vec<Breakpoint>) {
        self.active_breakpoints = breakpoints;
    }

    /// Dispatches an event from a source breakpoint to all other active breakpoints
    pub fn dispatch_event(&self, source: Breakpoint, event: ViewportEvent) {
        if !self.sync_enabled {
            return;
        }
        
        for bp in &self.active_breakpoints {
            if *bp != source {
                // In a real implementation, this would send the event over a channel
                // to the respective viewport's rendering/JS execution thread.
                // println!("Syncing event to {:?}: {:?}", bp, event);
            }
        }
    }
}

impl Default for CanvasEventBus {
    fn default() -> Self {
        Self::new()
    }
}
