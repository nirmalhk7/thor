use super::{A11yMatrix, A11yMatrixCell, A11yViolation, DomNode, FrontendEngine};

impl FrontendEngine {
    // 5. Accessibility Lens -------------------------------------------------
    pub fn accessibility_audit(&self, nodes: &[DomNode]) -> Vec<A11yViolation> {
        let mut violations = Vec::new();
        for n in nodes {
            Self::a11y_node(n, &mut violations);
        }
        violations
    }

    /// Build a rule × node accessibility matrix: for every captured node and
    /// every audited rule, mark whether that node violates the rule.
    pub fn a11y_matrix(&self, nodes: &[DomNode]) -> A11yMatrix {
        let rules = vec![
            "image-alt".to_string(),
            "control-name".to_string(),
            "form-label".to_string(),
            "color-contrast".to_string(),
        ];
        let violations = self.accessibility_audit(nodes);
        let mut node_selectors: Vec<String> = Vec::new();
        for n in nodes {
            Self::collect_selectors(n, &mut node_selectors);
        }
        node_selectors.dedup();
        let mut cells = Vec::new();
        for node in &node_selectors {
            for rule in &rules {
                let hit = violations
                    .iter()
                    .find(|v| &v.selector == node && &v.rule == rule);
                cells.push(A11yMatrixCell {
                    rule: rule.clone(),
                    node: node.clone(),
                    violated: hit.is_some(),
                    message: hit.map(|v| v.message.clone()).unwrap_or_default(),
                });
            }
        }
        A11yMatrix {
            rules,
            nodes: node_selectors,
            cells,
            total_violations: violations.len(),
        }
    }

    fn collect_selectors(n: &DomNode, out: &mut Vec<String>) {
        out.push(Self::node_selector(n));
        for c in &n.children {
            Self::collect_selectors(c, out);
        }
    }

    fn a11y_node(n: &DomNode, out: &mut Vec<A11yViolation>) {
        let tag = n.tag.to_lowercase();
        let selector = Self::node_selector(n);
        match tag.as_str() {
            "img" => {
                if !n.attributes.contains_key("alt") {
                    out.push(A11yViolation {
                        rule: "image-alt".into(),
                        selector,
                        severity: "serious".into(),
                        message: "<img> missing alt attribute".into(),
                    });
                }
            }
            "button" | "a" => {
                let has_text = n
                    .text
                    .as_deref()
                    .map(|t| !t.trim().is_empty())
                    .unwrap_or(false);
                let has_label = n.attributes.contains_key("aria-label")
                    || n.attributes.contains_key("aria-labelledby")
                    || n.attributes.contains_key("title");
                if !has_text && !has_label {
                    out.push(A11yViolation {
                        rule: "control-name".into(),
                        selector,
                        severity: "critical".into(),
                        message: format!("<{tag}> has no accessible name"),
                    });
                }
            }
            "input" | "select" | "textarea" => {
                let labelled = n.attributes.contains_key("aria-label")
                    || n.attributes.contains_key("aria-labelledby")
                    || n.attributes.contains_key("title")
                    || n.attributes
                        .get("type")
                        .map(|t| t == "hidden")
                        .unwrap_or(false);
                if !labelled {
                    out.push(A11yViolation {
                        rule: "form-label".into(),
                        selector,
                        severity: "serious".into(),
                        message: format!("<{tag}> is not associated with a label"),
                    });
                }
            }
            _ => {}
        }
        // Contrast check when explicit colors captured.
        if let (Some(fg), Some(bg)) = (n.attributes.get("data-color"), n.attributes.get("data-bg"))
        {
            if let Some(ratio) = Self::contrast_ratio(fg, bg) {
                if ratio < 4.5 {
                    out.push(A11yViolation {
                        rule: "color-contrast".into(),
                        selector: Self::node_selector(n),
                        severity: "serious".into(),
                        message: format!("contrast ratio {ratio:.2}:1 below 4.5:1"),
                    });
                }
            }
        }
        for c in &n.children {
            Self::a11y_node(c, out);
        }
    }

    fn node_selector(n: &DomNode) -> String {
        if let Some(id) = n.attributes.get("id") {
            format!("{}#{}", n.tag, id)
        } else if let Some(class) = n.attributes.get("class") {
            format!(
                "{}.{}",
                n.tag,
                class.split_whitespace().next().unwrap_or("")
            )
        } else {
            n.tag.clone()
        }
    }
}
