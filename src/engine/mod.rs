pub mod browser_defaults;
pub mod browser_features;
pub mod chromium_fork;
pub mod chromium_runtime;
pub mod compositor_sink;
pub mod containers;
pub mod engine_coordinator;
pub mod evidence_store;
pub mod fingerprinting;
pub mod freezing;
pub mod product;
pub mod profiles;
pub mod self_cdp;
pub mod tab_session;

pub use browser_defaults::{
    validate_doh_template, DevHostSplitDnsPolicy, DohHealthReadback, DohHealthState,
    DohPolicySource, DohProbeResult, DohProvider, ManagedDohPolicy, NetworkPrivacyEnvironment,
    NetworkPrivacyPolicy, PrivacySearchEngine, SecureDnsMode, SplitDnsReadback,
    ThorBrowserDefaults, WebRtcHealthReadback, WebRtcHealthState, WebRtcIpPolicy,
    WebRtcProbeResult, CLOUDFLARE_DOH_TEMPLATE, QUAD9_DOH_TEMPLATE,
};
pub use browser_features::{
    CorsMode, CorsSwitcher, DownloadItem, DownloadManager, DownloadState, FaviconService,
    FindInPage, FindOptions, FindResult, HostZoomMap, InstrumentationTier, NetworkCostMetrics,
    NetworkCostTracker, PictureInPicture, PipResult,
};
pub use chromium_fork::ChromiumFork;
pub use chromium_runtime::{
    block_on_handle, discover_chromium_path, spawn_engine_runtime, ChromiumError, ChromiumResult,
    ChromiumRuntime, ChromiumRuntimeConfig, THOR_CHROMIUM_PATH_ENV,
};
pub use engine_coordinator::{DisposableTab, EngineError, EngineHandle, IdentityContextId};
pub use freezing::{CdpPageFreezer, ChromiumLifecycleHooks, ProcessFreezer, WarmUpThread};
pub use product::{
    discover_owned_chromium_path, legacy_dual_window_requested, run_owned_chromium_browser,
};
pub use profiles::{
    DangerousApiPolicy, DownloadPolicy, JitPolicy, PrivacyControlStatus, PrivacyLimitation,
    PrivacyPolicy, PrivacyStatus, ProfileConfig, ProfileManager, ProfileMode, SecureDnsPolicy,
    SurfacePolicy, ThirdPartyCookiePolicy, WebRtcExposurePolicy,
};
pub use tab_session::{IdentityContext, TabEvalHandle, TabId, TabLifecycle, TabSession};
