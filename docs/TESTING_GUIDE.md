# Thor Developer Testing & Quality Assurance Manual

## Overview

Thor enforces strict testing discipline: **no code change is declared complete without empirical automated test verification**. The test suite covers unit tests, integration tests, MCP server RPC endpoints, performance benchmark assertions, and concurrency safety checks.

---

## Running Test Suites

### 1. Library Unit Tests (`cargo test --lib`)
Executes all 85 library unit tests across all Thor modules (`src/`):

```bash
cargo test --lib
```

Example Output:
```text
running 85 tests
test daemon::prewarm::tests::daemon_starts_empty ... ok
test devtools::cdp_integration::tests::devtools_url_from_ws ... ok
test dx::_3d_dom_snapshot::tests::test_dom_layer_extraction ... ok
test dx::css_overlay::tests::test_css_overlay_injection_script ... ok
test dx::llm_clean_context::tests::test_dom_cleaning ... ok
test dx::llm_studio::tests::test_sse_stream_parser ... ok
test dx::test_generator::tests::test_promote_to_test_export ... ok
test mcp::server::tests::test_mcp_message_endpoint ... ok
test mcp::server::tests::test_mcp_sse_endpoint ... ok
test mcp::tools::tests::test_mcp_initialize ... ok
test mcp::tools::tests::test_mcp_tools_list ... ok
test net::idor_matrix::tests::test_idor_role_config_builder ... ok
test net::tamper::tests::test_automated_tamper_rule ... ok
test shell::engine_status::tests::status_is_honest_chromium_m2 ... ok

test result: ok. 84 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.01s
```

---

### 2. MCP Server Test Suite (`cargo test --lib mcp`)
Executes the dedicated Model Context Protocol (MCP) server test suite:

```bash
cargo test --lib mcp
```

Output:
```text
running 8 tests
test mcp::tools::tests::test_mcp_initialize ... ok
test mcp::tools::tests::test_mcp_notifications_initialized ... ok
test mcp::tools::tests::test_mcp_tools_list ... ok
test mcp::tools::tests::test_mcp_unknown_method ... ok
test mcp::tools::tests::test_mcp_tools_call_unknown_tool ... ok
test mcp::tools::tests::test_mcp_tools_call_graceful_no_pages ... ok
test mcp::server::tests::test_mcp_sse_endpoint ... ok
test mcp::server::tests::test_mcp_message_endpoint ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 77 filtered out; finished in 0.02s
```

---

### 3. Integration Tests (`cargo test --test integration_tests`)
Executes integration tests across IPC, shell dispatch, and tamper engines:

```bash
cargo test --test integration_tests
```

---

### 4. Live Chromium Launch Smoke Test (`live_launch_smoke`)
Requires a installed Chrome/Chromium executable on the host machine:

```bash
cargo test --lib live_launch_smoke -- --ignored
```

---

## Benchmark Suite (`benches/`)

Thor includes Criterion benchmarks to track cold-start times, IPC ring buffer throughput, command palette fuzzy matching speed, and memory soak performance:

```bash
cargo bench --bench cold_start_bench
cargo bench --bench ipc_bench
cargo bench --bench cmd_palette_bench
cargo bench --bench memory_soak_bench
```

---

## Code Quality & Warning Discipline

Thor enforces zero compiler warnings across all compilation profiles:

```bash
cargo check
```

Expected Output:
```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.15s
```
