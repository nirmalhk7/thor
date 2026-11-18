# Thor MCP integration

Thor exposes browser operations through a local MCP server. The `thor-labs --stdio` bridge connects MCP clients to an already running Thor instance; it does not launch a browser.

## Ownership

| Path | Responsibility |
| --- | --- |
| `src/bin/thor_labs.rs` | Stdio bridge entry point and explicit `--self-cdp` tooling mode |
| `src/mcp/connection.rs` | Per-run connection record and private credential storage |
| `src/mcp/stdio.rs` | Newline-delimited JSON-RPC forwarding to Thor |
| `src/mcp/server.rs` | Loopback HTTP transports, authentication, sessions, request limits |
| `src/mcp/tools/mod.rs` | JSON-RPC dispatch, version negotiation, result envelopes |
| `src/mcp/tools/catalog.rs` | Tool names, descriptions, argument schemas, validation |
| `src/mcp/tools/browser.rs` | Target resolution and browser commands |
| `src/browser/cef/host/commands.rs` | Native CEF browser command execution |

Add or change a tool's schema in `catalog.rs` and its execution in `browser.rs`. Both transports share the same dispatch and validation.

## Connect a client

Launch the packaged Thor app first. Configure an MCP client to run its installed `thor-labs` binary:

```json
{
  "mcpServers": {
    "thor": {
      "command": "/absolute/path/to/thor-labs",
      "args": ["--stdio"]
    }
  }
}
```

Replace the command with the installed binary's absolute path. No token belongs in this configuration. The bridge reads `thor_data_dir()/Mcp/connection.json`, validates that it describes a loopback endpoint, and forwards requests with its bearer credential. On Unix the record must be private to its owner. A missing or invalid record fails with `Thor MCP connection unavailable; launch Thor before connecting an MCP client`.

The separate `--self-cdp` mode requires `THOR_SELF_CDP_URL`; it attaches a CDP runtime and starts an MCP server. It is a tooling entry point, not Thor's product launch path.

## HTTP transports

The server binds `127.0.0.1:8765`. All routes require `Authorization: Bearer <per-run credential>` and reject requests carrying an `Origin` header. Use the stdio bridge to discover and supply credentials.

| Route | Method | Behavior |
| --- | --- | --- |
| `/mcp` | `POST` | JSON-RPC response as JSON; notifications return HTTP 202 |
| `/mcp/sse` | `GET` | SSE stream beginning with an `endpoint` event |
| `/mcp/message?session_id=...` | `POST` | Accept a request for a live SSE session; responses arrive as `message` events |

Use the endpoint from the SSE event rather than inventing a session ID. Dropping a stream revokes its session. HTTP requests are limited to 256 KiB; concurrent work and SSE sessions are each capped at 32. The stdio bridge also limits input to 256 KiB and responses to 16 MiB.

## JSON-RPC contract

Requests use `"jsonrpc": "2.0"`. Supported methods are `initialize`, `ping`, `tools/list`, and `tools/call`. Valid notifications have no `id` and receive no JSON-RPC reply.

`initialize` requires `params.protocolVersion`. Supported versions are `2024-11-05`, `2025-03-26`, and `2025-06-18`; an unsupported requested version negotiates `2025-06-18`. The response advertises `tools.listChanged: false`, server name `Thor`, and the version from `Cargo.toml`.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "thor_navigate",
    "arguments": {"url": "http://localhost:3000"}
  }
}
```

Invalid request shapes return error code `-32600`; unknown methods return `-32601`; invalid tool names or arguments return `-32602`. Argument objects reject unknown properties. Browser execution failures instead return a tool result with `isError: true` and a text error. Clients must inspect that field.

## Browser tools

`tools/list` is the authoritative schema. The catalog currently exposes 15 tools:

| Tool | Arguments beyond optional target fields | Result or operation |
| --- | --- | --- |
| `thor_tabs_list` | None | Text containing JSON with `active_target`, `tabs`, and `runtime_status` |
| `thor_tabs_create` | Optional `url`, `identity_id` | Create a tab; defaults to `about:blank` and identity 0; text contains its `id` |
| `thor_tabs_activate` | Required `id` | Activate the selected tab |
| `thor_tabs_close` | Required `id` | Close the selected tab and erase unsaved browsing state |
| `thor_navigate` | Required `url` | Start navigation |
| `thor_back` | None | Navigate back |
| `thor_forward` | None | Navigate forward |
| `thor_reload` | None | Reload |
| `thor_eval_js` | Required `script` | Evaluate JavaScript and return its result as text |
| `thor_get_screenshot` | None | PNG image content with base64 `data` and `mimeType: image/png` |
| `thor_get_dom` | Optional `max_chars` | Read `document.documentElement.outerHTML`, truncated to the requested length |
| `thor_click` | Required `selector` | Scroll the element into view and click it |
| `thor_type` | Required `selector`, `text` | Set input, textarea, or editable content and dispatch input/change events |
| `thor_hover` | Required `selector` | Dispatch a `mouseover` event |
| `thor_scroll` | Optional `selector`, `y` | Scroll an element into view, or scroll the window to absolute position `(0, y)` |

Targeted operations accept optional integer `id` and `generation` fields, each at least 1. Omit `id` to use the active tab. Capture a target from `thor_tabs_list` and send its `generation` to reject stale navigation targets. Unavailable selections fail with `Selected tab unavailable`; a generation mismatch fails with `Stale target`.

`identity_id` is a nonnegative integer. Required strings are nonempty except `thor_type.text`, which permits clearing an input. `max_chars` defaults to 200,000 and accepts integers from 1 through 1,000,000. With no selector, `thor_scroll.y` defaults to 0; an `x` argument is not accepted.

Navigation and tab-control success returns `Browser operation accepted`; this acknowledges the command and does not prove that navigation finished. Element actions check their script's `ok` result. An empty screenshot is an error. A missing engine returns `Browser engine unavailable`, not a fabricated success.

## Verification coverage

Relevant integration suites live in `tests/mcp_protocol_tests.rs`, `tests/mcp_transport_tests.rs`, `tests/mcp_stdio_tests.rs`, and `tests/mcp_connection_tests.rs`. They cover dispatch and target generation, transport authorization and sessions, stdio forwarding, and connection credentials respectively. Consult the test source and current command output for results; this document does not record a passing run.

```bash
cargo test --locked --workspace -- --test-threads=1
```

A live-browser check must exercise the running CEF instance in addition to transport and engine tests.
