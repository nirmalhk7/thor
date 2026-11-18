#[inline(always)]
pub(super) fn to_lower_fast(b: u8) -> u8 {
    b.to_ascii_lowercase()
}

#[inline]
pub(super) fn equal_prefix(haystack: &[u8], query: &[u8]) -> bool {
    haystack.get(..query.len()).is_some_and(|prefix| {
        prefix
            .iter()
            .zip(query)
            .all(|(&a, &b)| to_lower_fast(a) == b)
    })
}

#[inline]
pub(super) fn is_subsequence(haystack: &[u8], query: &[u8]) -> bool {
    if query.len() > haystack.len() {
        return false;
    }
    let mut q_idx = 0;
    let mut target = query[0];
    for &b in haystack {
        if to_lower_fast(b) == target {
            q_idx += 1;
            if q_idx == query.len() {
                return true;
            }
            target = query[q_idx];
        }
    }
    false
}
