# Thor Feature Documentation & Reference Manual

## Complete Feature Matrix

Thor provides an extensive suite of features across core browsing, product specialist lenses, and the **Engineer Feature Suite** (144 registered `FeatureId`s).

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
  - **Developer Control Center**: loads the live FeatureEngine catalog (`feature_list`) — **144 features · 6 domain lenses** — with Invoke + optional `from_active_tab` CDP evaluate.
  - Product specialist tools (Full-Stack, Security, Performance, AI / LLM, Web3 / Chain).
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
- **Engineer AI lens** (see §8): tool-call forensics, cache-hit ledger (`saved_usd`), JSON Schema guard, MCP traffic lens, streaming SLO gates.

---

## 5. 🛡️ Security Specialist & Pentesting Suite

- **IDOR Matrix Testing (`src/net/idor_matrix.rs`)**: Replays intercepted HTTP requests across multiple Level 2 Identity Contexts. Uses `compute_json_structure_hash` to hash JSON response schemas, eliminating false-positive IDOR alerts on 200 OK error wrappers.
- **Traffic Tamper Engine (`src/net/tamper.rs`)**: Pause, edit, forward, drop, or inject custom HTTP headers into outgoing requests. Supports inline response mocking (`mock_status`, `mock_body`, `delay_ms`).
- **SEC-027 Secret Redaction**: Automatically masks sensitive `Authorization`, `Cookie`, and `X-API-Key` values with `[REDACTED]` in proxy logs.
- **Pure-Rust SSL CA Generator (`src/net/ssl_ca.rs`)**: Generates custom ECDSA P-256 root CAs (`Thor Dev CA`) and leaf certs (`*.localhost`, `*.test`). Auto-installs trust into macOS Keychain (`security`), Windows Cert Store (`certutil`), or Linux NSS DB.
- **JWT & Web3 Decoder (`src/dx/decoder.rs`, `src/dx/web3_inspector.rs`)**: Auto-detects and decodes JWT claims, base64 payloads, EVM 4-byte ABI selectors, and EIP-55 address checksums.
- **Consent-gated labs** (`src/net/consent.rs` + Security domain engines): SSRF / session-fixation / fuzz / race-live refuse public targets without `lab_consent`; expanded abuse-host blocklist.

---

## 6. ⚡ Performance & Load Engine (`src/load/`)

- **Thor Load Engine**: Canonical `ThorScenario` benchmark spec with dual runners:
  - **BrowserRunner**: Real CWV via `CdpBridge::web_vitals_injection_script`, soft-nav collection (refuses all-zero theater), Resource Timing → network ingest, LoAF/long-task → TBT, measured transfer/decoded/`lcp_media_bytes` into budgets.
  - **ProtocolRunner**: Multi-threaded Rust HTTP/WS scale runner pushing 10,000+ RPS.
- **Lock-Free Atomic Metrics (`metrics.rs`)**: Uses `[AtomicUsize; 200]` logarithmic bucket array for $O(1)$ zero-lock latency tracking (11.2 ns/op at 10,000+ RPS). SoftNav vitals attach to `LoadMetrics`.
- **P0 CDP Freezing**: `Page.setWebLifecycleState(Frozen)` with Instant-timed freeze/unfreeze samples feeding Freeze–Resume Bench.
- **Feature-path SLAs** (`src/features/sla.rs` + `benches/feature_sla_bench.rs`): hard latency budgets on registry lookup, HDR, authz, JWT, export, CDP parse/ingest, LCP forensics, and related hot paths.

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
- **MCP Traffic Lens** (AI domain): folds Thor JSON-RPC shapes with id-correlated request/response latency.

---

## 8. 🧪 Engineer Feature Suite (`src/features/`)

**144** stable `FeatureId`s registered in `FeatureRegistry` / `FeatureEngine`, routed by `FeatureOrchestrator`, exposed in DevCC.

| Domain | Wave-1 (original) | Wave-2+ (added) | Total |
|---|---:|---:|---:|
| Frontend | 11 | 12 | 23 |
| Backend | 12 | 12 | 24 |
| Security | 12 | 12 | 24 |
| Performance | 13 | 12 | 25 |
| QA | 12 | 12 | 24 |
| AI | 12 | 12 | 24 |
| **Total** | **72** | **72** | **144** |

