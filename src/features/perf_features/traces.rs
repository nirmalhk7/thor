use super::{
    BTreeMap, BlockingChain, BlockingChainReport, InitiatedResource, MetricDelta, PerfEngine,
    ResourceTiming, WaterfallReport,
};

impl PerfEngine {
    // 2. Trace Diff ---------------------------------------------------------
    pub fn diff_traces(
        &self,
        before: &BTreeMap<String, f64>,
        after: &BTreeMap<String, f64>,
        regression_threshold_pct: f64,
    ) -> Vec<MetricDelta> {
        let mut keys: Vec<&String> = before.keys().chain(after.keys()).collect();
        keys.sort();
        keys.dedup();
        let mut deltas = Vec::new();
        for k in keys {
            let b = *before.get(k).unwrap_or(&0.0);
            let a = *after.get(k).unwrap_or(&0.0);
            let delta_pct = if b.abs() > f64::EPSILON {
                (a - b) / b * 100.0
            } else if a != 0.0 {
                100.0
            } else {
                0.0
            };
            let kind = if delta_pct > regression_threshold_pct {
                "regression"
            } else if delta_pct < -regression_threshold_pct {
                "improvement"
            } else {
                "stable"
            };
            deltas.push(MetricDelta {
                metric: k.clone(),
                before: b,
                after: a,
                delta_pct,
                kind: kind.to_string(),
            });
        }
        deltas
    }

    // 3. Waterfall Forensics -----------------------------------------------
    pub fn analyze_waterfall(&self, resources: &[ResourceTiming]) -> WaterfallReport {
        let total_ms = resources.iter().map(|r| r.end_ms()).fold(0.0, f64::max);
        let mut by_dur: Vec<&ResourceTiming> = resources.iter().collect();
        by_dur.sort_by(|a, b| {
            b.total_ms()
                .partial_cmp(&a.total_ms())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let slowest = by_dur
            .iter()
            .take(5)
            .map(|r| format!("{} ({:.0}ms)", r.url, r.total_ms()))
            .collect();
        let render_blocking = resources
            .iter()
            .filter(|r| r.render_blocking)
            .map(|r| r.url.clone())
            .collect();
        let total_bytes = resources.iter().map(|r| r.bytes).sum();
        WaterfallReport {
            total_ms,
            slowest,
            render_blocking,
            total_bytes,
        }
    }

    /// Reconstruct the request blocking-chain (initiator graph) from resources
    /// carrying an `initiator` URL, then find the longest cumulative-latency
    /// root→leaf path — the network critical path.
    pub fn blocking_chain(&self, resources: &[InitiatedResource]) -> BlockingChainReport {
        use std::collections::HashMap;
        let dur: HashMap<&str, f64> = resources
            .iter()
            .map(|r| (r.url.as_str(), r.total_ms))
            .collect();
        let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut children: HashMap<String, Vec<String>> = HashMap::new();
        let known: std::collections::HashSet<&str> =
            resources.iter().map(|r| r.url.as_str()).collect();
        for r in resources {
            if let Some(init) = &r.initiator {
                edges.entry(init.clone()).or_default().push(r.url.clone());
                children
                    .entry(init.clone())
                    .or_default()
                    .push(r.url.clone());
            }
        }
        // Roots: resources whose initiator is unknown/absent.
        let roots: Vec<&InitiatedResource> = resources
            .iter()
            .filter(|r| {
                r.initiator
                    .as_deref()
                    .map(|i| !known.contains(i))
                    .unwrap_or(true)
            })
            .collect();
        let mut best: Vec<String> = Vec::new();
        let mut best_ms = 0.0f64;
        for root in roots {
            let mut path = Vec::new();
            let ms = longest_chain(&root.url, &children, &dur, &mut path);
            if ms > best_ms {
                best_ms = ms;
                best = path;
            }
        }
        BlockingChainReport {
            edges,
            longest_chain: BlockingChain {
                chain: best,
                cumulative_ms: best_ms,
            },
            critical_path_ms: best_ms,
        }
    }
}
/// Longest cumulative-latency path from `node` through the initiator graph.
fn longest_chain(
    node: &str,
    children: &std::collections::HashMap<String, Vec<String>>,
    dur: &std::collections::HashMap<&str, f64>,
    path: &mut Vec<String>,
) -> f64 {
    path.push(node.to_string());
    let self_ms = dur.get(node).copied().unwrap_or(0.0);
    let mut best_child_ms = 0.0;
    let mut best_child_path: Vec<String> = Vec::new();
    if let Some(kids) = children.get(node) {
        for k in kids {
            let mut cp = Vec::new();
            let ms = longest_chain(k, children, dur, &mut cp);
            if ms > best_child_ms {
                best_child_ms = ms;
                best_child_path = cp;
            }
        }
    }
    path.extend(best_child_path);
    self_ms + best_child_ms
}
