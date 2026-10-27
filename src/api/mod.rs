//! Shared API execution for workbench, feature engines, and automation.

mod auth;
pub(crate) mod blocking;
mod model;
mod secrets;
mod service;
mod variables;

pub use auth::Authentication;
pub use model::*;
pub use secrets::{SecretError, SecretRef, SecretValue, SecretVault};
pub use service::{ApiError, HttpService};
pub use tokio_util::sync::CancellationToken;
pub use variables::Variables;
