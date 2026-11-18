use super::{json_path, BackendEngine, ChainRunResult, ChainStep, ChainStepResult, HashMap};

impl BackendEngine {
    /// Execute a request chain end-to-end: resolve variables, send each step for
    /// real, capture response fields into the variable map, and thread them into
    /// downstream steps.
    pub fn run_chain(&self, steps: &[ChainStep]) -> Result<ChainRunResult, String> {
        self.resolve_chain(steps)?;
        let mut vars: HashMap<String, String> = HashMap::new();
        let mut results = Vec::new();
        let mut ok = true;
        for step in steps {
            let req = self.build_request(step.request.clone(), &vars);
            let resp = self.send_blocking(&req)?;
            let mut captured = Vec::new();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body) {
                self.apply_captures(step, &json, &mut vars);
                captured = step.captures.keys().cloned().collect();
            }
            if !(200..300).contains(&resp.status) {
                ok = false;
            }
            results.push(ChainStepResult {
                name: step.name.clone(),
                status: resp.status,
                latency_ms: resp.latency_ms,
                captured,
            });
        }
        Ok(ChainRunResult {
            steps: results,
            vars,
            ok,
        })
    }

    // 6. Request Chaining ---------------------------------------------------
    /// Statically resolve which variables each step captures and validate that
    /// every `{{var}}` referenced downstream is produced by an earlier step.
    pub fn resolve_chain(&self, steps: &[ChainStep]) -> Result<Vec<String>, String> {
        let mut available: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut order = Vec::new();
        for step in steps {
            for var in referenced_vars(&step.request.url).into_iter().chain(
                step.request
                    .body
                    .as_deref()
                    .map(referenced_vars)
                    .unwrap_or_default(),
            ) {
                if !available.contains(&var) {
                    return Err(format!(
                        "step '{}' references '{{{{{}}}}}' before it is captured",
                        step.name, var
                    ));
                }
            }
            for key in step.captures.keys() {
                available.insert(key.clone());
            }
            order.push(step.name.clone());
        }
        Ok(order)
    }

    /// Apply captured variables from a JSON response into the variable map.
    pub fn apply_captures(
        &self,
        step: &ChainStep,
        response_json: &serde_json::Value,
        vars: &mut HashMap<String, String>,
    ) {
        for (var, path) in &step.captures {
            if let Some(v) = json_path(response_json, path) {
                let s = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                vars.insert(var.clone(), s);
            }
        }
    }
}
fn referenced_vars(input: &str) -> Vec<String> {
    let mut vars = Vec::new();
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        if let Some(end) = rest[start + 2..].find("}}") {
            let name = rest[start + 2..start + 2 + end].trim().to_string();
            if !name.is_empty() {
                vars.push(name);
            }
            rest = &rest[start + 2 + end + 2..];
        } else {
            break;
        }
    }
    vars
}
