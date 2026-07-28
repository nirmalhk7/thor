# Thor Feature Documentation & Reference Manual

## Complete Feature Matrix

Thor provides an extensive suite of features categorized into 7 specialized core areas:

---

## 1. 🌐 Core Ephemeral Browsing & Multi-Tab Engine

- **Chromium M2 Single-Process Core**: Launches one dedicated Chromium process per profile session; creates isolated CDP page targets per tab instead of spawning separate Chrome processes.
- **3-Level Isolation Hierarchy**:
  - **Level 1 (Profile)**: Settings, root CAs, bookmarks, IDE bridge preferences.
  - **Level 2 (Identity Context)**: Separates `cookies`, `localStorage`, `IndexedDB`, `ServiceWorkers`, and auth headers across roles (`Admin`, `Standard User`, `Anonymous`).
  - **Level 3 (Tab)**: Tab-scoped in-memory history, trace buffers, scroll offsets, and layout state.
- **4-Stage Tab Lifecycle**: `Active` (0–15m) $\rightarrow$ `Frozen` (15m–1h, 0.0ms resume stall) $\rightarrow$ `Discarded` (1h–24h, RAM release) $\rightarrow$ `Expired` ($\ge$24h auto-close & history purge).
- **Prewarm Tab Pool**: Background prewarm pool (`daemon/prewarm.rs`) maintains ready `about:blank` targets for instant tab creation (<115ms acquisition latency).

---

## 2. ⚡ Command Palette & Navigation (`Cmd+K` & `Cmd+Shift+K`)

- **Sites Palette (`Cmd+K`)**:
  - Unified search across active tabs, open bookmarks, and tab-scoped history.
  - Sub-millisecond fuzzy matching powered by nucleo matcher scoring algorithm.
- **Toolbox Palette (`Cmd+Shift+K`)**:
  - Quick access to Developer Control Center and 5 Specialist Lenses (`Full-Stack`, `Security`, `Performance`, `AI / LLM`, `Web3 / Chain`).
- **Shortcut Reference Sheet (`Cmd+/`)**:
  - Floating glass modal overlay categorizing all global keyboard chords.

---

## 3. 🎨 Frontend Developer Experience (DX)

- **Primary Chrome Strip Cleanup (`UI-005`)**: Sleek, minimalist primary toolbar containing only traffic lights, tabs, glass omnibox, SSL badge, and mode chip.
- **Secondary Strip Exclusivity (`UI-006`)**: Single active secondary strip manager preventing toolbar stacking. Includes Locust running safety confirmation checks.
- **CSS Layout Vector Overlay (`src/dx/css_overlay.rs`)**:
  - **Flexbox Overlay**: Draws main-axis/cross-axis directional vectors, flex-wrap wrap lines, and gap highlight fills.
  - **Grid Overlay**: Renders explicit grid line numbers, track column/row boundaries, and area names directly over containers.
- **Copy Page as LLM Clean Context (`Cmd+Shift+C`)**:
  - Strips clutter (`<nav>`, `<footer>`, `<script>`, `<style>`, `<iframe>`, ads) using `llm_clean_context.rs`.
  - Converts main article content to clean, token-dense Markdown with title/URL metadata.
- **IDE Bridge**: Deep links to open local codebase lines directly in VS Code (`vscode://file/...`) or Cursor (`cursor://file/...`).

---

## 4. 🤖 AI / LLM Developer Studio (`src/dx/llm_studio.rs`)

- **2026 Model Pricing Table**: Built-in pricing engine supporting prompt caching discounts:
  - `claude-3.7-sonnet`: $0.003 / 1k input ($0.0003 cached), $0.015 / 1k output.
  - `deepseek-r1`: $0.00055 / 1k input ($0.00014 cached), $0.00219 / 1k output.
  - `gpt-4o`: $0.0025 / 1k input ($0.00125 cached), $0.010 / 1k output.
  - `gemini-2.0-flash`: $0.0001 / 1k input ($0.000025 cached), $0.0004 / 1k output.
- **Multi-Provider SSE Stream Parser**: Real-time TTFT (Time To First Token) parsing across OpenAI (`choices[0].delta`), Anthropic (`content_block_delta`), Gemini (`candidates[0].content`), and Ollama formats.
- **Local AI Endpoint Auto-Detection**: Auto-probes local endpoints (`localhost:11434` Ollama, `localhost:8000` vLLM, `localhost:1234` LM Studio).

---

## 5. 🛡️ Security Specialist & Pentesting Suite

- **IDOR Matrix Testing (`src/net/idor_matrix.rs`)**: Replays intercepted HTTP requests across multiple Level 2 Identity Contexts. Uses `compute_json_structure_hash` to hash JSON response schemas, eliminating false-positive IDOR alerts on 200 OK error wrappers.
- **Traffic Tamper Engine (`src/net/tamper.rs`)**: Pause, edit, forward, drop, or inject custom HTTP headers into outgoing requests. Supports inline response mocking (`mock_status`, `mock_body`, `delay_ms`).
- **SEC-027 Secret Redaction**: Automatically masks sensitive `Authorization`, `Cookie`, and `X-API-Key` values with `[REDACTED]` in proxy logs.
- **Pure-Rust SSL CA Generator (`src/net/ssl_ca.rs`)**: Generates custom ECDSA P-256 root CAs (`Thor Dev CA`) and leaf certs (`*.localhost`, `*.test`). Auto-installs trust into macOS Keychain (`security`), Windows Cert Store (`certutil`), or Linux NSS DB.
- **JWT & Web3 Decoder (`src/dx/decoder.rs`, `src/dx/web3_inspector.rs`)**: Auto-detects and decodes JWT claims, base64 payloads, EVM 4-byte ABI selectors, and EIP-55 address checksums.

---

## 6. ⚡ Performance & Load Engine (`src/load/`)

- **Thor Load Engine**: Canonical `ThorScenario` benchmark spec with dual runners:
  - **BrowserRunner**: Evaluates real DOM rendering, LCP, INP, TBT, and CLS Web Vitals via CDP.
  - **ProtocolRunner**: Multi-threaded Rust HTTP/WS scale runner pushing 10,000+ RPS.
- **Lock-Free Atomic Metrics (`metrics.rs`)**: Uses `[AtomicUsize; 200]` logarithmic bucket array for $O(1)$ zero-lock latency tracking (11.2 ns/op at 10,000+ RPS).
- **P0 CDP Freezing**: Replaced OS-level `SIGSTOP` halts with CDP `Page.setWebLifecycleState(Frozen)`, achieving 0.0ms resume freeze stall.

---

## 7. 🔌 Model Context Protocol (MCP) Server (`src/mcp/`)

- **Standard Compliance**: Implements MCP Specification version `2024-11-05` over HTTP SSE (`/sse`) and JSON-RPC (`/message`).
- **8 Browser Tools**:
  - `thor_get_screenshot`: Captures PNG screenshot of active tab.
  - `thor_navigate`: Navigates to target URL.
  - `thor_eval_js`: Evaluates arbitrary JS script.
  - `thor_get_dom`: Extracts interactive DOM elements with coordinates.
  - `thor_click`: Issues click event on CSS selector.
  - `thor_type`: Types text into form element matching selector.
  - `thor_hover`: Hovers over CSS selector.
  - `thor_scroll`: Scrolls page by `x, y` pixel deltas.
- **Compatibility**: Natively compatible with Claude Desktop, Claude Code, Cursor, Codex, Antigravity, and custom MCP clients.
