use thor::net::SecurityShield;

fn next_random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed
}

fn generated_url(seed: &mut u64) -> String {
    let characters =
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~:/?#[]@!$&'()*+,;=";
    let mut url = String::from("https://example.com/");
    let length = next_random(seed) % 100;
    for _ in 0..length {
        url.push(characters[(next_random(seed) % characters.len() as u64) as usize] as char);
    }
    for suffix in ["?utm_source=fuzz", "#hash1", "#hash2"] {
        if next_random(seed).is_multiple_of(2) {
            url.push_str(suffix);
        }
    }
    url
}

#[test]
fn fuzz_security_shield_url_sanitization() {
    let mut seed = 0;
    for _ in 0..10_000 {
        let url = generated_url(&mut seed);
        let result = std::panic::catch_unwind(|| SecurityShield::sanitize_url(&url));
        let (_, stripped) = result.unwrap_or_else(|_| panic!("Sanitizer panicked on {url}"));
        assert!(
            !stripped || url.contains('?'),
            "Stripped a URL without a query: {url}"
        );
    }
}
