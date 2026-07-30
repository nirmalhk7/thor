pub mod cdp_integration;
pub mod fetch_intercept;

pub use cdp_integration::{parse_web_vitals, parse_web_vitals_ex, CdpIntegration, ConsoleLogEvent};