### Frontend (23)
Component Tree Lens, Responsive Matrix, CSS Cascade, Design Tokens, A11y Lens, State Time-Travel, CLS Heatmap, Component Sandbox, Network↔Component Binding, Font Auditor, Z-Index Visualizer, **Hydration Diff**, **CSS Coverage Map**, **Focus Order Overlay**, **Event Listener Map**, **CSS Variable Inspector**, **Layout Thrash Detector**, **Media/CQ Matrix**, **Image Delivery Auditor**, **CSS Containment Advisor**, **ARIA Live Monitor**, **Selector Specificity Budget**, **DOM Snapshot Capture**.

### Backend (24)
API Workbench, Auth Vault, Contract Validation, Webhook Sink, IDOR Matrix, Request Chaining, gRPC/GraphQL Inspector, Trace Adjacency, Mock/Stub, Chaos Injection, Idempotency Auditor, DB Adjacency, **Webhook Signature Vault** (HMAC-SHA256), **Rate Limit Probe**, **Pagination Auditor**, **Cache Semantics Lens**, **Multipart Analyzer (offline)**, **SSE Server Lab**, **OpenAPI Diff Gate**, **OAuth Flow Capture**, **Queue Adjacency**, **gRPC Stream Analyzer (offline)**, **Live Stub Server**, **Header Contract Fuzzer**.

### Security (24)
Authz Matrix, Response Diff, Header Analyzer, JWT Deep, Promote Finding, Surface Crawler, Storage Auditor, Fuzz Repeater, CSRF Analyzer, Sink Pollution, TLS Inspector, OOB Listener, **DOM XSS Taint Analyzer (offline)**, **CORS Deep Probe**, **CSP Policy Forensics**, **OAuth/OIDC Misconfig**, **Cookie Session Forensics**, **Session Fixation Lab**, **GraphQL Authz Matrix**, **Mass Assignment Diff**, **Race Window Analyzer**, **SSRF Lab Harness**, **SRI Supply Chain Auditor**, **WebSocket Authz Probe**.

### Performance (25)
Perf Budgets, Trace Diff, Waterfall Forensics, Memory Leak Hunter, Synthetic Monitor, RUM Lab, HDR Histogram, INP Attribution, CDN Cache, Vitals Under Load, Flame Graph, Third-Party Attribution, Performance SLA, **LCP Element Forensics**, **Long Task Radar**, **Main-Thread Block Map**, **BFCache Probe**, **Freeze–Resume Bench**, **Soft-Nav Vitals**, **Speculation Lab**, **Conn Pool Inspector**, **HTTP/3 Path Audit**, **Compression Budget**, **Media Weight Gate**, **Continuous Profiler**.

### QA (24)
Resilient Locators, Flake Radar, Visual Regression, Identity Fixtures, Data-Driven Scenarios, Real Vitals Gates, Contract Testing, Deflaked Export, CI Bundle, Time-Travel Trace, Determinism Harness, Authz Matrix Tests, **Flake Triager**, **Quarantine Policy**, **Contract Drift Gate**, **Hermetic Fixture Pack**, **A11y Test Gen**, **Snapshot Strategy Pack**, **Coverage Adjacency Map**, **Parallel Shard Planner**, **Trace Property Miner**, **Runtime Self-Heal**, **Oracle Synthesizer**, **Mutation Hook Bridge**.

### AI (24)
LLM Inspector, Streaming Waterfall, Prompt Diff, Cost Dashboard, Eval Replay, Model Comparison, Agent Trace, Context Heatmap, RAG Inspector, Injection Scanner, LLM Load, Deterministic Replay, **Tool Call Forensics**, **JSON Schema Guard**, **Cache Hit Ledger**, **Token Waste Auditor**, **Streaming SLO Gates**, **Router Decision Trace**, **MCP Traffic Lens**, **Eval Dataset Miner**, **Hallucination Oracle**, **Embedding RAG Bench**, **Protocol Quirk Diff**, **Local/Cloud Parity**.

### IPC surface
- Chrome → Rust: `feature_list`, `feature_invoke` (`slug` + JSON payload; optional `from_active_tab`), `lens_payload` (`category`).
- Rust → Chrome: `__thorFeatureList`, `__thorFeatureResult`, `__thorLensPayload`.

### Honesty rules
Offline-only analyzers use **Analyzer (offline)** naming (e.g. Multipart, gRPC Stream, DOM XSS). Live labs (Live Stub, SSE, fixation, SSRF, race-live) are consent-gated and bind/target private hosts unless `lab_consent` / `public_ack` is set.
