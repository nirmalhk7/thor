use super::{DecoderStudio, SecurityEngine, StorageFinding, StorageItem};

impl SecurityEngine {
    // 7. Storage Auditor ----------------------------------------------------
    pub fn audit_storage(&self, items: &[StorageItem]) -> Vec<StorageFinding> {
        let mut findings = Vec::new();
        for item in items {
            let key_l = item.key.to_lowercase();
            let looks_sensitive = [
                "token", "secret", "password", "apikey", "api_key", "jwt", "auth", "session",
                "ssn", "credit",
            ]
            .iter()
            .any(|p| key_l.contains(p));
            let value_is_jwt = DecoderStudio::is_jwt(item.value.trim());
            if (looks_sensitive || value_is_jwt) && item.store != "cookie" {
                findings.push(StorageFinding {
                    key: item.key.clone(),
                    store: item.store.clone(),
                    severity: "high".into(),
                    detail:
                        "sensitive value stored in JS-accessible web storage (XSS-exfiltratable)"
                            .into(),
                });
            }
            if item.store == "cookie" && looks_sensitive {
                if !item.http_only {
                    findings.push(StorageFinding {
                        key: item.key.clone(),
                        store: item.store.clone(),
                        severity: "high".into(),
                        detail: "sensitive cookie missing HttpOnly flag".into(),
                    });
                }
                if !item.secure {
                    findings.push(StorageFinding {
                        key: item.key.clone(),
                        store: item.store.clone(),
                        severity: "medium".into(),
                        detail: "sensitive cookie missing Secure flag".into(),
                    });
                }
                if item
                    .same_site
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("none"))
                    .unwrap_or(true)
                {
                    findings.push(StorageFinding {
                        key: item.key.clone(),
                        store: item.store.clone(),
                        severity: "medium".into(),
                        detail: "cookie SameSite is None/unset (CSRF exposure)".into(),
                    });
                }
            }
        }
        findings
    }
}
