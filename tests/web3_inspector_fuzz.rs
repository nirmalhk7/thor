use thor::dx::web3_inspector::{Web3Inspector, Eip55Status};

#[test]
fn test_decode_abi_selector_panic_unicode() {
    let payload = "0x1234567🦀";
    let result = Web3Inspector::decode_abi_selector(payload);
    assert_eq!(result, None, "decode_abi_selector should return None on non-hex input");
}

#[test]
fn test_decode_abi_selector_panic_unicode_in_params() {
    let mut data = String::new();
    for _ in 0..23 { data.push('a'); }
    data.push('🦀');
    for _ in 0..(64 - 27) { data.push('b'); }
    
    let payload = format!("0xa9059cbb{}", data);
    let result = Web3Inspector::decode_abi_selector(&payload);
    assert_eq!(result, None, "decode_abi_selector should return None on invalid characters in params");
}

#[test]
fn test_eip55_checksum_panic() {
    let result = Web3Inspector::validate_eip55_checksum("0x12345678901234567890123456789012345678🦀");
    assert_eq!(result, Eip55Status::NotAnAddress);
}

#[test]
fn test_is_web3_rpc_panic() {
    let massive_string = format!("{{\"jsonrpc\":\"2.0\",\"method\":\"eth_call\",\"params\":[{}]}}", "[]".repeat(10_000));
    let res = Web3Inspector::is_web3_rpc(&massive_string);
    assert!(!res); 
}
