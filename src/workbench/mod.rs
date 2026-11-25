//! Trusted tool surface and shared commands. External pages never reach this API.

mod ai;
mod commands;
mod persistence;

pub use commands::{Workbench, WorkbenchCommand};

pub const AI_URL: &str = "thor://workbench/ai";
pub const API_URL: &str = "thor://workbench/api";
pub const HOME_URL: &str = "thor://workbench/";
pub const SECURITY_URL: &str = "thor://workbench/security";
pub const TOOLS_URL: &str = "thor://workbench/tools";
pub const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

pub fn navigation_url(input: &str) -> String {
    let input = input.trim();
    if input.is_empty() {
        return HOME_URL.into();
    }
    if input.contains("://") || input.starts_with("about:") {
        return input.into();
    }
    let host = input
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let local = ["localhost", "127.0.0.1", "[::1]"]
        .into_iter()
        .any(|candidate| is_host_or_port(host.as_str(), candidate));
    format!("{}://{input}", if local { "http" } else { "https" })
}

fn is_host_or_port(host: &str, candidate: &str) -> bool {
    host == candidate
        || host
            .strip_prefix(candidate)
            .is_some_and(|suffix| suffix.starts_with(':'))
}

pub fn is_tool_document(url: &str, main_frame: bool) -> bool {
    main_frame
        && trusted_url(url)
            .is_some_and(|url| matches!(url.path(), "/ai" | "/api" | "/security" | "/tools"))
}

pub fn command_allowed(url: &str, main_frame: bool, command: &WorkbenchCommand) -> bool {
    if !is_tool_document(url, main_frame) {
        return false;
    }
    trusted_url(url).is_some_and(|url| match url.path() {
        "/tools" => matches!(
            command,
            WorkbenchCommand::FeatureList | WorkbenchCommand::FeatureInvoke { .. }
        ),
        "/ai" => matches!(command, WorkbenchCommand::AiInspectRequest { .. }),
        "/security" => matches!(command, WorkbenchCommand::SecurityAnalyze { .. }),
        "/api" => matches!(
            command,
            WorkbenchCommand::ApiSend { .. }
                | WorkbenchCommand::SecretStore { .. }
                | WorkbenchCommand::ProjectList
                | WorkbenchCommand::ProjectCreate { .. }
                | WorkbenchCommand::RequestList { .. }
                | WorkbenchCommand::RequestOpen { .. }
                | WorkbenchCommand::RequestSave { .. }
        ),
        _ => false,
    })
}

fn trusted_url(url: &str) -> Option<reqwest::Url> {
    let url = reqwest::Url::parse(url).ok()?;
    if url.scheme() != "thor" || url.host_str() != Some("workbench") {
        return None;
    }
    if !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        return None;
    }
    Some(url)
}

pub struct Asset {
    pub mime: &'static str,
    pub bytes: &'static [u8],
}

pub fn asset(url: &str) -> Option<Asset> {
    let url = trusted_url(url)?;
    let (mime, bytes): (_, &[u8]) = match url.path() {
        "/" => ("text/html", include_bytes!("home.html")),
        "/ai" => ("text/html", include_bytes!("ai.html")),
        "/ai.js" => ("text/javascript", include_bytes!("ai.js")),
        "/api" => ("text/html", include_bytes!("api.html")),
        "/security" => ("text/html", include_bytes!("security.html")),
        "/tools" => ("text/html", include_bytes!("tools.html")),
        "/tools.js" => ("text/javascript", include_bytes!("tools.js")),
        "/workspace.css" => ("text/css", include_bytes!("workspace.css")),
        "/workspace.js" => ("text/javascript", include_bytes!("workspace.js")),
        "/security.js" => ("text/javascript", include_bytes!("security.js")),
        "/workbench.css" => ("text/css", include_bytes!("workbench.css")),
        "/workbench.js" => ("text/javascript", include_bytes!("workbench.js")),
        "/api-collections.js" => ("text/javascript", include_bytes!("api-collections.js")),
        _ => return None,
    };
    Some(Asset { mime, bytes })
}
