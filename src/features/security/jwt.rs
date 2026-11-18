use super::{hmac_sha256, DecoderStudio, JwtDeepReport, SecurityEngine};

impl SecurityEngine {
    // 4. JWT Deep -----------------------------------------------------------
    pub fn inspect_jwt(&self, jwt: &str, now_unix: u64) -> Result<JwtDeepReport, String> {
        let decoded = DecoderStudio::decode_jwt(jwt)?;
        let mut issues = Vec::new();
        let alg = decoded.algorithm.to_lowercase();
        if alg == "none" {
            issues.push("alg=none: signature not verified (critical)".into());
        }
        if alg.starts_with("hs") {
            issues.push(
                "HMAC algorithm: ensure the secret is high-entropy and not a public key".into(),
            );
        }
        if decoded.signature_raw.is_empty() {
            issues.push("empty signature segment".into());
        }
        let mut expired = None;
        if let Some(payload) = &decoded.payload_json {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(payload) {
                match val.get("exp").and_then(|e| e.as_u64()) {
                    Some(exp) => {
                        let is_exp = exp < now_unix;
                        expired = Some(is_exp);
                        if is_exp {
                            issues.push(format!("token expired at exp={exp} (now={now_unix})"));
                        }
                    }
                    None => issues.push("missing 'exp' claim (token never expires)".into()),
                }
                if val.get("alg").is_some() {
                    issues.push("'alg' present in payload — possible confusion with header".into());
                }
            }
        }
        Ok(JwtDeepReport {
            algorithm: decoded.algorithm,
            header_json: decoded.header_json,
            payload_json: decoded.payload_json,
            issues,
            expired,
        })
    }

    /// Locally verify an HS256 JWT signature against a candidate `secret` using a
    /// pure-Rust HMAC-SHA256 (no network, no external crates). Returns `Ok(true)`
    /// when the recomputed signature matches the token's signature segment.
    pub fn verify_jwt_hmac(&self, jwt: &str, secret: &str) -> Result<bool, String> {
        let parts: Vec<&str> = jwt.trim().split('.').collect();
        if parts.len() != 3 {
            return Err("invalid JWT structure".into());
        }
        let decoded = DecoderStudio::decode_jwt(jwt)?;
        if !decoded.algorithm.eq_ignore_ascii_case("HS256") {
            return Err(format!(
                "local verify supports HS256 only (token alg={})",
                decoded.algorithm
            ));
        }
        let signing_input = format!("{}.{}", parts[0], parts[1]);
        let mac = hmac_sha256(secret.as_bytes(), signing_input.as_bytes());
        let expected = base64_url_no_pad(&mac);
        Ok(constant_time_eq(expected.as_bytes(), parts[2].as_bytes()))
    }
}
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// URL-safe base64 encoding without padding (JWT signature format).
fn base64_url_no_pad(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
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
        }
        if chunk.len() > 2 {
            out.push(CHARS[(n & 63) as usize] as char);
        }
    }
    out
}
