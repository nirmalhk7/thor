//! Frontend specialist features (11 + 12 wave-2).
//!
//! Component tree, responsive matrix, CSS cascade, design tokens, accessibility,
//! state time-travel, CLS heatmap, component sandbox, network↔component binding,
//! font auditing, and z-index visualization. All logic is pure Rust operating on
//! captured DOM / CSS / layout data (no fake randomness).

mod accessibility;
mod cascade;
mod colors;
mod components;
mod coverage;
mod focus;
mod html;
mod hydration;
mod layout;
mod probes;
mod resources;
mod responsive;
mod snapshot;
mod state;
mod tokens;

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
        assert!(
            cov.rules.iter().any(|r| !r.used),
            "expected unused rule from count:0 range"
        );
        assert!(cov.unused_bytes > 0);
        assert!(cov.total_bytes > 0);

        let script = FrontendEngine::focus_order_probe_script();
        assert!(
            script.contains("tab_index")
                || script.contains("tabIndex")
                || script.contains("FOCUSABLE")
        );
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
            CssRuleCoverage {
                selector: ".a".into(),
                used: true,
                bytes: 40,
            },
            CssRuleCoverage {
                selector: ".b".into(),
                used: false,
                bytes: 60,
            },
        ]);
        assert_eq!(cov.unused_bytes, 60);
        assert!((cov.used_pct - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_focus_order_and_listeners() {
        let e = FrontendEngine::new();
        let fo = e.focus_order_overlay(&[
            FocusableNode {
                selector: "#a".into(),
                tab_index: 0,
                visible: true,
                disabled: false,
                order_hint: 1,
            },
            FocusableNode {
                selector: "#b".into(),
                tab_index: 2,
                visible: true,
                disabled: false,
                order_hint: 0,
            },
        ]);
        assert_eq!(fo.sequence[0], "#b");
        let el = e.event_listener_map(&[
            EventListenerEntry {
                selector: "win".into(),
                event_type: "scroll".into(),
                passive: false,
                once: false,
                capture: false,
            },
            EventListenerEntry {
                selector: "win".into(),
                event_type: "scroll".into(),
                passive: false,
                once: false,
                capture: false,
            },
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
            LayoutOp {
                kind: "measure".into(),
                selector: ".x".into(),
                timestamp_ms: 0.0,
            },
            LayoutOp {
                kind: "mutate".into(),
                selector: ".x".into(),
                timestamp_ms: 1.0,
            },
            LayoutOp {
                kind: "measure".into(),
                selector: ".x".into(),
                timestamp_ms: 2.0,
            },
        ]);
        assert!(thr.thrash_windows >= 2);
    }
}

#[cfg(test)]
mod behavior_tests;
