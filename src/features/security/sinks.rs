use super::{SecurityEngine, SinkFinding};

impl SecurityEngine {
    // 10. Sink / Pollution --------------------------------------------------
    /// Detect prototype-pollution keys in a JSON value and dangerous JS sinks in
    /// an optional script snippet.
    pub fn detect_sinks(
        &self,
        json_input: &serde_json::Value,
        script: Option<&str>,
    ) -> Vec<SinkFinding> {
        let mut findings = Vec::new();
        Self::scan_pollution(json_input, "$", &mut findings);
        if let Some(js) = script {
            for sink in [
                "innerHTML",
                "outerHTML",
                "document.write",
                "eval(",
                "setTimeout(",
                "dangerouslySetInnerHTML",
                "insertAdjacentHTML",
            ] {
                if js.contains(sink) {
                    findings.push(SinkFinding {
                        kind: "dom-xss-sink".into(),
                        location: sink.to_string(),
                        severity: if sink == "eval(" {
                            "critical".into()
                        } else {
                            "high".into()
                        },
                        detail: format!(
                            "dangerous sink `{sink}` present; verify input is sanitized"
                        ),
                    });
                }
            }
        }
        findings
    }

    fn scan_pollution(value: &serde_json::Value, path: &str, out: &mut Vec<SinkFinding>) {
        if let serde_json::Value::Object(obj) = value {
            for (k, v) in obj {
                if matches!(k.as_str(), "__proto__" | "constructor" | "prototype") {
                    out.push(SinkFinding {
                        kind: "prototype-pollution".into(),
                        location: format!("{path}.{k}"),
                        severity: "critical".into(),
                        detail: format!("dangerous key `{k}` in object payload"),
                    });
                }
                Self::scan_pollution(v, &format!("{path}.{k}"), out);
            }
        } else if let serde_json::Value::Array(arr) = value {
            for (i, v) in arr.iter().enumerate() {
                Self::scan_pollution(v, &format!("{path}[{i}]"), out);
            }
        }
    }
}
