use super::{ElementSignals, LocatorCandidate, QaEngine, RankedLocator};

impl QaEngine {
    // 1. Resilient Locators -------------------------------------------------
    /// Rank locator strategies by resilience (test-id > role+name > label >
    /// text > id > css) and return the ordered candidate list.
    pub fn rank_locators(&self, signals: &ElementSignals) -> Vec<RankedLocator> {
        let mut ranked = Vec::new();
        let mut push = |expr: Option<String>, strategy: &str, score: u32| {
            if let Some(e) = expr {
                ranked.push(RankedLocator {
                    expression: e,
                    strategy: strategy.to_string(),
                    resilience_score: score,
                });
            }
        };
        let cand = LocatorCandidate {
            test_id: signals.test_id.clone(),
            role: signals.role.clone(),
            name: signals.accessible_name.clone(),
            label: signals.label.clone(),
            text: signals.text.clone(),
            css: signals.css.clone(),
        };
        push(
            cand.test_id.as_ref().and_then(|_| {
                LocatorCandidate {
                    test_id: cand.test_id.clone(),
                    ..Default::default()
                }
                .to_playwright()
            }),
            "test-id",
            100,
        );
        if signals.role.is_some() {
            let rc = LocatorCandidate {
                role: cand.role.clone(),
                name: cand.name.clone(),
                ..Default::default()
            };
            push(
                rc.to_playwright(),
                "role",
                if signals.accessible_name.is_some() {
                    90
                } else {
                    70
                },
            );
        }
        push(
            signals.label.as_ref().and_then(|_| {
                LocatorCandidate {
                    label: cand.label.clone(),
                    ..Default::default()
                }
                .to_playwright()
            }),
            "label",
            80,
        );
        push(
            signals.text.as_ref().and_then(|_| {
                LocatorCandidate {
                    text: cand.text.clone(),
                    ..Default::default()
                }
                .to_playwright()
            }),
            "text",
            60,
        );
        if let Some(id) = &signals.id {
            push(Some(format!("page.locator('#{id}')")), "id", 50);
        }
        push(
            signals.css.as_ref().and_then(|_| {
                LocatorCandidate {
                    css: cand.css.clone(),
                    ..Default::default()
                }
                .to_playwright()
            }),
            "css",
            20,
        );
        ranked.sort_by_key(|item| std::cmp::Reverse(item.resilience_score));
        ranked
    }
}
