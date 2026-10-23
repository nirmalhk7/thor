use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{header::{HeaderName, HeaderValue, CONTENT_TYPE}, Client, Method, RequestBuilder, Url};

use super::{ApiResponse, Header, RequestBody, RequestSpec, ResponseBody, Variables};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Authentication conflicts with explicit credentials or is invalid")]
    InvalidAuth,
    #[error(transparent)]
    Credential(#[from] super::SecretError),
    #[error("Request cancelled")]
    Cancelled,
    #[error("Invalid HTTP method")]
    InvalidMethod,
    #[error("URL must be HTTP(S), without embedded credentials or a fragment")]
    InvalidUrl,
    #[error("Invalid HTTP header")]
    InvalidHeader,
    #[error("Invalid request options")]
    InvalidOptions,
    #[error("Unclosed variable template")]
    InvalidTemplate,
    #[error("Unresolved variable: {0}")]
    MissingVariable(String),
    #[error("Invalid base64 request body")]
    InvalidBase64,
    #[error("Response size limit exceeded")]
    ResponseLimit,
    #[error("Request body size limit exceeded")]
    RequestLimit,
    #[error("HTTP transport: {0}")]
    Transport(reqwest::Error),
}

impl From<reqwest::Error> for ApiError {
    fn from(error: reqwest::Error) -> Self {
        Self::Transport(error.without_url())
    }
}

#[derive(Clone)]
pub struct HttpService { client: Client, secrets: std::sync::Arc<super::SecretVault> }

impl HttpService {
    pub fn new() -> Result<Self, ApiError> {
        Self::with_secrets(std::sync::Arc::new(super::SecretVault::default()))
    }

    pub fn with_secrets(secrets: std::sync::Arc<super::SecretVault>) -> Result<Self, ApiError> {
        Ok(Self { client: Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10)).build()?, secrets })
    }

    pub fn secrets(&self) -> &super::SecretVault { &self.secrets }

    pub async fn send(&self, spec: &RequestSpec, variables: &Variables) -> Result<ApiResponse, ApiError> {
        self.send_cancellable(spec, variables, &super::CancellationToken::new()).await
    }

    pub async fn send_cancellable(&self, spec: &RequestSpec, variables: &Variables, cancellation: &super::CancellationToken) -> Result<ApiResponse, ApiError> {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(ApiError::Cancelled),
            result = self.execute(spec, variables) => result,
        }
    }

    async fn execute(&self, spec: &RequestSpec, variables: &Variables) -> Result<ApiResponse, ApiError> {
        let request = self.prepare(spec, variables)?;
        let start = Instant::now();
        let mut response = request.send().await?;
        let headers_ms = start.elapsed().as_secs_f64() * 1000.0;
        let status = response.status().as_u16();
        let headers = response.headers().iter().map(|(name, value)| {
            Header::new(name.as_str(), String::from_utf8_lossy(value.as_bytes()).into_owned())
        }).collect();
        let mut bytes = Vec::new();
        let limit = spec.options.response_limit_bytes;
        if response.content_length().is_some_and(|length| length > limit as u64) {
            return Err(ApiError::ResponseLimit);
        }
        while let Some(chunk) = response.chunk().await? {
            if chunk.len() > limit.saturating_sub(bytes.len()) { return Err(ApiError::ResponseLimit); }
            bytes.extend_from_slice(&chunk);
        }
        let body_bytes = bytes.len();
        let body = match String::from_utf8(bytes) {
            Ok(data) => ResponseBody { encoding: "utf8".into(), data },
            Err(error) => ResponseBody { encoding: "base64".into(), data: STANDARD.encode(error.into_bytes()) },
        };
        Ok(ApiResponse { status, headers, body, body_bytes, headers_ms, total_ms: start.elapsed().as_secs_f64() * 1000.0 })
    }

    fn prepare(&self, spec: &RequestSpec, variables: &Variables) -> Result<RequestBuilder, ApiError> {
        validate_options(spec)?;
        let method = Method::from_bytes(spec.method.as_bytes()).map_err(|_| ApiError::InvalidMethod)?;
        let resolved = variables.resolve(&spec.url)?;
        let mut url = Url::parse(&resolved).map_err(|_| ApiError::InvalidUrl)?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none()
            || !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
            return Err(ApiError::InvalidUrl);
        }
        for entry in &spec.query {
            url.query_pairs_mut().append_pair(&variables.resolve(&entry.name)?, &variables.resolve(&entry.value)?);
        }
        let auth = spec.auth.resolve(variables, &self.secrets)?;
        if let Some(query) = auth.query {
            if url.query_pairs().any(|(key, _)| key == query.name) { return Err(ApiError::InvalidAuth); }
            url.query_pairs_mut().append_pair(&query.name, &query.value);
        }
        let mut request = self.client.request(method, url).timeout(Duration::from_millis(spec.options.timeout_ms));
        for header in &spec.headers {
            let name = HeaderName::from_bytes(header.name.as_bytes()).map_err(|_| ApiError::InvalidHeader)?;
            let value = HeaderValue::from_str(&variables.resolve(&header.value)?).map_err(|_| ApiError::InvalidHeader)?;
            request = request.header(name, value);
        }
        if let Some(header) = auth.header {
            if spec.headers.iter().any(|entry| entry.name.eq_ignore_ascii_case(&header.name)) {
                return Err(ApiError::InvalidAuth);
            }
            let name = HeaderName::from_bytes(header.name.as_bytes()).map_err(|_| ApiError::InvalidHeader)?;
            let value = HeaderValue::from_str(&header.value).map_err(|_| ApiError::InvalidHeader)?;
            request = request.header(name, value);
        }
        let default_content_type = !spec.headers.iter().any(|header| header.name.eq_ignore_ascii_case("content-type"));
        apply_body(request, &spec.body, variables, default_content_type)
    }
}

