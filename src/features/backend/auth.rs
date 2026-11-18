use super::{ApiRequest, AuthScheme, BackendEngine, IdorFinding, IdorMatrixReport, IdorProbe};

impl BackendEngine {
    // 2. Auth Vault ---------------------------------------------------------
    pub fn store_auth(&self, name: &str, scheme: AuthScheme) {
        self.vault.lock().insert(name.to_string(), scheme);
    }

    pub fn auth_header(&self, name: &str) -> Option<(String, String)> {
        let scheme = self.vault.lock().get(name)?.clone();
        Some(match scheme {
            AuthScheme::Bearer { token } => ("Authorization".into(), format!("Bearer {token}")),
            AuthScheme::Basic { username, password } => {
                let enc = base64_encode(format!("{username}:{password}").as_bytes());
                ("Authorization".into(), format!("Basic {enc}"))
            }
            AuthScheme::ApiKey { header, key } => (header, key),
            AuthScheme::WebhookSecret { secret } => ("X-Thor-Webhook-Secret".into(), secret),
        })
    }

    /// Resolve a stored webhook HMAC secret from the auth vault (by context name).
    pub fn webhook_secret(&self, name: &str) -> Option<String> {
        match self.vault.lock().get(name)? {
            AuthScheme::WebhookSecret { secret } => Some(secret.clone()),
            _ => None,
        }
    }

    pub fn apply_auth(&self, name: &str, req: &mut ApiRequest) -> bool {
        if let Some((k, v)) = self.auth_header(name) {
            req.headers.insert(k, v);
            true
        } else {
            false
        }
    }

    /// All stored auth context ids (sorted), for the multi-context IDOR matrix.
    pub fn auth_contexts(&self) -> Vec<String> {
        let mut v: Vec<String> = self.vault.lock().keys().cloned().collect();
        v.sort();
        v
    }

    /// Actually send `req` once per auth-vault `context_id`, then classify the
    /// resulting status matrix for IDOR/BOLA. `owner_context` (if any) is the
    /// legitimate owner of the resource; every other context receiving 2xx is a
    /// broken-object-level-authorization finding.
    pub fn run_idor_matrix(
        &self,
        req: &ApiRequest,
        contexts: &[String],
        owner_context: Option<&str>,
    ) -> IdorMatrixReport {
        let mut probes = Vec::new();
        for ctx in contexts {
            let mut r = req.clone();
            let _ = self.apply_auth(ctx, &mut r);
            let status = self.send_blocking(&r).map(|resp| resp.status).unwrap_or(0);
            let is_owner = owner_context.map(|o| o == ctx).unwrap_or(false);
            probes.push(IdorProbe {
                identity: ctx.clone(),
                resource: req.url.clone(),
                is_owner,
                status,
            });
        }
        let findings = self.analyze_idor(&probes);
        IdorMatrixReport { probes, findings }
    }

    // 5. IDOR Matrix --------------------------------------------------------
    pub fn analyze_idor(&self, probes: &[IdorProbe]) -> Vec<IdorFinding> {
        let mut findings = Vec::new();
        for p in probes {
            // A non-owner receiving a success status indicates broken object-level
            // authorization (IDOR / BOLA).
            if !p.is_owner && (200..300).contains(&p.status) {
                findings.push(IdorFinding {
                    identity: p.identity.clone(),
                    resource: p.resource.clone(),
                    status: p.status,
                    severity: "critical".into(),
                    detail: "non-owner identity received a 2xx on another user's resource".into(),
                });
            }
            // An owner being denied their own resource is a false-positive authz bug.
            if p.is_owner && (p.status == 401 || p.status == 403) {
                findings.push(IdorFinding {
                    identity: p.identity.clone(),
                    resource: p.resource.clone(),
                    status: p.status,
                    severity: "medium".into(),
                    detail: "owner denied access to their own resource".into(),
                });
            }
        }
        findings
    }
}
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(CHARS[((n >> 18) & 63) as usize] as char);
        out.push(CHARS[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
