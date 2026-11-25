# Thor Developer Testing & Quality Assurance Manual

## QA quick start: check a local response contract

Before merging an API change, compare an observed status and JSON body with the
response your client expects. Open **Cmd+Shift+K**, search **verify response
contract**, and choose **Contract Testing**. Paste the
[widget response fixture](../tests/fixtures/qa_response_contract.json) into
**Input JSON**, then choose **Run feature**. Thor uses the pasted data; it does
not call the API.

The fixture reports one verified interaction with `satisfied: true`. Change
the observed `available` value to `"yes"` and run it again. The report
identifies the incorrect field and expected type. Omitting the observed body
does not pass when a response schema is supplied. A literal JSON `null` is a
body and is checked against the schema.

For your own fixture, include one observed `actuals` entry for each expected
method and path. Supply an observed status and the JSON body when checking a
response schema. This workflow checks supplied evidence; it does not generate
a Playwright test.

## Overview

Thor enforces strict testing discipline: **no code change is declared complete without empirical automated test verification**. The suite covers library unit tests, Engineer Feature Suite regression packs, integration tests, MCP RPC endpoints, performance/SLA benches, and concurrency safety checks. Some tests change process-wide environment variables and `AppState`; run regression suites with `--test-threads=1`, as CI does.

---

## Running Test Suites

### 1. Library Unit Tests (`cargo test --lib`)

```bash
cargo test --locked --lib -- --test-threads=1
cargo test --locked --lib features:: -- --test-threads=1
```

Feature-related lib coverage includes registry (144), domain analyzers, CDP bridge ingest/parse, quarantine hydrate/persist, SoftNav refuse-all-zero, HMAC webhook vectors, and feature-path SLA enforcement.

---

### 2. Engineer Feature Suite integration packs

| Binary | Focus |
|---|---|
| `features_tests` | Registry length, persona group smoke (all domains execute), critical paths |
| `features_v2_tests` | IPC invoke/lens, CDP bridge, IDOR/webhook/OOB active halves |
| `features_wave2_tests` | Wave-2 deepenings + QA composition (hermetic, flake-from-traces, quarantine) |
| `features_wave4_tests` | Active-tab Err-without-runtime, naming honesty, SSE tool_result, fixation URL, SoftNav zeros, budget bytes |
| `features_wave5_tests` | SoftNav on `LoadMetrics` + `lcp_media_bytes`, CiBundle non-empty artifacts, quarantine `persist_ok` |

```bash
cargo test --locked --test features_tests -- --test-threads=1
cargo test --locked --test features_v2_tests -- --test-threads=1
cargo test --locked --test features_wave2_tests -- --test-threads=1
cargo test --locked --test features_wave4_tests -- --test-threads=1
cargo test --locked --test features_wave5_tests -- --test-threads=1
```

Registry invariant: `FeatureId::ALL.len() == 144` (asserted in lib + wave packs).

---

### 3. MCP integration suites

```bash
cargo test --locked --test mcp_protocol_tests -- --test-threads=1
cargo test --locked --test mcp_transport_tests -- --test-threads=1
cargo test --locked --test mcp_stdio_tests -- --test-threads=1
cargo test --locked --test mcp_connection_tests -- --test-threads=1
```

Covers JSON-RPC validation, browser command dispatch and navigation generations, authenticated HTTP/SSE transports, stdio forwarding, and private connection credentials. Tool schemas and transport contracts are documented in [MCP_SERVER.md](MCP_SERVER.md).

---

### 4. Integration Tests (`cargo test --test integration_tests`)

```bash
cargo test --locked --test integration_tests -- --test-threads=1
```

Cross-module contracts: AppState, history scope, Locust local listener, profile toggle, shield/tamper, palette payloads.

---

### 5. Live Chromium Launch Smoke Test (`live_launch_smoke`)

This ignored test exercises the standalone CDP runtime, not the embedded CEF product. Configure an owned browser through `THOR_CHROMIUM_PATH`; system browser discovery additionally requires `THOR_ALLOW_SYSTEM_CHROME=1`.

```bash
cargo test --locked --lib live_launch_smoke -- --ignored --test-threads=1
```

DevCC **`from_active_tab`** paths need an evaluator registered through `CdpBridge::install_active_evaluator` in `src/features/cdp_bridge/mod.rs`. Without that evaluator, invocation must return a clear `Err`. A running browser alone does not prove this hook is installed. Native CEF browser command dispatch lives separately in `src/browser/cef/host/commands.rs`.

The standalone prewarm suites in `tests/test_prewarm_exhaust.rs` and `tests/test_runtime_prewarm.rs` also require an owned Chromium executable. They are explicitly ignored in ordinary regression runs. Run either with `--ignored --test-threads=1` after configuring `THOR_CHROMIUM_PATH`; a browser startup failure then fails the test instead of silently returning success.

---

## Benchmark Suite (`benches/`)

Choose benchmarks by the operation their source measures:

| Target | Actual measurement |
| --- | --- |
| `shield_sanitize_bench` | Security shield URL sanitization |
| `locust_scoring_bench` | Locust metric scoring |
| `cmd_palette_bench` | Palette search, URL sanitization, stack detection, and metric scoring |
| `consent_classification_bench` | Private/local host classification |
| `feature_sla_bench` | Feature-path latency SLAs |

```bash
cargo bench --bench shield_sanitize_bench
cargo bench --bench locust_scoring_bench
cargo bench --bench cmd_palette_bench
cargo bench --bench consent_classification_bench
cargo bench --bench feature_sla_bench   # Feature-path latency SLAs
```

---

## Code Quality & Warning Discipline

```bash
make check-structure
cargo check --locked --workspace --all-targets
cargo test --locked --workspace -- --test-threads=1
```

`check-structure` exercises the checker CLI, then checks Rust filenames, directory-owned module entries, and parent module declarations. It does not replace Rust compilation or prove feature behavior.

Prefer fixing warnings in touched modules; do not expand the registry without updating `FeatureId::ALL`, `src/features/catalog.rs`, handler dispatch under `src/features/orchestrator/`, DevCC count, and the feature test packs.
