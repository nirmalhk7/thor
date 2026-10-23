//! Frontend specialist features (11 + 12 wave-2).
//!
//! Component tree, responsive matrix, CSS cascade, design tokens, accessibility,
//! state time-travel, CLS heatmap, component sandbox, network↔component binding,
//! font auditing, and z-index visualization. All logic is pure Rust operating on
//! captured DOM / CSS / layout data (no fake randomness).

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// A captured DOM node with its tag, attributes and children.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DomNode {
    pub tag: String,
    #[serde(default)]
    pub attributes: HashMap<String, String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub children: Vec<DomNode>,
}

impl DomNode {
    pub fn new(tag: &str) -> Self {
        Self {
            tag: tag.to_string(),
            ..Default::default()
        }
    }
    pub fn attr(mut self, k: &str, v: &str) -> Self {
        self.attributes.insert(k.to_string(), v.to_string());
        self
    }
    pub fn child(mut self, node: DomNode) -> Self {
        self.children.push(node);
        self
    }
}

// ---------------------------------------------------------------------------
// 1. Component Tree Lens
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentNode {
    pub tag: String,
    pub component_name: Option<String>,
    pub is_component_boundary: bool,
    pub framework: Option<String>,
    pub depth: usize,
    pub children: Vec<ComponentNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentTree {
    pub root: ComponentNode,
    pub component_count: usize,
    pub max_depth: usize,
    pub node_count: usize,
}

// ---------------------------------------------------------------------------
// 2. Responsive Matrix
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub label: String,
    pub width: f64,
    pub height: f64,
}

