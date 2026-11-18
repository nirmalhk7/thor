use super::{AriaLiveEvent, AriaLiveReport, FocusOrderReport, FocusableNode, FrontendEngine};
use std::collections::BTreeMap;

impl FrontendEngine {
    /// Parse JSON produced by [`focus_order_probe_script`].
    pub fn parse_focus_order_probe(json: &str) -> Option<Vec<FocusableNode>> {
        let trimmed = json.trim();
        if let Ok(inner) = serde_json::from_str::<String>(trimmed) {
            return serde_json::from_str(&inner).ok();
        }
        serde_json::from_str(trimmed).ok()
    }

    pub fn focus_order_overlay(&self, nodes: &[FocusableNode]) -> FocusOrderReport {
        let mut eligible: Vec<&FocusableNode> =
            nodes.iter().filter(|n| n.visible && !n.disabled).collect();
        // HTML: positive tabindex (ascending) first, then tabindex 0 in DOM order.
        eligible.sort_by(|a, b| {
            let a_pos = a.tab_index > 0;
            let b_pos = b.tab_index > 0;
            match (a_pos, b_pos) {
                (true, true) => a
                    .tab_index
                    .cmp(&b.tab_index)
                    .then_with(|| a.order_hint.cmp(&b.order_hint)),
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                (false, false) => a.order_hint.cmp(&b.order_hint),
            }
        });
        let sequence: Vec<String> = eligible.iter().map(|n| n.selector.clone()).collect();
        let mut issues = Vec::new();
        let positive: Vec<_> = nodes.iter().filter(|n| n.tab_index > 0).collect();
        if positive.len() > 1 {
            issues.push(format!(
                "{} elements use positive tabindex (prefer natural DOM order)",
                positive.len()
            ));
        }
        for n in nodes {
            if n.tab_index < -1 {
                issues.push(format!(
                    "{} has invalid tabindex {}",
                    n.selector, n.tab_index
                ));
            }
            if !n.visible && n.tab_index >= 0 {
                issues.push(format!("{} is focusable but not visible", n.selector));
            }
        }
        FocusOrderReport { sequence, issues }
    }

    pub fn aria_live_monitor(&self, events: &[AriaLiveEvent]) -> AriaLiveReport {
        let mut regions: BTreeMap<String, usize> = BTreeMap::new();
        let mut assertive_bursts = 0usize;
        let mut empty = 0usize;
        let mut issues = Vec::new();
        let mut last_assertive_ms: Option<u64> = None;
        for e in events {
            *regions.entry(e.region.clone()).or_default() += 1;
            if e.text.trim().is_empty() {
                empty += 1;
                issues.push(format!("empty announcement in {}", e.region));
            }
            if e.politeness == "assertive" {
                if let Some(prev) = last_assertive_ms {
                    if e.timestamp_ms.saturating_sub(prev) < 500 {
                        assertive_bursts += 1;
                    }
                }
                last_assertive_ms = Some(e.timestamp_ms);
            }
        }
        if assertive_bursts > 0 {
            issues.push(format!(
                "{assertive_bursts} assertive announcements within 500ms windows"
            ));
        }
        AriaLiveReport {
            regions: regions.len(),
            assertive_bursts,
            empty_announcements: empty,
            issues,
        }
    }
}
