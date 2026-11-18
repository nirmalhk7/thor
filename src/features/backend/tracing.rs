use super::{BTreeMap, BackendEngine, HashMap, Span, TraceGraph};

impl BackendEngine {
    // 8. Trace Adjacency ----------------------------------------------------
    pub fn build_trace_graph(&self, spans: &[Span]) -> TraceGraph {
        let by_id: HashMap<&str, &Span> = spans.iter().map(|s| (s.span_id.as_str(), s)).collect();
        let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut children: HashMap<&str, Vec<&Span>> = HashMap::new();
        for span in spans {
            link_span(span, &by_id, &mut edges, &mut children);
        }
        // Critical path = longest cumulative duration root->leaf.
        let roots: Vec<&Span> = spans
            .iter()
            .filter(|s| {
                s.parent_id
                    .as_ref()
                    .map(|p| !by_id.contains_key(p.as_str()))
                    .unwrap_or(true)
            })
            .collect();
        let mut best_path = Vec::new();
        let mut best_ms = 0.0;
        for root in roots {
            let mut path = Vec::new();
            let ms = Self::longest_path(root, &children, &mut path);
            if ms > best_ms {
                best_ms = ms;
                best_path = path;
            }
        }
        TraceGraph {
            edges,
            critical_path: best_path,
            critical_path_ms: best_ms,
        }
    }

    fn longest_path(
        span: &Span,
        children: &HashMap<&str, Vec<&Span>>,
        path: &mut Vec<String>,
    ) -> f64 {
        path.push(format!("{}:{}", span.service, span.name));
        let mut best_child_ms = 0.0;
        let mut best_child_path: Vec<String> = Vec::new();
        if let Some(kids) = children.get(span.span_id.as_str()) {
            for k in kids {
                let mut cp = Vec::new();
                let ms = Self::longest_path(k, children, &mut cp);
                if ms > best_child_ms {
                    best_child_ms = ms;
                    best_child_path = cp;
                }
            }
        }
        path.extend(best_child_path);
        span.duration_ms + best_child_ms
    }
}
fn link_span<'a>(
    span: &'a Span,
    by_id: &HashMap<&str, &Span>,
    edges: &mut BTreeMap<String, Vec<String>>,
    children: &mut HashMap<&'a str, Vec<&'a Span>>,
) {
    let Some(parent_id) = &span.parent_id else {
        return;
    };
    let Some(parent) = by_id.get(parent_id.as_str()) else {
        return;
    };
    if parent.service != span.service {
        let neighbors = edges.entry(parent.service.clone()).or_default();
        if !neighbors.contains(&span.service) {
            neighbors.push(span.service.clone());
        }
    }
    children.entry(parent_id.as_str()).or_default().push(span);
}
