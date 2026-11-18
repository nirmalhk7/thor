use crate::load::scenario::{ScenarioStep, ThorScenario};

pub struct ScenarioCompiler;

impl ScenarioCompiler {
    /// Validate and compile a ThorScenario for execution
    pub fn compile(mut scenario: ThorScenario) -> Result<ThorScenario, String> {
        if scenario.vus == 0 {
            scenario.vus = 1;
        }
        if scenario.duration_secs == 0 {
            scenario.duration_secs = 5;
        }
        if scenario.target_url.is_empty() {
            return Err("Scenario target_url cannot be empty".to_string());
        }

        // Ensure at least 1 step exists if steps are empty
        if scenario.steps.is_empty() {
            scenario.steps.push(ScenarioStep::Navigate {
                url: scenario.target_url.clone(),
            });
        }

        for (idx, step) in scenario.steps.iter().enumerate() {
            validate_step(idx, step)?;
        }

        Ok(scenario)
    }

    /// Parse JSON and compile into validated ThorScenario
    pub fn compile_from_json(json_str: &str) -> Result<ThorScenario, String> {
        let raw: ThorScenario = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse scenario JSON: {}", e))?;
        Self::compile(raw)
    }

    /// Compile network trace endpoints into a ThorScenario
    pub fn compile_from_network_traces(
        name: &str,
        target_url: &str,
        traces: &[(String, String, Option<String>)],
    ) -> Result<ThorScenario, String> {
        let mut scenario = ThorScenario::new(name, target_url);
        for (method, url, body) in traces {
            scenario = scenario.add_step(ScenarioStep::HttpRequest {
                method: method.clone(),
                url: url.clone(),
                headers: std::collections::HashMap::new(),
                body: body.clone(),
            });
        }
        Self::compile(scenario)
    }
}

fn validate_step(idx: usize, step: &ScenarioStep) -> Result<(), String> {
    let error = match step {
        ScenarioStep::Navigate { url } => url.is_empty().then_some("Navigate URL cannot be empty"),
        ScenarioStep::HttpRequest { method, url, .. } => {
            (method.is_empty() || url.is_empty()).then_some("HttpRequest requires method and url")
        }
        ScenarioStep::Click { selector } => selector
            .is_empty()
            .then_some("Click selector cannot be empty"),
        ScenarioStep::Fill { selector, .. } => selector
            .is_empty()
            .then_some("Fill selector cannot be empty"),
        ScenarioStep::WebSocketConnect { url, .. } => {
            url.is_empty().then_some("WebSocket URL cannot be empty")
        }
        ScenarioStep::Wait { .. } | ScenarioStep::CustomScript { .. } => None,
    };
    match error {
        Some(message) => Err(format!("Step {}: {}", idx, message)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_scenario() {
        let s = ThorScenario::new("Test Run", "https://example.com");
        let compiled = ScenarioCompiler::compile(s).unwrap();
        assert_eq!(compiled.vus, 1);
        assert_eq!(compiled.steps.len(), 1);
    }
}
