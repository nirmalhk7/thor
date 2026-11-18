use super::{ascii, PreparedQuery};

#[inline]
pub(super) fn starts_with_prepared(s: &str, query: &PreparedQuery) -> bool {
    if query.lower_bytes.is_empty() {
        return true;
    }
    if s.is_ascii() && query.is_ascii {
        ascii::equal_prefix(s.as_bytes(), &query.lower_bytes)
    } else {
        s.to_lowercase().starts_with(&query.raw.to_lowercase())
    }
}

#[inline]
pub(super) fn word_starts_with_prepared(s: &str, query: &PreparedQuery) -> bool {
    let q_len = query.lower_bytes.len();
    if q_len == 0 {
        return true;
    }
    if s.is_ascii() && query.is_ascii {
        if s.len() < q_len {
            return false;
        }
        let bytes = s.as_bytes();
        (0..=s.len() - q_len).any(|i| {
            let word_start = i == 0 || matches!(bytes[i - 1], b' ' | b'-' | b'_' | b'/');
            word_start && ascii::equal_prefix(&bytes[i..], &query.lower_bytes)
        })
    } else {
        s.split_whitespace()
            .any(|word| starts_with_prepared(word, query))
    }
}

#[inline]
pub(super) fn contains_prepared(haystack: &str, query: &PreparedQuery) -> bool {
    let q_len = query.lower_bytes.len();
    if q_len == 0 {
        return true;
    }
    if haystack.is_ascii() && query.is_ascii {
        haystack
            .as_bytes()
            .windows(q_len)
            .any(|window| ascii::equal_prefix(window, &query.lower_bytes))
    } else {
        haystack.to_lowercase().contains(&query.raw.to_lowercase())
    }
}

#[inline]
pub(super) fn is_subsequence_prepared(haystack: &str, query: &PreparedQuery) -> bool {
    if query.lower_bytes.is_empty() {
        return true;
    }
    if haystack.is_ascii() && query.is_ascii {
        ascii::is_subsequence(haystack.as_bytes(), &query.lower_bytes)
    } else {
        unicode_subsequence(haystack, query.raw)
    }
}

fn unicode_subsequence(haystack: &str, query: &str) -> bool {
    let sub_lower = query.to_lowercase();
    let full_lower = haystack.to_lowercase();
    let mut sub_chars = sub_lower.chars();
    let mut current = sub_chars.next();
    for c in full_lower.chars() {
        match current {
            Some(target) if c == target => current = sub_chars.next(),
            None => return true,
            _ => {}
        }
    }
    current.is_none()
}
