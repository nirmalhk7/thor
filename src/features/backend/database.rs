use super::{BTreeMap, BackendEngine, CacheOp, DbCacheReport, DbReport, HashMap, QueryCount};

impl BackendEngine {
    // 12. DB Adjacency ------------------------------------------------------
    pub fn analyze_db(&self, queries: &[String]) -> DbReport {
        let mut adjacency: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut templates: HashMap<String, usize> = HashMap::new();
        for q in queries {
            let tables = extract_tables(q);
            link_tables(&tables, &mut adjacency);
            *templates.entry(normalize_sql(q)).or_default() += 1;
        }
        let mut n_plus_one: Vec<QueryCount> = templates
            .into_iter()
            .filter(|(_, c)| *c >= 3)
            .map(|(template, count)| QueryCount { template, count })
            .collect();
        n_plus_one.sort_by_key(|item| std::cmp::Reverse(item.count));
        DbReport {
            adjacency,
            n_plus_one,
            total_queries: queries.len(),
        }
    }

    /// DB adjacency analysis with a cache-key ↔ table adjacency graph layered
    /// alongside, so cache access can be reasoned about next to the queries it
    /// fronts (hit/miss ratio + which tables each key shields).
    pub fn analyze_db_cache(&self, queries: &[String], cache_ops: &[CacheOp]) -> DbCacheReport {
        let db = self.analyze_db(queries);
        let mut cache_adjacency: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut hits = 0usize;
        let mut misses = 0usize;
        for op in cache_ops {
            if op.hit {
                hits += 1;
            } else {
                misses += 1;
            }
            if let Some(table) = &op.table {
                let e = cache_adjacency.entry(op.key.clone()).or_default();
                if !e.contains(table) {
                    e.push(table.clone());
                }
            }
        }
        DbCacheReport {
            db,
            cache_adjacency,
            cache_hits: hits,
            cache_misses: misses,
        }
    }
}
/// Normalize a SQL string into a template by stripping literals & whitespace,
/// so repeated identical-shape queries collapse (used for N+1 detection).
fn normalize_sql(sql: &str) -> String {
    let lowered = sql.to_lowercase();
    let mut out = String::with_capacity(lowered.len());
    let mut chars = lowered.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                for n in chars.by_ref() {
                    if n == '\'' {
                        break;
                    }
                }
                out.push('?');
            }
            '0'..='9' => {
                out.push('?');
                while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    chars.next();
                }
            }
            c if c.is_whitespace() => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            c => out.push(c),
        }
    }
    out.trim().to_string()
}

pub(super) fn extract_tables(sql: &str) -> Vec<String> {
    let lowered = sql.to_lowercase();
    let tokens: Vec<&str> = lowered.split_whitespace().collect();
    let mut tables = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        if matches!(*t, "from" | "join" | "into" | "update") && i + 1 < tokens.len() {
            let name = tokens[i + 1].trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
            if !name.is_empty() && name != "select" {
                tables.push(name.to_string());
            }
        }
    }
    tables.dedup();
    tables
}

fn link_tables(tables: &[String], adjacency: &mut BTreeMap<String, Vec<String>>) {
    for (i, table) in tables.iter().enumerate() {
        let others = tables.iter().enumerate().filter(|(j, _)| i != *j);
        for (_, neighbor) in others {
            let neighbors = adjacency.entry(table.clone()).or_default();
            if !neighbors.contains(neighbor) {
                neighbors.push(neighbor.clone());
            }
        }
    }
}
