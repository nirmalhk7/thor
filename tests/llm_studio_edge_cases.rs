use thor::dx::llm_studio::{SseStreamParser, LlmStudio};

#[test]
fn test_fragmented_sse_chunks() {
    let mut parser = SseStreamParser::new(1000);
    
    // Chunk split right in the middle of a token or JSON
    let chunk1 = "data: {\"choices\":[{\"delta\":{\"content\":\"Hell";
    let chunk2 = "o\"}}]}\n\n";

    // Since feed_chunk processes line by line and requires well-formed JSON per line,
    // let's see how it behaves when the JSON is fragmented across lines or feed calls.
    let m1 = parser.feed_chunk(chunk1, 1050);
    assert_eq!(m1.accumulated_text, "");
    
    let m2 = parser.feed_chunk(chunk2, 1060);
    // Because chunk1 was "data: {...", it was not terminated with newline, so `.lines()`
    // would parse it. But wait, `chunk_raw.lines()` doesn't buffer across calls!
    // So if the network chunks are fragmented, SseStreamParser will drop the partial JSON!
    // Let's verify if it drops it.
    
    // We expect it to drop since SseStreamParser has no internal line buffer.
}

#[test]
fn test_enormous_streams() {
    let mut parser = SseStreamParser::new(1000);
    
    // Feed it 1 million lines
    let chunk = "data: {\"choices\":[{\"delta\":{\"content\":\"very large text \"}}]}\n\n";
    
    for i in 0..10_000 {
        parser.feed_chunk(chunk, 1000 + i as u64 * 10);
    }
    
    let metrics = parser.feed_chunk("data: [DONE]\n\n", 1000 + 10_000 * 10);
    
    assert_eq!(metrics.total_chunks, 10_001);
    assert!(metrics.total_tokens > 10_000); // 3 tokens per chunk? 'very', 'large', 'text' => 3
    assert!(metrics.accumulated_text.len() > 100_000);
}

#[test]
fn test_token_counter_weird_chars() {
    // Replacement character and emojis
    let text = "Hello  world 🦀🔥!!";
    let tokens = LlmStudio::count_tokens(text);
    assert!(tokens > 0);
    
    // Enormous string for token counter
    let large_text = "token ".repeat(1_000_000);
    let tokens_large = LlmStudio::count_tokens(&large_text);
    assert!(tokens_large >= 1_000_000, "Token count: {}", tokens_large);
}

#[test]
fn test_invalid_utf8_replacement() {
    // If invalid UTF-8 bytes arrive, `String::from_utf8_lossy` replaces them.
    let invalid_bytes = b"data: {\"choices\":[{\"delta\":{\"content\":\"Hello \xFF World\"}}]}\n\n";
    let lossy_str = String::from_utf8_lossy(invalid_bytes);
    
    let mut parser = SseStreamParser::new(1000);
    let metrics = parser.feed_chunk(&lossy_str, 1050);
    
    // JSON parse might fail due to the replacement char if it's not well-escaped?
    // Actually \xFF ->  which is valid JSON string char.
    // Let's see if it successfully extracts the text.
    println!("Accumulated: {}", metrics.accumulated_text);
}
