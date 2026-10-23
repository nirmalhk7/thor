//! Newline-delimited MCP bridge to the running Thor instance. No browser launch,
//! global environment credentials, stdout logging, or external proxy traffic.

use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use super::connection::{self, ConnectionInfo};

const MAX_INPUT_BYTES: usize = 256 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

pub async fn run() -> anyhow::Result<()> {
    let info = connection::load(&connection::connection_path()).map_err(|_| {
        anyhow::anyhow!(
            "Thor MCP connection unavailable; launch Thor before connecting an MCP client"
        )
    })?;
    bridge(
        info,
        BufReader::new(tokio::io::stdin()),
        tokio::io::stdout(),
    )
    .await
}

pub async fn bridge<R: AsyncBufRead + Unpin, W: AsyncWrite + Unpin>(
    info: ConnectionInfo,
    mut input: R,
    mut output: W,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(40))
        .build()?;
    while let Some(line) = read_line(&mut input).await? {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let response = match serde_json::from_slice::<Value>(&line) {
            Ok(payload) => forward(&client, &info, payload).await?,
            Err(_) => Some(super::tools::rpc_error(Value::Null, -32700, "Parse error")),
        };
        if let Some(response) = response {
            output.write_all(&serde_json::to_vec(&response)?).await?;
            output.write_all(b"\n").await?;
            output.flush().await?;
        }
    }
    Ok(())
}

async fn read_line<R: AsyncBufRead + Unpin>(input: &mut R) -> anyhow::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        let available = input.fill_buf().await?;
        if available.is_empty() {
            return Ok((!line.is_empty()).then_some(line));
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map(|index| index + 1).unwrap_or(available.len());
        if line.len() + consumed > MAX_INPUT_BYTES {
            anyhow::bail!("MCP input exceeds size limit");
        }
        line.extend_from_slice(&available[..consumed]);
        input.consume(consumed);
        if newline.is_some() {
            return Ok(Some(line));
        }
    }
}

async fn forward(
    client: &reqwest::Client,
    info: &ConnectionInfo,
    payload: Value,
) -> anyhow::Result<Option<Value>> {
    let expects_response = payload.get("id").is_some();
    let mut response = client
        .post(info.url())
        .bearer_auth(info.bearer_token())
        .header("accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2025-06-18")
        .json(&payload)
        .send()
        .await?;
    if response.status() == reqwest::StatusCode::ACCEPTED && !expects_response {
        return Ok(None);
    }
    if response.status() != reqwest::StatusCode::OK {
        anyhow::bail!("Thor MCP request failed with HTTP {}", response.status());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            anyhow::bail!("MCP response exceeds size limit");
        }
        body.extend_from_slice(&chunk);
    }
    let value = serde_json::from_slice(&body)?;
    Ok(Some(value))
}
