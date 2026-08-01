#[path = "3d_dom_snapshot.rs"]
pub mod _3d_dom_snapshot;
pub mod bitwarden;
pub mod bookmarks;
pub mod canvas;
pub mod cmd_palette;
pub mod css_overlay;
pub mod decoder;
pub mod ide_bridge;
pub mod llm_clean_context;
pub mod llm_studio;
pub mod stack_detector;
pub mod test_generator;
pub mod visit_recon;
pub mod wasm_plugin;
pub mod web3_inspector;

pub use _3d_dom_snapshot::DomSnapshot3D;
pub use bitwarden::{
    build_autofill_script, vault_palette_items, BitwardenVault, VaultError, VaultItemSummary,
    VaultLogin, VaultState, VaultStatus,
};
pub use bookmarks::{Bookmark, BookmarkStore};
pub use canvas::CanvasEventBus;
pub use cmd_palette::{
    build_cmd_k_items, cmd_k_payload_json, download_items_to_palette, downloads_payload_json,
    open_tabs_to_palette_items, palette_items_to_json, parse_chrome_bookmarks,
    parse_chrome_extension_manifest, read_chrome_extensions, score_palette_item,
    settings_payload_json, tab_history_entries_to_palette_items, CmdPalette, CmdPaletteMode,
    PaletteItem, PaletteItemType, SettingsRegistry, TabHistoryEntry, TabHistoryTracker,
};
pub use decoder::{DecodedJwt, DecoderStudio, PayloadType};
pub use ide_bridge::IdeBridge;
pub use llm_clean_context::LlmCleanContext;
pub use llm_studio::{LlmCostEstimate, LlmStudio, SseStreamMetrics, SseStreamParser};
pub use stack_detector::{DetectedStack, TechBadge, TechStackDetector};
pub use test_generator::{
    DomStep, ExportFormat, LocatorCandidate, NetworkTrace, RecordedSession, TestGenerator,
};
pub use visit_recon::{VisitReconEngine, VisitReconReport};
pub use wasm_plugin::{WasmPluginManifest, WasmPluginMicrokernel};
pub use web3_inspector::{AbiSelectorMatch, Eip55Status, Web3Inspector};
