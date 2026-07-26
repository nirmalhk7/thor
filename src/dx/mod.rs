pub mod bookmarks;
pub mod cmd_palette;
#[path = "3d_dom_snapshot.rs"]
pub mod _3d_dom_snapshot;
pub mod canvas;
pub mod decoder;
pub mod ide_bridge;
pub mod stack_detector;
pub mod wasm_plugin;

pub use _3d_dom_snapshot::DomSnapshot3D;
pub use bookmarks::{Bookmark, BookmarkStore};
pub use canvas::CanvasEventBus;
pub use cmd_palette::{
    build_cmd_k_items, cmd_k_payload_json, discover_installed_chrome_extensions,
    open_tabs_to_palette_items, palette_items_to_json, parse_chrome_bookmarks,
    parse_chrome_extension_manifest, read_chrome_extensions, score_palette_item,
    settings_payload_json, tab_history_entries_to_palette_items, CmdPalette, CmdPaletteMode,
    PaletteItem, PaletteItemType, SettingsRegistry, TabHistoryEntry, TabHistoryTracker,
};
pub use decoder::{DecodedJwt, DecoderStudio, PayloadType};
pub use ide_bridge::IdeBridge;
pub use stack_detector::{DetectedStack, TechBadge, TechStackDetector};
pub use wasm_plugin::{WasmPluginManifest, WasmPluginMicrokernel};



