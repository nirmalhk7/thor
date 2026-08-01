use base64::Engine as _;
use ring::signature::{UnparsedPublicKey, ED25519};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const FILTER_BUNDLE_KEY_ID: &str = "thor-filter-release-2026-02";
const FILTER_BUNDLE_PUBLIC_KEY_BASE64: &str = "H8NAbS0d4iubjJ17WXXbO5ZEOBVLcRFrRf4IAEJdXe0=";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FilterBundleManifest {
    schema_version: u32,
    bundle_version: String,
    ruleset_id: String,
    rules_path: String,
    rules_sha256: String,
    rule_count: usize,
    key_id: String,
    issued_at: String,
    expires_at: String,
    issued_at_unix: u64,
    expires_at_unix: u64,
    source: String,
    source_revision: String,
    signature_algorithm: String,
    signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SignedFilterBundleFields<'a> {
    schema_version: u32,
    bundle_version: &'a str,
    ruleset_id: &'a str,
    rules_path: &'a str,
    rules_sha256: &'a str,
    rule_count: usize,
    key_id: &'a str,
    issued_at: &'a str,
    expires_at: &'a str,
    issued_at_unix: u64,
    expires_at_unix: u64,
    source: &'a str,
    source_revision: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedFilterBundle {
    pub bundle_version: String,
    pub rule_count: usize,
    pub key_id: String,
    pub source: String,
    pub source_revision: String,
    pub expires_at_unix: u64,
    /// Honest implementation boundary: Chromium DNR extension rules, not a
    /// native network-service filter.
    pub enforcement: &'static str,
}

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
    /// Verify the packaged extension rules before Chromium launch. Callers must
    /// fail closed on `Err`; runtime extension verification is defense in depth.
    pub fn verify_filter_bundle(extension_dir: &Path) -> Result<VerifiedFilterBundle, String> {
        let manifest_path = extension_dir.join("filter_bundle.json");
        let manifest_bytes = read_regular_file(&manifest_path)?;
        let manifest: FilterBundleManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| format!("parse filter bundle manifest: {error}"))?;

        if manifest.schema_version != 1
            || manifest.ruleset_id != "thor_shield_rules"
            || manifest.rules_path != "rules.json"
            || manifest.key_id != FILTER_BUNDLE_KEY_ID
            || manifest.signature_algorithm != "Ed25519"
            || manifest.bundle_version.trim().is_empty()
            || manifest.source.trim().is_empty()
            || manifest.source_revision.trim().is_empty()
        {
            return Err("filter bundle identity, provenance, or algorithm is invalid".into());
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("read wall clock for filter validity: {error}"))?
            .as_secs();
        if manifest.issued_at_unix > now.saturating_add(Duration::from_secs(300).as_secs())
            || manifest.expires_at_unix <= now
            || manifest.expires_at_unix <= manifest.issued_at_unix
        {
            return Err("filter bundle validity window is invalid or expired".into());
        }

        let rules_path = extension_dir.join(&manifest.rules_path);
        let rules_bytes = read_regular_file(&rules_path)?;
        let digest = Sha256::digest(&rules_bytes);
        let actual_digest = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if actual_digest != manifest.rules_sha256 {
            return Err("filter rules SHA-256 does not match signed manifest".into());
        }
        let rules: serde_json::Value = serde_json::from_slice(&rules_bytes)
            .map_err(|error| format!("parse filter rules: {error}"))?;
        let actual_rule_count = rules
            .as_array()
            .ok_or("filter rules must be a JSON array")?
            .len();
        if actual_rule_count != manifest.rule_count || actual_rule_count == 0 {
            return Err("filter rule count does not match signed manifest".into());
        }

        let signed_fields = SignedFilterBundleFields {
            schema_version: manifest.schema_version,
            bundle_version: &manifest.bundle_version,
            ruleset_id: &manifest.ruleset_id,
            rules_path: &manifest.rules_path,
            rules_sha256: &manifest.rules_sha256,
            rule_count: manifest.rule_count,
            key_id: &manifest.key_id,
            issued_at: &manifest.issued_at,
            expires_at: &manifest.expires_at,
            issued_at_unix: manifest.issued_at_unix,
            expires_at_unix: manifest.expires_at_unix,
            source: &manifest.source,
            source_revision: &manifest.source_revision,
        };
        let payload = serde_json::to_vec(&signed_fields)
            .map_err(|error| format!("canonicalize filter manifest: {error}"))?;
        let public_key = base64::engine::general_purpose::STANDARD
            .decode(FILTER_BUNDLE_PUBLIC_KEY_BASE64)
            .map_err(|error| format!("decode pinned filter public key: {error}"))?;
        let signature = base64::engine::general_purpose::STANDARD
            .decode(&manifest.signature)
            .map_err(|error| format!("decode filter signature: {error}"))?;
        UnparsedPublicKey::new(&ED25519, public_key)
            .verify(&payload, &signature)
            .map_err(|_| "filter manifest Ed25519 signature is invalid".to_string())?;

        Ok(VerifiedFilterBundle {
            bundle_version: manifest.bundle_version,
            rule_count: manifest.rule_count,
            key_id: manifest.key_id,
            source: manifest.source,
            source_revision: manifest.source_revision,
            expires_at_unix: manifest.expires_at_unix,
            enforcement: "chromium-extension-dnr-transitional",
        })
    }

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
            ("Sec-CH-UA", r#""Not/A)Brand";v="8", "Chromium";v="151""#),
            ("Sec-CH-UA-Mobile", "?0"),
            ("Sec-CH-UA-Platform", r#""macOS""#),
            ("DNT", "1"),
            ("Sec-GPC", "1"), // Global Privacy Control
        ]
    }

    /// Get modern Chromium standard User-Agent header string
    pub fn chromium_user_agent() -> &'static str {
        CHROMIUM_USER_AGENT
    }

    /// Get specific DNT (Do Not Track) and GPC (Global Privacy Control) headers
    pub fn get_dnt_gpc_headers() -> Vec<(&'static str, &'static str)> {
        vec![("DNT", "1"), ("Sec-GPC", "1")]
    }
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("read {} metadata: {error}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(format!(
            "expected regular non-symlink file: {}",
            path.display()
        ));
    }
    std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))
}

/// Generic Chromium User-Agent matching Thor's pinned browser major.
pub const CHROMIUM_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36";

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

    #[test]
    fn packaged_filter_bundle_has_valid_pinned_signature() {
        let extension_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("browser/extensions/thor-shield");
        let verified = SecurityShield::verify_filter_bundle(&extension_dir).unwrap();
        assert_eq!(verified.rule_count, 44);
        assert_eq!(verified.key_id, FILTER_BUNDLE_KEY_ID);
        assert_eq!(verified.enforcement, "chromium-extension-dnr-transitional");
    }

    #[test]
    fn tampered_filter_rules_fail_closed() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("browser/extensions/thor-shield");
        let test_dir = std::env::temp_dir().join(format!(
            "thor-filter-tamper-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&test_dir).unwrap();
        std::fs::copy(
            source.join("filter_bundle.json"),
            test_dir.join("filter_bundle.json"),
        )
        .unwrap();
        let mut rules = std::fs::read(source.join("rules.json")).unwrap();
        rules.push(b' ');
        std::fs::write(test_dir.join("rules.json"), rules).unwrap();
        let error = SecurityShield::verify_filter_bundle(&test_dir).unwrap_err();
        assert!(error.contains("SHA-256"));
        let _ = std::fs::remove_dir_all(test_dir);
    }
}
