use serde::{Deserialize, Serialize};

/// EVM / Solana JSON-RPC Request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Web3RpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    pub params: Vec<serde_json::Value>,
}

/// Decoded ABI Parameter Value
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AbiParamValue {
    pub name: String,
    pub param_type: String,
    pub value_raw: String,
}

/// Decoded 4-byte ABI selector match
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AbiSelectorMatch {
    pub selector: String,
    pub function_name: String,
    pub signature: String,
    pub params: Vec<AbiParamValue>,
}

/// EIP-55 Checksum validation status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Eip55Status {
    Valid,
    InvalidChecksum { expected: String },
    NotAnAddress,
}

/// Pure Rust Keccak-256 implementation for EIP-55 checksum calculation
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    const ROUNDS: usize = 24;
    const RC: [u64; ROUNDS] = [
        0x0000000000000001, 0x0000000000008082, 0x800000000000808a, 0x8000000080008000,
        0x000000000000808b, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
        0x000000000000008a, 0x0000000000000088, 0x0000000080008009, 0x000000008000000a,
        0x000000008000808b, 0x800000000000008b, 0x8000000000008089, 0x8000000000008003,
        0x8000000000008002, 0x8000000000000080, 0x000000000000800a, 0x800000008000000a,
        0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
    ];

    const R: [u32; 25] = [
        0, 1, 62, 28, 27,
        36, 44, 6, 55, 20,
        3, 10, 43, 25, 39,
        41, 45, 15, 21, 8,
        18, 2, 61, 56, 14,
    ];

    let mut state = [0u64; 25];
    let block_size = 136; // Rate for Keccak-256 (1088 bits / 8)

    // Pad input (Keccak-256 uses 0x01 byte padding, not SHA-3 0x06)
    let mut padded = data.to_vec();
    padded.push(0x01);
    while (padded.len() % block_size) != (block_size - 1) {
        padded.push(0x00);
    }
    padded.push(0x80);

    for chunk in padded.chunks(block_size) {
        for i in 0..(block_size / 8) {
            let word = u64::from_le_bytes(chunk[i * 8..(i + 1) * 8].try_into().unwrap());
            state[i] ^= word;
        }

        // Keccak-f[1600] permutation
        for round in 0..ROUNDS {
            // Theta
            let mut c = [0u64; 5];
            for x in 0..5 {
                c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
            }
            let mut d = [0u64; 5];
            for x in 0..5 {
                d[x] = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
            }
            for x in 0..5 {
                for y in 0..5 {
                    state[x + y * 5] ^= d[x];
                }
            }

            // Rho and Pi
            let mut b = [0u64; 25];
            for x in 0..5 {
                for y in 0..5 {
                    let idx = x + y * 5;
                    let new_x = y;
                    let new_y = (2 * x + 3 * y) % 5;
                    b[new_x + 5 * new_y] = state[idx].rotate_left(R[idx]);
                }
            }

            // Chi
            for x in 0..5 {
                for y in 0..5 {
                    state[x + y * 5] = b[x + y * 5] ^ ((!b[((x + 1) % 5) + y * 5]) & b[((x + 2) % 5) + y * 5]);
                }
            }

            // Iota
            state[0] ^= RC[round];
        }
    }

    let mut out = [0u8; 32];
    for i in 0..4 {
        out[i * 8..(i + 1) * 8].copy_from_slice(&state[i].to_le_bytes());
    }
    out
}

/// Web3 & Blockchain Specialist Lens
pub struct Web3Inspector;

