use super::{percentile_of, FreezeResumeReport, FreezeTimingSample, PerfEngine};

impl PerfEngine {
    /// Record soft-freeze duration for `tab_id` (paired with a later unfreeze).
    pub fn note_soft_freeze(&self, tab_id: &str, soft_freeze_ms: f64) {
        self.freeze_pending
            .lock()
            .insert(tab_id.to_string(), soft_freeze_ms);
    }

    /// Complete a freeze/unfreeze pair and push a [`FreezeTimingSample`].
    pub fn note_soft_unfreeze(&self, tab_id: &str, soft_unfreeze_ms: f64) {
        let freeze_ms = self.freeze_pending.lock().remove(tab_id).unwrap_or(0.0);
        self.record_freeze_sample(FreezeTimingSample {
            tab_id: tab_id.to_string(),
            soft_freeze_ms: freeze_ms,
            soft_unfreeze_ms,
            discard_ms: None,
            restore_ms: None,
        });
    }

    /// Attach discard timing to the latest sample for `tab_id`, or push a new one.
    pub fn note_discard(&self, tab_id: &str, discard_ms: f64) {
        let mut samples = self.freeze_samples.lock();
        if let Some(s) = samples.iter_mut().rev().find(|s| s.tab_id == tab_id) {
            s.discard_ms = Some(discard_ms);
        } else {
            samples.push(FreezeTimingSample {
                tab_id: tab_id.to_string(),
                soft_freeze_ms: 0.0,
                soft_unfreeze_ms: 0.0,
                discard_ms: Some(discard_ms),
                restore_ms: None,
            });
        }
    }

    /// Attach restore timing to the latest sample for `tab_id`.
    pub fn note_restore(&self, tab_id: &str, restore_ms: f64) {
        let mut samples = self.freeze_samples.lock();
        if let Some(s) = samples.iter_mut().rev().find(|s| s.tab_id == tab_id) {
            s.restore_ms = Some(restore_ms);
        } else {
            samples.push(FreezeTimingSample {
                tab_id: tab_id.to_string(),
                soft_freeze_ms: 0.0,
                soft_unfreeze_ms: 0.0,
                discard_ms: None,
                restore_ms: Some(restore_ms),
            });
        }
    }

    pub fn record_freeze_sample(&self, sample: FreezeTimingSample) {
        self.freeze_samples.lock().push(sample);
    }

    pub fn freeze_timing_samples(&self) -> Vec<FreezeTimingSample> {
        self.freeze_samples.lock().clone()
    }

    pub fn clear_freeze_timing_samples(&self) {
        self.freeze_samples.lock().clear();
        self.freeze_pending.lock().clear();
    }

    // 17. Freeze / Resume Bench ---------------------------------------------
    pub fn freeze_resume_bench(&self, samples: &[FreezeTimingSample]) -> FreezeResumeReport {
        if samples.is_empty() {
            return FreezeResumeReport {
                samples: 0,
                avg_freeze_ms: 0.0,
                avg_unfreeze_ms: 0.0,
                avg_discard_ms: 0.0,
                avg_restore_ms: 0.0,
                p95_freeze_ms: 0.0,
                p95_unfreeze_ms: 0.0,
                slow_tabs: Vec::new(),
            };
        }
        let n = samples.len() as f64;
        let avg_freeze_ms = samples.iter().map(|s| s.soft_freeze_ms).sum::<f64>() / n;
        let avg_unfreeze_ms = samples.iter().map(|s| s.soft_unfreeze_ms).sum::<f64>() / n;
        let discards: Vec<f64> = samples.iter().filter_map(|s| s.discard_ms).collect();
        let restores: Vec<f64> = samples.iter().filter_map(|s| s.restore_ms).collect();
        let avg_discard_ms = if discards.is_empty() {
            0.0
        } else {
            discards.iter().sum::<f64>() / discards.len() as f64
        };
        let avg_restore_ms = if restores.is_empty() {
            0.0
        } else {
            restores.iter().sum::<f64>() / restores.len() as f64
        };
        let freeze_vals: Vec<f64> = samples.iter().map(|s| s.soft_freeze_ms).collect();
        let unfreeze_vals: Vec<f64> = samples.iter().map(|s| s.soft_unfreeze_ms).collect();
        let p95_freeze_ms = percentile_of(&freeze_vals, 95.0);
        let p95_unfreeze_ms = percentile_of(&unfreeze_vals, 95.0);
        let slow_threshold = p95_freeze_ms.max(50.0);
        let mut slow_tabs: Vec<String> = samples
            .iter()
            .filter(|s| s.soft_freeze_ms >= slow_threshold || s.soft_unfreeze_ms >= slow_threshold)
            .map(|s| s.tab_id.clone())
            .collect();
        slow_tabs.sort();
        slow_tabs.dedup();
        FreezeResumeReport {
            samples: samples.len(),
            avg_freeze_ms,
            avg_unfreeze_ms,
            avg_discard_ms,
            avg_restore_ms,
            p95_freeze_ms,
            p95_unfreeze_ms,
            slow_tabs,
        }
    }
}
