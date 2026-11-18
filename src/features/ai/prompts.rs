use super::{now_ms, AiEngine, PromptVersion};
use crate::dx::llm_studio::LlmStudio;

impl AiEngine {
    // Prompt version timeline ----------------------------------------------
    /// Append a new revision of `name`'s prompt, computing its diff against the
    /// previous revision. Returns the stored [`PromptVersion`].
    pub fn record_prompt_version(&self, name: &str, content: &str) -> PromptVersion {
        let mut timeline = self.prompt_timeline.lock();
        let history = timeline.entry(name.to_string()).or_default();
        let diff_from_prev = history
            .last()
            .map(|prev| self.prompt_diff(&prev.content, content));
        let version = PromptVersion {
            version: history.len() + 1,
            content: content.to_string(),
            tokens: LlmStudio::count_tokens(content),
            timestamp_ms: now_ms(),
            diff_from_prev,
        };
        history.push(version.clone());
        version
    }

    /// Full revision history for a named prompt (oldest first).
    pub fn prompt_timeline(&self, name: &str) -> Vec<PromptVersion> {
        self.prompt_timeline
            .lock()
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    // LLM mock store (record/replay) ---------------------------------------
    /// Record a canned response for a request signature. Use
    /// [`super::mock_signature`] to derive a stable key from `(model, body)`.
    pub fn record_mock(&self, signature: &str, response: &str) {
        self.mock_store
            .lock()
            .insert(signature.to_string(), response.to_string());
    }

    /// Replay a previously recorded response for a signature, if any.
    pub fn replay_mock(&self, signature: &str) -> Option<String> {
        self.mock_store.lock().get(signature).cloned()
    }

    pub fn mock_count(&self) -> usize {
        self.mock_store.lock().len()
    }
}
