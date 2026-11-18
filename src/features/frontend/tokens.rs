use super::{
    CssDeclaration, CssVarUsage, CssVariableReport, DesignTokenReport, FrontendEngine,
    TokenDriftReport, TokenUsage,
};
use std::collections::{BTreeMap, HashMap};

impl FrontendEngine {
    // 4. Design Tokens ------------------------------------------------------
    pub fn extract_design_tokens(&self, declarations: &[CssDeclaration]) -> DesignTokenReport {
        let mut colors: HashMap<String, usize> = HashMap::new();
        let mut fonts: HashMap<String, usize> = HashMap::new();
        let mut spacing: HashMap<String, usize> = HashMap::new();
        let mut radii: HashMap<String, usize> = HashMap::new();
        for d in declarations {
            let prop = d.property.to_lowercase();
            let val = d.value.trim().to_lowercase();
            if (prop.contains("color") || val.starts_with('#') || val.starts_with("rgb"))
                && (val.starts_with('#') || val.starts_with("rgb"))
            {
                *colors.entry(val.clone()).or_default() += 1;
            }
            if prop == "font-size" {
                *fonts.entry(val.clone()).or_default() += 1;
            }
            if prop.starts_with("margin") || prop.starts_with("padding") || prop == "gap" {
                *spacing.entry(val.clone()).or_default() += 1;
            }
            if prop.contains("radius") {
                *radii.entry(val.clone()).or_default() += 1;
            }
        }
        let one_offs = colors
            .iter()
            .chain(fonts.iter())
            .chain(spacing.iter())
            .chain(radii.iter())
            .filter(|(_, c)| **c == 1)
            .map(|(v, _)| v.clone())
            .collect();
        DesignTokenReport {
            colors: Self::to_sorted_usage(colors),
            font_sizes: Self::to_sorted_usage(fonts),
            spacing: Self::to_sorted_usage(spacing),
            radii: Self::to_sorted_usage(radii),
            one_offs,
        }
    }

    /// Compare captured style values against a canonical design-token set,
    /// flagging off-scale one-offs (drift), matched values, and unused tokens.
    pub fn token_drift(
        &self,
        declarations: &[CssDeclaration],
        canonical: &[String],
    ) -> TokenDriftReport {
        let report = self.extract_design_tokens(declarations);
        let canon: std::collections::HashSet<String> =
            canonical.iter().map(|c| c.trim().to_lowercase()).collect();
        let used: Vec<String> = report
            .colors
            .iter()
            .chain(report.font_sizes.iter())
            .chain(report.spacing.iter())
            .chain(report.radii.iter())
            .map(|u| u.value.clone())
            .collect();
        let mut matched = Vec::new();
        let mut off_scale = Vec::new();
        for v in &used {
            if canon.contains(&v.to_lowercase()) {
                matched.push(v.clone());
            } else {
                off_scale.push(v.clone());
            }
        }
        matched.sort();
        matched.dedup();
        off_scale.sort();
        off_scale.dedup();
        let used_set: std::collections::HashSet<String> =
            used.iter().map(|v| v.to_lowercase()).collect();
        let unused_tokens: Vec<String> = canonical
            .iter()
            .filter(|c| !used_set.contains(&c.trim().to_lowercase()))
            .cloned()
            .collect();
        let total = (matched.len() + off_scale.len()).max(1);
        TokenDriftReport {
            coverage: matched.len() as f64 / total as f64,
            off_scale,
            matched,
            unused_tokens,
        }
    }

    fn to_sorted_usage(map: HashMap<String, usize>) -> Vec<TokenUsage> {
        let mut v: Vec<TokenUsage> = map
            .into_iter()
            .map(|(value, count)| TokenUsage { value, count })
            .collect();
        v.sort_by(|a, b| b.count.cmp(&a.count).then(a.value.cmp(&b.value)));
        v
    }

    pub fn css_variable_inspector(
        &self,
        definitions: &BTreeMap<String, String>,
        references: &[String],
    ) -> CssVariableReport {
        let mut used_count: BTreeMap<String, usize> = BTreeMap::new();
        let mut undefined_refs = Vec::new();
        for r in references {
            let name = r
                .trim()
                .trim_start_matches("var(")
                .trim_end_matches(')')
                .trim();
            let name = name.split(',').next().unwrap_or(name).trim();
            if !name.starts_with("--") {
                continue;
            }
            *used_count.entry(name.to_string()).or_default() += 1;
            if !definitions.contains_key(name) {
                undefined_refs.push(name.to_string());
            }
        }
        undefined_refs.sort();
        undefined_refs.dedup();
        let unused_defs: Vec<String> = definitions
            .keys()
            .filter(|k| !used_count.contains_key(*k))
            .cloned()
            .collect();
        let mut variables: Vec<CssVarUsage> = definitions
            .iter()
            .map(|(name, value)| CssVarUsage {
                name: name.clone(),
                defined: true,
                used_count: used_count.get(name).copied().unwrap_or(0),
                value: Some(value.clone()),
            })
            .collect();
        for u in &undefined_refs {
            variables.push(CssVarUsage {
                name: u.clone(),
                defined: false,
                used_count: used_count.get(u).copied().unwrap_or(0),
                value: None,
            });
        }
        variables.sort_by(|a, b| a.name.cmp(&b.name));
        CssVariableReport {
            variables,
            undefined_refs,
            unused_defs,
        }
    }
}
