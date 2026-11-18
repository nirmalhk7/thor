use super::{LcpElementPayload, LcpElementReport, LcpPhaseBreakdown, PerfEngine};

impl PerfEngine {
    // 13. LCP Element Forensics ---------------------------------------------
    pub fn lcp_element_forensics(&self, payload: &LcpElementPayload) -> LcpElementReport {
        let mut observation = LcpObservation::from(payload);
        if let Some(raw) = &payload.vitals_json {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
                observation.fill_from_json(&value);
            }
        }
        observation.report()
    }
}

struct LcpObservation {
    url: String,
    element_type: String,
    size_bytes: u64,
    discover_ms: f64,
    load_ms: f64,
    render_ms: f64,
}

impl From<&LcpElementPayload> for LcpObservation {
    fn from(payload: &LcpElementPayload) -> Self {
        Self {
            url: payload.url.clone().unwrap_or_default(),
            element_type: payload
                .element_type
                .clone()
                .unwrap_or_else(|| "unknown".into()),
            size_bytes: payload.size_bytes.unwrap_or(0),
            discover_ms: payload.discover_ms.unwrap_or(0.0),
            load_ms: payload.load_ms.unwrap_or(0.0),
            render_ms: payload.render_ms.unwrap_or(0.0),
        }
    }
}

impl LcpObservation {
    fn fill_from_json(&mut self, value: &serde_json::Value) {
        if self.url.is_empty() {
            self.url = lcp_field(value, "/lcpElement/url", "lcpUrl")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
        }
        if self.element_type == "unknown" {
            self.element_type = lcp_field(value, "/lcpElement/type", "lcpType")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
        }
        if self.size_bytes == 0 {
            self.size_bytes = lcp_field(value, "/lcpElement/size", "lcpSize")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
        }
        fill_timing(
            &mut self.discover_ms,
            value,
            &["lcpDiscover", "/lcpElement/discoverTime", "discover_ms"],
        );
        fill_timing(
            &mut self.load_ms,
            value,
            &["lcpLoad", "/lcpElement/loadTime", "load_ms"],
        );
        fill_timing(
            &mut self.render_ms,
            value,
            &[
                "lcp",
                "lcp_ms",
                "lcpRender",
                "/lcpElement/renderTime",
                "render_ms",
            ],
        );
    }

    fn report(self) -> LcpElementReport {
        // Phase deltas: discover → load → render (clamp negatives to 0).
        let discover = self.discover_ms.max(0.0);
        let load_phase = (self.load_ms - discover).max(0.0);
        let render_phase = (self.render_ms - self.load_ms.max(discover)).max(0.0);
        let total = self.render_ms.max(self.load_ms).max(discover);
        let rating = if total <= 2500.0 {
            "good"
        } else if total <= 4000.0 {
            "needs-improvement"
        } else {
            "poor"
        };
        LcpElementReport {
            url: self.url,
            element_type: self.element_type,
            size_bytes: self.size_bytes,
            phases: LcpPhaseBreakdown {
                discover_ms: discover,
                load_ms: load_phase,
                render_ms: render_phase,
                total_ms: total,
            },
            rating: rating.to_string(),
        }
    }
}

fn lcp_field<'a>(
    value: &'a serde_json::Value,
    nested: &str,
    flat: &str,
) -> Option<&'a serde_json::Value> {
    value.pointer(nested).or_else(|| value.get(flat))
}

fn fill_timing(timing: &mut f64, value: &serde_json::Value, keys: &[&str]) {
    if *timing <= 0.0 {
        *timing = numeric_value(value, keys).unwrap_or(0.0);
    }
}

fn numeric_value(value: &serde_json::Value, keys: &[&str]) -> Option<f64> {
    for key in keys {
        if let Some(number) = value.get(*key).and_then(|v| v.as_f64()) {
            return Some(number);
        }
        if let Some(number) = value.pointer(key).and_then(|v| v.as_f64()) {
            return Some(number);
        }
    }
    None
}
