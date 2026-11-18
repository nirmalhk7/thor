//! IDOR (Insecure Direct Object Reference) Authorization Matrix Engine
//! Executes cross-role request replay across Level 2 Identity Contexts
//! (e.g. Admin, Standard User, Anonymous) and compares HTTP status codes,
//! response headers, and body hash/lengths to generate an IDOR vulnerability matrix.

use crate::net::tamper::InterceptedRequest;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Calculate a structural hash of JSON response keys ignoring dynamic values
pub fn compute_json_structure_hash(body: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
        let mut keys = Vec::new();
        collect_json_keys(&val, "", &mut keys);
        keys.sort();
        let mut hasher = DefaultHasher::new();
        keys.hash(&mut hasher);
        hasher.finish()
    } else {
        let mut hasher = DefaultHasher::new();
        (body.len() / 64).hash(&mut hasher);
        hasher.finish()
    }
}

fn collect_json_keys(val: &serde_json::Value, prefix: &str, keys: &mut Vec<String>) {
    match val {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let full_key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                keys.push(full_key.clone());
                collect_json_keys(v, &full_key, keys);
            }
        }
        serde_json::Value::Array(arr) => {
            if let Some(first) = arr.first() {
                collect_json_keys(first, &format!("{prefix}[]"), keys);
            }
        }
        _ => {}
    }
}

/// Configuration for a specific identity role in IDOR replay tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdorRoleConfig {
    pub role_name: String,
    pub auth_header: Option<String>,
    pub cookie_header: Option<String>,
    pub custom_headers: HashMap<String, String>,
}

impl IdorRoleConfig {
    pub fn new(role_name: impl Into<String>) -> Self {
        Self {
            role_name: role_name.into(),
            auth_header: None,
            cookie_header: None,
            custom_headers: HashMap::new(),
        }
    }

    pub fn with_auth(mut self, token: impl Into<String>) -> Self {
        self.auth_header = Some(token.into());
        self
    }

    pub fn with_cookie(mut self, cookie: impl Into<String>) -> Self {
        self.cookie_header = Some(cookie.into());
        self
    }
}

/// Replay result for a single role
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdorRoleResult {
    pub role_name: String,
    pub status_code: u16,
    pub content_length: usize,
    pub is_potential_vulnerability: bool,
    pub status_description: String,
}

/// Complete IDOR vulnerability matrix evaluation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdorMatrixResult {
    pub target_url: String,
    pub http_method: String,
    pub timestamp: u64,
    pub role_results: Vec<IdorRoleResult>,
    pub vulnerability_count: usize,
}

/// Central IDOR Matrix Engine singleton for Thor
#[derive(Debug, Clone)]
pub struct IdorMatrixEngine {
    client: Client,
    last_matrix: Arc<RwLock<Option<IdorMatrixResult>>>,
}

impl Default for IdorMatrixEngine {
    fn default() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            last_matrix: Arc::new(RwLock::new(None)),
        }
    }
}

impl IdorMatrixEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replay an intercepted request across configured identity roles and build vulnerability matrix
    pub async fn run_matrix(
        &self,
        base_req: &InterceptedRequest,
        roles: Vec<IdorRoleConfig>,
    ) -> IdorMatrixResult {
        let mut role_results = Vec::new();
        let mut vuln_count = 0;

        let active_roles = replay_roles(base_req, roles);
        for role in &active_roles {
            let result = self.replay_role(base_req, role).await;
            if result.is_potential_vulnerability {
                vuln_count += 1;
            }
            role_results.push(result);
        }

        let matrix = IdorMatrixResult {
            target_url: base_req.url.clone(),
            http_method: base_req.method.clone(),
            timestamp: base_req.timestamp,
            role_results,
            vulnerability_count: vuln_count,
        };

        if let Ok(mut last) = self.last_matrix.write() {
            *last = Some(matrix.clone());
        }

        matrix
    }

    /// Get last calculated IDOR matrix result
    pub fn get_last_matrix(&self) -> Option<IdorMatrixResult> {
        self.last_matrix
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idor_role_config_builder() {
        let role = IdorRoleConfig::new("Standard User")
            .with_auth("Bearer tok_123")
            .with_cookie("session=abc");

        assert_eq!(role.role_name, "Standard User");
        assert_eq!(role.auth_header.as_deref(), Some("Bearer tok_123"));
        assert_eq!(role.cookie_header.as_deref(), Some("session=abc"));
    }
}

#[cfg(test)]
mod behavior_tests;

fn replay_roles(base_req: &InterceptedRequest, roles: Vec<IdorRoleConfig>) -> Vec<IdorRoleConfig> {
    if roles.is_empty() {
        let mut original = IdorRoleConfig::new("Original Context");
        for (k, v) in &base_req.headers {
            let k_lower = k.to_lowercase();
            if k_lower == "authorization" {
                original = original.with_auth(v.clone());
            } else if k_lower == "cookie" {
                original = original.with_cookie(v.clone());
            }
        }
        vec![original, IdorRoleConfig::new("Anonymous")]
    } else {
        roles
    }
}

mod replay;
