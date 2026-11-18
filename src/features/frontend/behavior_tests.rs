use super::html::parse_html_to_dom;
use super::*;
use serde_json::json;

#[test]
fn hydration_parser_keeps_eof_text_and_attribute_replacement_rules() {
    let open = parse_html_to_dom("<DIV data-X='one' data-X=two>a<span>x</span>b");
    assert_eq!(open.tag, "div");
    assert_eq!(
        open.attributes.get("data-X").map(String::as_str),
        Some("two")
    );
    assert_eq!(open.text.as_deref(), Some("b"));
    assert_eq!(open.children[0].text.as_deref(), Some("x"));
    let closed = parse_html_to_dom("<div>a<span>x</span>b</div>");
    assert_eq!(closed.text.as_deref(), Some("a b"));
}

#[test]
fn hydration_parser_keeps_unquoted_slashes_and_mismatched_close_behavior() {
    let root = parse_html_to_dom("<custom q=x/><br><b>z</oops>");
    assert_eq!(root.tag, "custom");
    assert_eq!(root.attributes.get("q").map(String::as_str), Some("x/"));
    assert_eq!(
        root.children
            .iter()
            .map(|n| n.tag.as_str())
            .collect::<Vec<_>>(),
        ["br", "b"]
    );
    assert_eq!(root.children[1].text.as_deref(), Some("z"));
}

#[test]
fn css_coverage_keeps_unicode_byte_offsets_range_overlap_and_wrapper_precedence() {
    let input = json!({"coverage":[{"text":".é{a:b}.b{x:y}","url":"style.css","ranges":[{"startOffset":0,"endOffset":8,"count":1},{"startOffset":7,"endOffset":9,"count":1}]}]});
    let report = FrontendEngine::css_coverage_from_take_coverage_delta(&input.to_string());
    assert_eq!(report.rules.len(), 2);
    assert_eq!(report.rules[0].selector, ".é");
    assert!(report.rules[0].used);
    assert_eq!(report.rules[0].bytes, 8);
    assert_eq!(report.rules[1].selector, ".b");
    assert!(!report.rules[1].used);
    assert_eq!(report.total_bytes, 15);
    assert_eq!(report.unused_bytes, 7);
    assert_eq!(report.used_pct, 50.0);
    let wrapped = json!({"coverage":null,"result":input});
    assert!(
        FrontendEngine::css_coverage_from_take_coverage_delta(&wrapped.to_string())
            .rules
            .is_empty()
    );
    let encoded = serde_json::to_string(&input.to_string()).unwrap();
    assert!(
        FrontendEngine::css_coverage_from_take_coverage_delta(&encoded)
            .rules
            .is_empty()
    );
}

#[test]
fn snapshot_keeps_byte_counts_depth_case_folded_tags_and_digest() {
    let mut root = DomNode::new("DIV").attr("é", "雪");
    root.text = Some("ok".into());
    let mut child = DomNode::new("span");
    child.text = Some("é".into());
    root.children.push(child);
    let report = FrontendEngine::new().dom_snapshot_capture(&root);
    assert_eq!(report.node_count, 2);
    assert_eq!(report.max_depth, 1);
    assert_eq!(
        report.tag_histogram,
        [("div".into(), 1), ("span".into(), 1)]
            .into_iter()
            .collect()
    );
    assert_eq!(report.text_bytes, 4);
    assert_eq!(report.attr_bytes, 5);
    assert_eq!(report.digest_hex, "e27407027ccd6fc8");
}

#[test]
fn specificity_keeps_existing_pseudo_element_and_delimiter_scoring() {
    assert_eq!(FrontendEngine::specificity("::before"), [0, 2, 0]);
    assert_eq!(FrontendEngine::specificity("#a\tb"), [1, 0, 1]);
    assert_eq!(
        FrontendEngine::specificity("* > #main .row[data-x=\"x\"]"),
        [1, 2, 0]
    );
}
