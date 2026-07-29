# Thor Security Suite & Pentesting Documentation

## Overview

Thor provides a comprehensive, privileged security testing suite embedded directly into the browser kernel (`src/net/` and `src/dx/`). Designed for security specialists, penetration testers, and security auditors, Thor enables real-time traffic tampering, IDOR scanning across isolated identity contexts, SSL certificate authority generation, and secret redaction.

---

## 1. IDOR Matrix Engine (`src/net/idor_matrix.rs`)

The **IDOR (Insecure Direct Object Reference) Matrix Engine** automates authorization testing across multiple user roles (`Admin`, `Standard User`, `Anonymous`).

```mermaid
graph LR
    A[Intercepted Request: /api/user/1002] --> B[Identity Context: Admin (200 OK)]
    A --> C[Replay via Identity Context: Standard User]
    A --> D[Replay via Identity Context: Anonymous]
    C --> E[Structural JSON Hash Comparison]
    D --> E
    E -->|Matches Authorized Schema| F[🚨 IDOR Vulnerability Flagged!]
    E -->|Matches Error Schema| G[✅ Access Denied Verified]
```

### Key Technical Features

1. **Multi-Role Context Replay**: Replays intercepted HTTP requests using distinct header and cookie sets bound to Level 2 Identity Contexts.
2. **Structural JSON Hashing (`compute_json_structure_hash`)**:
   - Recursively traverses JSON response bodies, extracting key names and structural types while masking dynamic values (IDs, timestamps, strings).
   - Prevents false-positive IDOR alerts on `200 OK` HTTP responses that wrap error messages (e.g. `{"status": "error", "message": "Unauthorized"}`).
   - If an unauthorized role receives a `200 OK` response whose structural JSON hash matches an error schema rather than the authorized data schema, Thor suppresses the false alert.

---

## 2. Traffic Tamper Engine & Response Mocking (`src/net/tamper.rs`)

The **Traffic Tamper Engine** operates directly on outgoing and incoming HTTP traffic via CDP `Fetch` domain integration.

### Tamper Rule Configuration (`TamperRule`)

```rust
pub struct TamperRule {
    pub id: String,
    pub pattern: String,
    pub match_header: Option<String>,
    pub replace_header_key: Option<String>,
    pub replace_header_val: Option<String>,
    pub mock_status: Option<u16>,
    pub mock_body: Option<String>,
    pub delay_ms: Option<u64>,
    pub active: bool,
}
```

### Capabilities
- **Request Pausing & Modification**: Intercepts requests matching glob patterns, allowing live header key/value editing before forwarding to upstream servers.
- **Inline Response Mocking**: Intercepts matching requests and fulfills synthetic responses with custom HTTP status codes (`mock_status`), custom response bodies (`mock_body`), and artificial latency delays (`delay_ms`).
- **SEC-027 Secret Redaction**: Automatically sanitizes sensitive `Authorization: Bearer <token>`, `Cookie`, `X-API-Key`, and secret parameters in proxy history logs with `[REDACTED]`.

---

## 3. Pure-Rust SSL CA Generator (`src/net/ssl_ca.rs`)

Thor includes an embedded, zero-external-dependency Certificate Authority generator built on `rcgen`.

### Architecture & Trust Store Installation

1. **Root CA Generation**: Creates a local ECDSA P-256 root certificate (`Thor Dev CA`) valid for local HTTPS interception.
2. **Dynamic Wildcard Leaf Certificates**: Dynamically signs leaf certificates for local development domains (`*.localhost`, `*.test`, `dev.local`).
3. **Cross-Platform Trust Store Installer**:
   - **macOS**: `security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain`
   - **Windows**: `certutil -addstore -f "ROOT"`
   - **Linux**: Installs into NSS database (`~/.pki/nssdb`) and system trust store (`/etc/ssl/certs`).

---

## 4. JWT & Protocol Decoder Studio (`src/dx/decoder.rs`)

The **Decoder Studio** automatically identifies and decodes complex payload formats:

1. **JWT Auto-Detection & Claims Extraction**:
   - Detects `eyJ...` JWT patterns in headers, query params, or JSON bodies.
   - Decodes header metadata, algorithm types (`RS256`, `HS256`, `none`), payload claims (`sub`, `iss`, `exp`, `roles`), and signature components.
2. **Web3 EVM & Solana Decoder (`web3_inspector.rs`)**:
   - Decodes EVM 4-byte ABI method selectors (e.g. `0xa9059cbb` $\rightarrow$ `transfer(address,uint256)`).
   - Validates EIP-55 checksummed Ethereum addresses.
