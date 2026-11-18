use super::{BTreeMap, DomXssTaintReport, SecurityEngine, TaintFlow};

impl SecurityEngine {
    // ---- Wave-2 security --------------------------------------------------

    pub fn dom_xss_taint_map(&self, flows: &[TaintFlow]) -> DomXssTaintReport {
        let mut enriched = Vec::with_capacity(flows.len());
        let mut by_sink_kind: BTreeMap<String, usize> = BTreeMap::new();
        for f in flows {
            let mut flow = f.clone();
            let kind = flow
                .sink_kind
                .clone()
                .unwrap_or_else(|| classify_dom_xss_sink(&flow.sink));
            flow.sink_kind = Some(kind.clone());
            *by_sink_kind.entry(kind).or_default() += 1;
            enriched.push(flow);
        }
        let unsanitized = enriched.iter().filter(|f| !f.sanitized).count();
        let risk = if unsanitized == 0 {
            "low"
        } else if unsanitized <= 2 {
            "medium"
        } else {
            "high"
        };
        DomXssTaintReport {
            flows: enriched,
            unsanitized,
            risk: risk.to_string(),
            by_sink_kind,
        }
    }

    /// Parse CDP-shaped taint JSON (`[{source,sink,via,sanitized}]` or
    /// `{flows:[…]}`) into [`TaintFlow`]s for [`dom_xss_taint_map`].
    pub fn parse_dom_xss_taint_flows(json: &str) -> Vec<TaintFlow> {
        let v: serde_json::Value = match serde_json::from_str(json.trim()) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        let arr = if let Some(a) = v.as_array() {
            a.clone()
        } else if let Some(a) = v.get("flows").and_then(|f| f.as_array()) {
            a.clone()
        } else {
            return Vec::new();
        };
        arr.iter()
            .filter_map(|item| {
                let source = item.get("source")?.as_str()?.to_string();
                let sink = item.get("sink")?.as_str()?.to_string();
                let via = item
                    .get("via")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let sanitized = item
                    .get("sanitized")
                    .and_then(|s| s.as_bool())
                    .unwrap_or(false);
                let sink_kind = item
                    .get("sink_kind")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| Some(classify_dom_xss_sink(&sink)));
                Some(TaintFlow {
                    source,
                    sink,
                    via,
                    sanitized,
                    sink_kind,
                })
            })
            .collect()
    }
}
fn classify_dom_xss_sink(sink: &str) -> String {
    let s = sink.to_lowercase();
    if contains_any(
        &s,
        &[
            "innerhtml",
            "outerhtml",
            "insertadjacenthtml",
            "document.write",
        ],
    ) {
        "html".into()
    } else if contains_any(
        &s,
        &["eval", "function(", "settimeout", "setinterval", "script"],
    ) {
        "js".into()
    } else if contains_any(&s, &["location", "href", "src", "action", "window.open"]) {
        "url".into()
    } else if s.contains("setattribute") || s.contains("jquery") || s.contains("$(") {
        "dom".into()
    } else {
        "unknown".into()
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}
