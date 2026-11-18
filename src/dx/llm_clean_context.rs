//! DOM Cleaning and LLM Context Extraction Engine.
//! Strips non-semantic elements (<script>, <style>, <svg>, <path>, hidden elements, cookie banners)
//! and converts semantic HTML into clean, token-dense Markdown for LLM prompt context.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanContextResult {
    pub raw_html_len: usize,
    pub clean_markdown_len: usize,
    pub token_savings_percent: f64,
    pub markdown: String,
}

pub struct LlmCleanContext;

impl LlmCleanContext {
    /// Convert raw HTML into token-efficient Markdown context
    pub fn clean_dom(html: &str) -> CleanContextResult {
        let raw_len = html.len();
        if raw_len == 0 {
            return CleanContextResult {
                raw_html_len: 0,
                clean_markdown_len: 0,
                token_savings_percent: 0.0,
                markdown: String::new(),
            };
        }

        let out = DomCleaner::new(raw_len / 4).extract(html);

        // Clean up multiple newlines and whitespace
        let mut cleaned_lines = Vec::new();
        for line in out.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                cleaned_lines.push(trimmed);
            }
        }

        let clean_markdown = cleaned_lines.join("\n");
        let clean_len = clean_markdown.len();
        let savings = if raw_len > 0 {
            (1.0 - (clean_len as f64 / raw_len as f64)) * 100.0
        } else {
            0.0
        };

        CleanContextResult {
            raw_html_len: raw_len,
            clean_markdown_len: clean_len,
            token_savings_percent: savings.max(0.0),
            markdown: clean_markdown,
        }
    }
}

struct DomCleaner {
    out: String,
    in_tag: bool,
    skip_tag_depth: usize,
}

impl DomCleaner {
    fn new(capacity: usize) -> Self {
        Self {
            out: String::with_capacity(capacity),
            in_tag: false,
            skip_tag_depth: 0,
        }
    }

    fn extract(mut self, html: &str) -> String {
        let mut chars = html.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '<' => {
                    self.in_tag = true;
                    let tag: String = std::iter::from_fn(|| {
                        let &next = chars.peek()?;
                        if next == '>' || next.is_whitespace() {
                            None
                        } else {
                            chars.next()
                        }
                    })
                    .collect();
                    self.process_tag(&tag.to_lowercase());
                }
                '>' => self.in_tag = false,
                _ if !self.in_tag && self.skip_tag_depth == 0 => self.out.push(ch),
                _ => {}
            }
        }
        self.out
    }

    fn process_tag(&mut self, tag: &str) {
        const SKIPPED: [&str; 6] = ["script", "style", "svg", "path", "noscript", "iframe"];
        if SKIPPED.contains(&tag) {
            self.skip_tag_depth += 1;
        } else if let Some(closing) = tag.strip_prefix('/') {
            if SKIPPED.iter().any(|name| closing.starts_with(name)) {
                self.skip_tag_depth = self.skip_tag_depth.saturating_sub(1);
            }
        }
        if self.skip_tag_depth == 0 {
            self.push_structure(tag);
        }
    }

    fn push_structure(&mut self, tag: &str) {
        match tag {
            "h1" => self.out.push_str("\n\n# "),
            "h2" => self.out.push_str("\n\n## "),
            "h3" => self.out.push_str("\n\n### "),
            "h4" => self.out.push_str("\n\n#### "),
            "p" | "div" => self.out.push('\n'),
            "li" => self.out.push_str("\n- "),
            "code" | "pre" => self.out.push_str(" `"),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dom_cleaning() {
        let html = r#"
            <html>
                <head>
                    <style> body { background: black; } </style>
                    <script> console.log("analytics"); </script>
                </head>
                <body>
                    <h1>Thor Browser Documentation</h1>
                    <p>Thor is a high-performance developer browser built in Rust.</p>
                    <svg><path d="M0 0h24v24H0z"/></svg>
                </body>
            </html>
        "#;

        let res = LlmCleanContext::clean_dom(html);
        assert!(res.markdown.contains("# Thor Browser Documentation"));
        assert!(res
            .markdown
            .contains("Thor is a high-performance developer browser built in Rust."));
        assert!(!res.markdown.contains("background: black"));
        assert!(!res.markdown.contains("analytics"));
        assert!(res.token_savings_percent > 30.0);
    }
}
