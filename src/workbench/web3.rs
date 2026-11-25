use serde_json::{json, Value};

use crate::dx::Web3Inspector;

const MAX_RPC_BYTES: usize = 64 * 1024;
const BALANCE_OF_SELECTOR: &str = "70a08231";

pub(super) fn inspect_call(request: Value) -> Result<Value, String> {
    if serde_json::to_vec(&request)
        .map_err(|_| "Invalid RPC JSON")?
        .len()
        > MAX_RPC_BYTES
    {
        return Err("Use an RPC request no larger than 64 KiB.".into());
    }
    if request["jsonrpc"].as_str() != Some("2.0") || request["method"].as_str() != Some("eth_call")
    {
        return Err("Paste one JSON-RPC 2.0 eth_call request. Transaction and signing requests are not supported.".into());
    }
    let call = request["params"]
        .as_array()
        .and_then(|params| params.first())
        .and_then(Value::as_object)
        .ok_or("The first eth_call parameter must be a call object.")?;

    let contract = call
        .get("to")
        .and_then(Value::as_str)
        .and_then(prefixed_hex)
        .filter(|address| address.len() == 40)
        .ok_or("Add a 20-byte hex contract address in the call's to field.")?;
    let data = match (call.get("input"), call.get("data")) {
        (Some(input), Some(data)) if !identical_hex(input, data) => {
            return Err(
                "The input and data fields disagree. Keep the intended calldata in one field."
                    .into(),
            );
        }
        (input, data) => input.or(data),
    }
    .and_then(Value::as_str)
    .and_then(prefixed_hex)
    .filter(|hex| hex.len() >= 8 && hex.len() % 2 == 0)
    .ok_or("Add 0x-prefixed calldata with a four-byte selector in input or data.")?;

    let selector = format!("0x{}", data[..8].to_ascii_lowercase());
    let contract = format!("0x{contract}");
    if !data[..8].eq_ignore_ascii_case(BALANCE_OF_SELECTOR) {
        return Ok(json!({
            "state": "selector_only",
            "rpc_method": "eth_call",
            "contract_address": contract,
            "selector": selector,
            "note": "This workflow decodes balanceOf(address) only. No arguments were inferred."
        }));
    }

    // An ABI address occupies one 32-byte word, with twelve zero padding bytes.
    if data.len() != 72 || !data[8..32].bytes().all(|byte| byte == b'0') {
        return Err(
            "balanceOf(address) requires one 32-byte argument with zero address padding.".into(),
        );
    }
    let decoded = Web3Inspector::decode_abi_selector(data)
        .ok_or("Could not decode the function selector.")?;
    let account = decoded
        .params
        .first()
        .ok_or("Could not decode the account argument.")?;
    Ok(json!({
        "state": "decoded",
        "rpc_method": "eth_call",
        "contract_address": contract,
        "selector": selector,
        "signature": decoded.signature,
        "account_address": account.value_raw
    }))
}

fn prefixed_hex(value: &str) -> Option<&str> {
    let hex = value.strip_prefix("0x")?;
    hex.bytes()
        .all(|byte| byte.is_ascii_hexdigit())
        .then_some(hex)
}

fn identical_hex(input: &Value, data: &Value) -> bool {
    input
        .as_str()
        .zip(data.as_str())
        .is_some_and(|(first, second)| first.eq_ignore_ascii_case(second))
}
