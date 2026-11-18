mod ascii;
mod text;

use text::{
    contains_prepared, is_subsequence_prepared, starts_with_prepared, word_starts_with_prepared,
};

pub struct PreparedQuery<'a> {
    pub raw: &'a str,
    pub lower_bytes: Vec<u8>,
    pub is_ascii: bool,
}

impl<'a> PreparedQuery<'a> {
    pub fn new(query: &'a str) -> Self {
        let raw = query.trim();
        let is_ascii = raw.is_ascii();
        let lower_bytes = if is_ascii {
            raw.bytes().map(|b| b.to_ascii_lowercase()).collect()
        } else {
            raw.to_lowercase().into_bytes()
        };

        Self {
            raw,
            lower_bytes,
            is_ascii,
        }
    }
}

/// Calculate fuzzy search match score for a palette item (zero-allocation fast path)
pub fn score_palette_item(query: &str, title: &str, url: &str, badge: Option<&str>) -> i32 {
    let prepared = PreparedQuery::new(query);
    score_palette_item_prepared(&prepared, title, url, badge)
}

pub fn score_palette_item_prepared(
    query: &PreparedQuery,
    title: &str,
    url: &str,
    badge: Option<&str>,
) -> i32 {
    if query.raw.is_empty() {
        return 0;
    }

    if starts_with_prepared(title, query) {
        if title.len() == query.lower_bytes.len() {
            1000
        } else {
            800
        }
    } else if word_starts_with_prepared(title, query) {
        600
    } else if contains_prepared(title, query) {
        400
    } else if contains_prepared(url, query) {
        300
    } else if badge.is_some_and(|b| contains_prepared(b, query)) {
        200
    } else if query.lower_bytes.len() <= 4 && is_subsequence_prepared(title, query) {
        100
    } else {
        0
    }
}

#[allow(dead_code)]
#[inline]
fn eq_prepared(s: &str, query: &PreparedQuery) -> bool {
    if s.len() != query.lower_bytes.len() {
        return false;
    }
    if s.is_ascii() && query.is_ascii {
        let s_bytes = s.as_bytes();
        let q_bytes = &query.lower_bytes[..];
        for i in 0..s_bytes.len() {
            if ascii::to_lower_fast(s_bytes[i]) != q_bytes[i] {
                return false;
            }
        }
        true
    } else {
        s.to_lowercase() == query.raw.to_lowercase()
    }
}
