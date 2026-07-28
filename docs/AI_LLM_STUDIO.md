# AI / LLM Developer Studio & Telemetry Documentation

## Overview

Thor incorporates a dedicated **AI / LLM Developer Studio** (`src/dx/llm_studio.rs` and `src/dx/llm_clean_context.rs`) tailored for vibecoders, AI engineers, and prompt developers. Thor tracks real-time SSE stream latency, token generation rates (tok/s), API token costs with prompt caching discounts, and provides 1-click DOM-to-LLM clean context copying.

---

## 1. 2026 Model Pricing Engine & Prompt Caching

Thor includes an embedded pricing engine supporting 2026 models and prompt caching discounts:

```rust
pub struct LlmModelPricing {
    pub name: &'static str,
    pub input_price_per_1k: f64,
    pub cached_input_price_per_1k: f64,
    pub output_price_per_1k: f64,
}
```

### Model Pricing Table

| Model Name | Input Price / 1k | Cached Input / 1k | Output Price / 1k | Prompt Caching Discount |
| :--- | :--- | :--- | :--- | :--- |
| `claude-3.7-sonnet` | $0.00300 | **$0.00030** | $0.01500 | **90% Off** |
| `deepseek-r1` | $0.00055 | **$0.00014** | $0.00219 | **75% Off** |
| `gpt-4o` | $0.00250 | **$0.00125** | $0.01000 | **50% Off** |
| `gemini-2.0-flash` | $0.00010 | **$0.000025** | $0.00040 | **75% Off** |

---

## 2. Multi-Provider SSE Stream Latency Parser

Thor's `SseStreamParser` tracks streaming telemetry across multiple AI provider API formats:

- **OpenAI / DeepSeek**: Parses `choices[0].delta.content` and `usage` completion tokens.
- **Anthropic Claude**: Parses `content_block_delta` events and `delta.text`.
- **Google Gemini**: Parses `candidates[0].content.parts[0].text`.
- **Local Ollama / vLLM**: Parses `response` field and local token metrics.

### Measured Metrics
- **TTFT (Time To First Token)**: Microsecond latency from request sent to first stream chunk received.
- **Throughput (tokens/sec)**: Generation velocity in tokens per second.
- **Total Token Count**: Combined prompt and completion token counts.
- **Est. Cost ($)**: Calculated cost accounting for prompt cache hit ratio.

---

## 3. Local AI Endpoint Auto-Detection

Thor automatically probes local machine endpoints in the background:

- **Ollama**: `http://127.0.0.1:11434/api/tags`
- **vLLM**: `http://127.0.0.1:8000/v1/models`
- **LM Studio**: `http://127.0.0.1:1234/v1/models`

When local endpoints are active, Thor displays a local AI status indicator, allowing developers to route browser AI queries directly to local GPUs without cloud API keys or data egress.

---

## 4. Copy Page as LLM Clean Context (`Cmd+Shift+C`)

Wired to `Cmd+Shift+C` shortcut, `src/dx/llm_clean_context.rs` provides 1-click context extraction for LLM prompts:

1. **Noise Reduction**: Strips non-content HTML elements (`<nav>`, `<footer>`, `<script>`, `<style>`, `<iframe>`, ad units, header bars).
2. **Markdown Conversion**: Converts main article/code content to structured GitHub-Flavored Markdown.
3. **Metadata Header**: Prepends Page Title, Source URL, and ISO timestamp.
4. **Clipboard Injection**: Automatically calculates estimated token count and copies the payload to the OS clipboard.
