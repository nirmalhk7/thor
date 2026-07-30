# Thor Developer Testing & Quality Assurance Manual

## Overview

Thor enforces strict testing discipline: **no code change is declared complete without empirical automated test verification**. The suite covers library unit tests, Engineer Feature Suite regression packs, integration tests, MCP RPC endpoints, performance/SLA benches, and concurrency safety checks.

---

## Running Test Suites

### 1. Library Unit Tests (`cargo test --lib`)

```bash
cargo test --lib
cargo test --lib features::   # FeatureEngine, orchestrator, domain engines, SLAs
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
cargo test --test features_tests
cargo test --test features_v2_tests
cargo test --test features_wave2_tests
cargo test --test features_wave4_tests
cargo test --test features_wave5_tests
```

Registry invariant: `FeatureId::ALL.len() == 144` (asserted in lib + wave packs).

---

### 3. MCP Server Test Suite (`cargo test --lib mcp`)

```bash
cargo test --lib mcp
```

Covers initialize, tools/list, tools/call graceful paths, SSE + JSON-RPC HTTP endpoints.

---

### 4. Integration Tests (`cargo test --test integration_tests`)

```bash
cargo test --test integration_tests
```

Cross-module contracts: AppState, history scope, Locust local listener, profile toggle, shield/tamper, palette payloads.

---

### 5. Live Chromium Launch Smoke Test (`live_launch_smoke`)

Requires an installed Chrome/Chromium executable:

```bash
cargo test --lib live_launch_smoke -- --ignored
```

DevCC **`from_active_tab`** happy-path also needs a live `TabSession` evaluator (hook published from `chromium_tabs`). Without Chromium, invoke must return a clear `Err` — never empty-success theater.

---

## Benchmark Suite (`benches/`)

```bash
cargo bench --bench cold_start_bench
cargo bench --bench ipc_bench
cargo bench --bench cmd_palette_bench
cargo bench --bench memory_soak_bench
cargo bench --bench feature_sla_bench   # Feature-path latency SLAs
```

---

## Code Quality & Warning Discipline

```bash
cargo check
```

Prefer fixing warnings in touched modules; do not expand the registry without updating `FeatureId::ALL`, orchestrator arms, DevCC count, and the feature test packs.
