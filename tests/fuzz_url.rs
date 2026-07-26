use thor::net::SecurityShield;

#[test]
fn fuzz_security_shield_url_sanitization() {
    let mut bugs = Vec::new();
    
    // Some specific cases based on our manual analysis
    let manual_cases = vec![
        ("https://example.com/page#fragment?utm_source=track", false),
        ("https://example.com/?utm_source=track#frag1#frag2", true),
        ("https://example.com/page??utm_source=abc", true),
        ("https://example.com/page?query=?utm_source=abc", true),
    ];
    
    for (url, should_strip) in manual_cases {
        let (sanitized, stripped) = SecurityShield::sanitize_url(url);
        println!("Original: {}", url);
        println!("Sanitized: {}", sanitized);
        println!("Stripped: {}", stripped);
        println!("------------------------");
    }

    // Fuzz generation
    let mut rng_seed: u64 = 0;
    fn next_rand(seed: &mut u64) -> u64 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        *seed
    }

    let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~:/?#[]@!$&'()*+,;=".as_bytes();
    
    for i in 0..10000 {
        let mut url = String::new();
        url.push_str("https://example.com/");
        
        let len = (next_rand(&mut rng_seed) % 100) as usize;
        for _ in 0..len {
            let c = chars[(next_rand(&mut rng_seed) % chars.len() as u64) as usize];
            url.push(c as char);
        }
        
        // Ensure some utm_source and # appear sometimes
        if next_rand(&mut rng_seed) % 2 == 0 {
            url.push_str("?utm_source=fuzz");
        }
        if next_rand(&mut rng_seed) % 2 == 0 {
            url.push_str("#hash1");
        }
        if next_rand(&mut rng_seed) % 2 == 0 {
            url.push_str("#hash2");
        }

        // Catch panics
        let result = std::panic::catch_unwind(|| {
            SecurityShield::sanitize_url(&url)
        });
        
        if result.is_err() {
            bugs.push(format!("Panic on URL: {}", url));
        } else {
            let (sanitized, stripped) = result.unwrap();
            
            // Check invariant: if it doesn't have `?`, it shouldn't strip anything.
            if !url.contains('?') && stripped {
                bugs.push(format!("Stripped without '?' in URL: {}", url));
            }
            
            // Note: If we had a real URL parser we'd check if utm_source is still in query.
        }
    }
    
    if !bugs.is_empty() {
        println!("FOUND BUGS:");
        for bug in &bugs {
            println!("{}", bug);
        }
        panic!("Found {} bugs", bugs.len());
    }
}
