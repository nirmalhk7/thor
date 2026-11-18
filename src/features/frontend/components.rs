use super::{
    json_type, ComponentBinding, ComponentNode, ComponentTree, DomNode, FrontendEngine,
    NetworkEvent, RenderWindow,
};
use std::collections::BTreeMap;

impl FrontendEngine {
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
}
