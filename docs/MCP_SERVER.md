# Model Context Protocol (MCP) Server Specification & Integration Guide

## Overview

Thor embeds a high-performance **Model Context Protocol (MCP)** server implemented in pure Rust (`src/mcp/server.rs` and `src/mcp/tools.rs`). The MCP server allows external AI coding assistants (Claude Desktop, Cursor, OpenAI Codex, Antigravity, Windsurf) to programmatically inspect, automate, and drive the active Thor browser tab via standard JSON-RPC 2.0.

---

## Server Endpoints & Architecture

The MCP server runs locally on **`http://127.0.0.1:8765`**:

| Endpoint | Method | Content-Type | Purpose |
| :--- | :--- | :--- | :--- |
| `/sse` | `GET` | `text/event-stream` | Establishes Server-Sent Events (SSE) stream connection. Emits `endpoint` initial event and JSON-RPC responses. |
| `/message` | `POST` | `application/json` | Receives JSON-RPC 2.0 requests from client. |

---

## JSON-RPC 2.0 Protocol Methods

Thor's MCP server implements version `2024-11-05` of the MCP specification:

### 1. `initialize`
- **Request**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2024-11-05",
      "capabilities": {},
      "clientInfo": { "name": "claude-desktop", "version": "1.0.0" }
    }
  }
  ```
- **Response**:
  ```json
  {
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
      "protocolVersion": "2024-11-05",
      "capabilities": {},
      "serverInfo": { "name": "Thor", "version": "1.0.0" }
    }
  }
  ```

### 2. `notifications/initialized`
- **Request**: `{"jsonrpc": "2.0", "method": "notifications/initialized"}`
- **Response**: `None` (non-blocking notification).

### 3. `tools/list`
- **Request**: `{"jsonrpc": "2.0", "id": 2, "method": "tools/list"}`
- **Response**: Returns array of 8 registered tool definitions with JSON schema inputs.

---

## 🛠️ The 8 Browser Automation Tools

### 1. `thor_get_screenshot`
- **Description**: Takes a PNG screenshot of the active browser tab via CDP `Page.captureScreenshot`.
- **Arguments**: None.
- **Result Format**: `[{"type": "text", "text": "Screenshot captured successfully"}]`

### 2. `thor_navigate`
- **Description**: Navigates the active browser tab to a URL.
- **Arguments**:
  - `url` (`string`, required): Target URL (e.g. `"http://localhost:3000"`).
- **Result Format**: `[{"type": "text", "text": "Navigated to http://localhost:3000"}]`

### 3. `thor_eval_js`
- **Description**: Evaluates arbitrary JavaScript inside the active tab context.
- **Arguments**:
  - `script` (`string`, required): JavaScript code to run (e.g. `"document.title"`).
- **Result Format**: `[{"type": "text", "text": "Result: \"My Application\""}]`

### 4. `thor_get_dom`
- **Description**: Extracts interactive DOM elements (`<a>`, `<button>`, `<input>`) with coordinates.
- **Arguments**: None.
- **Result Format**: JSON string containing element array: `[{"tag": "BUTTON", "id": "submit-btn", "text": "Submit", "rect": {"x": 100, "y": 200, "width": 80, "height": 30}}]`

### 5. `thor_click`
- **Description**: Clicks on an element matching a CSS selector.
- **Arguments**:
  - `selector` (`string`, required): CSS selector (e.g. `"#submit-btn"`).
- **Result Format**: `[{"type": "text", "text": "Clicked #submit-btn"}]`

### 6. `thor_type`
- **Description**: Types text into an element matching a CSS selector.
- **Arguments**:
  - `selector` (`string`, required): CSS selector (e.g. `"input[name='username']"`).
  - `text` (`string`, required): Text to type.
- **Result Format**: `[{"type": "text", "text": "Typed into input[name='username']"}]`

### 7. `thor_hover`
- **Description**: Hovers over an element matching a CSS selector.
- **Arguments**:
  - `selector` (`string`, required): CSS selector (e.g. `".dropdown-menu"`).
- **Result Format**: `[{"type": "text", "text": "Hovered over .dropdown-menu"}]`

### 8. `thor_scroll`
- **Description**: Scrolls the page by `x, y` pixel deltas.
- **Arguments**:
  - `x` (`number`, required): Horizontal scroll amount.
  - `y` (`number`, required): Vertical scroll amount.
- **Result Format**: `[{"type": "text", "text": "Scrolled by 0, 500"}]`

---

## Client Integration Examples

### Claude Desktop Setup (`claude_desktop_config.json`)

Add Thor to your Claude Desktop configuration file:

```json
{
  "mcpServers": {
    "thor": {
      "url": "http://127.0.0.1:8765/sse"
    }
  }
}
```

### Python MCP Client Integration

```python
import asyncio
from mcp import ClientSession
from mcp.client.sse import sse_client

async def main():
    async with sse_client("http://127.0.0.1:8765/sse") as streams:
        async with ClientSession(streams[0], streams[1]) as session:
            await session.initialize()
            
            # List available tools
            tools = await session.list_tools()
            print(f"Available tools: {[t.name for t in tools.tools]}")
            
            # Navigate Thor browser
            await session.call_tool("thor_navigate", {"url": "http://localhost:8080"})
            
            # Inspect interactive DOM elements
            dom = await session.call_tool("thor_get_dom", {})
            print(f"Interactive DOM: {dom.content[0].text}")

asyncio.run(main())
```

---

## Unit & Integration Test Suite

The MCP server test suite is defined in [`src/mcp/server.rs`](file:///Users/nirmalhk7/Documents/DevWorld/thor/src/mcp/server.rs#L85-L135) and [`src/mcp/tools.rs`](file:///Users/nirmalhk7/Documents/DevWorld/thor/src/mcp/tools.rs#L295-L420):

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
