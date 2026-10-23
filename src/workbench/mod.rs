//! Trusted tool surface and shared commands. External pages never reach this API.

mod commands;
mod persistence;

pub use commands::{Workbench, WorkbenchCommand};

pub const API_URL: &str = "thor://workbench/api";
pub const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

pub fn is_tool_document(url: &str, main_frame: bool) -> bool {
    main_frame && trusted_url(url).is_some_and(|url| url.path() == "/api")
}

fn trusted_url(url: &str) -> Option<reqwest::Url> {
    let url = reqwest::Url::parse(url).ok()?;
    if url.scheme() != "thor" || url.host_str() != Some("workbench") { return None; }
    if !url.username().is_empty() || url.password().is_some() || url.port().is_some() { return None; }
    Some(url)
}

pub struct Asset {
    pub mime: &'static str,
    pub bytes: &'static [u8],
}

pub fn asset(url: &str) -> Option<Asset> {
    let url = trusted_url(url)?;
    let (mime, bytes): (_, &[u8]) = match url.path() {
        "/api" => ("text/html", include_bytes!("api.html")),
        "/workbench.css" => ("text/css", include_bytes!("workbench.css")),
        "/workbench.js" => ("text/javascript", include_bytes!("workbench.js")),
        _ => return None,
    };
    Some(Asset { mime, bytes })
}
