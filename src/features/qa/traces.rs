use super::{dhash, now_ms, PlaywrightTraceExport, QaEngine, TraceFrame};

impl QaEngine {
    // 10. Time-Travel Trace -------------------------------------------------
    /// Record a trace frame, storing both a change-detection digest and the full
    /// DOM markup so a frame can be replayed, not merely diffed.
    pub fn record_trace_frame(&self, test: &str, step: &str, action: &str, dom: &str) -> usize {
        let mut traces = self.traces.lock();
        let frames = traces.entry(test.to_string()).or_default();
        let index = frames.len();
        frames.push(TraceFrame {
            index,
            step: step.to_string(),
            action: action.to_string(),
            dom_digest: format!("{:016x}", dhash(dom.as_bytes())),
            dom_snapshot: dom.to_string(),
            timestamp_ms: now_ms(),
        });
        index
    }

    pub fn trace_frames(&self, test: &str) -> Vec<TraceFrame> {
        self.traces.lock().get(test).cloned().unwrap_or_default()
    }

    pub fn trace_frame_at(&self, test: &str, index: usize) -> Option<TraceFrame> {
        self.traces
            .lock()
            .get(test)
            .and_then(|f| f.get(index).cloned())
    }

    /// Export recorded frames to a Playwright-style on-disk trace directory
    /// (`trace.meta.json` + per-frame HTML snapshots) under Thor data dir.
    pub fn export_playwright_trace(&self, test: &str) -> Result<PlaywrightTraceExport, String> {
        let frames = self.trace_frames(test);
        if frames.is_empty() {
            return Err(format!(
                "no time-travel frames for test '{test}' — record with from_active_tab first"
            ));
        }
        let safe: String = test
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let dir = crate::paths::thor_data_dir().join("traces").join(&safe);
        std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
        let mut frame_paths = Vec::new();
        for frame in &frames {
            let name = format!("frame-{:04}.html", frame.index);
            let path = dir.join(&name);
            std::fs::write(&path, &frame.dom_snapshot)
                .map_err(|e| format!("write {}: {e}", path.display()))?;
            frame_paths.push(name);
        }
        let meta = serde_json::json!({
            "version": 1,
            "test": test,
            "frame_count": frames.len(),
            "frames": frames.iter().map(|f| serde_json::json!({
                "index": f.index,
                "step": f.step,
                "action": f.action,
                "dom_digest": f.dom_digest,
                "timestamp_ms": f.timestamp_ms,
                "snapshot": format!("frame-{:04}.html", f.index),
            })).collect::<Vec<_>>(),
        });
        let meta_path = dir.join("trace.meta.json");
        std::fs::write(
            &meta_path,
            serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".into()),
        )
        .map_err(|e| format!("write meta: {e}"))?;
        Ok(PlaywrightTraceExport {
            test: test.to_string(),
            dir: dir.display().to_string(),
            frame_count: frames.len(),
            frame_paths,
            meta_path: meta_path.display().to_string(),
            open_hint: format!(
                "open {} — Thor DOM time-travel export (HTML snapshots + meta)",
                dir.display()
            ),
        })
    }
}
