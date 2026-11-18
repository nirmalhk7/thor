use super::{DomNode, FrontendEngine, HydrationDiffReport, HydrationMismatch};

use super::html::parse_html_to_dom;
impl FrontendEngine {
    // ---- Wave-2 frontend --------------------------------------------------

    /// Structural hydration diff: parse HTML fragments into [`DomNode`] trees and
    /// walk them. Paths look like `html>body>div[0]` (tag + sibling index).
    pub fn hydration_diff_lens(&self, server_html: &str, client_html: &str) -> HydrationDiffReport {
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
}

fn walk_hydration(
    server: &DomNode,
    client: &DomNode,
    path: &str,
    out: &mut Vec<HydrationMismatch>,
) {
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
        let sv = server
            .attributes
            .get(&k)
            .map(|s| s.as_str())
            .unwrap_or("<missing>");
        let cv = client
            .attributes
            .get(&k)
            .map(|s| s.as_str())
            .unwrap_or("<missing>");
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
