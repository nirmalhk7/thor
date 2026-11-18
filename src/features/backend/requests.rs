use super::{
    now_ms, substitute, ApiRequest, ApiResponse, BackendEngine, Duration, HashMap,
    InterceptedRequest,
};

impl BackendEngine {
    // 1. API Workbench ------------------------------------------------------
    /// Resolve `{{var}}` templates in URL/headers/body against a variable map.
    pub fn build_request(&self, mut req: ApiRequest, vars: &HashMap<String, String>) -> ApiRequest {
        req.url = substitute(&req.url, vars);
        req.body = req.body.map(|b| substitute(&b, vars));
        req.headers = req
            .headers
            .into_iter()
            .map(|(k, v)| (k, substitute(&v, vars)))
            .collect();
        req
    }

    /// Convert a workbench request to a tamper-engine [`InterceptedRequest`].
    pub fn to_intercepted(&self, req: &ApiRequest, id: &str) -> InterceptedRequest {
        InterceptedRequest {
            id: id.to_string(),
            timestamp: now_ms(),
            method: req.method.clone(),
            url: req.url.clone(),
            headers: req.headers.clone(),
            body: req.body.clone(),
            is_modified: false,
        }
    }

    /// Send a request for real over HTTP and record it in the workbench history.
    ///
    /// Any registered chaos rule matching the URL is applied first: injected
    /// latency is slept and a forced fault short-circuits the real request with a
    /// synthetic error response (deterministic per request id — no fake RNG).
    pub fn send_blocking(&self, req: &ApiRequest) -> Result<ApiResponse, String> {
        let request_id = format!("{} {}", req.method, req.url);
        let chaos = self.apply_chaos(&req.url, &request_id);
        if chaos.added_latency_ms > 0 {
            std::thread::sleep(Duration::from_millis(chaos.added_latency_ms));
        }
        if let Some(status) = chaos.forced_status {
            let resp = ApiResponse {
                status,
                headers: HashMap::new(),
                body: "{\"thor_chaos\":\"injected fault\"}".to_string(),
                latency_ms: chaos.added_latency_ms as f64,
            };
            self.history.lock().push((req.clone(), resp.clone()));
            return Ok(resp);
        }
        let resp = crate::api::blocking::run(self.send(req))??;
        self.history.lock().push((req.clone(), resp.clone()));
        Ok(resp)
    }

    pub async fn send(&self, req: &ApiRequest) -> Result<ApiResponse, String> {
        let mut request = crate::api::RequestSpec::new(&req.method, &req.url);
        request.headers = req
            .headers
            .iter()
            .map(|(name, value)| crate::api::Header::new(name, value))
            .collect();
        if let Some(text) = &req.body {
            let content_type = req
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| "text/plain".into());
            request.body = crate::api::RequestBody::Text {
                text: text.clone(),
                content_type,
            };
        }
        let response = self
            .http_service()?
            .send(&request, &crate::api::Variables::default())
            .await
            .map_err(|error| error.to_string())?;
        if response.body.encoding != "utf8" {
            return Err("Binary response requires the API workbench binary viewer".into());
        }
        Ok(ApiResponse {
            status: response.status,
            headers: response
                .headers
                .into_iter()
                .map(|header| (header.name, header.value))
                .collect(),
            body: response.body.data,
            latency_ms: response.total_ms,
        })
    }

    /// Shared connection pool used by feature execution and the API workbench.
    pub fn http_service(&self) -> Result<&crate::api::HttpService, String> {
        self.http
            .get_or_init(|| crate::api::HttpService::new().map_err(|error| error.to_string()))
            .as_ref()
            .map_err(Clone::clone)
    }

    pub fn history(&self) -> Vec<(ApiRequest, ApiResponse)> {
        self.history.lock().clone()
    }
}
