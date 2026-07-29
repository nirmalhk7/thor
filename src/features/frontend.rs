//! Frontend specialist features (11).
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

/// Frontend specialist engine.
pub struct FrontendEngine {
    snapshots: Mutex<Vec<StateSnapshot>>,
}

impl Default for FrontendEngine {
    fn default() -> Self {
        Self::new()
    }
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
            if prop.contains("color") || val.starts_with('#') || val.starts_with("rgb") {
                if val.starts_with('#') || val.starts_with("rgb") {
                    *colors.entry(val.clone()).or_default() += 1;
                }
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
        ordered.sort_by(|a, b| a.z_index.unwrap_or(0).cmp(&b.z_index.unwrap_or(0)));
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
}
