use super::{BTreeMap, ContinuousProfileReport, FlameNode, PerfEngine, StackSample};

impl PerfEngine {
    /// Export a folded-stack profile as speedscope-compatible JSON (evented
    /// profile), so a flame graph can be opened directly in speedscope.app.
    pub fn promote_to_profile(&self, folded: &[String], name: &str) -> serde_json::Value {
        // Build a frame table + open/close event pairs from folded stacks.
        let mut frames: Vec<String> = Vec::new();
        let mut frame_index = std::collections::HashMap::new();
        let mut events = Vec::new();
        let mut at: u64 = 0;
        let intern = |frame: &str,
                      frames: &mut Vec<String>,
                      idx: &mut std::collections::HashMap<String, usize>|
         -> usize {
            if let Some(i) = idx.get(frame) {
                *i
            } else {
                let i = frames.len();
                frames.push(frame.to_string());
                idx.insert(frame.to_string(), i);
                i
            }
        };
        for line in folded {
            let (stack, value) = match line.rsplit_once(' ') {
                Some((s, v)) => (s, v.trim().parse::<u64>().unwrap_or(0)),
                None => continue,
            };
            let stack_frames: Vec<usize> = stack
                .split(';')
                .filter(|f| !f.is_empty())
                .map(|f| intern(f, &mut frames, &mut frame_index))
                .collect();
            for &f in &stack_frames {
                events.push(serde_json::json!({ "type": "O", "frame": f, "at": at }));
            }
            at += value;
            for &f in stack_frames.iter().rev() {
                events.push(serde_json::json!({ "type": "C", "frame": f, "at": at }));
            }
        }
        let shared_frames: Vec<serde_json::Value> = frames
            .iter()
            .map(|f| serde_json::json!({ "name": f }))
            .collect();
        serde_json::json!({
            "$schema": "https://www.speedscope.app/file-format-schema.json",
            "shared": { "frames": shared_frames },
            "profiles": [{
                "type": "evented",
                "name": name,
                "unit": "none",
                "startValue": 0,
                "endValue": at,
                "events": events,
            }],
            "name": name,
            "exporter": "Thor",
        })
    }

    // 11. Flame Graph -------------------------------------------------------
    /// Build a flame graph tree from folded stacks (`a;b;c value`).
    pub fn build_flame_graph(&self, folded: &[String]) -> FlameNode {
        let mut root = FlameNode {
            frame: "root".to_string(),
            value: 0,
            children: Vec::new(),
        };
        for line in folded {
            let (stack, value) = match line.rsplit_once(' ') {
                Some((s, v)) => (s, v.trim().parse::<u64>().unwrap_or(0)),
                None => continue,
            };
            root.value += value;
            let mut node = &mut root;
            for frame in stack.split(';').filter(|f| !f.is_empty()) {
                let idx = match node.children.iter().position(|c| c.frame == frame) {
                    Some(i) => i,
                    None => {
                        node.children.push(FlameNode {
                            frame: frame.to_string(),
                            value: 0,
                            children: Vec::new(),
                        });
                        node.children.len() - 1
                    }
                };
                node = &mut node.children[idx];
                node.value += value;
            }
        }
        root
    }

    // 24. Continuous Profiler -----------------------------------------------
    pub fn continuous_profile(&self, samples: &[StackSample]) -> ContinuousProfileReport {
        let mut folded_map: BTreeMap<String, u64> = BTreeMap::new();
        let mut total_weight = 0u64;
        for s in samples {
            if s.stack.is_empty() {
                continue;
            }
            let key = s.stack.join(";");
            *folded_map.entry(key).or_default() += s.weight.max(1);
            total_weight += s.weight.max(1);
        }
        let folded: Vec<String> = folded_map.iter().map(|(k, v)| format!("{k} {v}")).collect();
        let flame = self.build_flame_graph(&folded);
        let speedscope = self.promote_to_profile(&folded, "thor-continuous-profile");
        ContinuousProfileReport {
            samples: samples.len(),
            total_weight,
            folded,
            flame,
            speedscope,
        }
    }
}