/// Bounding box of an element captured at a specific viewport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementBox {
    pub selector: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub font_size_px: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsiveIssue {
    pub viewport: String,
    pub selector: String,
    pub kind: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsiveReport {
    pub issues: Vec<ResponsiveIssue>,
    pub viewports_tested: usize,
}

// ---------------------------------------------------------------------------
// 3. CSS Cascade
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssDeclaration {
    pub selector: String,
    pub property: String,
    pub value: String,
    #[serde(default)]
    pub important: bool,
    /// Source order index (later wins ties).
    #[serde(default)]
    pub order: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeWinner {
    pub property: String,
    pub winning_value: String,
    pub winning_selector: String,
    pub specificity: [u32; 3],
    pub important: bool,
    pub overridden: Vec<String>,
}

// ---------------------------------------------------------------------------
// 4. Design Tokens
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignTokenReport {
    pub colors: Vec<TokenUsage>,
    pub font_sizes: Vec<TokenUsage>,
    pub spacing: Vec<TokenUsage>,
    pub radii: Vec<TokenUsage>,
    /// Values that appear only once (candidate one-offs / scale violations).
    pub one_offs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub value: String,
    pub count: usize,
}

// ---------------------------------------------------------------------------
// 5. Accessibility Lens
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A11yViolation {
    pub rule: String,
    pub selector: String,
    pub severity: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// 6. State Time-Travel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub index: usize,
    pub label: String,
    pub timestamp_ms: u64,
    pub state: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiffEntry {
    pub path: String,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// 7. CLS Heatmap
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutShiftEntry {
    pub node: String,
    pub score: f64,
    pub had_recent_input: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClsHeatmapReport {
    pub total_cls: f64,
    pub contributors: Vec<TokenUsageF>,
    pub rating: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageF {
    pub node: String,
    pub score: f64,
    pub percent: f64,
}

// ---------------------------------------------------------------------------
// 9. Network↔Component binding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderWindow {
    pub component: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEvent {
    pub url: String,
    pub start_ms: u64,
    #[serde(default)]
    pub initiator_component: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentBinding {
    pub component: String,
    pub requests: Vec<String>,
}

// ---------------------------------------------------------------------------
// 10. Font Auditor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontFace {
    pub family: String,
    pub weight: u32,
    #[serde(default)]
    pub font_display: Option<String>,
    #[serde(default)]
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontAuditReport {
    pub unused_families: Vec<String>,
    pub missing_swap: Vec<String>,
    pub total_bytes: u64,
    pub weight_variants: usize,
}

// ---------------------------------------------------------------------------
// 11. Z-Index Visualizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackingElement {
    pub selector: String,
    pub z_index: Option<i64>,
    pub creates_stacking_context: bool,
    pub position: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZIndexReport {
    pub paint_order: Vec<String>,
    pub conflicts: Vec<String>,
    pub excessive: Vec<String>,
    pub max_z: i64,
}

// ---------------------------------------------------------------------------
// 4b. Design Token Drift
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDriftReport {
    /// Used values that are NOT part of the canonical token set.
    pub off_scale: Vec<String>,
    /// Used values that match the canonical token set.
    pub matched: Vec<String>,
    /// Canonical tokens never referenced by the captured styles.
    pub unused_tokens: Vec<String>,
    /// Fraction of used values that are on-scale, in [0,1].
    pub coverage: f64,
}

// ---------------------------------------------------------------------------
// 5b. Accessibility Live Matrix (rule × node grid)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A11yMatrixCell {
    pub rule: String,
    pub node: String,
    pub violated: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A11yMatrix {
    pub rules: Vec<String>,
    pub nodes: Vec<String>,
    pub cells: Vec<A11yMatrixCell>,
    pub total_violations: usize,
}

// ---------------------------------------------------------------------------
// 7b. Paint Heatmap (spatial regions from layout shifts)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutShiftRect {
    pub node: String,
    pub score: f64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub had_recent_input: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaintHeatmapRegion {
    /// Grid cell label, e.g. "top-left", "center".
    pub region: String,
    pub score: f64,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaintHeatmapReport {
    pub regions: Vec<PaintHeatmapRegion>,
    pub total_score: f64,
    pub hottest_region: String,
}

/// Frontend specialist engine.
pub struct FrontendEngine {
    snapshots: Mutex<Vec<StateSnapshot>>,
}

impl Default for FrontendEngine {
    fn default() -> Self {
        Self::new()
    }
}


// ===========================================================================
// Wave-2 frontend features (12)
// ===========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydrationMismatch {
    pub path: String,
    pub server: String,
    pub client: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydrationDiffReport {
    pub mismatches: Vec<HydrationMismatch>,
    pub mismatch_count: usize,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssRuleCoverage {
    pub selector: String,
    pub used: bool,
    pub bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssCoverageReport {
    pub rules: Vec<CssRuleCoverage>,
    pub used_pct: f64,
    pub unused_bytes: usize,
    pub total_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusableNode {
    pub selector: String,
    pub tab_index: i32,
    pub visible: bool,
    pub disabled: bool,
    pub order_hint: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusOrderReport {
    pub sequence: Vec<String>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventListenerEntry {
    pub selector: String,
    pub event_type: String,
    pub passive: bool,
    pub once: bool,
    pub capture: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventListenerReport {
    pub by_type: BTreeMap<String, usize>,
    pub non_passive_scroll: Vec<String>,
    pub duplicate_bindings: Vec<String>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssVarUsage {
    pub name: String,
    pub defined: bool,
    pub used_count: usize,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssVariableReport {
    pub variables: Vec<CssVarUsage>,
    pub undefined_refs: Vec<String>,
    pub unused_defs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutOp {
    pub kind: String,
    pub selector: String,
    pub timestamp_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutThrashReport {
    pub thrash_windows: usize,
    pub worst_burst: usize,
    pub offending_selectors: Vec<String>,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaQueryCapture {
    pub query: String,
    pub matches: bool,
    pub applied_rules: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaCqReport {
    pub queries: Vec<MediaQueryCapture>,
    pub active_count: usize,
    pub container_queries: usize,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageAsset {
    pub url: String,
    pub natural_w: u32,
    pub natural_h: u32,
    pub display_w: f64,
    pub display_h: f64,
    pub bytes: u64,
    pub format: String,
    pub has_srcset: bool,
    pub loading: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageDeliveryFinding {
    pub url: String,
    pub kind: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageDeliveryReport {
    pub findings: Vec<ImageDeliveryFinding>,
    pub total_bytes: u64,
    pub oversized_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainCandidate {
    pub selector: String,
    pub has_layout_isolation: bool,
    pub paints_independently: bool,
    pub size_known: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainmentAdvice {
    pub selector: String,
    pub recommend: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainmentReport {
    pub advice: Vec<ContainmentAdvice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AriaLiveEvent {
    pub region: String,
    pub politeness: String,
    pub text: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AriaLiveReport {
    pub regions: usize,
    pub assertive_bursts: usize,
    pub empty_announcements: usize,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecificityBudgetReport {
    pub selectors_over_budget: Vec<String>,
    pub max_specificity: [u32; 3],
    pub budget: [u32; 3],
    pub average_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomSnapshotDigest {
    pub node_count: usize,
    pub max_depth: usize,
    pub tag_histogram: BTreeMap<String, usize>,
    pub text_bytes: usize,
    pub attr_bytes: usize,
    pub digest_hex: String,
}

impl FrontendEngine {
    pub fn new() -> Self {
        Self {
            snapshots: Mutex::new(Vec::new()),
        }
    }

    // 1. Component Tree Lens ------------------------------------------------
    pub fn build_component_tree(&self, root: &DomNode) -> ComponentTree {
        let mut component_count = 0;
        let mut max_depth = 0;
        let mut node_count = 0;
        let node = Self::walk_component(
            root,
            0,
            &mut component_count,
            &mut max_depth,
            &mut node_count,
        );
        ComponentTree {
            root: node,
            component_count,
            max_depth,
            node_count,
        }
    }

    fn walk_component(
        n: &DomNode,
        depth: usize,
        components: &mut usize,
        max_depth: &mut usize,
        node_count: &mut usize,
    ) -> ComponentNode {
        *node_count += 1;
        *max_depth = (*max_depth).max(depth);
        let (is_boundary, framework, name) = Self::detect_component(n);
        if is_boundary {
            *components += 1;
        }
        let children = n
            .children
            .iter()
            .map(|c| Self::walk_component(c, depth + 1, components, max_depth, node_count))
            .collect();
        ComponentNode {
            tag: n.tag.clone(),
            component_name: name,
            is_component_boundary: is_boundary,
            framework,
            depth,
            children,
        }
    }

    fn detect_component(n: &DomNode) -> (bool, Option<String>, Option<String>) {
        // Custom elements (contain a hyphen) or PascalCase tags are components.
        if n.tag.contains('-')
            || n.tag
                .chars()
                .next()
                .map(|c| c.is_ascii_uppercase())
                .unwrap_or(false)
        {
            return (true, Some("web-component".into()), Some(n.tag.clone()));
        }
        for (k, v) in &n.attributes {
            if k.starts_with("data-react") || k == "data-reactroot" {
                return (
                    true,
                    Some("react".into()),
                    n.attributes.get("data-component").cloned(),
                );
            }
            if k.starts_with("data-v-") || k == "data-vue" {
                return (true, Some("vue".into()), None);
            }
            if k.starts_with("ng-") || k.starts_with("_ngcontent") || k == "ng-version" {
                return (true, Some("angular".into()), None);
            }
            if k == "data-component" || k == "data-testid" && v.contains("component") {
                return (true, None, Some(v.clone()));
            }
        }
        (false, None, None)
    }

    // 2. Responsive Matrix --------------------------------------------------
    pub fn standard_viewports() -> Vec<Viewport> {
        vec![
            Viewport {
                label: "mobile".into(),
                width: 375.0,
                height: 667.0,
            },
            Viewport {
                label: "tablet".into(),
                width: 768.0,
                height: 1024.0,
            },
            Viewport {
                label: "laptop".into(),
                width: 1366.0,
                height: 768.0,
            },
            Viewport {
                label: "desktop".into(),
                width: 1920.0,
                height: 1080.0,
            },
        ]
    }

    pub fn responsive_matrix(&self, captures: &[(Viewport, Vec<ElementBox>)]) -> ResponsiveReport {
        let mut issues = Vec::new();
        for (vp, boxes) in captures {
            for b in boxes {
                if b.x + b.width > vp.width + 0.5 {
                    issues.push(ResponsiveIssue {
                        viewport: vp.label.clone(),
                        selector: b.selector.clone(),
                        kind: "horizontal-overflow".into(),
                        detail: format!(
                            "right edge {:.0}px exceeds viewport {:.0}px",
                            b.x + b.width,
                            vp.width
                        ),
                    });
                }
                if b.font_size_px > 0.0 && b.font_size_px < 12.0 && vp.width <= 480.0 {
                    issues.push(ResponsiveIssue {
                        viewport: vp.label.clone(),
                        selector: b.selector.clone(),
                        kind: "tiny-text".into(),
                        detail: format!(
                            "font-size {:.1}px below 12px on small viewport",
                            b.font_size_px
                        ),
                    });
                }
                // Touch target minimum 44x44 per WCAG 2.5.5 on mobile.
                if vp.width <= 480.0 && (b.width < 44.0 || b.height < 44.0) && b.width > 0.0 {
                    issues.push(ResponsiveIssue {
                        viewport: vp.label.clone(),
                        selector: b.selector.clone(),
                        kind: "small-touch-target".into(),
                        detail: format!(
                            "{:.0}x{:.0}px below 44x44 touch target",
                            b.width, b.height
                        ),
                    });
                }
            }
        }
        ResponsiveReport {
            issues,
            viewports_tested: captures.len(),
        }
    }

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
        let mut ids = 0;
        let mut classes = 0;
        let mut types = 0;
        for token in selector
            .split([' ', '>', '+', '~'])
            .filter(|t| !t.is_empty())
        {
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
                        while chars.peek().map(|c| Self::is_ident(*c)).unwrap_or(false) {
                            chars.next();
                        }
                    }
                    '.' | '[' | ':' => {
                        flush(&mut buf, &mut types);
                        classes += 1;
                        while chars
                            .peek()
                            .map(|c| Self::is_ident(*c) || *c == '=' || *c == '"' || *c == ']')
                            .unwrap_or(false)
                        {
                            chars.next();
                        }
                    }
                    _ => buf.push(c),
                }
            }
            flush(&mut buf, &mut types);
        }
        [ids, classes, types]
    }

    fn is_ident(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '-' || c == '_'
    }

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
                && (val.starts_with('#') || val.starts_with("rgb")) {
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

    /// WCAG relative-luminance contrast ratio between two `#rrggbb` colors.
    pub fn contrast_ratio(fg: &str, bg: &str) -> Option<f64> {
        let l1 = Self::relative_luminance(fg)?;
        let l2 = Self::relative_luminance(bg)?;
        let (light, dark) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
        Some((light + 0.05) / (dark + 0.05))
    }

    fn relative_luminance(hex: &str) -> Option<f64> {
        let h = hex.trim().trim_start_matches('#');
        if h.len() != 6 {
            return None;
        }
        let chan = |s: &str| u8::from_str_radix(s, 16).ok().map(|v| v as f64 / 255.0);
        let r = chan(&h[0..2])?;
        let g = chan(&h[2..4])?;
        let b = chan(&h[4..6])?;
        let lin = |c: f64| {
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        Some(0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b))
    }

    // 6. State Time-Travel --------------------------------------------------
    pub fn record_state(&self, label: &str, state: serde_json::Value) -> usize {
        let mut snaps = self.snapshots.lock();
        let index = snaps.len();
        let timestamp_ms = now_ms();
        snaps.push(StateSnapshot {
            index,
            label: label.to_string(),
            timestamp_ms,
            state,
        });
        index
    }

    pub fn state_snapshots(&self) -> Vec<StateSnapshot> {
        self.snapshots.lock().clone()
    }

    pub fn clear_state(&self) {
        self.snapshots.lock().clear();
    }

    /// Restore (return) the recorded state at `index` so a caller can apply it
    /// back into the running app (time-travel jump). `None` when out of range.
    pub fn restore_state(&self, index: usize) -> Option<serde_json::Value> {
        self.snapshots.lock().get(index).map(|s| s.state.clone())
    }

    pub fn state_diff(&self, from: usize, to: usize) -> Vec<StateDiffEntry> {
        let snaps = self.snapshots.lock();
        match (snaps.get(from), snaps.get(to)) {
            (Some(a), Some(b)) => {
                let mut out = Vec::new();
                Self::json_diff("", &a.state, &b.state, &mut out);
                out
            }
            _ => Vec::new(),
        }
    }

    fn json_diff(
        path: &str,
        a: &serde_json::Value,
        b: &serde_json::Value,
        out: &mut Vec<StateDiffEntry>,
    ) {
        use serde_json::Value;
        match (a, b) {
            (Value::Object(oa), Value::Object(ob)) => {
                let mut keys: Vec<&String> = oa.keys().chain(ob.keys()).collect();
                keys.sort();
                keys.dedup();
                for k in keys {
                    let child = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{path}.{k}")
                    };
                    match (oa.get(k), ob.get(k)) {
                        (Some(va), Some(vb)) => Self::json_diff(&child, va, vb, out),
                        (Some(va), None) => out.push(StateDiffEntry {
                            path: child,
                            before: Some(va.clone()),
                            after: None,
                        }),
                        (None, Some(vb)) => out.push(StateDiffEntry {
                            path: child,
                            before: None,
                            after: Some(vb.clone()),
                        }),
                        (None, None) => {}
                    }
                }
            }
            _ => {
                if a != b {
                    out.push(StateDiffEntry {
                        path: path.to_string(),
                        before: Some(a.clone()),
                        after: Some(b.clone()),
                    });
                }
            }
        }
    }

    // 7. CLS Heatmap --------------------------------------------------------
    pub fn cls_heatmap(&self, entries: &[LayoutShiftEntry]) -> ClsHeatmapReport {
        let mut per_node: HashMap<String, f64> = HashMap::new();
        let mut total = 0.0;
        for e in entries {
            if e.had_recent_input {
                continue; // Excluded from CLS per spec.
            }
            total += e.score;
            *per_node.entry(e.node.clone()).or_default() += e.score;
        }
        let mut contributors: Vec<TokenUsageF> = per_node
            .into_iter()
            .map(|(node, score)| TokenUsageF {
                node,
                score,
                percent: if total > 0.0 {
                    score / total * 100.0
                } else {
                    0.0
                },
            })
            .collect();
        contributors.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let rating = if total <= 0.1 {
            "good"
        } else if total <= 0.25 {
            "needs-improvement"
        } else {
            "poor"
        };
        ClsHeatmapReport {
            total_cls: total,
            contributors,
            rating: rating.to_string(),
        }
    }

    /// Bucket layout shifts into a 3×3 spatial grid of the viewport, producing a
    /// paint heatmap that shows *where* on the page instability concentrates.
    pub fn paint_heatmap(
        &self,
        shifts: &[LayoutShiftRect],
        viewport_w: f64,
        viewport_h: f64,
    ) -> PaintHeatmapReport {
        let vw = viewport_w.max(1.0);
        let vh = viewport_h.max(1.0);
        let col_labels = ["left", "center", "right"];
        let row_labels = ["top", "middle", "bottom"];
        let mut region_score: BTreeMap<String, (f64, usize)> = BTreeMap::new();
        let mut total = 0.0;
        for s in shifts {
            if s.had_recent_input {
                continue;
            }
            let cx = s.x + s.width / 2.0;
            let cy = s.y + s.height / 2.0;
            let col = ((cx / vw) * 3.0).floor().clamp(0.0, 2.0) as usize;
            let row = ((cy / vh) * 3.0).floor().clamp(0.0, 2.0) as usize;
            let label = format!("{}-{}", row_labels[row], col_labels[col]);
            let e = region_score.entry(label).or_insert((0.0, 0));
            e.0 += s.score;
            e.1 += 1;
            total += s.score;
        }
        let mut regions: Vec<PaintHeatmapRegion> = region_score
            .into_iter()
            .map(|(region, (score, count))| PaintHeatmapRegion {
                region,
                score,
                count,
            })
            .collect();
        regions.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let hottest_region = regions
            .first()
            .map(|r| r.region.clone())
            .unwrap_or_default();
        PaintHeatmapReport {
            regions,
            total_score: total,
            hottest_region,
        }
    }

    // 8. Component Sandbox --------------------------------------------------
    /// Build an isolated single-file HTML sandbox document for a component.
    pub fn component_sandbox(
        &self,
        tag: &str,
        props: &serde_json::Value,
        template_html: &str,
    ) -> String {
        let props_json = serde_json::to_string(props).unwrap_or_else(|_| "{}".to_string());
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\">\
<title>Thor Sandbox: {tag}</title>\
<style>:root{{color-scheme:light dark}}body{{margin:0;padding:16px;font-family:system-ui}}\
#thor-sandbox-root{{all:initial;contain:layout style paint}}</style></head>\
<body><div id=\"thor-sandbox-root\" data-component=\"{tag}\">{template_html}</div>\
<script>window.__THOR_PROPS__={props_json};</script></body></html>"
        )
    }

    /// Validate props against a minimal schema of required keys + JSON types.
    pub fn validate_props(
        &self,
        props: &serde_json::Value,
        required: &[(&str, &str)],
    ) -> Vec<String> {
        let mut errs = Vec::new();
        let obj = match props.as_object() {
            Some(o) => o,
            None => return vec!["props must be a JSON object".into()],
        };
        for (key, ty) in required {
            match obj.get(*key) {
                None => errs.push(format!("missing required prop '{key}'")),
                Some(v) => {
                    let actual = json_type(v);
                    if actual != *ty {
                        errs.push(format!("prop '{key}' expected {ty}, got {actual}"));
                    }
                }
            }
        }
        errs
    }

    // 9. Network↔Component Binding -----------------------------------------
    pub fn bind_network_to_components(
        &self,
        windows: &[RenderWindow],
        events: &[NetworkEvent],
    ) -> Vec<ComponentBinding> {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for ev in events {
            // Prefer explicit initiator, else correlate by render window timing.
            let owner = ev
                .initiator_component
                .clone()
                .or_else(|| {
                    windows
                        .iter()
                        .filter(|w| ev.start_ms >= w.start_ms && ev.start_ms <= w.end_ms)
                        .min_by_key(|w| w.end_ms - w.start_ms)
                        .map(|w| w.component.clone())
                })
                .unwrap_or_else(|| "<unbound>".to_string());
            map.entry(owner).or_default().push(ev.url.clone());
        }
        map.into_iter()
            .map(|(component, requests)| ComponentBinding {
                component,
                requests,
            })
            .collect()
    }

    // 10. Font Auditor ------------------------------------------------------
    pub fn audit_fonts(&self, faces: &[FontFace], used_families: &[String]) -> FontAuditReport {
        let used: std::collections::HashSet<String> =
            used_families.iter().map(|f| f.to_lowercase()).collect();
        let mut unused = Vec::new();
        let mut missing_swap = Vec::new();
        let mut total_bytes = 0u64;
        for f in faces {
            total_bytes += f.bytes;
            if !used.contains(&f.family.to_lowercase()) {
                unused.push(format!("{} {}", f.family, f.weight));
            }
            let display_ok = f
                .font_display
                .as_deref()
                .map(|d| matches!(d, "swap" | "optional" | "fallback"))
                .unwrap_or(false);
            if !display_ok {
                missing_swap.push(format!("{} {}", f.family, f.weight));
            }
        }
        FontAuditReport {
            unused_families: unused,
            missing_swap,
            total_bytes,
            weight_variants: faces.len(),
        }
    }

    // 11. Z-Index Visualizer ------------------------------------------------
    pub fn visualize_zindex(&self, elements: &[StackingElement]) -> ZIndexReport {
        let mut ordered: Vec<&StackingElement> = elements.iter().collect();
        ordered.sort_by_key(|a| a.z_index.unwrap_or(0));
        let paint_order = ordered.iter().map(|e| e.selector.clone()).collect();

        // Conflicts: same z-index among sibling-level stacking contexts.
        let mut by_z: BTreeMap<i64, Vec<String>> = BTreeMap::new();
        for e in elements {
            if let Some(z) = e.z_index {
                by_z.entry(z).or_default().push(e.selector.clone());
            }
        }
        let conflicts = by_z
            .iter()
            .filter(|(_, v)| v.len() > 1)
            .map(|(z, v)| format!("z-index {z} shared by {}", v.join(", ")))
            .collect();

        // z-index has no effect without positioning / stacking context.
        let excessive = elements
            .iter()
            .filter(|e| {
                e.z_index.map(|z| z >= 1000).unwrap_or(false)
                    || (e.z_index.is_some()
                        && e.position == "static"
                        && !e.creates_stacking_context)
            })
            .map(|e| e.selector.clone())
            .collect();

        let max_z = elements.iter().filter_map(|e| e.z_index).max().unwrap_or(0);
        ZIndexReport {
            paint_order,
            conflicts,
            excessive,
            max_z,
        }
    }

    // ---- Wave-2 frontend --------------------------------------------------

    /// Structural hydration diff: parse HTML fragments into [`DomNode`] trees and
    /// walk them. Paths look like `html>body>div[0]` (tag + sibling index).
    pub fn hydration_diff_lens(
        &self,
        server_html: &str,
        client_html: &str,
    ) -> HydrationDiffReport {
        let server = parse_html_to_dom(server_html);
        let client = parse_html_to_dom(client_html);
        self.hydration_diff_dom(&server, &client)
    }

    /// Structural DomNode walk used by [`hydration_diff_lens`].
    pub fn hydration_diff_dom(&self, server: &DomNode, client: &DomNode) -> HydrationDiffReport {
        let mut mismatches = Vec::new();
        walk_hydration(server, client, "root", &mut mismatches);
        let count = mismatches.len();
        let severity = if count == 0 {
            "none"
        } else if count <= 3 {
            "low"
        } else if count <= 15 {
            "medium"
        } else {
            "high"
        };
        HydrationDiffReport {
            mismatches,
            mismatch_count: count,
            severity: severity.to_string(),
        }
    }

    /// Map CDP `CSS.takeCoverageDelta` coverage entries into [`CssRuleCoverage`].
    ///
    /// Expected shape (CDP 1.3):
    /// ```json
    /// { "coverage": [
    ///   { "styleSheetId": "…", "url": "…", "text": ".a{} .b{}",
    ///     "ranges": [{"startOffset":0,"endOffset":4,"count":1}] }
    /// ]}
    /// ```
    /// Each stylesheet is split on rule-ish boundaries; a rule is `used` when any
    /// coverage range with `count > 0` overlaps its byte span.
    pub fn css_coverage_from_take_coverage_delta(json: &str) -> CssCoverageReport {
        let rules = parse_css_take_coverage_delta(json);
        FrontendEngine::new().css_coverage_map(&rules)
    }

    pub fn css_coverage_map(&self, rules: &[CssRuleCoverage]) -> CssCoverageReport {
        let total_bytes: usize = rules.iter().map(|r| r.bytes).sum();
        let unused_bytes: usize = rules.iter().filter(|r| !r.used).map(|r| r.bytes).sum();
        let used = rules.iter().filter(|r| r.used).count();
        let used_pct = if rules.is_empty() {
            100.0
        } else {
            used as f64 / rules.len() as f64 * 100.0
        };
        CssCoverageReport {
            rules: rules.to_vec(),
            used_pct,
            unused_bytes,
            total_bytes,
        }
    }

    /// Probe script: collect focusable nodes in document order for
    /// [`parse_focus_order_probe`] → [`focus_order_overlay`].
    /// Probe React DevTools global hook when present (no extension required).
    pub fn react_devtools_probe_script() -> &'static str {
        r#"(function(){
  var hook = window.__REACT_DEVTOOLS_GLOBAL_HOOK__;
  if (!hook) return JSON.stringify({ present:false, renderers:0, roots:[] });
  var renderers = 0;
  try { renderers = hook.renderers ? hook.renderers.size : 0; } catch(e) {}
  var roots = [];
  try {
    if (hook.renderers) {
      hook.renderers.forEach(function(r){
        try {
          var version = (r && r.version) || (r && r.reconcilerVersion) || '?';
          roots.push({ version: String(version) });
        } catch(e2){}
      });
    }
  } catch(e3){}
  return JSON.stringify({ present:true, renderers: renderers, roots: roots, supportsFiber: !!hook.supportsFiber });
})()"#
    }

    /// Collect buffered Layout Shift entries for CLS heatmap (`from_active_tab`).
    pub fn cls_entries_probe_script() -> &'static str {
        r#"(function(){
  var entries = [];
  try {
    var list = performance.getEntriesByType('layout-shift') || [];
    for (var i = 0; i < list.length; i++) {
      var e = list[i];
      if (e.hadRecentInput) continue;
      var node = 'unknown';
      try {
        if (e.sources && e.sources[0] && e.sources[0].node) {
          var n = e.sources[0].node;
          node = n.id ? ('#' + n.id) : (n.tagName || 'node').toLowerCase();
        }
      } catch (err) {}
      entries.push({ node: node, score: e.value || 0, had_recent_input: !!e.hadRecentInput });
    }
  } catch (e2) {}
  return JSON.stringify(entries);
})()"#
    }

    /// Best-effort accessibility DOM probe for `from_active_tab`.
    pub fn a11y_dom_probe_script() -> &'static str {
        r#"(function(){
  function walk(el, depth){
    if(!el || depth>10) return null;
    var tag=(el.tagName||'').toLowerCase();
    if(!tag || tag==='script' || tag==='style' || tag==='noscript') return null;
    var attrs={};
    if(el.attributes){
      for(var i=0;i<el.attributes.length;i++){
        attrs[el.attributes[i].name]=el.attributes[i].value;
      }
    }
    var kids=[];
    var children=el.children||[];
    for(var j=0;j<children.length;j++){
      var n=walk(children[j], depth+1);
      if(n) kids.push(n);
    }
    var text='';
    var nodes=el.childNodes||[];
    for(var k=0;k<nodes.length;k++){
      if(nodes[k].nodeType===3) text+=nodes[k].textContent||'';
    }
    text=text.replace(/\s+/g,' ').trim();
    return {tag:tag, attributes:attrs, text:text||null, children:kids};
  }
  var root=walk(document.body, 0);
  return JSON.stringify(root ? [root] : []);
})()"#
    }

    /// Probe script: collect focusable nodes in document order for
    /// [`parse_focus_order_probe`] → [`focus_order_overlay`].
    pub fn focus_order_probe_script() -> &'static str {
        r#"(() => {
  const FOCUSABLE = 'a[href],button,input,select,textarea,summary,[tabindex],[contenteditable="true"]';
  const nodes = [];
  let hint = 0;
  document.querySelectorAll(FOCUSABLE).forEach((el) => {
    const style = window.getComputedStyle(el);
    const rect = el.getBoundingClientRect();
    const hidden = style.visibility === 'hidden' || style.display === 'none'
      || (rect.width === 0 && rect.height === 0);
    let tab = el.tabIndex;
    if (tab === undefined || tab === null) tab = 0;
    const sel = el.id ? ('#' + el.id)
      : (el.getAttribute('name') ? (el.tagName.toLowerCase() + '[name="' + el.getAttribute('name') + '"]')
      : (el.tagName.toLowerCase() + ':nth-of-type(' + ([].indexOf.call(el.parentNode ? el.parentNode.children : [], el) + 1) + ')'));
    nodes.push({
      selector: sel,
      tab_index: tab,
      visible: !hidden,
      disabled: !!(el.disabled || el.getAttribute('aria-disabled') === 'true'),
      order_hint: hint++
    });
  });
  return JSON.stringify(nodes);
})()"#
    }

    /// Best-effort CSS rule coverage probe for `from_active_tab` (stylesheet walk).
    /// Prefer CDP `CSS.takeCoverageDelta` JSON when available.
    pub fn css_coverage_probe_script() -> &'static str {
        r#"(() => {
  const rules = [];
  for (const sheet of document.styleSheets) {
    let list;
    try { list = sheet.cssRules; } catch (e) { continue; }
    if (!list) continue;
    for (const rule of list) {
      if (!rule.selectorText) continue;
      const sel = rule.selectorText;
      let used = false;
      try {
        const first = sel.split(',')[0].trim();
        if (first) used = !!document.querySelector(first);
      } catch (e2) {}
      rules.push({
        selector: sel,
        used: used,
        bytes: (rule.cssText || '').length
      });
    }
  }
  return JSON.stringify(rules);
})()"#
    }

    /// Parse JSON produced by [`focus_order_probe_script`].
    pub fn parse_focus_order_probe(json: &str) -> Option<Vec<FocusableNode>> {
        let trimmed = json.trim();
        if let Ok(inner) = serde_json::from_str::<String>(trimmed) {
            return serde_json::from_str(&inner).ok();
        }
        serde_json::from_str(trimmed).ok()
    }

    pub fn focus_order_overlay(&self, nodes: &[FocusableNode]) -> FocusOrderReport {
        let mut eligible: Vec<&FocusableNode> = nodes
            .iter()
            .filter(|n| n.visible && !n.disabled)
            .collect();
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
                issues.push(format!("{} has invalid tabindex {}", n.selector, n.tab_index));
            }
            if !n.visible && n.tab_index >= 0 {
                issues.push(format!("{} is focusable but not visible", n.selector));
            }
        }
        FocusOrderReport { sequence, issues }
    }

    pub fn event_listener_map(&self, entries: &[EventListenerEntry]) -> EventListenerReport {
        let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
        let mut non_passive_scroll = Vec::new();
        let mut seen: BTreeMap<(String, String), usize> = BTreeMap::new();
        for e in entries {
            *by_type.entry(e.event_type.clone()).or_default() += 1;
            *seen
                .entry((e.selector.clone(), e.event_type.clone()))
                .or_default() += 1;
            if matches!(e.event_type.as_str(), "scroll" | "touchstart" | "wheel") && !e.passive {
                non_passive_scroll.push(format!("{}@{}", e.selector, e.event_type));
            }
        }
        let duplicate_bindings: Vec<String> = seen
            .into_iter()
            .filter(|(_, c)| *c > 1)
            .map(|((sel, ev), c)| format!("{sel}@{ev} x{c}"))
            .collect();
        EventListenerReport {
            by_type,
            non_passive_scroll,
            duplicate_bindings,
            total: entries.len(),
        }
    }

    pub fn css_variable_inspector(
        &self,
        definitions: &BTreeMap<String, String>,
        references: &[String],
    ) -> CssVariableReport {
        let mut used_count: BTreeMap<String, usize> = BTreeMap::new();
        let mut undefined_refs = Vec::new();
        for r in references {
            let name = r.trim().trim_start_matches("var(").trim_end_matches(')').trim();
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

    pub fn layout_thrash_detector(&self, ops: &[LayoutOp]) -> LayoutThrashReport {
        // Thrash = read (measure) immediately followed by write (mutate) in a tight window.
        let mut thrash_windows = 0usize;
        let mut worst_burst = 0usize;
        let mut burst = 0usize;
        let mut offending: BTreeMap<String, usize> = BTreeMap::new();
        for w in ops.windows(2) {
            let a = &w[0];
            let b = &w[1];
            let read_write = (a.kind == "measure" && b.kind == "mutate")
                || (a.kind == "mutate" && b.kind == "measure");
            let tight = (b.timestamp_ms - a.timestamp_ms).abs() <= 16.0;
            if read_write && tight {
                thrash_windows += 1;
                burst += 1;
                worst_burst = worst_burst.max(burst);
                *offending.entry(a.selector.clone()).or_default() += 1;
                *offending.entry(b.selector.clone()).or_default() += 1;
            } else {
                burst = 0;
            }
        }
        let mut offending_selectors: Vec<(String, usize)> = offending.into_iter().collect();
        offending_selectors.sort_by(|a, b| b.1.cmp(&a.1));
        let offending_selectors: Vec<String> = offending_selectors
            .into_iter()
            .take(10)
            .map(|(s, _)| s)
            .collect();
        let score = (thrash_windows as f64) * 10.0 + (worst_burst as f64) * 5.0;
        LayoutThrashReport {
            thrash_windows,
            worst_burst,
            offending_selectors,
            score,
        }
    }

    pub fn media_cq_matrix(&self, captures: &[MediaQueryCapture]) -> MediaCqReport {
        let active_count = captures.iter().filter(|c| c.matches).count();
        let container_queries = captures
            .iter()
            .filter(|c| c.query.contains("container") || c.query.contains("cq-"))
            .count();
        let mut issues = Vec::new();
        for c in captures {
            if c.matches && c.applied_rules == 0 {
                issues.push(format!("query matches but applies 0 rules: {}", c.query));
            }
        }
        if captures.len() > 40 {
            issues.push(format!(
                "high media-query count ({}) may indicate breakpoint sprawl",
                captures.len()
            ));
        }
        MediaCqReport {
            queries: captures.to_vec(),
            active_count,
            container_queries,
            issues,
        }
    }

    pub fn image_delivery_auditor(&self, images: &[ImageAsset]) -> ImageDeliveryReport {
        let mut findings = Vec::new();
        let mut oversized = 0usize;
        let mut total_bytes = 0u64;
        for img in images {
            total_bytes += img.bytes;
            if img.display_w > 0.0 && img.natural_w as f64 > img.display_w * 2.0 {
                oversized += 1;
                findings.push(ImageDeliveryFinding {
                    url: img.url.clone(),
                    kind: "oversized-dimensions".into(),
                    detail: format!(
                        "natural {}px vs display {:.0}px (>2x)",
                        img.natural_w, img.display_w
                    ),
                });
            }
            if img.bytes > 200_000 {
                findings.push(ImageDeliveryFinding {
                    url: img.url.clone(),
                    kind: "heavy-bytes".into(),
                    detail: format!("{} bytes", img.bytes),
                });
            }
            let fmt = img.format.to_lowercase();
            if matches!(fmt.as_str(), "png" | "bmp" | "tiff") && img.bytes > 50_000 {
                findings.push(ImageDeliveryFinding {
                    url: img.url.clone(),
                    kind: "format".into(),
                    detail: format!("{fmt} candidate for webp/avif"),
                });
            }
            if !img.has_srcset && img.display_w > 0.0 {
                findings.push(ImageDeliveryFinding {
                    url: img.url.clone(),
                    kind: "missing-srcset".into(),
                    detail: "no responsive srcset".into(),
                });
            }
            if img.loading != "lazy" && img.display_w > 0.0 {
                // only flag below-fold heuristic via large y would need more data; skip
            }
        }
        ImageDeliveryReport {
            findings,
            total_bytes,
            oversized_count: oversized,
        }
    }

    pub fn css_containment_advisor(&self, nodes: &[ContainCandidate]) -> ContainmentReport {
        let mut advice = Vec::new();
        for n in nodes {
            if n.size_known && n.paints_independently && !n.has_layout_isolation {
                advice.push(ContainmentAdvice {
                    selector: n.selector.clone(),
                    recommend: "contain: layout paint".into(),
                    reason: "independent paint subtree with known size".into(),
                });
            } else if n.size_known && !n.has_layout_isolation {
                advice.push(ContainmentAdvice {
                    selector: n.selector.clone(),
                    recommend: "contain: layout".into(),
                    reason: "known size but layout not isolated".into(),
                });
            } else if n.paints_independently && !n.has_layout_isolation {
                advice.push(ContainmentAdvice {
                    selector: n.selector.clone(),
                    recommend: "contain: paint".into(),
                    reason: "independent paint without containment".into(),
                });
            }
        }
        ContainmentReport { advice }
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

    pub fn dom_snapshot_capture(&self, root: &DomNode) -> DomSnapshotDigest {
        let mut tag_histogram: BTreeMap<String, usize> = BTreeMap::new();
        let mut text_bytes = 0usize;
        let mut attr_bytes = 0usize;
        let mut node_count = 0usize;
        let mut max_depth = 0usize;
        fn walk(
            n: &DomNode,
            depth: usize,
            tag_histogram: &mut BTreeMap<String, usize>,
            text_bytes: &mut usize,
            attr_bytes: &mut usize,
            node_count: &mut usize,
            max_depth: &mut usize,
        ) {
            *node_count += 1;
            *max_depth = (*max_depth).max(depth);
            *tag_histogram.entry(n.tag.to_lowercase()).or_default() += 1;
            if let Some(t) = &n.text {
                *text_bytes += t.len();
            }
            for (k, v) in &n.attributes {
                *attr_bytes += k.len() + v.len();
            }
            for c in &n.children {
                walk(
                    c,
                    depth + 1,
                    tag_histogram,
                    text_bytes,
                    attr_bytes,
                    node_count,
                    max_depth,
                );
            }
        }
        walk(
            root,
            0,
            &mut tag_histogram,
            &mut text_bytes,
            &mut attr_bytes,
            &mut node_count,
            &mut max_depth,
        );
        // FNV-1a 64-bit digest over tags + counts (stable, no crypto deps).
        let mut hash: u64 = 0xcbf29ce484222325;
        for (tag, count) in &tag_histogram {
            for b in tag.as_bytes() {
                hash ^= u64::from(*b);
                hash = hash.wrapping_mul(0x100000001b3);
            }
            hash ^= *count as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= node_count as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        DomSnapshotDigest {
            node_count,
            max_depth,
            tag_histogram,
            text_bytes,
            attr_bytes,
            digest_hex: format!("{hash:016x}"),
        }
    }

}

fn json_type(v: &serde_json::Value) -> &'static str {
    use serde_json::Value;
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn walk_hydration(server: &DomNode, client: &DomNode, path: &str, out: &mut Vec<HydrationMismatch>) {
    if out.len() >= 50 {
        return;
    }
    if server.tag.to_lowercase() != client.tag.to_lowercase() {
        out.push(HydrationMismatch {
            path: path.to_string(),
            server: format!("tag:{}", server.tag),
            client: format!("tag:{}", client.tag),
        });
        return;
    }
    let mut s_keys: Vec<_> = server.attributes.keys().cloned().collect();
    s_keys.sort();
    let mut c_keys: Vec<_> = client.attributes.keys().cloned().collect();
    c_keys.sort();
    let mut all = s_keys.clone();
    for k in &c_keys {
        if !all.contains(k) {
            all.push(k.clone());
        }
    }
    all.sort();
    all.dedup();
    for k in all {
        let sv = server.attributes.get(&k).map(|s| s.as_str()).unwrap_or("<missing>");
        let cv = client.attributes.get(&k).map(|s| s.as_str()).unwrap_or("<missing>");
        if sv != cv {
            out.push(HydrationMismatch {
                path: format!("{path}@{k}"),
                server: sv.to_string(),
                client: cv.to_string(),
            });
            if out.len() >= 50 {
                return;
            }
        }
    }
    let st = server.text.as_deref().unwrap_or("").trim();
    let ct = client.text.as_deref().unwrap_or("").trim();
    if st != ct {
        out.push(HydrationMismatch {
            path: format!("{path}#text"),
            server: st.to_string(),
            client: ct.to_string(),
        });
    }
    let max = server.children.len().max(client.children.len());
    for i in 0..max {
        match (server.children.get(i), client.children.get(i)) {
            (Some(s), Some(c)) => {
                let child_path = format!("{path}>{}[{i}]", s.tag.to_lowercase());
                walk_hydration(s, c, &child_path, out);
            }
            (Some(s), None) => out.push(HydrationMismatch {
                path: format!("{path}>{}[{i}]", s.tag.to_lowercase()),
                server: "present".into(),
                client: "<missing>".into(),
            }),
            (None, Some(c)) => out.push(HydrationMismatch {
                path: format!("{path}>{}[{i}]", c.tag.to_lowercase()),
                server: "<missing>".into(),
                client: "present".into(),
            }),
            (None, None) => {}
        }
        if out.len() >= 50 {
            return;
        }
    }
}

/// Minimal HTML → DomNode parser for hydration diffs (tags, attrs, text, nesting).
fn parse_html_to_dom(html: &str) -> DomNode {
    let mut root = DomNode::new("root");
    let mut stack: Vec<DomNode> = vec![DomNode::new("__frag__")];
    let bytes = html.as_bytes();
    let mut i = 0;
    let mut text_buf = String::new();
    let void_tags = [
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ];
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if !text_buf.trim().is_empty() {
                if let Some(top) = stack.last_mut() {
                    if top.text.is_none() {
                        top.text = Some(text_buf.trim().to_string());
                    } else if let Some(t) = top.text.as_mut() {
                        t.push(' ');
                        t.push_str(text_buf.trim());
                    }
                }
            }
            text_buf.clear();
            i += 1;
            if i < bytes.len() && bytes[i] == b'/' {
                // closing tag
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i] != b'>' {
                    i += 1;
                }
                let name = html[start..i].trim().to_lowercase();
                if i < bytes.len() {
                    i += 1;
                }
                if stack.len() > 1 {
                    let finished = stack.pop().unwrap();
                    if finished.tag == name || name.is_empty() {
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(finished);
                        }
                    } else {
                        // mismatched — still attach
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(finished);
                        }
                    }
                }
                continue;
            }
            if i < bytes.len() && (bytes[i] == b'!' || bytes[i] == b'?') {
                while i < bytes.len() && bytes[i] != b'>' {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
                continue;
            }
            let start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' && bytes[i] != b'/' {
                i += 1;
            }
            let tag = html[start..i].to_lowercase();
            let mut attrs = HashMap::new();
            loop {
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i >= bytes.len() || bytes[i] == b'>' || bytes[i] == b'/' {
                    break;
                }
                let kstart = i;
                while i < bytes.len()
                    && bytes[i] != b'='
                    && !bytes[i].is_ascii_whitespace()
                    && bytes[i] != b'>'
                    && bytes[i] != b'/'
                {
                    i += 1;
                }
                let key = html[kstart..i].to_string();
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                let mut val = String::new();
                if i < bytes.len() && bytes[i] == b'=' {
                    i += 1;
                    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                        let q = bytes[i];
                        i += 1;
                        let vstart = i;
                        while i < bytes.len() && bytes[i] != q {
                            i += 1;
                        }
                        val = html[vstart..i].to_string();
                        if i < bytes.len() {
                            i += 1;
                        }
                    } else {
                        let vstart = i;
                        while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                            i += 1;
                        }
                        val = html[vstart..i].to_string();
                    }
                }
                if !key.is_empty() {
                    attrs.insert(key, val);
                }
            }
            let mut self_close = void_tags.contains(&tag.as_str());
            if i < bytes.len() && bytes[i] == b'/' {
                self_close = true;
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'>' {
                i += 1;
            }
            let node = DomNode {
                tag: tag.clone(),
                attributes: attrs,
                text: None,
                children: Vec::new(),
            };
            if self_close {
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                }
            } else {
                stack.push(node);
            }
        } else {
            text_buf.push(bytes[i] as char);
            i += 1;
        }
    }
    if !text_buf.trim().is_empty() {
        if let Some(top) = stack.last_mut() {
            top.text = Some(text_buf.trim().to_string());
        }
    }
    while stack.len() > 1 {
        let finished = stack.pop().unwrap();
        if let Some(parent) = stack.last_mut() {
            parent.children.push(finished);
        }
    }
    let frag = stack.pop().unwrap_or_else(|| DomNode::new("__frag__"));
    if frag.children.len() == 1 {
        frag.children.into_iter().next().unwrap()
    } else {
        root.children = frag.children;
        root
    }
}

/// Parse CDP `CSS.takeCoverageDelta`-shaped JSON into rule coverage rows.
fn parse_css_take_coverage_delta(json: &str) -> Vec<CssRuleCoverage> {
    let v: serde_json::Value = match serde_json::from_str(json.trim()) {
        Ok(v) => v,
        Err(_) => {
            if let Ok(inner) = serde_json::from_str::<String>(json.trim()) {
                serde_json::from_str(&inner).unwrap_or(serde_json::Value::Null)
            } else {
                return Vec::new();
            }
        }
    };
    let sheets = v
        .get("coverage")
        .or_else(|| v.get("result").and_then(|r| r.get("coverage")))
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    let mut rules = Vec::new();
    for sheet in sheets {
        let text = sheet
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        let url = sheet
            .get("url")
            .and_then(|u| u.as_str())
            .unwrap_or("inline")
            .to_string();
        let ranges: Vec<(usize, usize, u64)> = sheet
            .get("ranges")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        Some((
                            r.get("startOffset")?.as_u64()? as usize,
                            r.get("endOffset")?.as_u64()? as usize,
                            r.get("count").and_then(|c| c.as_u64()).unwrap_or(0),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Also accept CDP entries that are only used ranges (no text): synthesize one rule.
        if text.is_empty() {
            let used = ranges.iter().any(|(_, _, c)| *c > 0);
            let bytes = ranges
                .iter()
                .map(|(s, e, _)| e.saturating_sub(*s))
                .sum::<usize>()
                .max(1);
            rules.push(CssRuleCoverage {
                selector: url.clone(),
                used,
                bytes,
            });
            continue;
        }
        // Split stylesheet on `}` rule terminators; track byte offsets.
        let mut start = 0usize;
        let chars: Vec<char> = text.chars().collect();
        let mut byte_at: Vec<usize> = vec![0];
        for ch in &chars {
            byte_at.push(byte_at.last().unwrap() + ch.len_utf8());
        }
        let mut rule_idx = 0usize;
        for (ci, ch) in chars.iter().enumerate() {
            if *ch == '}' {
                let end = byte_at[ci + 1];
                let slice = &text[start..end];
                let selector = slice
                    .split('{')
                    .next()
                    .unwrap_or(slice)
                    .trim()
                    .lines()
                    .last()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let sel = if selector.is_empty() {
                    format!("{url}#rule{rule_idx}")
                } else {
                    selector
                };
                let used = ranges.iter().any(|(rs, re, c)| {
                    *c > 0 && *rs < end && *re > start && (*re).min(end).saturating_sub((*rs).max(start)) > 0
                        && {
                            // Prefer coverage of the selector/body interior, not a 1-byte bleed.
                            let overlap = (*re).min(end).saturating_sub((*rs).max(start));
                            overlap * 2 >= end.saturating_sub(start) || (*rs >= start && *rs < end)
                        }
                });
                rules.push(CssRuleCoverage {
                    selector: sel,
                    used,
                    bytes: end.saturating_sub(start).max(1),
                });
                rule_idx += 1;
                start = end;
            }
        }
        if start < text.len() {
            let used = ranges
                .iter()
                .any(|(rs, re, c)| *c > 0 && *re > start && *rs < text.len());
            rules.push(CssRuleCoverage {
                selector: format!("{url}#tail"),
                used,
                bytes: text.len().saturating_sub(start).max(1),
            });
        }
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_component_tree_detects_boundaries() {
        let root = DomNode::new("div").child(DomNode::new("app-header")).child(
            DomNode::new("div")
                .attr("data-reactroot", "")
                .child(DomNode::new("span")),
        );
        let engine = FrontendEngine::new();
        let tree = engine.build_component_tree(&root);
        assert_eq!(tree.node_count, 4);
        assert!(tree.component_count >= 2);
        assert!(tree.max_depth >= 2);
    }

    #[test]
    fn test_css_specificity_and_cascade() {
        assert_eq!(FrontendEngine::specificity("#id .cls div"), [1, 1, 1]);
        let decls = vec![
            CssDeclaration {
                selector: "div".into(),
                property: "color".into(),
                value: "red".into(),
                important: false,
                order: 0,
            },
            CssDeclaration {
                selector: "#main".into(),
                property: "color".into(),
                value: "blue".into(),
                important: false,
                order: 1,
            },
        ];
        let engine = FrontendEngine::new();
        let winners = engine.resolve_cascade(&decls);
        let color = winners.iter().find(|w| w.property == "color").unwrap();
        assert_eq!(color.winning_value, "blue");
    }

    #[test]
    fn test_contrast_ratio_black_white() {
        let ratio = FrontendEngine::contrast_ratio("#000000", "#ffffff").unwrap();
        assert!((ratio - 21.0).abs() < 0.1);
    }

    #[test]
    fn test_a11y_finds_missing_alt_and_label() {
        let nodes = vec![
            DomNode::new("img"),
            DomNode::new("input"),
            DomNode::new("button").attr("aria-label", "Close"),
        ];
        let engine = FrontendEngine::new();
        let v = engine.accessibility_audit(&nodes);
        assert!(v.iter().any(|x| x.rule == "image-alt"));
        assert!(v.iter().any(|x| x.rule == "form-label"));
        assert!(!v.iter().any(|x| x.rule == "control-name"));
    }

    #[test]
    fn test_state_time_travel_diff() {
        let engine = FrontendEngine::new();
        engine.record_state("initial", json!({"count": 0, "user": "a"}));
        engine.record_state("clicked", json!({"count": 1, "user": "a"}));
        let diff = engine.state_diff(0, 1);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].path, "count");
    }

    #[test]
    fn test_cls_heatmap_excludes_recent_input() {
        let engine = FrontendEngine::new();
        let entries = vec![
            LayoutShiftEntry {
                node: "header".into(),
                score: 0.05,
                had_recent_input: false,
            },
            LayoutShiftEntry {
                node: "ad".into(),
                score: 0.2,
                had_recent_input: false,
            },
            LayoutShiftEntry {
                node: "btn".into(),
                score: 0.5,
                had_recent_input: true,
            },
        ];
        let report = engine.cls_heatmap(&entries);
        assert!((report.total_cls - 0.25).abs() < 1e-9);
        assert_eq!(report.contributors[0].node, "ad");
        assert_eq!(report.rating, "needs-improvement");
    }

    #[test]
    fn test_zindex_conflict_detection() {
        let engine = FrontendEngine::new();
        let els = vec![
            StackingElement {
                selector: ".modal".into(),
                z_index: Some(9999),
                creates_stacking_context: true,
                position: "fixed".into(),
            },
            StackingElement {
                selector: ".a".into(),
                z_index: Some(5),
                creates_stacking_context: true,
                position: "relative".into(),
            },
            StackingElement {
                selector: ".b".into(),
                z_index: Some(5),
                creates_stacking_context: true,
                position: "relative".into(),
            },
        ];
        let report = engine.visualize_zindex(&els);
        assert_eq!(report.max_z, 9999);
        assert!(report.conflicts.iter().any(|c| c.contains("z-index 5")));
        assert!(report.excessive.iter().any(|e| e == ".modal"));
    }

    #[test]
    fn test_responsive_overflow() {
        let engine = FrontendEngine::new();
        let vp = Viewport {
            label: "mobile".into(),
            width: 375.0,
            height: 667.0,
        };
        let boxes = vec![ElementBox {
            selector: ".wide".into(),
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 40.0,
            font_size_px: 16.0,
        }];
        let report = engine.responsive_matrix(&[(vp, boxes)]);
        assert!(report
            .issues
            .iter()
            .any(|i| i.kind == "horizontal-overflow"));
    }

    #[test]
    fn test_hydration_diff_structural() {
        let e = FrontendEngine::new();
        let identical = e.hydration_diff_lens(
            r#"<html><body><div id="a">hi</div></body></html>"#,
            r#"<html><body><div id="a">hi</div></body></html>"#,
        );
        assert_eq!(identical.mismatch_count, 0);
        assert_eq!(identical.severity, "none");

        let attr_drift = e.hydration_diff_lens(
            r#"<html><body><div class="x">hi</div></body></html>"#,
            r#"<html><body><div class="y">hi</div></body></html>"#,
        );
        assert!(attr_drift.mismatch_count >= 1);
        assert!(attr_drift
            .mismatches
            .iter()
            .any(|m| m.path.contains("div[0]@class") || m.path.contains("@class")));
    }

    #[test]
    fn test_css_coverage_take_delta_and_focus_probe() {
        let e = FrontendEngine::new();
        let delta = r#"{
          "coverage": [{
            "url": "app.css",
            "text": ".a{color:red} .b{color:blue}",
            "ranges": [{"startOffset":0,"endOffset":13,"count":1},{"startOffset":13,"endOffset":28,"count":0}]
          }]
        }"#;
        let cov = FrontendEngine::css_coverage_from_take_coverage_delta(delta);
        assert!(!cov.rules.is_empty());
        assert!(cov.rules.iter().any(|r| r.used));
        assert!(cov.rules.iter().any(|r| !r.used), "expected unused rule from count:0 range");
        assert!(cov.unused_bytes > 0);
        assert!(cov.total_bytes > 0);

        let script = FrontendEngine::focus_order_probe_script();
        assert!(script.contains("tab_index") || script.contains("tabIndex") || script.contains("FOCUSABLE"));
        let probe = r##"[{"selector":"#a","tab_index":0,"visible":true,"disabled":false,"order_hint":0},{"selector":"#b","tab_index":1,"visible":true,"disabled":false,"order_hint":1}]"##;
        let nodes = FrontendEngine::parse_focus_order_probe(probe).expect("nodes");
        let fo = e.focus_order_overlay(&nodes);
        assert!(!fo.sequence.is_empty());
        assert_eq!(fo.sequence[0], "#b");
    }

    #[test]
    fn test_hydration_diff_and_css_coverage() {
        let e = FrontendEngine::new();
        let r = e.hydration_diff_lens("<div>a</div>", "<div>b</div>");
        assert!(r.mismatch_count >= 1);
        let cov = e.css_coverage_map(&[
            CssRuleCoverage { selector: ".a".into(), used: true, bytes: 40 },
            CssRuleCoverage { selector: ".b".into(), used: false, bytes: 60 },
        ]);
        assert_eq!(cov.unused_bytes, 60);
        assert!((cov.used_pct - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_focus_order_and_listeners() {
        let e = FrontendEngine::new();
        let fo = e.focus_order_overlay(&[
            FocusableNode { selector: "#a".into(), tab_index: 0, visible: true, disabled: false, order_hint: 1 },
            FocusableNode { selector: "#b".into(), tab_index: 2, visible: true, disabled: false, order_hint: 0 },
        ]);
        assert_eq!(fo.sequence[0], "#b");
        let el = e.event_listener_map(&[
            EventListenerEntry { selector: "win".into(), event_type: "scroll".into(), passive: false, once: false, capture: false },
            EventListenerEntry { selector: "win".into(), event_type: "scroll".into(), passive: false, once: false, capture: false },
        ]);
        assert!(!el.non_passive_scroll.is_empty());
        assert!(!el.duplicate_bindings.is_empty());
    }

    #[test]
    fn test_dom_snapshot_digest_stable() {
        let e = FrontendEngine::new();
        let root = DomNode::new("div").child(DomNode::new("span").attr("id", "x"));
        let a = e.dom_snapshot_capture(&root);
        let b = e.dom_snapshot_capture(&root);
        assert_eq!(a.digest_hex, b.digest_hex);
        assert_eq!(a.node_count, 2);
    }

    #[test]
    fn test_specificity_budget_and_thrash() {
        let e = FrontendEngine::new();
        let bud = e.selector_specificity_budget(&["#a .b c".into(), "div".into()], [0, 2, 1]);
        assert!(!bud.selectors_over_budget.is_empty());
        let thr = e.layout_thrash_detector(&[
            LayoutOp { kind: "measure".into(), selector: ".x".into(), timestamp_ms: 0.0 },
            LayoutOp { kind: "mutate".into(), selector: ".x".into(), timestamp_ms: 1.0 },
            LayoutOp { kind: "measure".into(), selector: ".x".into(), timestamp_ms: 2.0 },
        ]);
        assert!(thr.thrash_windows >= 2);
    }

}
