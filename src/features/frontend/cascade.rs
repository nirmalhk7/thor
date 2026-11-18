use super::{CascadeWinner, CssDeclaration, FrontendEngine, SpecificityBudgetReport};
use std::collections::BTreeMap;

impl FrontendEngine {
    // 3. CSS Cascade --------------------------------------------------------
    pub fn resolve_cascade(&self, declarations: &[CssDeclaration]) -> Vec<CascadeWinner> {
        let mut by_prop: BTreeMap<String, Vec<&CssDeclaration>> = BTreeMap::new();
        for d in declarations {
            by_prop.entry(d.property.clone()).or_default().push(d);
        }
        let mut winners = Vec::new();
        for (property, decls) in by_prop {
            let mut best: Option<&CssDeclaration> = None;
            let mut best_key = (0u8, [0u32; 3], 0usize);
            for d in &decls {
                let spec = Self::specificity(&d.selector);
                let key = (d.important as u8, spec, d.order);
                if best.is_none() || key > best_key {
                    best = Some(d);
                    best_key = key;
                }
            }
            if let Some(w) = best {
                let overridden = decls
                    .iter()
                    .filter(|d| d.value != w.value || d.selector != w.selector)
                    .map(|d| format!("{} {{ {}: {} }}", d.selector, d.property, d.value))
                    .collect();
                winners.push(CascadeWinner {
                    property,
                    winning_value: w.value.clone(),
                    winning_selector: w.selector.clone(),
                    specificity: Self::specificity(&w.selector),
                    important: w.important,
                    overridden,
                });
            }
        }
        winners
    }

    /// Compute (id, class/attr/pseudo-class, type/pseudo-element) specificity.
    pub fn specificity(selector: &str) -> [u32; 3] {
        let mut total = [0, 0, 0];
        for token in selector
            .split([' ', '>', '+', '~'])
            .filter(|t| !t.is_empty())
        {
            let score = token_specificity(token);
            for i in 0..3 {
                total[i] += score[i];
            }
        }
        total
    }

    fn is_ident(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '-' || c == '_'
    }

    pub fn selector_specificity_budget(
        &self,
        selectors: &[String],
        budget: [u32; 3],
    ) -> SpecificityBudgetReport {
        let mut over = Vec::new();
        let mut max_spec = [0u32; 3];
        let mut score_sum = 0.0f64;
        for sel in selectors {
            let spec = Self::specificity(sel);
            let score = spec[0] as f64 * 100.0 + spec[1] as f64 * 10.0 + spec[2] as f64;
            score_sum += score;
            if spec[0] > max_spec[0]
                || (spec[0] == max_spec[0] && spec[1] > max_spec[1])
                || (spec[0] == max_spec[0] && spec[1] == max_spec[1] && spec[2] > max_spec[2])
            {
                max_spec = spec;
            }
            let over_budget = spec[0] > budget[0]
                || (spec[0] == budget[0] && spec[1] > budget[1])
                || (spec[0] == budget[0] && spec[1] == budget[1] && spec[2] > budget[2]);
            if over_budget {
                over.push(format!("{sel} {:?}", spec));
            }
        }
        let average_score = if selectors.is_empty() {
            0.0
        } else {
            score_sum / selectors.len() as f64
        };
        SpecificityBudgetReport {
            selectors_over_budget: over,
            max_specificity: max_spec,
            budget,
            average_score,
        }
    }
}

fn token_specificity(token: &str) -> [u32; 3] {
    let mut ids = 0;
    let mut classes = 0;
    let mut types = 0;
    let mut chars = token.chars().peekable();
    let mut buf = String::new();
    let flush = |buf: &mut String, types: &mut u32| {
        if !buf.is_empty() && buf != "*" {
            *types += 1;
        }
        buf.clear();
    };
    while let Some(c) = chars.next() {
        match c {
            '#' => {
                flush(&mut buf, &mut types);
                ids += 1;
                while chars
                    .peek()
                    .map(|c| FrontendEngine::is_ident(*c))
                    .unwrap_or(false)
                {
                    chars.next();
                }
            }
            '.' | '[' | ':' => {
                flush(&mut buf, &mut types);
                classes += 1;
                while chars
                    .peek()
                    .map(|c| FrontendEngine::is_ident(*c) || *c == '=' || *c == '"' || *c == ']')
                    .unwrap_or(false)
                {
                    chars.next();
                }
            }
            _ => buf.push(c),
        }
    }
    flush(&mut buf, &mut types);

    [ids, classes, types]
}
