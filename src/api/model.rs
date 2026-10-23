use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    pub name: String,
    pub value: String,
}

impl Header {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self { name: name.into(), value: value.into() }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RequestBody {
    #[default]
    Empty,
    Text { text: String, content_type: String },
    Json { value: serde_json::Value },
    Form { entries: Vec<Header> },
    Binary { data: String, content_type: String },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RequestOptions {
    pub timeout_ms: u64,
    pub response_limit_bytes: usize,
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self { timeout_ms: 30_000, response_limit_bytes: 8 * 1024 * 1024 }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestSpec {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub auth: super::Authentication,
    #[serde(default)]
    pub headers: Vec<Header>,
    #[serde(default)]
    pub query: Vec<Header>,
    #[serde(default)]
    pub body: RequestBody,
    #[serde(default)]
    pub options: RequestOptions,
}

impl RequestSpec {
    pub fn new(method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            method: method.into(), url: url.into(), auth: super::Authentication::None, headers: Vec::new(), query: Vec::new(),
            body: RequestBody::Empty, options: RequestOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseBody {
    /// `utf8` if lossless; otherwise `base64` (never replacement characters).
    pub encoding: String,
    pub data: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    pub status: u16,
    pub headers: Vec<Header>,
    pub body: ResponseBody,
    pub body_bytes: usize,
    pub headers_ms: f64,
    pub total_ms: f64,
}

// Payloads and headers can contain credentials; diagnostics reveal metadata only.
impl std::fmt::Debug for ApiResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiResponse")
            .field("status", &self.status).field("body_bytes", &self.body_bytes)
            .field("total_ms", &self.total_ms).finish_non_exhaustive()
    }
}
