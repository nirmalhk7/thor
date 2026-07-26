use std::collections::HashSet;

/// Default Tracking Parameter Stripper & Security Anti-Tracking Shield
pub struct SecurityShield;

impl SecurityShield {
    /// List of known tracking parameters to strip automatically from all URLs
    pub fn tracking_parameters() -> HashSet<&'static str> {
        let mut set = HashSet::new();
        set.insert("utm_source");
        set.insert("utm_medium");
        set.insert("utm_campaign");
        set.insert("utm_term");
        set.insert("utm_content");
        set.insert("gclid");
        set.insert("fbclid");
        set.insert("msclkid");
        set.insert("yclid");
        set.insert("mc_eid");
        set.insert("_hsenc");
        set.insert("dclid");
        set.insert("ref_");
        set.insert("s_kwcid");
        set.insert("si"); // YouTube tracking ID
        set.insert("igshid"); // Instagram tracking ID
        set.insert("ttclid"); // TikTok tracking ID
        set
    }

    /// Checks whether a parameter key is a tracking parameter (including any starting with utm_)
    pub fn is_tracking_parameter(key: &str) -> bool {
        let key_lower = key.to_lowercase();
        if key_lower.starts_with("utm_") {
            return true;
        }
        let tracker_set = Self::tracking_parameters();
        tracker_set.contains(key_lower.as_str())
    }

    /// Strips tracking parameters from a URL string, returning the sanitized URL and whether any params were stripped.
    pub fn sanitize_url(url_str: &str) -> (String, bool) {
        if !url_str.contains('?') {
            return (url_str.to_string(), false);
        }

        // Separate hash fragment if present
        let (url_without_hash, fragment) = match url_str.rsplit_once('#') {
            Some((before, hash)) => (before, Some(hash)),
            None => (url_str, None),
        };

        if !url_without_hash.contains('?') {
            return (url_str.to_string(), false);
        }

        let parts: Vec<&str> = url_without_hash.splitn(2, '?').collect();
        let base = parts[0];
        let query = parts[1];

        let mut clean_params = Vec::new();
        let mut stripped = false;

        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let key = pair.split('=').next().unwrap_or("");
            if Self::is_tracking_parameter(key) {
                stripped = true;
            } else {
                clean_params.push(pair);
            }
        }

        if !stripped {
            return (url_str.to_string(), false);
        }

        let clean_query_url = if clean_params.is_empty() {
            base.to_string()
        } else {
            format!("{}?{}", base, clean_params.join("&"))
        };

        let clean_url = match fragment {
            Some(frag) => format!("{}#{}", clean_query_url, frag),
            None => clean_query_url,
        };

        (clean_url, true)
    }

    /// Get standard anti-tracking security headers to attach to requests (DNT and GPC included)
    pub fn get_security_headers() -> Vec<(&'static str, &'static str)> {
        vec![
            ("DNT", "1"),
            ("Sec-GPC", "1"), // Global Privacy Control
            ("X-Content-Type-Options", "nosniff"),
            ("X-Frame-Options", "SAMEORIGIN"),
        ]
    }

    /// Get specific DNT (Do Not Track) and GPC (Global Privacy Control) headers
    pub fn get_dnt_gpc_headers() -> Vec<(&'static str, &'static str)> {
        vec![
            ("DNT", "1"),
            ("Sec-GPC", "1"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_url_tracking_params() {
        let dirty = "https://example.com/page?utm_source=facebook&utm_medium=cpc&gclid=12345&fbclid=abc&msclkid=xyz&yclid=789&mc_eid=def&keep=me";
        let (clean, stripped) = SecurityShield::sanitize_url(dirty);
        assert!(stripped);
        assert_eq!(clean, "https://example.com/page?keep=me");
    }

    #[test]
    fn test_sanitize_url_clean_url() {
        let clean_input = "https://github.com/thor/thor?q=rust";
        let (clean_output, stripped) = SecurityShield::sanitize_url(clean_input);
        assert!(!stripped);
        assert_eq!(clean_output, clean_input);
    }

    #[test]
    fn test_sanitize_url_with_fragment() {
        let dirty = "https://example.com/item?utm_campaign=sale&gclid=111#section2";
        let (clean, stripped) = SecurityShield::sanitize_url(dirty);
        assert!(stripped);
        assert_eq!(clean, "https://example.com/item#section2");
    }

    #[test]
    fn test_dnt_gpc_headers() {
        let headers = SecurityShield::get_dnt_gpc_headers();
        assert!(headers.contains(&("DNT", "1")));
        assert!(headers.contains(&("Sec-GPC", "1")));

        let sec_headers = SecurityShield::get_security_headers();
        assert!(sec_headers.contains(&("DNT", "1")));
        assert!(sec_headers.contains(&("Sec-GPC", "1")));
    }
}

