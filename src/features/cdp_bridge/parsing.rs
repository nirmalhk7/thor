use super::{HashMap, NetworkEntry};

impl NetworkEntry {
    pub(super) fn total_ms(&self) -> f64 {
        self.dns_ms + self.connect_ms + self.ttfb_ms + self.download_ms
    }

    /// Parse a CDP-shaped Network JSON object into a [`NetworkEntry`].
    ///
    /// Recognizes flattened capture objects and Chromium CDP envelopes
    /// (`method` + `params.response` / `params.encodedDataLength`).
    pub fn from_cdp_json(value: &serde_json::Value) -> Option<Self> {
        let params = value.get("params").unwrap_or(value);
        let response = params
            .get("response")
            .or_else(|| value.get("response"))
            .unwrap_or(params);

        let url = response
            .get("url")
            .or_else(|| params.get("url"))
            .or_else(|| value.get("url"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if url.is_empty() {
            return None;
        }

        let method = params
            .get("request")
            .and_then(|r| r.get("method"))
            .or_else(|| value.get("method"))
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_string();

        let status = response
            .get("status")
            .and_then(|v| v.as_u64())
            .or_else(|| value.get("status").and_then(|v| v.as_u64()))
            .unwrap_or(0) as u16;

        let connection_id = response
            .get("connectionId")
            .or_else(|| value.get("connectionId"))
            .or_else(|| value.get("connection_id"))
            .and_then(|v| match v {
                serde_json::Value::Number(n) => Some(n.to_string()),
                serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
                _ => None,
            });

        let protocol = response
            .get("protocol")
            .or_else(|| response.get("alpn"))
            .or_else(|| value.get("protocol"))
            .or_else(|| value.get("alpn"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let transfer_bytes = response
            .get("encodedDataLength")
            .or_else(|| params.get("encodedDataLength"))
            .or_else(|| value.get("encodedDataLength"))
            .or_else(|| value.get("transfer_bytes"))
            .and_then(|v| v.as_u64());

        let decoded_bytes = response
            .get("decodedBodyLength")
            .or_else(|| value.get("decodedBodyLength"))
            .or_else(|| value.get("decoded_bytes"))
            .and_then(|v| v.as_u64());

        let bytes = decoded_bytes
            .or(transfer_bytes)
            .or_else(|| value.get("bytes").and_then(|v| v.as_u64()))
            .unwrap_or(0);

        let connection_reused = response
            .get("connectionReused")
            .or_else(|| value.get("connectionReused"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let from_disk_cache = response
            .get("fromDiskCache")
            .or_else(|| value.get("fromDiskCache"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let from_cache = connection_reused
            || from_disk_cache
            || value
                .get("from_cache")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

        let timing = response.get("timing").or_else(|| value.get("timing"));
        let (dns_ms, connect_ms, ttfb_ms, download_ms, start_ms) =
            parse_cdp_timing(timing, connection_reused);

        let mut request_headers = HashMap::new();
        if let Some(h) = params
            .get("request")
            .and_then(|r| r.get("headers"))
            .or_else(|| value.get("request_headers"))
            .or_else(|| value.get("requestHeaders"))
        {
            collect_headers(h, &mut request_headers);
        }
        let mut response_headers = HashMap::new();
        if let Some(h) = response
            .get("headers")
            .or_else(|| value.get("response_headers"))
            .or_else(|| value.get("responseHeaders"))
        {
            collect_headers(h, &mut response_headers);
        }

        let initiator = params
            .get("initiator")
            .and_then(|i| i.get("url").or_else(|| i.get("stack")))
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else {
                    v.get("callFrames")
                        .and_then(|cf| cf.as_array())
                        .and_then(|a| a.first())
                        .and_then(|f| f.get("url"))
                        .and_then(|u| u.as_str())
                        .map(|s| s.to_string())
                }
            })
            .or_else(|| {
                value
                    .get("initiator")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

        let render_blocking = value
            .get("render_blocking")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Some(Self {
            url,
            method,
            status,
            start_ms,
            dns_ms,
            connect_ms,
            ttfb_ms,
            download_ms,
            bytes,
            transfer_bytes,
            decoded_bytes,
            connection_id,
            protocol,
            initiator,
            render_blocking,
            from_cache,
            request_headers,
            response_headers,
        })
    }
}

fn collect_headers(value: &serde_json::Value, out: &mut HashMap<String, String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                if let Some(s) = v.as_str() {
                    out.insert(k.clone(), s.to_string());
                } else if let Some(n) = v.as_f64() {
                    out.insert(k.clone(), n.to_string());
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let (Some(k), Some(v)) = (
                    item.get("name").and_then(|x| x.as_str()),
                    item.get("value").and_then(|x| x.as_str()),
                ) {
                    out.insert(k.to_string(), v.to_string());
                }
            }
        }
        _ => {}
    }
}

/// CDP `Network.Response.timing` uses ms offsets relative to `requestTime`.
fn parse_cdp_timing(
    timing: Option<&serde_json::Value>,
    connection_reused: bool,
) -> (f64, f64, f64, f64, f64) {
    let Some(t) = timing else {
        return (0.0, 0.0, 0.0, 0.0, 0.0);
    };
    let f = |key: &str| t.get(key).and_then(|v| v.as_f64()).unwrap_or(-1.0);
    let request_time = f("requestTime");
    let start_ms = if request_time > 0.0 {
        request_time * 1000.0
    } else {
        t.get("start_ms").and_then(|v| v.as_f64()).unwrap_or(0.0)
    };
    let dns_start = f("dnsStart");
    let dns_end = f("dnsEnd");
    let connect_start = f("connectStart");
    let connect_end = f("connectEnd");
    let send_start = f("sendStart");
    let receive_headers_end = f("receiveHeadersEnd");

    let dns_ms = if dns_start >= 0.0 && dns_end >= dns_start {
        dns_end - dns_start
    } else {
        0.0
    };
    let connect_ms = if connection_reused {
        0.0
    } else if connect_start >= 0.0 && connect_end >= connect_start {
        connect_end - connect_start
    } else {
        t.get("connect_ms").and_then(|v| v.as_f64()).unwrap_or(0.0)
    };
    let ttfb_ms = if receive_headers_end >= 0.0 && send_start >= 0.0 {
        (receive_headers_end - send_start).max(0.0)
    } else {
        t.get("ttfb_ms").and_then(|v| v.as_f64()).unwrap_or(0.0)
    };
    let download_ms = t.get("download_ms").and_then(|v| v.as_f64()).unwrap_or(0.0);
    (dns_ms, connect_ms, ttfb_ms, download_ms, start_ms)
}
