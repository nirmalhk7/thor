//! Shared API execution for workbench, feature engines, and automation.

mod model;
mod auth;
mod secrets;
pub(crate) mod blocking;
mod service;
mod variables;

pub use model::*;
pub use auth::Authentication;
pub use secrets::{SecretError, SecretRef, SecretValue, SecretVault};
pub use service::{ApiError, HttpService};
pub use variables::Variables;
pub use tokio_util::sync::CancellationToken;
