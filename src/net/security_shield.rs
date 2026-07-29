use std::collections::HashSet;

/// Default Tracking Parameter Stripper & Security Anti-Tracking Shield
pub struct SecurityShield;

const TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "gclid",
    "fbclid",
    "msclkid",
    "yclid",
    "mc_eid",
    "_hsenc",
    "dclid",
    "ref_",
    "s_kwcid",
    "si",
    "igshid",
    "ttclid",
];

impl SecurityShield {
    /// List of known tracking parameters to strip automatically from all URLs
    pub fn tracking_parameters() -> HashSet<&'static str> {
        TRACKING_PARAMS.iter().copied().collect()
    }

    /// Checks whether a parameter key is a tracking parameter (including any starting with utm_)
    pub fn is_tracking_parameter(key: &str) -> bool {
        let key_lower = key.to_lowercase();
        if key_lower.starts_with("utm_") {
            return true;
        }
        TRACKING_PARAMS.contains(&key_lower.as_str())
    }

    /// Helper to strip tracking parameters from a URL path or fragment component containing a `?`.
    fn sanitize_query_string(url_part: &str) -> (String, bool) {
        if !url_part.contains('?') {
            return (url_part.to_string(), false);
        }
        let parts: Vec<&str> = url_part.splitn(2, '?').collect();
        let base = parts[0];
        let query = parts[1];

        let mut clean_params = Vec::new();
        let mut stripped = false;

        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let key = pair.split('=').next().unwrap_or("").trim_start_matches('?');
            if Self::is_tracking_parameter(key) {
                stripped = true;
            } else {
                clean_params.push(pair);
            }
        }

        if !stripped {
            return (url_part.to_string(), false);
        }

        if clean_params.is_empty() {
            (base.to_string(), true)
        } else {
            (format!("{}?{}", base, clean_params.join("&")), true)
        }
    }

    /// Strips tracking parameters from a URL string, returning the sanitized URL and whether any params were stripped.
    pub fn sanitize_url(url_str: &str) -> (String, bool) {
        if !url_str.contains('?') {
            return (url_str.to_string(), false);
        }

        // Separate hash fragment if present (must use split_once to avoid truncating multiple hashes)
        let (url_without_hash, fragment) = match url_str.split_once('#') {
            Some((before, hash)) => (before, Some(hash)),
            None => (url_str, None),
        };

        let (clean_query_url, query_stripped) = Self::sanitize_query_string(url_without_hash);

        let (clean_fragment, frag_stripped) = match fragment {
            Some(frag) => {
                if frag.contains('?') {
                    let (c, s) = Self::sanitize_query_string(frag);
                    (Some(c), s)
                } else {
                    (Some(frag.to_string()), false)
                }
            }
            None => (None, false),
        };

        let stripped = query_stripped || frag_stripped;
        
        if !stripped {
            return (url_str.to_string(), false);
        }

        let clean_url = match clean_fragment {
            Some(frag) => format!("{}#{}", clean_query_url, frag),
            None => clean_query_url,
        };

        (clean_url, true)
    }

    /// Get standard anti-tracking security headers to attach to requests (DNT and GPC included)
    pub fn get_security_headers() -> Vec<(&'static str, &'static str)> {
        vec![
            ("User-Agent", CHROMIUM_USER_AGENT),
            ("Sec-CH-UA", r#""Not/A)Brand";v="8", "Chromium";v="126", "Thor";v="1""#),
            ("Sec-CH-UA-Mobile", "?0"),
            ("Sec-CH-UA-Platform", r#""macOS""#),
            ("DNT", "1"),
            ("Sec-GPC", "1"), // Global Privacy Control
            ("X-Content-Type-Options", "nosniff"),
            ("X-Frame-Options", "SAMEORIGIN"),
        ]
    }

    /// Get modern Chromium standard User-Agent header string
    pub fn chromium_user_agent() -> &'static str {
        CHROMIUM_USER_AGENT
    }

    /// Get specific DNT (Do Not Track) and GPC (Global Privacy Control) headers
    pub fn get_dnt_gpc_headers() -> Vec<(&'static str, &'static str)> {
        vec![
            ("DNT", "1"),
            ("Sec-GPC", "1"),
        ]
    }
}

/// Modern Chromium standard User-Agent header string (Chrome 126+)
pub const CHROMIUM_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Thor/1.0";

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

