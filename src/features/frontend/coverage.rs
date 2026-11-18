use super::{CssCoverageReport, CssRuleCoverage, FrontendEngine};

impl FrontendEngine {
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
}

/// Parse CDP `CSS.takeCoverageDelta`-shaped JSON into rule coverage rows.
fn parse_css_take_coverage_delta(json: &str) -> Vec<CssRuleCoverage> {
    let Some(value) = coverage_value(json) else {
        return Vec::new();
    };
    let sheets = value
        .get("coverage")
        .or_else(|| value.get("result").and_then(|r| r.get("coverage")))
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    let mut rules = Vec::new();
    for sheet in sheets {
        parse_sheet(&sheet, &mut rules);
    }
    rules
}

fn coverage_value(json: &str) -> Option<serde_json::Value> {
    match serde_json::from_str(json.trim()) {
        Ok(value) => Some(value),
        Err(_) => {
            if let Ok(inner) = serde_json::from_str::<String>(json.trim()) {
                Some(serde_json::from_str(&inner).unwrap_or(serde_json::Value::Null))
            } else {
                None
            }
        }
    }
}

type CoverageRange = (usize, usize, u64);

fn parse_sheet(sheet: &serde_json::Value, rules: &mut Vec<CssRuleCoverage>) {
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
    let ranges = sheet
        .get("ranges")
        .and_then(|r| r.as_array())
        .map(|array| array.iter().filter_map(parse_range).collect::<Vec<_>>())
        .unwrap_or_default();
    if text.is_empty() {
        let used = ranges.iter().any(|(_, _, count)| *count > 0);
        let bytes = ranges
            .iter()
            .map(|(start, end, _)| end.saturating_sub(*start))
            .sum::<usize>()
            .max(1);
        rules.push(CssRuleCoverage {
            selector: url.clone(),
            used,
            bytes,
        });
        return;
    }
    StylesheetCoverage {
        text: &text,
        url: &url,
        ranges: &ranges,
    }
    .append_rules(rules);
}

fn parse_range(range: &serde_json::Value) -> Option<CoverageRange> {
    Some((
        range.get("startOffset")?.as_u64()? as usize,
        range.get("endOffset")?.as_u64()? as usize,
        range.get("count").and_then(|c| c.as_u64()).unwrap_or(0),
    ))
}

struct StylesheetCoverage<'a> {
    text: &'a str,
    url: &'a str,
    ranges: &'a [CoverageRange],
}

impl StylesheetCoverage<'_> {
    fn append_rules(&self, rules: &mut Vec<CssRuleCoverage>) {
        let mut start = 0;
        let mut index = 0;
        for (offset, character) in self.text.char_indices() {
            if character == '}' {
                let end = offset + character.len_utf8();
                rules.push(self.rule(start, end, index));
                index += 1;
                start = end;
            }
        }
        if start < self.text.len() {
            rules.push(self.tail(start));
        }
    }

    fn rule(&self, start: usize, end: usize, index: usize) -> CssRuleCoverage {
        let slice = &self.text[start..end];
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
        let selector = if selector.is_empty() {
            format!("{}#rule{index}", self.url)
        } else {
            selector
        };
        let used = self
            .ranges
            .iter()
            .any(|range| css_range_covers_rule(range, start, end));
        CssRuleCoverage {
            selector,
            used,
            bytes: end.saturating_sub(start).max(1),
        }
    }

    fn tail(&self, start: usize) -> CssRuleCoverage {
        let used = self
            .ranges
            .iter()
            .any(|(begin, end, count)| *count > 0 && *end > start && *begin < self.text.len());
        CssRuleCoverage {
            selector: format!("{}#tail", self.url),
            used,
            bytes: self.text.len().saturating_sub(start).max(1),
        }
    }
}

fn css_range_covers_rule(range: &(usize, usize, u64), start: usize, end: usize) -> bool {
    let (range_start, range_end, count) = *range;
    if count == 0 || range_start >= end || range_end <= start {
        return false;
    }
    let overlap_start = range_start.max(start);
    let overlap_end = range_end.min(end);
    let overlap = overlap_end.saturating_sub(overlap_start);
    // Prefer coverage of the selector/body interior, not a 1-byte bleed.
    overlap.saturating_mul(2) >= end.saturating_sub(start)
        || (range_start >= start && range_start < end)
}
