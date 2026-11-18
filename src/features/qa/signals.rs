use super::{FlakeVector, NetworkTrace, TraceFrame};

impl FlakeVector {
    /// True when every signal is unset (hand-fed empty / default).
    pub fn is_empty(&self) -> bool {
        [
            self.timing_cv,
            self.locator_churn,
            self.race_score,
            self.network_jitter_ms,
            self.animation_score,
        ]
        .into_iter()
        .all(|signal| signal == 0.0)
    }

    /// Merge two vectors by taking the per-field maximum (strongest signal wins).
    pub fn merge(self, other: Self) -> Self {
        Self {
            timing_cv: self.timing_cv.max(other.timing_cv),
            locator_churn: self.locator_churn.max(other.locator_churn),
            race_score: self.race_score.max(other.race_score),
            network_jitter_ms: self.network_jitter_ms.max(other.network_jitter_ms),
            animation_score: self.animation_score.max(other.animation_score),
        }
    }

    /// Derive timing CV, locator churn, race, and animation scores from
    /// time-travel trace frames (no hand-fed vector required).
    pub fn from_trace_frames(frames: &[TraceFrame]) -> Self {
        if frames.len() < 2 {
            return Self::default();
        }
        let mut deltas = Vec::with_capacity(frames.len() - 1);
        let mut digest_churns = 0usize;
        let mut race_hits = 0usize;
        for w in frames.windows(2) {
            let dt = w[1].timestamp_ms.saturating_sub(w[0].timestamp_ms) as f64;
            deltas.push(dt);
            if w[0].dom_digest != w[1].dom_digest {
                digest_churns += 1;
            }
            // Sub-16ms adjacent frames suggest a timing race / animation frame fight.
            if dt > 0.0 && dt < 16.0 {
                race_hits += 1;
            }
        }
        let n = deltas.len() as f64;
        let mean = deltas.iter().sum::<f64>() / n;
        let variance = deltas
            .iter()
            .map(|d| {
                let x = d - mean;
                x * x
            })
            .sum::<f64>()
            / n;
        let std = variance.sqrt();
        let timing_cv = if mean > 0.0 {
            (std / mean).min(2.0)
        } else if std > 0.0 {
            1.0
        } else {
            0.0
        };
        let locator_churn = (digest_churns as f64 / n).clamp(0.0, 1.0);
        let race_score = (race_hits as f64 / n).clamp(0.0, 1.0);
        // Animation score: high digest churn with low mean inter-frame gap.
        let animation_score = if mean > 0.0 && mean < 50.0 {
            (locator_churn * (1.0 - (mean / 50.0))).clamp(0.0, 1.0)
        } else {
            (locator_churn * 0.25).clamp(0.0, 1.0)
        };
        Self {
            timing_cv: timing_cv.clamp(0.0, 1.0),
            locator_churn,
            race_score,
            network_jitter_ms: 0.0,
            animation_score,
        }
    }

    /// Derive network jitter from recorded traces — prefers numeric timing
    /// headers (`x-response-time`, `server-timing`), else status flip rate.
    pub fn from_network_traces(traces: &[NetworkTrace]) -> Self {
        if traces.is_empty() {
            return Self::default();
        }
        let mut timings = Vec::new();
        for t in traces {
            if let Some(ms) = extract_trace_timing_ms(t) {
                timings.push(ms);
            }
        }
        let network_jitter_ms = if timings.len() >= 2 {
            let mean = timings.iter().sum::<f64>() / timings.len() as f64;
            let var = timings
                .iter()
                .map(|d| {
                    let x = d - mean;
                    x * x
                })
                .sum::<f64>()
                / timings.len() as f64;
            var.sqrt()
        } else {
            // No timing headers — use status flip rate × 100ms as jitter proxy.
            let flips = traces
                .windows(2)
                .filter(|w| w[0].status != w[1].status)
                .count();
            let pairs = traces.len().saturating_sub(1).max(1);
            (flips as f64 / pairs as f64) * 100.0
        };
        Self {
            network_jitter_ms,
            ..Default::default()
        }
    }
}

fn extract_trace_timing_ms(t: &NetworkTrace) -> Option<f64> {
    for (k, v) in &t.headers {
        let key = k.to_ascii_lowercase();
        if key == "x-response-time" || key == "x-runtime" || key == "x-request-time" {
            let cleaned = v.trim_end_matches("ms").trim();
            if let Ok(ms) = cleaned.parse::<f64>() {
                return Some(ms);
            }
        }
        if key == "server-timing" {
            // e.g. "app;dur=42.5" — take first dur=
            if let Some(idx) = v.to_ascii_lowercase().find("dur=") {
                let rest = &v[idx + 4..];
                let num: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                if let Ok(ms) = num.parse::<f64>() {
                    return Some(ms);
                }
            }
        }
    }
    None
}
