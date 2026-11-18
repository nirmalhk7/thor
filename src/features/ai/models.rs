use super::{
    jaccard, mock_signature, AiEngine, ModelComparisonRow, ModelEndpoint, ModelRun, RouterDecision,
    RouterDecisionReport,
};
use crate::dx::llm_studio::LlmStudio;
use std::collections::BTreeMap;
use std::time::Duration;

impl AiEngine {
    // 6. Model Comparison ---------------------------------------------------
    pub fn compare_models(&self, runs: &[ModelRun]) -> Vec<ModelComparisonRow> {
        let baseline = runs.first().map(|r| r.output.clone()).unwrap_or_default();
        runs.iter()
            .map(|r| {
                let est = LlmStudio::estimate_cost(&r.model, r.prompt_tokens, r.completion_tokens);
                ModelComparisonRow {
                    model: r.model.clone(),
                    cost_usd: est.total_cost_usd,
                    latency_ms: r.latency_ms,
                    tokens: r.prompt_tokens + r.completion_tokens,
                    similarity_to_first: jaccard(&baseline, &r.output),
                }
            })
            .collect()
    }

    /// Fan out the same prompt to multiple model endpoints in parallel and
    /// collect a [`ModelRun`] per endpoint. When a mock is recorded for an
    /// endpoint's signature (see [`AiEngine::record_mock`]) it is replayed
    /// instead of hitting the network, so the comparison is testable offline.
    pub fn compare_models_fanout(&self, endpoints: &[ModelEndpoint]) -> Vec<ModelRun> {
        // Resolve mocks up front (cheap, lock-free path for the offline case).
        let mut prefilled: Vec<Option<ModelRun>> = Vec::with_capacity(endpoints.len());
        let mut live: Vec<usize> = Vec::new();
        for (i, ep) in endpoints.iter().enumerate() {
            let sig = mock_signature(&ep.model, &ep.body);
            if let Some(output) = self.replay_mock(&sig) {
                let prompt_tokens = self.inspect_request(&ep.body).prompt_tokens;
                let completion_tokens = LlmStudio::count_tokens(&output);
                prefilled.push(Some(ModelRun {
                    model: ep.model.clone(),
                    output,
                    prompt_tokens,
                    completion_tokens,
                    latency_ms: 0.0,
                }));
            } else {
                prefilled.push(None);
                live.push(i);
            }
        }

        if !live.is_empty() {
            fill_live_runs(endpoints, &live, &mut prefilled);
        }

        prefilled
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                r.unwrap_or_else(|| ModelRun {
                    model: endpoints[i].model.clone(),
                    output: String::new(),
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    latency_ms: 0.0,
                })
            })
            .collect()
    }

    pub fn router_decision_trace(&self, decisions: &[RouterDecision]) -> RouterDecisionReport {
        let mut model_histogram: BTreeMap<String, usize> = BTreeMap::new();
        let mut low_confidence = 0usize;
        let mut issues = Vec::new();
        for d in decisions {
            *model_histogram.entry(d.chosen_model.clone()).or_default() += 1;
            if d.conf < 0.5 {
                low_confidence += 1;
                issues.push(format!(
                    "low conf {:.2} chose {} ({})",
                    d.conf, d.chosen_model, d.reason
                ));
            }
            if !d.candidates.is_empty() && !d.candidates.iter().any(|c| c == &d.chosen_model) {
                issues.push(format!(
                    "chosen {} not in candidates {:?}",
                    d.chosen_model, d.candidates
                ));
            }
        }
        RouterDecisionReport {
            decisions: decisions.len(),
            low_confidence,
            model_histogram,
            issues,
        }
    }
}

async fn fetch_model_run(client: &reqwest::Client, ep: &ModelEndpoint) -> ModelRun {
    let start = std::time::Instant::now();
    let mut req = client.post(&ep.url).json(&ep.body);
    if let Some(key) = &ep.api_key {
        req = req.bearer_auth(key);
    }
    let (output, completion_tokens) = match req.send().await {
        Ok(resp) => {
            let json: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            let text = extract_completion(&json);
            let ct = json
                .get("usage")
                .and_then(|u| u.get("completion_tokens"))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or_else(|| LlmStudio::count_tokens(&text));
            (text, ct)
        }
        Err(e) => (format!("<error: {e}>"), 0),
    };
    let prompt_tokens = ep
        .body
        .get("messages")
        .and_then(|m| m.as_array())
        .map(|msgs| {
            msgs.iter()
                .filter_map(|m| m.get("content").and_then(|c| c.as_str()))
                .map(LlmStudio::count_tokens)
                .sum()
        })
        .unwrap_or(0);
    ModelRun {
        model: ep.model.clone(),
        output,
        prompt_tokens,
        completion_tokens,
        latency_ms: start.elapsed().as_secs_f64() * 1000.0,
    }
}

/// Pull the assistant text out of an OpenAI/Anthropic-style completion response.
fn extract_completion(json: &serde_json::Value) -> String {
    if let Some(text) = json
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| {
            c.get("message")
                .and_then(|m| m.get("content"))
                .and_then(|v| v.as_str())
                .or_else(|| c.get("text").and_then(|v| v.as_str()))
        })
    {
        return text.to_string();
    }
    if let Some(text) = json
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("text"))
        .and_then(|v| v.as_str())
    {
        return text.to_string();
    }
    String::new()
}

fn fill_live_runs(endpoints: &[ModelEndpoint], live: &[usize], prefilled: &mut [Option<ModelRun>]) {
    if let Ok(rt) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        let results = rt.block_on(async {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
            let mut handles = Vec::new();
            for &i in live {
                let ep = endpoints[i].clone();
                let client = client.clone();
                handles.push(tokio::spawn(async move {
                    (i, fetch_model_run(&client, &ep).await)
                }));
            }
            let mut out = Vec::new();
            for h in handles {
                if let Ok(pair) = h.await {
                    out.push(pair);
                }
            }
            out
        });
        for (i, run) in results {
            prefilled[i] = Some(run);
        }
    }
}