impl Web3Inspector {
    /// Check if payload is EVM or Solana JSON-RPC
    pub fn is_web3_rpc(input: &str) -> bool {
        let trimmed = input.trim();
        if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
            return false;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if val.get("jsonrpc").and_then(|v| v.as_str()) == Some("2.0") {
                if let Some(method) = val.get("method").and_then(|m| m.as_str()) {
                    return method.starts_with("eth_")
                        || method.starts_with("solana")
                        || method.starts_with("net_")
                        || method.starts_with("web3_");
                }
            }
        }
        false
    }

    /// Decode 4-byte ABI selector from raw EVM calldata hex (e.g. `0xa9059cbb...`)
    pub fn decode_abi_selector(calldata_hex: &str) -> Option<AbiSelectorMatch> {
        let clean = calldata_hex.trim().strip_prefix("0x").unwrap_or(calldata_hex.trim());
        if clean.len() < 8 || !clean.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }

        let selector = format!("0x{}", &clean[..8].to_lowercase());
        let data = &clean[8..];

        let (func_name, sig_owned, param_defs): (&str, String, &[(&str, &str)]) = match selector.as_str() {
            "0xa9059cbb" => ("transfer", "transfer(address,uint256)".into(), &[("to", "address"), ("value", "uint256")]),
            "0x095ea7b3" => ("approve", "approve(address,uint256)".into(), &[("spender", "address"), ("value", "uint256")]),
            "0x70a08231" => ("balanceOf", "balanceOf(address)".into(), &[("account", "address")]),
            "0x23b872dd" => ("transferFrom", "transferFrom(address,address,uint256)".into(), &[("from", "address"), ("to", "address"), ("value", "uint256")]),
            "0x38ed1739" => ("swapExactTokensForTokens", "swapExactTokensForTokens(uint256,uint256,address[],address,uint256)".into(), &[("amountIn", "uint256"), ("amountOutMin", "uint256")]),
            "0x18c974f0" => ("swapExactETHForTokens", "swapExactETHForTokens(uint256,address[],address,uint256)".into(), &[("amountOutMin", "uint256")]),
            "0xd058711f" => ("deposit", "deposit()".into(), &[]),
            "0x2e1a7d4d" => ("withdraw", "withdraw(uint256)".into(), &[("wad", "uint256")]),
            _ => ("unknown", format!("{}(bytes)", selector), &[]),
        };

        let mut params = Vec::new();
        for (i, (p_name, p_type)) in param_defs.iter().enumerate() {
            if data.len() >= (i + 1) * 64 {
                let slot = &data[i * 64..(i + 1) * 64];
                let value_raw = match *p_type {
                    "address" => format!("0x{}", &slot[24..]),
                    "uint256" => format!("0x{}", slot.trim_start_matches('0')),
                    _ => format!("0x{}", slot),
                };
                params.push(AbiParamValue {
                    name: p_name.to_string(),
                    param_type: p_type.to_string(),
                    value_raw,
                });
            }
        }

        Some(AbiSelectorMatch {
            selector,
            function_name: func_name.to_string(),
            signature: sig_owned,
            params,
        })
    }

    /// EIP-55 Checksum Validator for Ethereum Addresses
    pub fn validate_eip55_checksum(address: &str) -> Eip55Status {
        let clean = address.trim().strip_prefix("0x").unwrap_or(address.trim());
        if clean.len() != 40 || !clean.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Eip55Status::NotAnAddress;
        }

        let lower = clean.to_lowercase();
        let hash = keccak256(lower.as_bytes());

        let mut expected = String::with_capacity(42);
        expected.push_str("0x");

        for (i, c) in lower.chars().enumerate() {
            if c.is_numeric() {
                expected.push(c);
            } else {
                let byte = hash[i / 2];
                let hash_nibble = if i % 2 == 0 { (byte >> 4) & 0x0F } else { byte & 0x0F };
                if hash_nibble >= 8 {
                    expected.push(c.to_ascii_uppercase());
                } else {
                    expected.push(c.to_ascii_lowercase());
                }
            }
        }

        let full_input = if address.trim().starts_with("0x") { address.trim().to_string() } else { format!("0x{}", clean) };
        if full_input == expected {
            Eip55Status::Valid
        } else {
            Eip55Status::InvalidChecksum { expected }
        }
    }
}

// Simple hex encoder module helper
mod hex {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    #[allow(dead_code)]
    pub fn encode(data: impl AsRef<[u8]>) -> String {
        let bytes = data.as_ref();
        let mut out = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            out.push(HEX_CHARS[(b >> 4) as usize] as char);
            out.push(HEX_CHARS[(b & 0x0f) as usize] as char);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web3_rpc_detection() {
        let rpc = r#"{"jsonrpc":"2.0","method":"eth_call","params":[{"to":"0xa9059cbb"},"latest"],"id":1}"#;
        assert!(Web3Inspector::is_web3_rpc(rpc));
    }

    #[test]
    fn test_abi_selector_decoding() {
        // transfer(address to, uint256 value) calldata
        let calldata = "0xa9059cbb0000000000000000000000005aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed0000000000000000000000000000000000000000000000000de0b6b3a7640000";
        let decoded = Web3Inspector::decode_abi_selector(calldata).unwrap();
        assert_eq!(decoded.selector, "0xa9059cbb");
        assert_eq!(decoded.function_name, "transfer");
        assert_eq!(decoded.params[0].value_raw, "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed");
    }

    #[test]
    fn test_eip55_checksum_validation() {
        // Known valid EIP-55 address
        let addr = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
        assert_eq!(Web3Inspector::validate_eip55_checksum(addr), Eip55Status::Valid);

        // Invalid checksum (all lowercase when should have uppercase chars)
        let invalid_addr = "0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed";
        assert_eq!(
            Web3Inspector::validate_eip55_checksum(invalid_addr),
            Eip55Status::InvalidChecksum {
                expected: "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed".to_string()
            }
        );
    }
}
