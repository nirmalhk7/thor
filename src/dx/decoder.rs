use serde::{Deserialize, Serialize};

/// Type of decoded payload detected
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayloadType {
    Jwt,
    Base64,
    Hex,
    Json,
    LlmStream,
    Web3Rpc,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtHeader {
    pub alg: Option<String>,
    pub typ: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedJwt {
    pub header_raw: String,
    pub payload_raw: String,
    pub signature_raw: String,
    pub header_json: Option<String>,
    pub payload_json: Option<String>,
    pub algorithm: String,
}

/// Protocol & Serialized Data Decoder Studio
pub struct DecoderStudio;

impl DecoderStudio {
    /// Detect payload type from raw string with auto-detection for JWT, Base64, Hex, JSON, LLM Stream, and Web3 RPC
    pub fn detect_type(input: &str) -> PayloadType {
        let trimmed = input.trim();
        if Self::is_jwt(trimmed) {
            PayloadType::Jwt
        } else if crate::dx::web3_inspector::Web3Inspector::is_web3_rpc(trimmed)
            || trimmed.starts_with("0xa9059cbb")
            || trimmed.starts_with("0x095ea7b3")
            || (trimmed.starts_with("0x")
                && trimmed.len() >= 10
                && crate::dx::web3_inspector::Web3Inspector::decode_abi_selector(trimmed).is_some())
        {
            PayloadType::Web3Rpc
        } else if trimmed.starts_with("data: ") || trimmed.contains("data: [DONE]") {
            PayloadType::LlmStream
        } else if trimmed.starts_with('{') && trimmed.ends_with('}') {
            PayloadType::Json
        } else if Self::is_hex(trimmed) {
            PayloadType::Hex
        } else if Self::is_base64(trimmed) {
            PayloadType::Base64
        } else {
            PayloadType::Unknown
        }
    }

    /// Check if input is a JWT (3 dot-separated base64 parts)
    pub fn is_jwt(input: &str) -> bool {
        let parts: Vec<&str> = input.split('.').collect();
        if parts.len() != 3 {
            return false;
        }
        parts.iter().all(|p| !p.is_empty())
    }

    /// Decode JWT string into header, claims payload, and signature
    pub fn decode_jwt(jwt_str: &str) -> Result<DecodedJwt, String> {
        let parts: Vec<&str> = jwt_str.trim().split('.').collect();
        if parts.len() != 3 {
            return Err("Invalid JWT structure: must contain 3 dot-separated segments".to_string());
        }

        let header_json = Self::decode_base64_url(parts[0])
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok());
        let payload_json = Self::decode_base64_url(parts[1])
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok());

        let alg = header_json
            .as_ref()
            .and_then(|h| serde_json::from_str::<JwtHeader>(h).ok())
            .and_then(|h| h.alg)
            .unwrap_or_else(|| "none".to_string());

        Ok(DecodedJwt {
            header_raw: parts[0].to_string(),
            payload_raw: parts[1].to_string(),
            signature_raw: parts[2].to_string(),
            header_json,
            payload_json,
            algorithm: alg,
        })
    }

    /// Helper parser for streaming LLM SSE frame
    pub fn decode_llm_stream(input: &str, start_ms: u64) -> crate::dx::llm_studio::SseStreamMetrics {
        let mut parser = crate::dx::llm_studio::SseStreamParser::new(start_ms);
        parser.feed_chunk(input, start_ms + 100)
    }

    /// Helper decoder for Web3 ABI calldata selector
    pub fn decode_web3_rpc(input: &str) -> Option<crate::dx::web3_inspector::AbiSelectorMatch> {
        crate::dx::web3_inspector::Web3Inspector::decode_abi_selector(input)
    }

    fn is_hex(s: &str) -> bool {
        !s.is_empty() && s.len() % 2 == 0 && s.chars().all(|c| c.is_ascii_hexdigit())
    }

    fn is_base64(s: &str) -> bool {
        if s.is_empty() || s.len() % 4 != 0 {
            return false;
        }
        s.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
    }

    /// Helper URL-safe base64 decoder
    pub fn decode_base64_url(s: &str) -> Result<Vec<u8>, String> {
        let mut base64_str = s.replace('-', "+").replace('_', "/");
        while base64_str.len() % 4 != 0 {
            base64_str.push('=');
        }

        use std::collections::HashMap;
        let b64_chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let char_map: HashMap<char, u8> = b64_chars.chars().zip(0..64).collect();

        let clean_bytes: Vec<u8> = base64_str.bytes().filter(|&b| b != b'=').collect();
        let mut out = Vec::new();

        let mut i = 0;
        while i < clean_bytes.len() {
            let b0 = *char_map.get(&(clean_bytes[i] as char)).ok_or("Invalid char")? as u32;
            let b1 = if i + 1 < clean_bytes.len() { *char_map.get(&(clean_bytes[i + 1] as char)).ok_or("Invalid char")? as u32 } else { 0 };
            let b2 = if i + 2 < clean_bytes.len() { *char_map.get(&(clean_bytes[i + 2] as char)).ok_or("Invalid char")? as u32 } else { 0 };
            let b3 = if i + 3 < clean_bytes.len() { *char_map.get(&(clean_bytes[i + 3] as char)).ok_or("Invalid char")? as u32 } else { 0 };

            let triple = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;

            out.push(((triple >> 16) & 0xFF) as u8);
            if i + 2 < clean_bytes.len() {
                out.push(((triple >> 8) & 0xFF) as u8);
            }
            if i + 3 < clean_bytes.len() {
                out.push((triple & 0xFF) as u8);
            }

            i += 4;
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_detection_and_decoding() {
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IlRob3IgU2VjdXJpdHkifQ.signature_bytes";
        assert_eq!(DecoderStudio::detect_type(jwt), PayloadType::Jwt);

        let decoded = DecoderStudio::decode_jwt(jwt).unwrap();
        assert_eq!(decoded.algorithm, "HS256");
        assert!(decoded.header_json.unwrap().contains("HS256"));
        assert!(decoded.payload_json.unwrap().contains("Thor Security"));
    }

    #[test]
    fn test_llm_and_web3_auto_detection() {
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"Thor AI\"}}]}\n\n";
        assert_eq!(DecoderStudio::detect_type(sse), PayloadType::LlmStream);

        let web3 = r#"{"jsonrpc":"2.0","method":"eth_call","params":[],"id":1}"#;
        assert_eq!(DecoderStudio::detect_type(web3), PayloadType::Web3Rpc);

        let calldata = "0xa9059cbb0000000000000000000000005aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed0000000000000000000000000000000000000000000000000de0b6b3a7640000";
        assert_eq!(DecoderStudio::detect_type(calldata), PayloadType::Web3Rpc);
    }

    #[test]
    fn test_base64_url_decoding() {
        let encoded = "eyJhbGciOiJIUzI1NiJ9";
        let bytes = DecoderStudio::decode_base64_url(encoded).unwrap();
        let decoded_str = String::from_utf8(bytes).unwrap();
        assert_eq!(decoded_str, r#"{"alg":"HS256"}"#);
    }
}
