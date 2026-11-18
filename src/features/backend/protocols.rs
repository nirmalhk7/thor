use super::{
    BTreeMap, BackendEngine, GraphQlAnalysis, GrpcCall, GrpcStreamFrame, GrpcStreamReport,
};

impl BackendEngine {
    // 7. gRPC / GraphQL -----------------------------------------------------
    pub fn analyze_graphql(&self, query: &str) -> GraphQlAnalysis {
        let trimmed = query.trim();
        let operation_type = if trimmed.starts_with("mutation") {
            "mutation"
        } else if trimmed.starts_with("subscription") {
            "subscription"
        } else {
            "query"
        };
        // Operation name follows the keyword.
        let operation_name = trimmed
            .split_whitespace()
            .nth(1)
            .filter(|s| *s != "{" && !s.starts_with('{'))
            .map(|s| s.trim_end_matches('{').trim_end_matches('(').to_string())
            .filter(|s| !s.is_empty());

        let is_introspection = trimmed.contains("__schema") || trimmed.contains("__type");

        // Brace-depth walk to compute max nesting and top-level fields.
        let mut depth: i32 = 0;
        let mut max_depth = 0i32;
        let mut top_level = Vec::new();
        let mut token = String::new();
        for ch in trimmed.chars() {
            match ch {
                '{' => {
                    let t = token.trim().to_string();
                    if depth == 1 && !t.is_empty() {
                        top_level.push(last_ident(&t));
                    }
                    depth += 1;
                    max_depth = max_depth.max(depth);
                    token.clear();
                }
                '}' => {
                    depth -= 1;
                    token.clear();
                }
                '\n' | ',' => {
                    let t = token.trim().to_string();
                    if depth == 1 && !t.is_empty() {
                        top_level.push(last_ident(&t));
                    }
                    token.clear();
                }
                _ => token.push(ch),
            }
        }
        top_level.retain(|f| !f.is_empty() && !f.starts_with(operation_type));
        top_level.dedup();
        GraphQlAnalysis {
            operation_type: operation_type.to_string(),
            operation_name,
            top_level_fields: top_level,
            max_depth: max_depth.max(0) as usize,
            is_introspection,
        }
    }

    pub fn parse_grpc_path(&self, path: &str) -> Option<GrpcCall> {
        let clean = path.trim_start_matches('/');
        let (svc_full, method) = clean.split_once('/')?;
        let (package, service) = match svc_full.rsplit_once('.') {
            Some((p, s)) => (p.to_string(), s.to_string()),
            None => (String::new(), svc_full.to_string()),
        };
        if method.is_empty() || service.is_empty() {
            return None;
        }
        Some(GrpcCall {
            package,
            service,
            method: method.to_string(),
        })
    }

    pub fn grpc_stream_workbench(&self, frames: &[GrpcStreamFrame]) -> GrpcStreamReport {
        let mut streams = BTreeMap::<u32, usize>::new();
        let mut total_bytes = 0usize;
        let mut issues = Vec::new();
        for f in frames {
            *streams.entry(f.stream_id).or_default() += 1;
            total_bytes += f.message_bytes;
            if f.flags.contains("END") && f.message_bytes == 0 && streams[&f.stream_id] == 1 {
                issues.push(format!(
                    "stream {} ended with empty sole frame",
                    f.stream_id
                ));
            }
        }
        for w in frames.windows(2) {
            if w[0].stream_id == w[1].stream_id && w[1].timestamp_ms < w[0].timestamp_ms {
                issues.push(format!(
                    "out-of-order timestamps on stream {}",
                    w[0].stream_id
                ));
            }
        }
        GrpcStreamReport {
            frames: frames.len(),
            total_bytes,
            streams: streams.len(),
            issues,
        }
    }
}
fn last_ident(s: &str) -> String {
    let s = s.trim().trim_end_matches('(');
    s.split(|c: char| c.is_whitespace() || c == ':' || c == '(')
        .rfind(|t| !t.is_empty())
        .unwrap_or("")
        .to_string()
}
