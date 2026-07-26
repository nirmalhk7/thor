use thor::dx::web3_inspector::{Web3Inspector, Eip55Status};

#[test]
fn test_decode_abi_selector_panic_unicode() {
    // 8 bytes of emoji "🦀🦀" (1 crab = 4 bytes)
    // clean.len() >= 8 is true
    // clean[..8] slices the two crabs.
    // clean[8..] slices the rest.
    // Wait, let's see if we can trigger a panic in slicing.
    // "0x" + "🦀" (4 bytes) + "🦀" (4 bytes) -> "0x🦀🦀"
    // After strip_prefix("0x"), it is "🦀🦀" (8 bytes).
    // &clean[..8] is the two crabs. It's a valid char boundary.
    // So this won't panic on &clean[..8].
    
    // To panic on &clean[..8], we need an 8-byte slice to fall INSIDE a multi-byte char.
    // Example: "1234567" (7 bytes) + "🦀" (4 bytes). Total 11 bytes.
    // &clean[..8] splits at byte 8, which is inside "🦀".
    let payload = "0x1234567🦀";
    
    // We catch panic to see if it panics.
    let result = std::panic::catch_unwind(|| {
        Web3Inspector::decode_abi_selector(payload);
    });
    
    assert!(result.is_err(), "decode_abi_selector should panic on unaligned slice");
}

#[test]
fn test_decode_abi_selector_panic_unicode_in_params() {
    // Selector needs to match an existing one to parse params, like "0xa9059cbb" (transfer)
    // 8 bytes for selector + at least 64 bytes for param 1.
    // We want the slice `&slot[24..]` to panic.
    // `slot` is `&data[0..64]`.
    // Let's make `data` 64 bytes long, where byte 24 is inside a multi-byte char.
    // 23 bytes of 'a' + '🦀' (4 bytes) + padding = 23 + 4 = 27 bytes.
    // We need 64 bytes total.
    let mut data = String::new();
    for _ in 0..23 { data.push('a'); }
    data.push('🦀'); // starts at 23, ends at 27
    for _ in 0..(64 - 27) { data.push('b'); }
    
    let payload = format!("0xa9059cbb{}", data);
    
    let result = std::panic::catch_unwind(|| {
        Web3Inspector::decode_abi_selector(&payload);
    });
    
    assert!(result.is_err(), "decode_abi_selector should panic on &slot[24..] due to unaligned slice");
}

#[test]
fn test_eip55_checksum_panic() {
    // EIP-55 requires 40 bytes.
    // Let's provide 40 bytes where some are multi-byte chars, but wait, `clean.len() != 40` checks byte length.
    // What if we pass 40 bytes of non-ascii hex digits?
    // The code:
    // if clean.len() != 40 || !clean.bytes().all(|b| b.is_ascii_hexdigit()) {
    //     return Eip55Status::NotAnAddress;
    // }
    // It returns gracefully. So EIP-55 probably won't panic on weird unicode.
}

#[test]
fn test_is_web3_rpc_panic() {
    // is_web3_rpc:
    // serde_json::from_str
    // How can we make serde_json panic or OOM? Deeply nested JSON.
    // But serde_json defaults to recursion limit 128 and fails gracefully with an error, which is caught by `if let Ok(val) = ...`.
    
    // What if `trimmed` is something that panics `starts_with('{')`? No, starts_with on char or string is safe.
    
    // Let's check `is_web3_rpc` with a massive string.
    let massive_string = format!("{{\"jsonrpc\":\"2.0\",\"method\":\"eth_call\",\"params\":[{}]}}", "[]".repeat(10_000));
    let res = Web3Inspector::is_web3_rpc(&massive_string);
    assert!(!res); // Should gracefully fail or return false.
}