fn validate_options(spec: &RequestSpec) -> Result<(), ApiError> {
    if !(1..=300_000).contains(&spec.options.timeout_ms)
        || !(1..=64 * 1024 * 1024).contains(&spec.options.response_limit_bytes) {
        return Err(ApiError::InvalidOptions);
    }
    Ok(())
}

fn apply_body(mut request: RequestBuilder, body: &RequestBody, variables: &Variables, default_content_type: bool) -> Result<RequestBuilder, ApiError> {
    let (bytes, content_type) = match body {
        RequestBody::Empty => return Ok(request),
        RequestBody::Text { text, content_type } => (variables.resolve(text)?.into_bytes(), content_type.clone()),
        RequestBody::Json { value } => (resolve_json(value, variables)?.to_string().into_bytes(), "application/json".into()),
        RequestBody::Form { entries } => (url_form(entries, variables)?.into_bytes(), "application/x-www-form-urlencoded".into()),
        RequestBody::Binary { data, content_type } => (STANDARD.decode(data).map_err(|_| ApiError::InvalidBase64)?, content_type.clone()),
    };
    if bytes.len() > 32 * 1024 * 1024 { return Err(ApiError::RequestLimit); }
    if default_content_type {
        let content_type = HeaderValue::from_str(&content_type).map_err(|_| ApiError::InvalidHeader)?;
        request = request.header(CONTENT_TYPE, content_type);
    }
    Ok(request.body(bytes))
}

fn url_form(entries: &[Header], variables: &Variables) -> Result<String, ApiError> {
    let mut url = Url::parse("http://localhost/").map_err(|_| ApiError::InvalidUrl)?;
    for entry in entries {
        url.query_pairs_mut().append_pair(&variables.resolve(&entry.name)?, &variables.resolve(&entry.value)?);
    }
    Ok(url.query().unwrap_or_default().into())
}

fn resolve_json(value: &serde_json::Value, variables: &Variables) -> Result<serde_json::Value, ApiError> {
    use serde_json::Value;
    Ok(match value {
        Value::String(text) => Value::String(variables.resolve(text)?),
        Value::Array(values) => Value::Array(values.iter().map(|value| resolve_json(value, variables)).collect::<Result<_, _>>()?),
        Value::Object(values) => Value::Object(values.iter().map(|(key, value)| Ok((key.clone(), resolve_json(value, variables)?))).collect::<Result<_, ApiError>>()?),
        value => value.clone(),
    })
}
