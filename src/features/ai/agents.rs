use super::{
    AgentStep, AgentTraceReport, AiEngine, ContextHeatmap, ContextSegment, ContextSegmentUsage,
};
use std::collections::BTreeMap;

impl AiEngine {
    // 7. Agent Trace --------------------------------------------------------
    pub fn analyze_agent_trace(&self, steps: &[AgentStep]) -> AgentTraceReport {
        let mut tool_calls: BTreeMap<String, usize> = BTreeMap::new();
        let mut action_sigs: Vec<String> = Vec::new();
        for s in steps {
            if s.kind == "action" {
                if let Some(tool) = &s.tool {
                    *tool_calls.entry(tool.clone()).or_default() += 1;
                }
                action_sigs.push(format!(
                    "{}::{}",
                    s.tool.clone().unwrap_or_default(),
                    s.content.trim()
                ));
            }
        }
        // Loop detection: same (tool, content) action repeated consecutively.
        let mut max_repeat = 1usize;
        let mut cur = 1usize;
        for w in action_sigs.windows(2) {
            if w[0] == w[1] {
                cur += 1;
                max_repeat = max_repeat.max(cur);
            } else {
                cur = 1;
            }
        }
        if action_sigs.is_empty() {
            max_repeat = 0;
        }
        AgentTraceReport {
            step_count: steps.len(),
            tool_calls,
            detected_loop: max_repeat >= 3,
            max_repeated_action: max_repeat,
        }
    }

    // 8. Context Heatmap ----------------------------------------------------
    pub fn context_heatmap(
        &self,
        window_tokens: usize,
        segments: &[ContextSegment],
    ) -> ContextHeatmap {
        let mut used = 0usize;
        let mut seg_usage = Vec::new();
        let mut truncated = Vec::new();
        for seg in segments {
            let fits = used + seg.tokens <= window_tokens;
            if fits {
                used += seg.tokens;
            } else {
                truncated.push(seg.label.clone());
            }
            seg_usage.push(ContextSegmentUsage {
                label: seg.label.clone(),
                tokens: seg.tokens,
                percent: if window_tokens > 0 {
                    seg.tokens as f64 / window_tokens as f64 * 100.0
                } else {
                    0.0
                },
                fits,
            });
        }
        ContextHeatmap {
            window_tokens,
            used_tokens: used,
            utilization_pct: if window_tokens > 0 {
                used as f64 / window_tokens as f64 * 100.0
            } else {
                0.0
            },
            segments: seg_usage,
            truncated,
        }
    }
}
