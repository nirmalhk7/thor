use super::DomNode;
use std::collections::HashMap;

const VOID_TAGS: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Minimal HTML → DomNode parser for hydration diffs (tags, attrs, text, nesting).
pub(super) fn parse_html_to_dom(html: &str) -> DomNode {
    let root = DomNode::new("root");
    let mut parser = HtmlParser {
        html,
        cursor: 0,
        stack: vec![DomNode::new("__frag__")],
        text: String::new(),
    };
    while parser.peek().is_some() {
        parser.step();
    }
    parser.finish(root)
}

struct HtmlParser<'a> {
    html: &'a str,
    cursor: usize,
    stack: Vec<DomNode>,
    text: String,
}

impl<'a> HtmlParser<'a> {
    fn peek(&self) -> Option<u8> {
        self.html.as_bytes().get(self.cursor).copied()
    }

    fn consume(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn take_while(&mut self, mut predicate: impl FnMut(u8) -> bool) -> &'a str {
        let start = self.cursor;
        while self.peek().is_some_and(&mut predicate) {
            self.cursor += 1;
        }
        &self.html[start..self.cursor]
    }

    fn whitespace(&mut self) {
        self.take_while(|b| b.is_ascii_whitespace());
    }

    fn step(&mut self) {
        if self.peek() == Some(b'<') {
            self.flush_text();
            self.cursor += 1;
            if self.consume(b'/') {
                self.take_while(|b| b != b'>');
                self.consume(b'>');
                // Mismatched closing tags also pop and attach one node.
                self.pop_node();
            } else if matches!(self.peek(), Some(b'!' | b'?')) {
                self.take_while(|b| b != b'>');
                self.consume(b'>');
            } else {
                self.open_tag();
            }
        } else if let Some(byte) = self.peek() {
            self.text.push(byte as char);
            self.cursor += 1;
        }
    }

    fn flush_text(&mut self) {
        if !self.text.trim().is_empty() {
            if let Some(top) = self.stack.last_mut() {
                if top.text.is_none() {
                    top.text = Some(self.text.trim().to_string());
                } else if let Some(text) = top.text.as_mut() {
                    text.push(' ');
                    text.push_str(self.text.trim());
                }
            }
        }
        self.text.clear();
    }

    fn open_tag(&mut self) {
        let tag = self.take_while(tag_byte).to_lowercase();
        let attributes = self.attributes();
        let mut self_close = VOID_TAGS.contains(&tag.as_str());
        if self.consume(b'/') {
            self_close = true;
        }
        self.consume(b'>');
        let node = DomNode {
            tag,
            attributes,
            text: None,
            children: Vec::new(),
        };
        if self_close {
            self.attach(node);
        } else {
            self.stack.push(node);
        }
    }

    fn attributes(&mut self) -> HashMap<String, String> {
        let mut attributes = HashMap::new();
        loop {
            self.whitespace();
            if matches!(self.peek(), None | Some(b'>' | b'/')) {
                break;
            }
            let key = self.take_while(attribute_byte).to_string();
            self.whitespace();
            let value = self.attribute_value();
            if !key.is_empty() {
                attributes.insert(key, value);
            }
        }
        attributes
    }

    fn attribute_value(&mut self) -> String {
        if !self.consume(b'=') {
            return String::new();
        }
        self.whitespace();
        if let Some(quote @ (b'"' | b'\'')) = self.peek() {
            self.cursor += 1;
            let value = self.take_while(|b| b != quote).to_string();
            self.consume(quote);
            value
        } else {
            self.take_while(|b| !b.is_ascii_whitespace() && b != b'>')
                .to_string()
        }
    }

    fn attach(&mut self, node: DomNode) {
        if let Some(parent) = self.stack.last_mut() {
            parent.children.push(node);
        }
    }

    fn pop_node(&mut self) {
        if self.stack.len() > 1 {
            let node = self.stack.pop().unwrap();
            self.attach(node);
        }
    }

    fn finish(mut self, mut root: DomNode) -> DomNode {
        // EOF text replaces existing text; text before a tag is appended above.
        if !self.text.trim().is_empty() {
            if let Some(top) = self.stack.last_mut() {
                top.text = Some(self.text.trim().to_string());
            }
        }
        while self.stack.len() > 1 {
            self.pop_node();
        }
        let fragment = self.stack.pop().unwrap_or_else(|| DomNode::new("__frag__"));
        if fragment.children.len() == 1 {
            fragment.children.into_iter().next().unwrap()
        } else {
            root.children = fragment.children;
            root
        }
    }
}

fn tag_byte(byte: u8) -> bool {
    !byte.is_ascii_whitespace() && !matches!(byte, b'>' | b'/')
}
fn attribute_byte(byte: u8) -> bool {
    !byte.is_ascii_whitespace() && !matches!(byte, b'=' | b'>' | b'/')
}
