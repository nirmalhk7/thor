pub struct CrashUI {
    pub is_visible: bool,
    pub last_frame_snapshot: Option<Vec<u8>>,
}

impl CrashUI {
    pub fn new() -> Self {
        Self {
            is_visible: false,
            last_frame_snapshot: None,
        }
    }

    pub fn trigger_crash_state(&mut self, frame: Vec<u8>) {
        self.last_frame_snapshot = Some(frame);
        self.is_visible = true;
        self.render();
    }

    pub fn render(&self) {
        if self.is_visible {
            println!("Rendering Glass-morphic UI: 'Renderer Crashed - Click to Rehydrate'");
            println!("Overlaying on last visual frame (size: {} bytes)", 
                self.last_frame_snapshot.as_ref().map_or(0, |f| f.len()));
        }
    }

    pub fn on_click_rehydrate(&mut self) {
        println!("Rehydrating renderer...");
        self.is_visible = false;
        self.last_frame_snapshot = None;
    }
}
