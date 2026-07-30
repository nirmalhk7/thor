pub mod browser_features;
pub mod chromium_fork;
pub mod chromium_runtime;
pub mod compositor_sink;
pub mod containers;
pub mod engine_coordinator;
pub mod evidence_store;
pub mod freezing;
pub mod product;
pub mod profiles;
pub mod self_cdp;
pub mod tab_session;

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
pub use freezing::{
    CdpPageFreezer, ChromiumLifecycleHooks, ProcessFreezer, WarmUpThread,
};
pub use product::{
    discover_owned_chromium_path, legacy_dual_window_requested, run_owned_chromium_browser,
};
pub use profiles::{ProfileConfig, ProfileManager, ProfileMode};
pub use tab_session::{IdentityContext, TabEvalHandle, TabId, TabLifecycle, TabSession};
