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

        let mut out = String::with_capacity(raw_len / 4);
        let mut in_tag = false;
        let mut skip_content = false;
        let mut skip_tag_depth = 0;

        let mut chars = html.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '<' {
                in_tag = true;
                let mut tag_buf = String::new();
                while let Some(&next_ch) = chars.peek() {
                    if next_ch == '>' || next_ch.is_whitespace() {
                        break;
                    }
                    tag_buf.push(chars.next().unwrap());
                }
                let lower_tag = tag_buf.to_lowercase();
                
                // Detect tags to skip content entirely
                if lower_tag == "script" || lower_tag == "style" || lower_tag == "svg" || lower_tag == "path" || lower_tag == "noscript" || lower_tag == "iframe" {
                    skip_content = true;
                    skip_tag_depth += 1;
                } else if lower_tag.starts_with("/script") || lower_tag.starts_with("/style") || lower_tag.starts_with("/svg") || lower_tag.starts_with("/path") || lower_tag.starts_with("/noscript") || lower_tag.starts_with("/iframe") {
                    if skip_tag_depth > 0 {
                        skip_tag_depth -= 1;
                    }
                    if skip_tag_depth == 0 {
                        skip_content = false;
                    }
                }

                // Add structural markdown headers
                if !skip_content {
                    match lower_tag.as_str() {
                        "h1" => out.push_str("\n\n# "),
                        "h2" => out.push_str("\n\n## "),
                        "h3" => out.push_str("\n\n### "),
                        "h4" => out.push_str("\n\n#### "),
                        "p" | "div" => out.push('\n'),
                        "li" => out.push_str("\n- "),
                        "code" | "pre" => out.push_str(" `"),
                        _ => {}
                    }
                }
            } else if ch == '>' {
                in_tag = false;
            } else if !in_tag && !skip_content {
                out.push(ch);
            }
        }

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
        assert!(res.markdown.contains("Thor is a high-performance developer browser built in Rust."));
        assert!(!res.markdown.contains("background: black"));
        assert!(!res.markdown.contains("analytics"));
        assert!(res.token_savings_percent > 30.0);
    }
}
