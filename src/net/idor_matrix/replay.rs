use super::*;

impl IdorMatrixEngine {
    fn role_request(
        &self,
        base_req: &InterceptedRequest,
        role: &IdorRoleConfig,
    ) -> reqwest::RequestBuilder {
        let mut req_builder = match base_req.method.to_uppercase().as_str() {
            "POST" => self.client.post(&base_req.url),
            "PUT" => self.client.put(&base_req.url),
            "DELETE" => self.client.delete(&base_req.url),
            "PATCH" => self.client.patch(&base_req.url),
            _ => self.client.get(&base_req.url),
        };

        // Copy base headers except auth and cookies
        for (k, v) in &base_req.headers {
            let k_lower = k.to_lowercase();
            if k_lower != "authorization" && k_lower != "cookie" {
                req_builder = req_builder.header(k, v);
            }
        }

        // Inject role specific auth & cookie tokens
        if let Some(ref auth) = role.auth_header {
            req_builder = req_builder.header("Authorization", auth);
        }
        if let Some(ref cookie) = role.cookie_header {
            req_builder = req_builder.header("Cookie", cookie);
        }
        for (k, v) in &role.custom_headers {
            req_builder = req_builder.header(k, v);
        }

        if let Some(ref body) = base_req.body {
            req_builder = req_builder.body(body.clone());
        }

        req_builder
    }

    pub(super) async fn replay_role(
        &self,
        base_req: &InterceptedRequest,
        role: &IdorRoleConfig,
    ) -> IdorRoleResult {
        match self.role_request(base_req, role).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                classify_response(role, status, &body)
            }
            Err(error) => IdorRoleResult {
                role_name: role.role_name.clone(),
                status_code: 0,
                content_length: 0,
                is_potential_vulnerability: false,
                status_description: format!("Error: {}", error),
            },
        }
    }
}

fn classify_response(role: &IdorRoleConfig, status: u16, body_text: &str) -> IdorRoleResult {
    let bytes = body_text.len();
    let struct_hash = compute_json_structure_hash(body_text);
    // Keep the replay heuristic's role substring and error-body matching.
    let is_error_body = body_text.to_lowercase().contains("unauthorized")
        || body_text.to_lowercase().contains("forbidden")
        || body_text.to_lowercase().contains("access denied");
    let vulnerable = (role.role_name.to_lowercase().contains("user")
        || role.role_name.to_lowercase().contains("anonymous"))
        && status < 300
        && !is_error_body;
    IdorRoleResult {
        role_name: role.role_name.clone(),
        status_code: status,
        content_length: bytes,
        is_potential_vulnerability: vulnerable,
        status_description: if vulnerable {
            format!(
                "⚠️ POTENTIAL IDOR: Status {} (Hash: {:x})",
                status, struct_hash
            )
        } else {
            format!("Status {}", status)
        },
    }
}
