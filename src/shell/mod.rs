#![allow(dead_code)]

pub mod engine_status;
pub mod gui;
pub mod ipc_dispatch;
#[cfg(target_os = "macos")]
pub mod native_toolbar;
pub mod chromium_tabs;
pub mod chromium_shell;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use tao::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    window::WindowBuilder,
};
use wry::{dpi, PageLoadEvent, Rect, WebView, WebViewBuilder};

use crate::app_state::AppState;
use crate::dx::bookmarks::BookmarkStore;
use crate::dx::cmd_palette::{
    cmd_k_payload_json, open_tabs_to_palette_items, settings_payload_json,
};
use crate::dx::decoder::{DecoderStudio, PayloadType};
use crate::dx::stack_detector::TechStackDetector;
use crate::net::locust::LocustScoredMetrics;

/// Toolbar strip height (logical px).
pub(crate) const CHROME_HEIGHT: f64 = 52.0;

/// How often the shell sweeps idle content tabs.
pub(crate) const TAB_GC_INTERVAL: Duration = Duration::from_secs(3 * 60);
/// Close background tabs idle at least this long.
pub(crate) const TAB_IDLE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone)]
pub(crate) enum ToolbarCmd {
    Back,
    Forward,
    Reload,
    NewTab,
    SubmitOmnibox,
    OpenCmdK,
    OpenCmdShiftK,
    /// Toggle Locust inline editor in the chrome strip.
    LocustToggle,
    /// Primary action for the active omnibar tool mode (Go / Toggle / Apply…).
    ToolGo,
    /// Dismiss omnibar tool mode (✕ / Esc).
    ToolCancel,
    ToggleProfile,
}

/// Custom event-loop messages (native toolbar / window → main loop).
#[derive(Debug, Clone)]
pub(crate) enum UserEvent {
    Exit,
    Toolbar(ToolbarCmd),
    OpenCmdK,
    OpenCmdShiftK,
    ClosePalette,
    LocustToggle,
    ToggleProfile,
    /// Activate an existing content tab by id (Cmd+K open-tabs, etc.).
    SwitchTab(String),
    /// Close the currently active content tab (⌘W).
    CloseActiveTab,
    /// Open an omnibar tool mode from ⌘⇧K (`action_key`).
    OpenTool(String),
    /// Navigate the active Chromium tab (palette / IPC).
    Navigate(String),
    /// Web Vitals IPC result from content page.
    WebVitalsResult(String),
    /// Focus the native omnibox and select-all (⌘L, click omnibox).
    FocusOmnibox,
}

/// Per-tab metadata tracked in Rust (mirrors chrome `lastActive`).
pub(crate) struct TabMeta {
    pub(crate) last_active: Instant,
    pub(crate) url: String,
    pub(crate) title: String,
}

/// In-window wry content tabs keyed by chrome UI tab id.
///
/// Classic dual-webview: one chrome strip (toolbar / Cmd+K / Locust / decoder)
/// plus one content WebView per tab below `CHROME_HEIGHT`. ChromiumRuntime /
/// CEF stay in-tree for later CDP embed — shell does not launch a visible
/// Chromium window.
#[allow(dead_code)]
struct ShellTabs {
    webviews: HashMap<String, WebView>,
    meta: HashMap<String, TabMeta>,
    active: String,
}

impl ShellTabs {
    fn touch(&mut self, tab_id: &str) {
        if let Some(meta) = self.meta.get_mut(tab_id) {
            meta.last_active = Instant::now();
        }
    }

    fn set_url_title(&mut self, tab_id: &str, url: &str, title: &str) {
        if let Some(meta) = self.meta.get_mut(tab_id) {
            if !url.is_empty() {
                meta.url = url.to_string();
            }
            if !title.is_empty() {
                meta.title = title.to_string();
            }
            meta.last_active = Instant::now();
        }
    }

    fn open_tab_palette_refs(&self) -> Vec<(&str, &str, &str)> {
        let mut rows: Vec<_> = self
            .meta
            .iter()
            .map(|(id, m)| (id.as_str(), m.title.as_str(), m.url.as_str(), m.last_active))
            .collect();
        rows.sort_by(|a, b| b.3.cmp(&a.3));
        rows.into_iter()
            .map(|(id, title, url, _)| (id, title, url))
            .collect()
    }

    fn content_bounds(w: f64, h: f64, visible: bool) -> Rect {
        if visible {
            Rect {
                position: dpi::LogicalPosition::new(0.0, CHROME_HEIGHT).into(),
                size: dpi::LogicalSize::new(w, (h - CHROME_HEIGHT).max(1.0)).into(),
            }
        } else {
            Rect {
                position: dpi::LogicalPosition::new(-10000.0, -10000.0).into(),
                size: dpi::LogicalSize::new(w, (h - CHROME_HEIGHT).max(1.0)).into(),
            }
        }
    }

    fn show_only(&self, target_id: &str, w: f64, h: f64) {
        for (id, wv) in self.webviews.iter() {
            let visible = id == target_id;
            let _ = wv.set_bounds(Self::content_bounds(w, h, visible));
            let _ = wv.set_visible(visible);
            // Do NOT focus content here — that steals first-responder from the
            // chrome omnibox (child WKWebViews swallow key events when focused).
        }
    }

    fn active_url(&self) -> String {
        self.meta
            .get(&self.active)
            .map(|m| m.url.clone())
            .unwrap_or_default()
    }

    /// Prefer the live WebView URL; fall back to stored meta.
    fn resolved_url(&self, tab_id: &str) -> String {
        if let Some(wv) = self.webviews.get(tab_id) {
            if let Ok(live) = wv.url() {
                let live = live.trim().to_string();
                if !is_ephemeral_url(&live) {
                    return live;
                }
            }
        }
        self.meta
            .get(tab_id)
            .map(|m| m.url.clone())
            .unwrap_or_default()
    }

    fn focus_content(&self, target_id: &str) {
        if let Some(wv) = self.webviews.get(target_id) {
            let _ = wv.focus();
        }
    }
}

#[allow(dead_code)]
fn chrome_eval(chrome: &Option<WebView>, script: &str) {
    if let Some(wv) = chrome.as_ref() {
        let _ = wv.evaluate_script(script);
    }
}

fn log_webview_frame(label: &str, wv: &WebView) {
    match wv.bounds() {
        Ok(b) => println!("⚡ [Thor Frame] {label}: {b:?}"),
        Err(e) => println!("⚡ [Thor Frame] {label}: <err {e}>"),
    }
}

fn set_content_webviews_visible(tabs: &ShellTabs, visible: bool) {
    for wv in tabs.webviews.values() {
        let _ = wv.set_visible(visible);
    }
}

fn show_palette_overlay(tabs: &ShellTabs, chrome: &Option<WebView>, w: f64, h: f64) {
    // Hide content while the palette is up — dual always-on WKWebViews break hit-testing.
    set_content_webviews_visible(tabs, false);
    if let Some(wv) = chrome {
        let _ = wv.set_bounds(ShellTabs::content_bounds(w, h, true));
        let _ = wv.set_visible(true);
        let _ = wv.focus();
    }
}

fn hide_palette_overlay(tabs: &ShellTabs, chrome: &Option<WebView>, w: f64, h: f64) {
    if let Some(wv) = chrome {
        let _ = wv.evaluate_script(
            "if (window.__thorCloseModalSilent) window.__thorCloseModalSilent();",
        );
        let _ = wv.set_visible(false);
        let _ = wv.set_bounds(ShellTabs::content_bounds(w, h, false));
    }
    tabs.show_only(&tabs.active.clone(), w, h);
}

fn push_locust_stats(chrome: &Option<WebView>, metrics: &LocustScoredMetrics, running: bool) {
    let payload = serde_json::json!({
        "rps": metrics.rps,
        "failures": metrics.error_rate,
        "p50": (metrics.p50_latency_us as f64 / 1000.0).round() as u64,
        "p95": (metrics.p95_latency_us as f64 / 1000.0).round() as u64,
        "total": metrics.total_requests,
        "score": metrics.score,
        "status": if running { "RUNNING" } else { "STOPPED" },
        "running": running,
    });
    chrome_eval(
        chrome,
        &format!(
            "if (window.__thorUpdateLocustStats) window.__thorUpdateLocustStats({});",
            payload
        ),
    );
}

fn push_decode_result(chrome: &Option<WebView>, badge: &str, html: Option<&str>, text: Option<&str>) {
    let payload = serde_json::json!({
        "badge": badge,
        "html": html,
        "text": text,
    });
    chrome_eval(
        chrome,
        &format!(
            "if (window.__thorUpdateDecodeResult) window.__thorUpdateDecodeResult({});",
            payload
        ),
    );
}

fn sync_tab_chrome_ui(chrome: &Option<WebView>, tab_id: &str, url: &str, title: &str) {
    let id_js = serde_json::to_string(tab_id).unwrap_or_else(|_| "\"\"".into());
    let url_js = serde_json::to_string(url).unwrap_or_else(|_| "\"\"".into());
    let title_js = serde_json::to_string(title).unwrap_or_else(|_| "\"\"".into());
    chrome_eval(
        chrome,
        &format!("updateTabUrl({id_js}, {url_js}); updateTabTitle({id_js}, {title_js});"),
    );
}

pub(crate) fn sanitize_and_record(tab_id: &str, url: &str, title: &str) -> String {
    let state = AppState::global();
    let (clean, _) = state.security().sanitize_navigate_url(url);
    state
        .tab_history()
        .record_visit(tab_id, &clean, if title.is_empty() { &clean } else { title });
    clean
}

fn is_ephemeral_url(url: &str) -> bool {
    let u = url.trim();
    u.is_empty()
        || u == "about:blank"
        || u.starts_with("about:blank?")
        || u.starts_with("about:blank#")
}

fn dismiss_page_modals(tabs: &ShellTabs) {
    for wv in tabs.webviews.values() {
        let _ = wv.evaluate_script(
            "try{if(window.closeThorModal)window.closeThorModal();}catch(e){}",
        );
    }
}

#[cfg(target_os = "macos")]
fn sync_omnibox_to_tab(
    tabs: &mut ShellTabs,
    toolbar: &Option<native_toolbar::NativeToolbar>,
    tab_id: &str,
) {
    let url = tabs.resolved_url(tab_id);
    if !is_ephemeral_url(&url) {
        tabs.set_url_title(tab_id, &url, "");
    }
    if let Some(tb) = toolbar.as_ref() {
        if !is_ephemeral_url(&url) {
            tb.set_url(&url);
        }
        tb.raise();
    }
}

fn apply_committed_url(
    tabs: &Rc<RefCell<ShellTabs>>,
    chrome: &Rc<RefCell<Option<WebView>>>,
    #[cfg(target_os = "macos")] toolbar: &Rc<RefCell<Option<native_toolbar::NativeToolbar>>>,
    tab_id: &str,
    url: &str,
) {
    if is_ephemeral_url(url) {
        return;
    }
    let clean = sanitize_and_record(tab_id, url, "");
    let is_active = {
        let mut tabs = tabs.borrow_mut();
        tabs.set_url_title(tab_id, &clean, "");
        tabs.active == tab_id
    };
    if let Some(ref wv) = *chrome.borrow() {
        let url_js = serde_json::to_string(&clean).unwrap_or_default();
        let id_js = serde_json::to_string(tab_id).unwrap_or_default();
        let _ = wv.evaluate_script(&format!("updateTabUrl({id_js}, {url_js});"));
    }
    #[cfg(target_os = "macos")]
    if is_active {
        if let Some(tb) = toolbar.borrow().as_ref() {
            tb.set_url(&clean);
        }
    }
}

pub(crate) fn content_hotkey_script() -> &'static str {
    r#"
        (function() {
            if (window.__thorCmdPaletteInstalled) return;
            window.__thorCmdPaletteInstalled = true;

            function sendIpc(msg) {
                try {
                    window.__thorIpcQueue = window.__thorIpcQueue || [];
                    window.__thorIpcQueue.push(String(msg));
                } catch (e) {}
            }

            var thorModalMode = 'nav';
            var thorItems = [];
            var thorSelected = 0;

            function defaultNavItems() {
                return [
                    { icon: '🐙', title: 'GitHub', url: 'https://github.com', badge: 'bookmark' },
                    { icon: '🔍', title: 'Google', url: 'https://www.google.com', badge: 'bookmark' },
                    { icon: '📚', title: 'MDN Web Docs', url: 'https://developer.mozilla.org', badge: 'bookmark' },
                    { icon: '💬', title: 'Stack Overflow', url: 'https://stackoverflow.com', badge: 'bookmark' },
                    { icon: '⚙️', title: 'Localhost:3000', url: 'http://localhost:3000', badge: 'localhost' },
                    { icon: '⚡', title: 'Vite :5173', url: 'http://localhost:5173', badge: 'localhost' }
                ];
            }

            function defaultSettingsItems() {
                return [
                    { icon: '🛠️', title: 'Toggle Developer Tools', action_key: 'toggle_devtools', badge: 'F12' },
                    { icon: 'ℹ️', title: 'Engine status', action_key: 'engine_status', badge: 'chromium' },
                    { icon: '👤', title: 'Toggle profile', action_key: 'toggle_profile', badge: '⌘⇧P' },
                    { icon: '📊', title: 'Load test (Locust)', action_key: 'locust_active_origin', badge: '⌘⇧L' },
                    { icon: '💻', title: 'Open in VS Code', action_key: 'open_ide_vscode', badge: 'path' },
                    { icon: '⚡', title: 'Open in Cursor', action_key: 'open_ide_cursor', badge: 'path' },
                    { icon: '🚀', title: 'Open in WebStorm', action_key: 'open_ide_webstorm', badge: 'path' },
                    { icon: '🛡️', title: 'SSL CA install', action_key: 'show_ssl_details', badge: 'CA' },
                    { icon: '🛡️', title: 'Tracker shield', action_key: 'toggle_security_shield', badge: 'strip' },
                    { icon: '🗑️', title: 'Clear history', action_key: 'clear_history', badge: 'privacy' },
                    { icon: '❄️', title: 'Compact idle tabs', action_key: 'trigger_memory_compaction', badge: 'GC' },
                    { icon: '📊', title: 'Web Vitals', action_key: 'show_web_vitals', badge: 'CWV' },
                    { icon: '📈', title: 'SLA readout', action_key: 'show_sla_metrics', badge: 'M2' }
                ];
            }

            function mapRustItem(item) {
                return {
                    icon: item.icon || '⚡',
                    title: item.title || item.path_or_url || 'Item',
                    url: item.path_or_url || item.url || '',
                    badge: item.badge || item.category || '',
                    action_key: item.action_key || null,
                    id: item.id || null
                };
            }

            function poolForMode() {
                if (thorModalMode === 'settings') {
                    if (Array.isArray(window.__thorSettingsPayload) && window.__thorSettingsPayload.length) {
                        return window.__thorSettingsPayload.map(mapRustItem);
                    }
                    return defaultSettingsItems();
                }
                if (Array.isArray(window.__thorCmdKPayload) && window.__thorCmdKPayload.length) {
                    return window.__thorCmdKPayload.map(mapRustItem);
                }
                return defaultNavItems();
            }

            window.__thorEnsureCmdPalette = function() {
                if (document.getElementById('thorModalOverlay')) return;
                var style = document.createElement('style');
                style.textContent = [
                    '#thorModalOverlay{position:fixed;inset:0;background:rgba(0,0,0,.78);z-index:2147483646;display:none;align-items:flex-start;justify-content:center;padding-top:48px;font-family:-apple-system,BlinkMacSystemFont,sans-serif;isolation:isolate;}',
                    '#thorModalOverlay.active{display:flex !important;}',
                    '#thorModalOverlay .box{width:650px;max-width:92vw;background:#161820;border:1px solid rgba(255,255,255,.18);border-radius:10px;box-shadow:0 16px 48px rgba(0,0,0,.85);overflow:hidden;color:#f1f5f9;}',
                    '#thorModalOverlay .search{padding:14px;background:rgba(0,0,0,.3);border-bottom:1px solid rgba(255,255,255,.1);display:flex;align-items:center;gap:10px;}',
                    '#thorModalOverlay input{flex:1;background:transparent;border:none;outline:none;color:#fff;font-size:14px;}',
                    '#thorModalOverlay .list{max-height:340px;overflow-y:auto;padding:8px;display:flex;flex-direction:column;gap:4px;}',
                    '#thorModalOverlay .item{padding:10px 12px;border-radius:6px;display:flex;align-items:center;justify-content:space-between;cursor:pointer;}',
                    '#thorModalOverlay .item:hover,#thorModalOverlay .item.selected{background:rgba(255,255,255,.1);}',
                    '#thorModalOverlay .title{font-weight:500;font-size:13px;}',
                    '#thorModalOverlay .sub{font-size:11px;color:#94a3b8;margin-top:2px;}',
                    '#thorModalOverlay .badge{font-size:10px;padding:2px 6px;border-radius:4px;background:rgba(255,255,255,.1);color:#60a5fa;font-family:ui-monospace,monospace;}'
                ].join('');
                (document.head || document.documentElement).appendChild(style);

                var overlay = document.createElement('div');
                overlay.id = 'thorModalOverlay';
                overlay.innerHTML = '<div class="box"><div class="search"><span style="color:#60a5fa">⌕</span><input id="thorModalInput" placeholder="Search bookmarks, history, tabs, settings…" autocomplete="off" spellcheck="false"><span style="font-size:10px;color:#64748b;background:rgba(255,255,255,.08);padding:2px 6px;border-radius:4px;">ESC</span></div><div class="list" id="thorModalList"></div></div>';
                overlay.addEventListener('mousedown', function(e) {
                    if (e.target === overlay) { e.preventDefault(); closeThorModal(); }
                });
                (document.body || document.documentElement).appendChild(overlay);

                var input = document.getElementById('thorModalInput');
                input.addEventListener('input', function() { thorSelected = 0; renderThorItems(); });
                input.addEventListener('keydown', function(e) {
                    if (e.key === 'Escape') { e.preventDefault(); closeThorModal(); return; }
                    if (e.key === 'ArrowDown') { e.preventDefault(); thorSelected = Math.min(thorSelected + 1, Math.max(thorItems.length - 1, 0)); renderThorItems(); return; }
                    if (e.key === 'ArrowUp') { e.preventDefault(); thorSelected = Math.max(thorSelected - 1, 0); renderThorItems(); return; }
                    if (e.key === 'Enter') {
                        e.preventDefault();
                        if (thorItems[thorSelected]) { selectThorItem(thorItems[thorSelected]); return; }
                        var q = input.value.trim();
                        if (!q) return;
                        var url = (q.indexOf('.') >= 0 && q.indexOf(' ') < 0) ? ('https://' + q) : ('https://www.google.com/search?q=' + encodeURIComponent(q));
                        sendIpc(JSON.stringify({ cmd: 'navigate', url: url }));
                        closeThorModal();
                    }
                });
            };

            function renderThorItems() {
                window.__thorEnsureCmdPalette();
                var list = document.getElementById('thorModalList');
                var input = document.getElementById('thorModalInput');
                if (!list || !input) return;
                var q = (input.value || '').toLowerCase().trim();
                thorItems = poolForMode().filter(function(it) {
                    if (!q) return true;
                    return (it.title || '').toLowerCase().indexOf(q) >= 0
                        || (it.url || '').toLowerCase().indexOf(q) >= 0
                        || (it.badge || '').toLowerCase().indexOf(q) >= 0;
                });
                if (thorSelected >= thorItems.length) thorSelected = Math.max(thorItems.length - 1, 0);
                list.innerHTML = '';
                if (!thorItems.length) {
                    list.innerHTML = '<div style="padding:16px;text-align:center;color:#64748b;font-size:12px;">No matches — press Enter to search the web</div>';
                    return;
                }
                thorItems.forEach(function(it, idx) {
                    var el = document.createElement('div');
                    el.className = 'item' + (idx === thorSelected ? ' selected' : '');
                    el.innerHTML = '<div><div class="title">' + (it.icon || '⚡') + ' ' + (it.title || '') + '</div><div class="sub">' + (it.url || it.action_key || '') + '</div></div><div class="badge">' + (it.badge || '') + '</div>';
                    el.addEventListener('mousedown', function(e) { e.preventDefault(); selectThorItem(it); });
                    list.appendChild(el);
                });
            }

            function selectThorItem(it) {
                if (!it) return;
                if (it.action_key && String(it.action_key).indexOf('focus_tab_') === 0) {
                    sendIpc(JSON.stringify({ cmd: 'switch_tab', id: String(it.action_key).slice(10) }));
                    closeThorModal();
                    return;
                }
                if (it.action_key) {
                    sendIpc(JSON.stringify({ cmd: 'open_tool', action_key: String(it.action_key) }));
                    closeThorModal();
                    return;
                }
                if (it.url && String(it.url).indexOf('Thor://') !== 0 && String(it.url).indexOf('thor://') !== 0) {
                    sendIpc(JSON.stringify({ cmd: 'navigate', url: it.url }));
                }
                closeThorModal();
            }

            window.openThorModal = function(type) {
                thorModalMode = (type === 'settings' || type === 'tools') ? 'settings' : 'nav';
                window.__thorEnsureCmdPalette();
                var overlay = document.getElementById('thorModalOverlay');
                var input = document.getElementById('thorModalInput');
                if (!overlay) return;
                overlay.classList.add('active');
                thorSelected = 0;
                if (input) {
                    input.value = '';
                    input.placeholder = thorModalMode === 'settings'
                        ? 'Settings, security, extensions, DevTools…'
                        : 'Bookmarks, tab history, open tabs…';
                }
                renderThorItems();
                setTimeout(function() { if (input) input.focus(); }, 20);
            };

            window.closeThorModal = function() {
                var overlay = document.getElementById('thorModalOverlay');
                if (overlay) overlay.classList.remove('active');
            };

            function handleKey(e) {
                var isCmdOrCtrl = e.metaKey || e.ctrlKey;
                if (!isCmdOrCtrl) {
                    if (e.key === 'Escape') closeThorModal();
                    return;
                }
                var key = e.key ? e.key.toLowerCase() : '';
                var code = e.code ? e.code.toLowerCase() : '';

                if (e.shiftKey && (key === 'k' || code === 'keyk')) {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'hotkey_cmd_shift_k' }));
                } else if (!e.shiftKey && (key === 'k' || code === 'keyk')) {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'hotkey_cmd_k' }));
                } else if (e.shiftKey && (key === 'l' || code === 'keyl')) {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'hotkey_locust_toggle' }));
                } else if (key === 't') {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'hotkey_new_tab' }));
                } else if (key === 'w') {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'hotkey_close_tab' }));
                } else if (key === 'l') {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'hotkey_focus_url' }));
                } else if (key === 'r') {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'hotkey_reload' }));
                } else if (key === '[' || code === 'bracketleft') {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'back' }));
                } else if (key === ']' || code === 'bracketright') {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'forward' }));
                }
            }

            window.addEventListener('keydown', handleKey, true);
            document.addEventListener('keydown', handleKey, true);

            // Mobile-style pull-to-reload: swipe/scroll down while already at top.
            (function installPullToReload() {
                var pull = 0;
                var threshold = 90;
                var cooldownUntil = 0;
                var touchStartY = 0;
                var touching = false;

                function atTop() {
                    var y = window.scrollY || document.documentElement.scrollTop || document.body.scrollTop || 0;
                    return y <= 0;
                }

                function maybeReload() {
                    var now = Date.now();
                    if (now < cooldownUntil) return;
                    cooldownUntil = now + 1200;
                    pull = 0;
                    sendIpc(JSON.stringify({ cmd: 'hotkey_reload' }));
                }

                window.addEventListener('wheel', function(e) {
                    if (!atTop()) { pull = 0; return; }
                    if (e.deltaY < 0) {
                        pull += -e.deltaY;
                        if (pull >= threshold) maybeReload();
                    } else {
                        pull = Math.max(0, pull - e.deltaY * 0.5);
                    }
                }, { passive: true });

                window.addEventListener('touchstart', function(e) {
                    if (!e.touches || !e.touches.length) return;
                    touching = atTop();
                    touchStartY = e.touches[0].clientY;
                    pull = 0;
                }, { passive: true });

                window.addEventListener('touchmove', function(e) {
                    if (!touching || !e.touches || !e.touches.length) return;
                    if (!atTop()) { touching = false; pull = 0; return; }
                    var dy = e.touches[0].clientY - touchStartY;
                    if (dy > 0) {
                        pull = dy;
                        if (pull >= threshold) {
                            touching = false;
                            maybeReload();
                        }
                    }
                }, { passive: true });

                window.addEventListener('touchend', function() {
                    touching = false;
                    pull = 0;
                }, { passive: true });
            })();

            if (document.readyState === 'loading') {
                document.addEventListener('DOMContentLoaded', function() { window.__thorEnsureCmdPalette(); });
            } else {
                window.__thorEnsureCmdPalette();
            }
        })();
    "#
}

fn create_content_tab(
    window: &tao::window::Window,
    url: &str,
    tab_id: String,
    chrome_ref: Rc<RefCell<Option<WebView>>>,
    tabs_meta: Rc<RefCell<ShellTabs>>,
    proxy: EventLoopProxy<UserEvent>,
    #[cfg(target_os = "macos")] toolbar: Rc<RefCell<Option<native_toolbar::NativeToolbar>>>,
    w: f64,
    h: f64,
    visible: bool,
) -> anyhow::Result<WebView> {
    let chrome_for_hotkeys = chrome_ref.clone();
    let chrome_for_title = chrome_ref.clone();
    let chrome_for_nav = chrome_ref.clone();
    let chrome_for_load = chrome_ref.clone();
    #[cfg(target_os = "macos")]
    let toolbar_for_nav = toolbar.clone();
    #[cfg(target_os = "macos")]
    let toolbar_for_ipc = toolbar.clone();
    #[cfg(target_os = "macos")]
    let toolbar_for_load = toolbar.clone();
    let tabs_for_title = tabs_meta.clone();
    let tabs_for_nav = tabs_meta.clone();
    let tabs_for_load = tabs_meta.clone();
    let tabs_for_ipc = tabs_meta.clone();
    let id_for_title = tab_id.clone();
    let id_for_nav = tab_id.clone();
    let id_for_load = tab_id.clone();
    let id_for_ipc = tab_id.clone();
    let proxy_for_ipc = proxy.clone();

    let clean = sanitize_and_record(&tab_id, url, "");

    let wv = WebViewBuilder::new()
        .with_url(&clean)
        .with_user_agent(crate::net::security_shield::CHROMIUM_USER_AGENT)
        .with_back_forward_navigation_gestures(true)
        .with_bounds(ShellTabs::content_bounds(w, h, visible))
        .with_focused(false)
        .with_devtools(true)
        .with_initialization_script(content_hotkey_script())
        .with_ipc_handler(move |request: http::Request<String>| {
            let msg = request.body();
            println!("⚡ [Thor Tab IPC Rx] {msg}");
            let payload: serde_json::Value = serde_json::from_str(msg).unwrap_or_default();
            let Some(cmd) = payload["cmd"].as_str() else {
                return;
            };
            match cmd {
                "log" => {
                    if let Some(log_msg) = payload["message"].as_str() {
                        println!("⚡ [Thor Tab Log] {log_msg}");
                    }
                }
                "hotkey_cmd_k" | "open_cmd_k" => {
                    let _ = proxy_for_ipc.send_event(UserEvent::OpenCmdK);
                }
                "hotkey_cmd_shift_k" | "open_cmd_shift_k" => {
                    let _ = proxy_for_ipc.send_event(UserEvent::OpenCmdShiftK);
                }
                "hotkey_locust_toggle" | "locust_toggle_active" => {
                    let _ = proxy_for_ipc.send_event(UserEvent::LocustToggle);
                }
                "hotkey_toggle_profile" | "toggle_profile" => {
                    let _ = proxy_for_ipc.send_event(UserEvent::ToggleProfile);
                }
                "hotkey_new_tab" => {
                    let _ = proxy_for_ipc.send_event(UserEvent::Toolbar(ToolbarCmd::NewTab));
                }
                "hotkey_close_tab" => {
                    let _ = proxy_for_ipc.send_event(UserEvent::CloseActiveTab);
                }
                // Privileged lab cmds are NOT accepted from content pages (SEC-091).
                "locust_start" | "locust_stop" | "set_interception_enabled" | "add_tamper_rule"
                | "install_ca" | "decode_jwt_crack" => {
                    println!(
                        "⚡ [Thor] rejected privileged IPC from content: {cmd} (use toolbar/⌘⇧K)"
                    );
                }
                "hotkey_focus_url" => {
                    #[cfg(target_os = "macos")]
                    if let Some(tb) = toolbar_for_ipc.borrow().as_ref() {
                        tb.focus_omnibox();
                    }
                }
                "hotkey_reload" | "reload" => {
                    let tabs = tabs_for_ipc.borrow();
                    // Only reload if this IPC originates from the active tab
                    // (or the page itself — id_for_ipc is that page's tab).
                    if tabs.active == id_for_ipc {
                        if let Some(wv) = tabs.webviews.get(&id_for_ipc) {
                            let _ = wv.evaluate_script("location.reload()");
                        }
                    }
                }
                "navigate" => {
                    if let Some(url) = payload["url"].as_str() {
                        let mut tabs = tabs_for_ipc.borrow_mut();
                        let clean = sanitize_and_record(&id_for_ipc, url, "");
                        println!("⚡ [Thor] content navigate [{id_for_ipc}] → {clean}");
                        if let Some(wv) = tabs.webviews.get(&id_for_ipc) {
                            let _ = wv.load_url(&clean);
                            tabs.set_url_title(&id_for_ipc, &clean, "");
                        }
                        let is_active = tabs.active == id_for_ipc;
                        drop(tabs);
                        #[cfg(target_os = "macos")]
                        if is_active {
                            if let Some(tb) = toolbar_for_ipc.borrow().as_ref() {
                                tb.set_url(&clean);
                            }
                        }
                    }
                }
                "switch_tab" => {
                    if let Some(target) = payload["id"].as_str() {
                        let _ = proxy_for_ipc
                            .send_event(UserEvent::SwitchTab(target.to_string()));
                    }
                }
                "open_tool" => {
                    if let Some(key) = payload["action_key"].as_str() {
                        let _ = proxy_for_ipc.send_event(UserEvent::OpenTool(key.to_string()));
                    }
                }
                "web_vitals_result" => {
                    let summary = payload["summary"]
                        .as_str()
                        .unwrap_or("no vitals")
                        .to_string();
                    let _ = proxy_for_ipc.send_event(UserEvent::WebVitalsResult(summary));
                }
                "open_ide" | "add_bookmark" | "remove_bookmark" => {
                    // Prefer Rust gate. Allowlisted carefully.
                    let result = ipc_dispatch::dispatch_privileged(cmd, &payload);
                    if let ipc_dispatch::DispatchResult::Err(e) = result {
                        eprintln!("⚡ [Thor IPC] {cmd} error: {e}");
                    }
                }
                "back" => {
                    let tabs = tabs_for_ipc.borrow();
                    if let Some(wv) = tabs.webviews.get(&id_for_ipc) {
                        let _ = wv.evaluate_script("history.back()");
                    }
                }
                "forward" => {
                    let tabs = tabs_for_ipc.borrow();
                    if let Some(wv) = tabs.webviews.get(&id_for_ipc) {
                        let _ = wv.evaluate_script("history.forward()");
                    }
                }
                "toggle_devtools" => {
                    let tabs = tabs_for_ipc.borrow();
                    if let Some(wv) = tabs.webviews.get(&id_for_ipc) {
                        #[cfg(any(debug_assertions, feature = "devtools"))]
                        {
                            if wv.is_devtools_open() {
                                wv.close_devtools();
                            } else {
                                wv.open_devtools();
                            }
                        }
                    }
                }
                _ => {}
            }
            let _ = &chrome_for_hotkeys;
        })
        .with_document_title_changed_handler(move |title: String| {
            {
                let mut tabs = tabs_for_title.borrow_mut();
                tabs.set_url_title(&id_for_title, "", &title);
                if let Some(meta) = tabs.meta.get(&id_for_title) {
                    let url = meta.url.clone();
                    drop(tabs);
                    AppState::global()
                        .tab_history()
                        .record_visit(&id_for_title, &url, &title);
                }
            }
            if let Some(ref wv) = *chrome_for_title.borrow() {
                let title_js = serde_json::to_string(&title).unwrap_or_default();
                let id_js = serde_json::to_string(&id_for_title).unwrap_or_default();
                let _ = wv.evaluate_script(&format!("updateTabTitle({id_js}, {title_js});"));
            }
        })
        .with_navigation_handler(move |url: String| {
            // Allow navigation, but only commit chrome/meta for real URLs.
            // Hiding/showing WKWebViews can emit about:blank and must not clobber the omnibox.
            if is_ephemeral_url(&url) {
                return true;
            }
            apply_committed_url(
                &tabs_for_nav,
                &chrome_for_nav,
                #[cfg(target_os = "macos")]
                &toolbar_for_nav,
                &id_for_nav,
                &url,
            );
            true
        })
        .with_on_page_load_handler(move |event, url| {
            if !matches!(event, PageLoadEvent::Finished) {
                return;
            }
            apply_committed_url(
                &tabs_for_load,
                &chrome_for_load,
                #[cfg(target_os = "macos")]
                &toolbar_for_load,
                &id_for_load,
                &url,
            );
        })
        .with_new_window_req_handler(|_url: String| false)
        .build_as_child(window)?;

    let _ = wv.set_visible(visible);
    Ok(wv)
}

fn create_palette_webview(
    window: &tao::window::Window,
    tabs: Rc<RefCell<ShellTabs>>,
    proxy: EventLoopProxy<UserEvent>,
    #[cfg(target_os = "macos")] toolbar: Rc<RefCell<Option<native_toolbar::NativeToolbar>>>,
    w: f64,
    h: f64,
) -> anyhow::Result<WebView> {
    let tabs_for_ipc = tabs.clone();
    let proxy_for_ipc = proxy.clone();
    #[cfg(target_os = "macos")]
    let toolbar_for_ipc = toolbar.clone();

    let init = r#"
        document.addEventListener('DOMContentLoaded', function () {
          var row = document.getElementById('chromeRow');
          if (row) row.style.display = 'none';
          document.documentElement.style.background = 'transparent';
          if (document.body) document.body.style.background = 'transparent';
          ['stackDetectorBar','locustBar','interceptorBar','decoderBar'].forEach(function (id) {
            var el = document.getElementById(id);
            if (el) el.style.display = 'none';
          });
        });
    "#;

    let wv = WebViewBuilder::new()
        .with_html(gui::get_chrome_html())
        .with_transparent(true)
        .with_bounds(ShellTabs::content_bounds(w, h, false))
        .with_focused(false)
        .with_initialization_script(init)
        .with_ipc_handler(move |request: http::Request<String>| {
            let msg = request.body();
            println!("⚡ [Thor Palette IPC Rx] {msg}");
            let payload: serde_json::Value = serde_json::from_str(msg).unwrap_or_default();
            let Some(cmd) = payload["cmd"].as_str() else {
                return;
            };
            match cmd {
                "close_palette" => {
                    let _ = proxy_for_ipc.send_event(UserEvent::ClosePalette);
                }
                "navigate" => {
                    if let Some(url) = payload["url"].as_str() {
                        let mut tabs_slot = tabs_for_ipc.borrow_mut();
                        let cur = tabs_slot.active.clone();
                        let clean = sanitize_and_record(&cur, url, "");
                        println!("⚡ [Thor] palette navigate [{cur}] → {clean}");
                        if let Some(wv) = tabs_slot.webviews.get(&cur) {
                            let _ = wv.load_url(&clean);
                            tabs_slot.set_url_title(&cur, &clean, "");
                        }
                        drop(tabs_slot);
                        #[cfg(target_os = "macos")]
                        if let Some(tb) = toolbar_for_ipc.borrow().as_ref() {
                            tb.set_url(&clean);
                        }
                    }
                    let _ = proxy_for_ipc.send_event(UserEvent::ClosePalette);
                }
                "switch_tab" => {
                    if let Some(id) = payload["id"].as_str() {
                        let _ = proxy_for_ipc.send_event(UserEvent::SwitchTab(id.to_string()));
                    }
                    let _ = proxy_for_ipc.send_event(UserEvent::ClosePalette);
                }
                "toggle_devtools" => {
                    let tabs_slot = tabs_for_ipc.borrow();
                    if let Some(wv) = tabs_slot.webviews.get(&tabs_slot.active) {
                        #[cfg(any(debug_assertions, feature = "devtools"))]
                        {
                            if wv.is_devtools_open() {
                                wv.close_devtools();
                            } else {
                                wv.open_devtools();
                            }
                        }
                    }
                    let _ = proxy_for_ipc.send_event(UserEvent::ClosePalette);
                }
                "open_ide" => {
                    let ide_name = payload["ide"].as_str().unwrap_or("vscode");
                    let path = payload["path"].as_str().unwrap_or(".");
                    let line = payload["line"].as_u64().map(|l| l as usize);
                    let col = payload["col"].as_u64().map(|c| c as usize);
                    let ide = match ide_name {
                        "cursor" => crate::dx::ide_bridge::Ide::Cursor,
                        "webstorm" => crate::dx::ide_bridge::Ide::WebStorm,
                        _ => crate::dx::ide_bridge::Ide::VSCode,
                    };
                    let _ = crate::dx::ide_bridge::IdeBridge::open_file(ide, path, line, col);
                    let _ = proxy_for_ipc.send_event(UserEvent::ClosePalette);
                }
                "log" => {
                    if let Some(m) = payload["message"].as_str() {
                        println!("⚡ [Thor Palette Log] {m}");
                    }
                }
                _ => {}
            }
        })
        .build_as_child(window)?;

    let _ = wv.set_visible(false);
    Ok(wv)
}

fn close_tab_session(tabs: &mut ShellTabs, tab_id: &str, w: f64, h: f64) {
    AppState::global().tab_history().clear_tab_history(tab_id);
    tabs.meta.remove(tab_id);
    tabs.webviews.remove(tab_id);
    if tabs.active == tab_id {
        // Prefer the most recently used remaining tab.
        let next = tabs
            .meta
            .iter()
            .max_by_key(|(_, m)| m.last_active)
            .map(|(id, _)| id.clone())
            .or_else(|| tabs.webviews.keys().next().cloned())
            .unwrap_or_default();
        tabs.active = next;
        if !tabs.active.is_empty() {
            let active = tabs.active.clone();
            tabs.show_only(&active, w, h);
            tabs.touch(&active);
        }
    }
}

fn switch_tab_session(tabs: &mut ShellTabs, target_id: &str, w: f64, h: f64) -> Option<String> {
    if !tabs.webviews.contains_key(target_id) {
        return None;
    }
    dismiss_page_modals(tabs);
    tabs.active = target_id.to_string();
    tabs.show_only(target_id, w, h);
    tabs.touch(target_id);
    let url = tabs.resolved_url(target_id);
    if !is_ephemeral_url(&url) {
        tabs.set_url_title(target_id, &url, "");
    }
    Some(url)
}

fn gc_idle_tabs(tabs: &mut ShellTabs, chrome: &Option<WebView>, w: f64, h: f64) {
    let now = Instant::now();
    let victims: Vec<String> = tabs
        .meta
        .iter()
        .filter(|(id, meta)| {
            tabs.webviews.len() > 1
                && now.duration_since(meta.last_active) >= TAB_IDLE_TTL
                && (*id != &tabs.active || tabs.webviews.len() > 1)
        })
        .map(|(id, _)| id.clone())
        .collect();

    for id in victims {
        if tabs.webviews.len() <= 1 {
            break;
        }
        println!("⚡ [Thor GC] Closing idle tab (≥24h): {id}");
        close_tab_session(tabs, &id, w, h);
        let id_js = serde_json::to_string(&id).unwrap_or_else(|_| "\"\"".into());
        chrome_eval(
            chrome,
            &format!("if (window.__thorRemoveTab) window.__thorRemoveTab({id_js});"),
        );
    }
}

fn open_cmd_k_in_active_tab(tabs: &ShellTabs) {
    let Some(wv) = tabs.webviews.get(&tabs.active) else {
        eprintln!("⚡ [Thor] ⌘K: no active content tab");
        return;
    };
    let bookmarks = BookmarkStore::open()
        .map(|s| s.to_palette_items())
        .unwrap_or_default();
    let open_refs = tabs.open_tab_palette_refs();
    let open_items = open_tabs_to_palette_items(&open_refs);
    let history = AppState::global()
        .tab_history()
        .palette_items_for_tab(&tabs.active);
    let payload_js = match cmd_k_payload_json(&bookmarks, &history, &open_items) {
        Ok(json) => format!("window.__thorCmdKPayload = {json};"),
        Err(_) => "window.__thorCmdKPayload = [];".to_string(),
    };
    let status_js = engine_status::engine_status_script();
    let hotkey_script = content_hotkey_script();
    let script = format!(
        r#"(function(){{
          {hotkey_script}
          {payload_js}
          {status_js}
          if (window.__thorEnsureCmdPalette) window.__thorEnsureCmdPalette();
          if (window.openThorModal) window.openThorModal('bookmarks_history');
          else console.warn('[Thor] openThorModal missing');
        }})();"#
    );
    println!("⚡ [Thor] Opening ⌘K palette in content tab {}", tabs.active);
    let _ = wv.evaluate_script(&script);
    let _ = wv.focus();
}

fn open_cmd_shift_k_in_active_tab(tabs: &ShellTabs) {
    let Some(wv) = tabs.webviews.get(&tabs.active) else {
        eprintln!("⚡ [Thor] ⌘⇧K: no active content tab");
        return;
    };
    let payload_js = match settings_payload_json() {
        Ok(json) => format!("window.__thorSettingsPayload = {json};"),
        Err(_) => "window.__thorSettingsPayload = [];".to_string(),
    };
    let status_js = engine_status::engine_status_script();
    let hotkey_script = content_hotkey_script();
    let script = format!(
        r#"(function(){{
          {hotkey_script}
          {payload_js}
          {status_js}
          if (window.__thorEnsureCmdPalette) window.__thorEnsureCmdPalette();
          if (window.openThorModal) window.openThorModal('settings');
          else console.warn('[Thor] openThorModal missing');
        }})();"#
    );
    println!("⚡ [Thor] Opening ⌘⇧K palette in content tab {}", tabs.active);
    let _ = wv.evaluate_script(&script);
    let _ = wv.focus();
}

fn start_locust_from_bar(tabs: &ShellTabs, params: &native_toolbar::LocustBarParams) -> bool {
    let url = tabs
        .meta
        .get(&tabs.active)
        .map(|m| m.url.clone())
        .unwrap_or_default();
    if url.is_empty() {
        eprintln!("⚡ [Thor Locust] no active URL");
        return false;
    }
    let payload = serde_json::json!({
        "url": url,
        "users": params.users,
        "ramp": params.ramp_up_secs,
        "runtime": params.runtime_secs,
        "profile": params.profile,
        "public_ack": false,
    });
    println!(
        "⚡ [Thor Locust] start users={} ramp={}s runtime={}s profile={} url={}",
        params.users, params.ramp_up_secs, params.runtime_secs, params.profile, url
    );
    match ipc_dispatch::dispatch_privileged("locust_start", &payload) {
        ipc_dispatch::DispatchResult::Message(m) => {
            println!("⚡ [Thor Locust] {m}");
            true
        }
        ipc_dispatch::DispatchResult::Err(e) => {
            eprintln!("⚡ [Thor Locust] {e}");
            false
        }
        _ => false,
    }
}

fn locust_toggle_active_tab(tabs: &ShellTabs) {
    // If already running, stop. Otherwise shell opens the editor (handled by caller).
    if AppState::global().locust_is_running() {
        match ipc_dispatch::dispatch_privileged("locust_stop", &serde_json::json!({})) {
            ipc_dispatch::DispatchResult::LocustMetrics(m) => {
                println!(
                    "⚡ [Thor Locust] stopped — reqs={} rps={:.1} score={:.0}",
                    m.total_requests, m.rps, m.score
                );
            }
            ipc_dispatch::DispatchResult::Message(m) => println!("⚡ [Thor Locust] {m}"),
            ipc_dispatch::DispatchResult::Err(e) => eprintln!("⚡ [Thor Locust] {e}"),
            _ => {}
        }
        let _ = tabs;
    }
}

pub(crate) fn toggle_profile_mode() {
    match ipc_dispatch::dispatch_privileged("toggle_profile", &serde_json::json!({})) {
        ipc_dispatch::DispatchResult::Message(m) => println!("⚡ [Thor Profile] {m}"),
        ipc_dispatch::DispatchResult::Err(e) => eprintln!("⚡ [Thor Profile] {e}"),
        _ => {}
    }
}

#[cfg(target_os = "macos")]
fn open_tool_in_omnibar(
    tabs: &ShellTabs,
    toolbar: &mut Option<native_toolbar::NativeToolbar>,
    action_key: &str,
) {
    let Some(mode) = native_toolbar::ChromeToolMode::from_action_key(action_key) else {
        eprintln!("⚡ [Thor] unknown tool action_key: {action_key}");
        return;
    };
    // DevTools: toggle immediately — no intermediate omnibar confirm.
    if mode == native_toolbar::ChromeToolMode::DevTools {
        if let Some(wv) = tabs.webviews.get(&tabs.active) {
            #[cfg(any(debug_assertions, feature = "devtools"))]
            {
                if wv.is_devtools_open() {
                    wv.close_devtools();
                    println!("⚡ [Thor] DevTools closed");
                } else {
                    wv.open_devtools();
                    println!("⚡ [Thor] DevTools opened");
                }
            }
        }
        if let Some(tb) = toolbar.as_mut() {
            if !tb.tool_mode().is_browse() {
                tb.set_tool_mode(native_toolbar::ChromeToolMode::Browse);
            }
        }
        return;
    }
    // Locust shortcut: if swarm already running, stop; if editor open, close.
    if mode == native_toolbar::ChromeToolMode::Locust {
        if AppState::global().locust_is_running() {
            locust_toggle_active_tab(tabs);
            if let Some(tb) = toolbar.as_mut() {
                tb.set_tool_mode(native_toolbar::ChromeToolMode::Browse);
                tb.focus_omnibox();
            }
            return;
        }
        if let Some(tb) = toolbar.as_ref() {
            if tb.tool_mode() == native_toolbar::ChromeToolMode::Locust {
                if let Some(tb) = toolbar.as_mut() {
                    tb.set_tool_mode(native_toolbar::ChromeToolMode::Browse);
                    tb.focus_omnibox();
                }
                return;
            }
        }
    }
    if let Some(tb) = toolbar.as_mut() {
        // Refresh status labels that depend on live state.
        match mode {
            native_toolbar::ChromeToolMode::DevTools => {
                let open = tabs
                    .webviews
                    .get(&tabs.active)
                    .map(|wv| {
                        #[cfg(any(debug_assertions, feature = "devtools"))]
                        {
                            wv.is_devtools_open()
                        }
                        #[cfg(not(any(debug_assertions, feature = "devtools")))]
                        {
                            false
                        }
                    })
                    .unwrap_or(false);
                tb.set_tool_mode(mode);
                tb.set_status(if open {
                    "Inspector: OPEN"
                } else {
                    "Inspector: closed"
                });
            }
            native_toolbar::ChromeToolMode::Profile => {
                let mode_name = format!("{:?}", AppState::global().profiles().get_profile().mode);
                tb.set_tool_mode(mode);
                tb.set_status(&format!("Now: {mode_name}"));
            }
            native_toolbar::ChromeToolMode::Shield => {
                let on = AppState::global().security().strip_tracking_enabled();
                tb.set_tool_mode(mode);
                tb.set_status(if on {
                    "Tracker strip: ON"
                } else {
                    "Tracker strip: OFF"
                });
            }
            native_toolbar::ChromeToolMode::Engine | native_toolbar::ChromeToolMode::Sla => {
                let s = engine_status::engine_status();
                tb.set_tool_mode(mode);
                tb.set_status(&format!(
                    "{} · cdp={} · intercept={}",
                    s["backend"].as_str().unwrap_or("?"),
                    s["cdp"],
                    s["intercept"]
                ));
            }
            _ => {
                tb.set_tool_mode(mode);
            }
        }
        tb.focus_tool_field();
        println!("⚡ [Thor] omnibar tool → {action_key:?}");
    }
}

#[cfg(target_os = "macos")]
fn run_omnibar_tool_go(
    tabs: &mut ShellTabs,
    toolbar: &mut Option<native_toolbar::NativeToolbar>,
    w: f64,
    h: f64,
) -> bool {
    let Some(tb) = toolbar.as_mut() else {
        return false;
    };
    let mode = tb.tool_mode();
    match mode {
        native_toolbar::ChromeToolMode::Browse => false,
        native_toolbar::ChromeToolMode::Locust => {
            let params = tb.read_locust_params();
            start_locust_from_bar(tabs, &params)
        }
        native_toolbar::ChromeToolMode::DevTools => {
            if let Some(wv) = tabs.webviews.get(&tabs.active) {
                #[cfg(any(debug_assertions, feature = "devtools"))]
                {
                    if wv.is_devtools_open() {
                        wv.close_devtools();
                        tb.set_status("Inspector: closed");
                    } else {
                        wv.open_devtools();
                        tb.set_status("Inspector: OPEN");
                    }
                }
            }
            false // keep strip open so user sees status
        }
        native_toolbar::ChromeToolMode::Engine | native_toolbar::ChromeToolMode::Sla => {
            let s = engine_status::engine_status();
            let msg = format!(
                "{} M{} · cdp={} · freeze={}",
                s["backend"].as_str().unwrap_or("?"),
                s["milestone"].as_str().unwrap_or("?"),
                s["cdp"],
                s["freeze"].as_str().unwrap_or("?")
            );
            tb.set_status(&msg);
            println!("⚡ [Thor Engine] {s}");
            false
        }
        native_toolbar::ChromeToolMode::Profile => {
            toggle_profile_mode();
            let mode_name = format!("{:?}", AppState::global().profiles().get_profile().mode);
            tb.set_status(&format!("Now: {mode_name}"));
            false
        }
        native_toolbar::ChromeToolMode::IdeVscode
        | native_toolbar::ChromeToolMode::IdeCursor
        | native_toolbar::ChromeToolMode::IdeWebstorm => {
            let path = tb.read_ide_path();
            let ide = match mode {
                native_toolbar::ChromeToolMode::IdeCursor => crate::dx::ide_bridge::Ide::Cursor,
                native_toolbar::ChromeToolMode::IdeWebstorm => crate::dx::ide_bridge::Ide::WebStorm,
                _ => crate::dx::ide_bridge::Ide::VSCode,
            };
            match crate::dx::ide_bridge::IdeBridge::open_file(ide, &path, None, None) {
                Ok(()) => {
                    tb.set_status("Opened");
                    true
                }
                Err(e) => {
                    tb.set_status(&format!("err: {e}"));
                    eprintln!("⚡ [Thor IDE] {e}");
                    false
                }
            }
        }
        native_toolbar::ChromeToolMode::Ssl => {
            match crate::net::ssl_ca::SslManager::generate_ca_and_leaf() {
                Ok(paths) => {
                    let ca = paths.ca_cert_pem.display().to_string();
                    match crate::net::ssl_ca::SslManager::install_ca_trust(&ca) {
                        Ok(()) => {
                            tb.set_status("CA installed");
                            println!("⚡ [Thor SSL] installed {ca}");
                            true
                        }
                        Err(e) => {
                            tb.set_status(&format!("generated; install: {e}"));
                            eprintln!("⚡ [Thor SSL] install: {e}");
                            false
                        }
                    }
                }
                Err(e) => {
                    tb.set_status(&format!("err: {e}"));
                    eprintln!("⚡ [Thor SSL] {e}");
                    false
                }
            }
        }
        native_toolbar::ChromeToolMode::Shield => {
            let cur = AppState::global().security().strip_tracking_enabled();
            AppState::global().security().set_strip_tracking(!cur);
            let on = !cur;
            tb.set_status(if on {
                "Tracker strip: ON"
            } else {
                "Tracker strip: OFF"
            });
            println!("⚡ [Thor Shield] strip_tracking={on}");
            false
        }
        native_toolbar::ChromeToolMode::ClearHistory => {
            AppState::global().tab_history().clear_all_history();
            tb.set_status("History cleared");
            println!("⚡ [Thor] cleared all tab history");
            true
        }
        native_toolbar::ChromeToolMode::Memory => {
            let chrome = None;
            let before = tabs.webviews.len();
            gc_idle_tabs(tabs, &chrome, w, h);
            let after = tabs.webviews.len();
            tb.set_status(&format!("Tabs {before}→{after}"));
            println!("⚡ [Thor Memory] compacted {before} → {after} tabs");
            true
        }
        native_toolbar::ChromeToolMode::WebVitals => {
            if let Some(wv) = tabs.webviews.get(&tabs.active) {
                let script = r#"(function(){
                  try {
                    var lcp = null, cls = 0, inp = null;
                    try {
                      var entries = performance.getEntriesByType('largest-contentful-paint');
                      if (entries && entries.length) lcp = entries[entries.length-1].startTime;
                    } catch(e) {}
                    try {
                      performance.getEntriesByType('layout-shift').forEach(function(e){
                        if (!e.hadRecentInput) cls += e.value;
                      });
                    } catch(e) {}
                    var nav = performance.getEntriesByType('navigation')[0];
                    var ttfb = nav ? nav.responseStart : null;
                    var summary = 'LCP=' + (lcp!=null?Math.round(lcp)+'ms':'?')
                      + ' CLS=' + cls.toFixed(3)
                      + ' TTFB=' + (ttfb!=null?Math.round(ttfb)+'ms':'?');
                    var msg = JSON.stringify({cmd:'web_vitals_result', summary: summary});
                    if (window.ipc && window.ipc.postMessage) window.ipc.postMessage(msg);
                    else if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.ipc)
                      window.webkit.messageHandlers.ipc.postMessage(msg);
                  } catch (e) {
                    var msg2 = JSON.stringify({cmd:'web_vitals_result', summary: 'vitals error'});
                    if (window.ipc && window.ipc.postMessage) window.ipc.postMessage(msg2);
                  }
                })();"#;
                let _ = wv.evaluate_script(script);
                tb.set_status("Capturing…");
            }
            false
        }
        native_toolbar::ChromeToolMode::Idor => {
            tb.set_status("Running IDOR scan…");
            false
        }
    }
}

fn handle_decode_payload(chrome: &Option<WebView>, input: &str) {
    match DecoderStudio::detect_type(input) {
        PayloadType::Jwt => match DecoderStudio::decode_jwt(input) {
            Ok(decoded) => {
                let header = decoded.header_json.unwrap_or(decoded.header_raw);
                let payload = decoded.payload_json.unwrap_or(decoded.payload_raw);
                let html = format!(
                    "<strong>HEADER:</strong> {}<br><strong>PAYLOAD:</strong> {}<br><em style=\"color:var(--text-dim)\">alg={} — DecoderStudio</em>",
                    html_escape(&header),
                    html_escape(&payload),
                    html_escape(&decoded.algorithm)
                );
                push_decode_result(chrome, "JWT DECODED", Some(&html), None);
            }
            Err(e) => push_decode_result(chrome, "JWT ERROR", None, Some(&e)),
        },
        PayloadType::Base64 => match DecoderStudio::decode_base64_url(input) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                push_decode_result(
                    chrome,
                    "BASE64 DECODED",
                    None,
                    Some(&format!("DECODED: {text}")),
                );
            }
            Err(e) => push_decode_result(chrome, "BASE64 ERROR", None, Some(&e)),
        },
        PayloadType::Hex => {
            push_decode_result(chrome, "HEX DETECTED", None, Some(input));
        }
        PayloadType::Json => {
            push_decode_result(chrome, "JSON", None, Some(input));
        }
        PayloadType::LlmStream => {
            let metrics = DecoderStudio::decode_llm_stream(input, 0);
            let html = format!(
                "<strong>LLM STREAM DETECTED:</strong><br>TTFT: {}ms | Tokens: {} | Chunks: {}<br><pre style=\"white-space:pre-wrap;background:rgba(0,0,0,0.3);padding:6px;border-radius:4px;\">{}</pre>",
                metrics.ttft_ms.unwrap_or(0),
                metrics.total_tokens,
                metrics.total_chunks,
                html_escape(&metrics.accumulated_text)
            );
            push_decode_result(chrome, "LLM SSE STREAM", Some(&html), None);
        }
        PayloadType::Web3Rpc => {
            if let Some(abi_match) = DecoderStudio::decode_web3_rpc(input) {
                let html = format!(
                    "<strong>WEB3 ABI SELECTOR:</strong> {} ({})<br>Function: <code>{}</code>",
                    abi_match.selector,
                    abi_match.signature,
                    abi_match.function_name
                );
                push_decode_result(chrome, "WEB3 RPC / CALLED", Some(&html), None);
            } else if crate::dx::web3_inspector::Web3Inspector::is_web3_rpc(input) {
                push_decode_result(chrome, "WEB3 JSON-RPC", None, Some(input));
            } else {
                push_decode_result(chrome, "WEB3 PAYLOAD", None, Some(input));
            }
        }
        PayloadType::Unknown => {
            push_decode_result(chrome, "RAW STRING", None, Some(input));
        }
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn rescan_tech_stack(tabs: &ShellTabs, chrome: &Option<WebView>, url_hint: Option<&str>) {
    let tab_id = tabs.active.clone();
    let url = url_hint
        .map(str::to_string)
        .or_else(|| tabs.meta.get(&tab_id).map(|m| m.url.clone()))
        .unwrap_or_else(|| "about:blank".into());

    // Ask the content WebView to post HTML back via IPC (callback isn't Send-safe with Rc).
    if let Some(wv) = tabs.webviews.get(&tab_id) {
        let url_js = serde_json::to_string(&url).unwrap_or_else(|_| "\"\"".into());
        let _ = wv.evaluate_script(&format!(
            r#"(function(){{
              try {{
                var html = (document.documentElement && document.documentElement.outerHTML || '').slice(0, 80000);
                var msg = JSON.stringify({{cmd:'tech_stack_html', url:{url_js}, html: html}});
                if (window.ipc && window.ipc.postMessage) window.ipc.postMessage(msg);
                else if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.ipc)
                  window.webkit.messageHandlers.ipc.postMessage(msg);
              }} catch (e) {{}}
            }})();"#
        ));
    }

    // Immediate URL-only estimate until HTML IPC arrives.
    let stack = TechStackDetector::detect(&url, "", None);
    push_tech_stack(chrome, &stack);
}

fn push_tech_stack(chrome: &Option<WebView>, stack: &crate::dx::stack_detector::DetectedStack) {
    let pills: Vec<serde_json::Value> = stack
        .badges
        .iter()
        .map(|b| {
            serde_json::json!({
                "icon": b.icon,
                "name": b.name,
                "ver": b.category,
            })
        })
        .collect();
    chrome_eval(
        chrome,
        &format!(
            "if (window.__thorUpdateTechStack) window.__thorUpdateTechStack({});",
            serde_json::Value::Array(pills)
        ),
    );
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
pub(crate) fn setup_macos_app_menu() {
    use icrate::AppKit::{NSApplication, NSMenu, NSMenuItem};
    use icrate::Foundation::{MainThreadMarker, NSString};

    unsafe {
        let mtm = MainThreadMarker::new_unchecked();
        let app = NSApplication::sharedApplication(mtm);
        let main_menu = NSMenu::new(mtm);
        let app_menu_item = NSMenuItem::new(mtm);
        let app_menu = NSMenu::new(mtm);

        let quit_item = NSMenuItem::new(mtm);
        let quit_title = NSString::from_str("Quit Thor");
        let quit_key = NSString::from_str("q");

        quit_item.setTitle(&quit_title);
        quit_item.setAction(Some(objc2::sel!(terminate:)));
        quit_item.setKeyEquivalent(&quit_key);

        app_menu.addItem(&quit_item);
        app_menu_item.setSubmenu(Some(&app_menu));
        main_menu.addItem(&app_menu_item);
        app.setMainMenu(Some(&main_menu));
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn setup_macos_app_icon() {
    use cocoa::appkit::NSApp;
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSData;
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let icon_bytes = include_bytes!("../../assets/thor_app_icon.jpg");
        let data = NSData::dataWithBytes_length_(
            nil,
            icon_bytes.as_ptr() as *const std::ffi::c_void,
            icon_bytes.len() as u64,
        );
        let image: id = msg_send![class!(NSImage), alloc];
        let image: id = msg_send![image, initWithData: data];
        if !image.is_null() {
            let app = NSApp();
            let _: () = msg_send![app, setApplicationIconImage: image];
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    if std::env::var("THOR_DUAL_WINDOW").map(|v| v == "1").unwrap_or(false) {
        chromium_shell::run_chromium_shell()
    } else {
        run_wry_shell()
    }
}


/// Wry-based in-window shell: AppKit native toolbar on top, WKWebView content
/// fills the area below CHROME_HEIGHT. No external Chromium process is spawned.
#[allow(dead_code)]
fn run_wry_shell() -> anyhow::Result<()> {
    println!("[Thor Shell] Initializing wry in-window shell...");

    #[cfg(target_os = "macos")]
    {
        setup_macos_app_menu();
        setup_macos_app_icon();
    }

    let _ = AppState::global();
    let _ = CHROME_HEIGHT; // keep referenced

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy: EventLoopProxy<UserEvent> = event_loop.create_proxy();

    let builder = {
        #[cfg(target_os = "macos")]
        {
            use tao::dpi::LogicalPosition;
            use tao::platform::macos::WindowBuilderExtMacOS;
            WindowBuilder::new()
                .with_title("Thor — Developer Browser")
                .with_inner_size(tao::dpi::LogicalSize::new(1440.0, 900.0))
                .with_decorations(true)
                .with_fullsize_content_view(true)
                .with_titlebar_transparent(true)
                .with_title_hidden(true)
                .with_traffic_light_inset(LogicalPosition::new(14.0, 18.0))
        }
        #[cfg(not(target_os = "macos"))]
        {
            WindowBuilder::new()
                .with_title("Thor — Developer Browser")
                .with_inner_size(tao::dpi::LogicalSize::new(1440.0, 900.0))
                .with_decorations(true)
        }
    };

    let window = Rc::new(builder.build(&event_loop)?);

    let scale_factor = window.scale_factor();
    let initial_logical = window.inner_size().to_logical::<f64>(scale_factor);
    let (init_w, init_h) = (initial_logical.width, initial_logical.height);

    let tabs: Rc<RefCell<ShellTabs>> = Rc::new(RefCell::new(ShellTabs {
        webviews: HashMap::new(),
        meta: HashMap::new(),
        active: String::new(),
    }));

    // chrome_ref holds the optional ⌘K palette overlay (hidden initially).
    let chrome_ref: Rc<RefCell<Option<WebView>>> = Rc::new(RefCell::new(None));

    #[cfg(target_os = "macos")]
    let toolbar_ref: Rc<RefCell<Option<native_toolbar::NativeToolbar>>> =
        Rc::new(RefCell::new(None));

    // Create the initial content tab (Google home, visible).
    {
        let first_tab_id = "tab_1".to_string();
        let wv = create_content_tab(
            &window,
            "https://www.google.com",
            first_tab_id.clone(),
            chrome_ref.clone(),
            tabs.clone(),
            proxy.clone(),
            #[cfg(target_os = "macos")]
            toolbar_ref.clone(),
            init_w,
            init_h,
            true,
        )?;
        let mut t = tabs.borrow_mut();
        t.webviews.insert(first_tab_id.clone(), wv);
        t.meta.insert(
            first_tab_id.clone(),
            TabMeta {
                last_active: Instant::now(),
                url: "https://www.google.com".to_string(),
                title: "Google".to_string(),
            },
        );
        t.active = first_tab_id;
    }

    #[cfg(target_os = "macos")]
    {
        let tb = native_toolbar::NativeToolbar::attach(&window, proxy.clone());
        tb.set_url("https://www.google.com");
        tb.focus_omnibox();
        *toolbar_ref.borrow_mut() = Some(tb);
    }

    let win_size: Rc<RefCell<(f64, f64)>> = Rc::new(RefCell::new((init_w, init_h)));
    let win_size_for_resize = win_size.clone();

    let tabs_for_loop = tabs.clone();
    let chrome_for_loop = chrome_ref.clone();
    #[cfg(target_os = "macos")]
    let toolbar_for_loop = toolbar_ref.clone();
    let window_for_events = window.clone();
    let proxy_for_loop = proxy.clone();

    let mut next_gc_at = Instant::now() + TAB_GC_INTERVAL;

    println!("============================================================");
    println!("⚡ [Thor Shell] AppKit chrome + wry WKWebView (in-window — no external Chromium)");
    println!("⚡ [Thor Shell] ⌘K palette · omnibox Enter navigates WKWebView");
    println!("============================================================");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(next_gc_at);

        match event {
            Event::UserEvent(UserEvent::Exit) => {
                *control_flow = ControlFlow::Exit;
            }

            Event::UserEvent(UserEvent::FocusOmnibox) => {
                #[cfg(target_os = "macos")]
                if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                    tb.focus_omnibox();
                }
            }

            Event::UserEvent(UserEvent::OpenCmdK)
            | Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::OpenCmdK)) => {
                open_cmd_k_in_active_tab(&tabs_for_loop.borrow());
            }

            Event::UserEvent(UserEvent::OpenCmdShiftK)
            | Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::OpenCmdShiftK)) => {
                open_cmd_shift_k_in_active_tab(&tabs_for_loop.borrow());
            }

            Event::UserEvent(UserEvent::LocustToggle)
            | Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::LocustToggle)) => {
                #[cfg(target_os = "macos")]
                {
                    let tabs_slot = tabs_for_loop.borrow();
                    let mut tb = toolbar_for_loop.borrow_mut();
                    open_tool_in_omnibar(&tabs_slot, &mut *tb, "locust_active_origin");
                }
            }

            Event::UserEvent(UserEvent::OpenTool(ref action_key)) => {
                #[cfg(target_os = "macos")]
                {
                    let key = action_key.clone();
                    let tabs_slot = tabs_for_loop.borrow();
                    let mut tb = toolbar_for_loop.borrow_mut();
                    open_tool_in_omnibar(&tabs_slot, &mut *tb, &key);
                }
            }

            Event::UserEvent(UserEvent::Navigate(ref url)) => {
                let url = normalize_omnibox_url(url);
                if !url.is_empty() {
                    let tabs = tabs_for_loop.borrow();
                    if let Some(wv) = tabs.webviews.get(&tabs.active) {
                        let _ = wv.load_url(&url);
                        drop(tabs);
                        #[cfg(target_os = "macos")]
                        if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                            tb.set_url(&url);
                        }
                    }
                }
            }

            Event::UserEvent(UserEvent::WebVitalsResult(ref summary)) => {
                let s = summary.clone();
                #[cfg(target_os = "macos")]
                if let Some(tb) = toolbar_for_loop.borrow_mut().as_mut() {
                    tb.set_status(&s);
                    println!("⚡ [Thor Vitals] {s}");
                }
            }

            Event::UserEvent(UserEvent::ToggleProfile)
            | Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::ToggleProfile)) => {
                #[cfg(target_os = "macos")]
                {
                    let tabs_slot = tabs_for_loop.borrow();
                    let mut tb = toolbar_for_loop.borrow_mut();
                    open_tool_in_omnibar(&tabs_slot, &mut *tb, "toggle_profile");
                }
                #[cfg(not(target_os = "macos"))]
                toggle_profile_mode();
            }

            Event::UserEvent(UserEvent::ClosePalette) => {}

            Event::UserEvent(UserEvent::SwitchTab(ref target)) => {
                let (w, h) = *win_size_for_resize.borrow();
                let target = target.clone();
                let url = switch_tab_session(&mut tabs_for_loop.borrow_mut(), &target, w, h);
                #[cfg(target_os = "macos")]
                if let Some(url) = url {
                    if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                        tb.set_url(&url);
                        tb.raise();
                    }
                }
                println!("⚡ [Thor] switch tab → {target}");
            }

            Event::UserEvent(UserEvent::CloseActiveTab) => {
                let (w, h) = *win_size_for_resize.borrow();
                let mut tabs = tabs_for_loop.borrow_mut();
                if tabs.webviews.len() <= 1 {
                    if let Some(wv) = tabs.webviews.get(&tabs.active) {
                        let _ = wv.evaluate_script("location.reload()");
                    }
                } else {
                    let closing = tabs.active.clone();
                    println!("⚡ [Thor] close tab {closing}");
                    close_tab_session(&mut tabs, &closing, w, h);
                    let url = tabs.resolved_url(&tabs.active.clone());
                    drop(tabs);
                    #[cfg(target_os = "macos")]
                    if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                        tb.set_url(&url);
                    }
                }
            }

            // Omnibox Enter key: read text from the native AppKit field, navigate WKWebView.
            Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::SubmitOmnibox)) => {
                #[cfg(target_os = "macos")]
                {
                    let raw = toolbar_for_loop
                        .borrow()
                        .as_ref()
                        .map(|t| t.read_url())
                        .unwrap_or_default();
                    let url = normalize_omnibox_url(&raw);
                    if !url.is_empty() {
                        let tabs = tabs_for_loop.borrow();
                        if let Some(wv) = tabs.webviews.get(&tabs.active) {
                            let _ = wv.load_url(&url);
                            println!("⚡ [Thor] omnibox navigate → {url}");
                            drop(tabs);
                            if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                                tb.set_url(&url);
                            }
                        }
                    }
                }
            }

            Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::Back)) => {
                let tabs = tabs_for_loop.borrow();
                if let Some(wv) = tabs.webviews.get(&tabs.active) {
                    let _ = wv.evaluate_script("history.back()");
                }
            }

            Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::Forward)) => {
                let tabs = tabs_for_loop.borrow();
                if let Some(wv) = tabs.webviews.get(&tabs.active) {
                    let _ = wv.evaluate_script("history.forward()");
                }
            }

            Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::Reload)) => {
                let tabs = tabs_for_loop.borrow();
                if let Some(wv) = tabs.webviews.get(&tabs.active) {
                    let _ = wv.evaluate_script("location.reload()");
                }
            }

            Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::NewTab)) => {
                let (w, h) = *win_size_for_resize.borrow();
                let tab_id = format!("tab_{}", Instant::now().elapsed().as_nanos());
                let url = "https://www.google.com";
                match create_content_tab(
                    &window_for_events,
                    url,
                    tab_id.clone(),
                    chrome_for_loop.clone(),
                    tabs_for_loop.clone(),
                    proxy_for_loop.clone(),
                    #[cfg(target_os = "macos")]
                    toolbar_for_loop.clone(),
                    w,
                    h,
                    false,
                ) {
                    Ok(wv) => {
                        let mut tabs = tabs_for_loop.borrow_mut();
                        tabs.webviews.insert(tab_id.clone(), wv);
                        tabs.meta.insert(
                            tab_id.clone(),
                            TabMeta {
                                last_active: Instant::now(),
                                url: url.to_string(),
                                title: "New Tab".to_string(),
                            },
                        );
                        let old_active = tabs.active.clone();
                        tabs.active = tab_id.clone();
                        tabs.show_only(&tab_id, w, h);
                        tabs.touch(&old_active);
                        drop(tabs);
                        #[cfg(target_os = "macos")]
                        if let Some(tb) = toolbar_for_loop.borrow_mut().as_mut() {
                            tb.set_url(url);
                            tb.reflow(w, h);
                        }
                        println!("⚡ [Thor] new tab {tab_id}");
                    }
                    Err(e) => eprintln!("⚡ [Thor] new_tab failed: {e}"),
                }
            }

            Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::ToolGo)) => {
                #[cfg(target_os = "macos")]
                {
                    let (w, h) = *win_size_for_resize.borrow();
                    let mut tabs_slot = tabs_for_loop.borrow_mut();
                    let mut tb = toolbar_for_loop.borrow_mut();
                    let done = run_omnibar_tool_go(&mut tabs_slot, &mut *tb, w, h);
                    if done {
                        if let Some(t) = tb.as_mut() {
                            t.set_tool_mode(native_toolbar::ChromeToolMode::Browse);
                            t.focus_omnibox();
                        }
                    }
                }
            }

            Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::ToolCancel)) => {
                #[cfg(target_os = "macos")]
                if let Some(tb) = toolbar_for_loop.borrow_mut().as_mut() {
                    if !tb.tool_mode().is_browse() {
                        tb.set_tool_mode(native_toolbar::ChromeToolMode::Browse);
                        tb.focus_omnibox();
                    }
                }
            }

            Event::NewEvents(StartCause::ResumeTimeReached { .. })
            | Event::NewEvents(StartCause::Init) => {
                let now = Instant::now();
                if now >= next_gc_at {
                    next_gc_at = now + TAB_GC_INTERVAL;
                    let (w, h) = *win_size_for_resize.borrow();
                    let chrome = chrome_for_loop.borrow();
                    gc_idle_tabs(&mut tabs_for_loop.borrow_mut(), &*chrome, w, h);
                }
            }

            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::Resized(new_size) => {
                    let sf = window_for_events.scale_factor();
                    let logical = new_size.to_logical::<f64>(sf);
                    let (w, h) = (logical.width, logical.height);
                    *win_size_for_resize.borrow_mut() = (w, h);
                    let tabs = tabs_for_loop.borrow();
                    tabs.show_only(&tabs.active.clone(), w, h);
                    drop(tabs);
                    #[cfg(target_os = "macos")]
                    if let Some(tb) = toolbar_for_loop.borrow_mut().as_mut() {
                        tb.reflow(w, h);
                    }
                }
                _ => {}
            },

            _ => {}
        }
    });
}

pub(crate) fn normalize_omnibox_url(raw: &str) -> String {
    let mut url = raw.trim().to_string();
    if url.is_empty() {
        return url;
    }
    if !(url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("about:")
        || url.starts_with("file://")
        || url.starts_with("data:"))
    {
        if url.contains('.') && !url.contains(' ') {
            url = format!("https://{url}");
        } else {
            url = format!(
                "https://www.google.com/search?q={}",
                url.replace(' ', "+")
            );
        }
    }
    url
}

#[cfg(target_os = "macos")]
pub(crate) fn start_locust_from_bar_chromium(
    tabs: &chromium_tabs::ChromiumTabs,
    params: &native_toolbar::LocustBarParams,
) -> bool {
    let url = tabs.active_url();
    if url.is_empty() {
        eprintln!("⚡ [Thor Locust] no active URL");
        return false;
    }
    let payload = serde_json::json!({
        "url": url,
        "users": params.users,
        "ramp": params.ramp_up_secs,
        "runtime": params.runtime_secs,
        "profile": params.profile,
        "public_ack": false,
    });
    println!(
        "⚡ [Thor Locust] start users={} ramp={}s runtime={}s profile={} url={}",
        params.users, params.ramp_up_secs, params.runtime_secs, params.profile, url
    );
    match ipc_dispatch::dispatch_privileged("locust_start", &payload) {
        ipc_dispatch::DispatchResult::Message(m) => {
            println!("⚡ [Thor Locust] {m}");
            true
        }
        ipc_dispatch::DispatchResult::Err(e) => {
            eprintln!("⚡ [Thor Locust] {e}");
            false
        }
        _ => false,
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn open_tool_in_omnibar_chromium(
    tabs: &chromium_tabs::ChromiumTabs,
    toolbar: &mut Option<native_toolbar::NativeToolbar>,
    action_key: &str,
) {
    let Some(mode) = native_toolbar::ChromeToolMode::from_action_key(action_key) else {
        eprintln!("⚡ [Thor] unknown tool action_key: {action_key}");
        return;
    };

    if mode == native_toolbar::ChromeToolMode::DevTools {
        tabs.open_devtools_active();
        return;
    }

    if mode == native_toolbar::ChromeToolMode::Locust {
        if AppState::global().locust_is_running() {
            match ipc_dispatch::dispatch_privileged("locust_stop", &serde_json::json!({})) {
                ipc_dispatch::DispatchResult::LocustMetrics(m) => {
                    println!(
                        "⚡ [Thor Locust] stopped — reqs={} rps={:.1} score={:.0}",
                        m.total_requests, m.rps, m.score
                    );
                }
                ipc_dispatch::DispatchResult::Message(m) => println!("⚡ [Thor Locust] {m}"),
                ipc_dispatch::DispatchResult::Err(e) => eprintln!("⚡ [Thor Locust] {e}"),
                _ => {}
            }
            if let Some(tb) = toolbar.as_mut() {
                tb.set_tool_mode(native_toolbar::ChromeToolMode::Browse);
                tb.focus_omnibox();
            }
            return;
        }
        if let Some(tb) = toolbar.as_ref() {
            if tb.tool_mode() == native_toolbar::ChromeToolMode::Locust {
                if let Some(tb) = toolbar.as_mut() {
                    tb.set_tool_mode(native_toolbar::ChromeToolMode::Browse);
                    tb.focus_omnibox();
                }
                return;
            }
        }
    }

    if let Some(tb) = toolbar.as_mut() {
        match mode {
            native_toolbar::ChromeToolMode::Profile => {
                let mode_name = format!("{:?}", AppState::global().profiles().get_profile().mode);
                tb.set_tool_mode(mode);
                tb.set_status(&format!("Now: {mode_name}"));
            }
            native_toolbar::ChromeToolMode::Shield => {
                let on = AppState::global().security().strip_tracking_enabled();
                tb.set_tool_mode(mode);
                tb.set_status(if on {
                    "Tracker strip: ON"
                } else {
                    "Tracker strip: OFF"
                });
            }
            native_toolbar::ChromeToolMode::Engine | native_toolbar::ChromeToolMode::Sla => {
                let s = engine_status::engine_status();
                tb.set_tool_mode(mode);
                tb.set_status(&format!(
                    "{} · cdp={} · intercept={}",
                    s["backend"].as_str().unwrap_or("?"),
                    s["cdp"],
                    s["intercept"]
                ));
            }
            native_toolbar::ChromeToolMode::Idor => {
                tb.set_tool_mode(mode);
                match ipc_dispatch::dispatch_privileged("run_idor_matrix", &serde_json::json!({})) {
                    ipc_dispatch::DispatchResult::Message(m) => {
                        let summary = if m.contains("vulnerability_count") {
                            "IDOR scan finished"
                        } else {
                            &m
                        };
                        tb.set_status(summary);
                    }
                    ipc_dispatch::DispatchResult::Err(e) => tb.set_status(&format!("IDOR Err: {e}")),
                    _ => tb.set_status("IDOR scan completed"),
                }
            }
            _ => {
                tb.set_tool_mode(mode);
            }
        }
        tb.focus_tool_field();
        println!("⚡ [Thor] omnibar tool → {action_key:?}");
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn run_omnibar_tool_go_chromium(
    tabs: &mut chromium_tabs::ChromiumTabs,
    toolbar: &mut Option<native_toolbar::NativeToolbar>,
) -> bool {
    let Some(tb) = toolbar.as_mut() else {
        return false;
    };
    let mode = tb.tool_mode();
    match mode {
        native_toolbar::ChromeToolMode::Browse => false,
        native_toolbar::ChromeToolMode::Locust => {
            let params = tb.read_locust_params();
            start_locust_from_bar_chromium(tabs, &params)
        }
        native_toolbar::ChromeToolMode::DevTools => {
            tabs.open_devtools_active();
            false
        }
        native_toolbar::ChromeToolMode::Engine | native_toolbar::ChromeToolMode::Sla => {
            let s = engine_status::engine_status();
            let msg = format!(
                "{} M{} · cdp={} · freeze={}",
                s["backend"].as_str().unwrap_or("?"),
                s["milestone"].as_str().unwrap_or("?"),
                s["cdp"],
                s["freeze"].as_str().unwrap_or("?")
            );
            tb.set_status(&msg);
            println!("⚡ [Thor Engine] {s}");
            false
        }
        native_toolbar::ChromeToolMode::Profile => {
            toggle_profile_mode();
            let mode_name = format!("{:?}", AppState::global().profiles().get_profile().mode);
            tb.set_status(&format!("Now: {mode_name}"));
            false
        }
        native_toolbar::ChromeToolMode::IdeVscode
        | native_toolbar::ChromeToolMode::IdeCursor
        | native_toolbar::ChromeToolMode::IdeWebstorm => {
            let path = tb.read_ide_path();
            let ide = match mode {
                native_toolbar::ChromeToolMode::IdeCursor => crate::dx::ide_bridge::Ide::Cursor,
                native_toolbar::ChromeToolMode::IdeWebstorm => crate::dx::ide_bridge::Ide::WebStorm,
                _ => crate::dx::ide_bridge::Ide::VSCode,
            };
            match crate::dx::ide_bridge::IdeBridge::open_file(ide, &path, None, None) {
                Ok(()) => {
                    tb.set_status("Opened");
                    true
                }
                Err(e) => {
                    tb.set_status(&format!("err: {e}"));
                    false
                }
            }
        }
        native_toolbar::ChromeToolMode::Ssl => {
            match crate::net::ssl_ca::SslManager::generate_ca_and_leaf() {
                Ok(paths) => {
                    let ca = paths.ca_cert_pem.display().to_string();
                    match crate::net::ssl_ca::SslManager::install_ca_trust(&ca) {
                        Ok(()) => {
                            tb.set_status("CA installed");
                            true
                        }
                        Err(e) => {
                            tb.set_status(&format!("generated; install: {e}"));
                            false
                        }
                    }
                }
                Err(e) => {
                    tb.set_status(&format!("err: {e}"));
                    false
                }
            }
        }
        native_toolbar::ChromeToolMode::Shield => {
            let cur = AppState::global().security().strip_tracking_enabled();
            AppState::global().security().set_strip_tracking(!cur);
            tb.set_status(if !cur {
                "Tracker strip: ON"
            } else {
                "Tracker strip: OFF"
            });
            false
        }
        native_toolbar::ChromeToolMode::ClearHistory => {
            AppState::global().tab_history().clear_all_history();
            tb.set_status("History cleared");
            true
        }
        native_toolbar::ChromeToolMode::Memory => {
            let before = tabs.sessions.len();
            tabs.gc_idle(TAB_IDLE_TTL);
            let after = tabs.sessions.len();
            tb.set_status(&format!("Tabs {before}→{after}"));
            true
        }
        native_toolbar::ChromeToolMode::WebVitals => {
            let script = r#"(function(){
              try {
                var lcp = null, cls = 0;
                try {
                  var entries = performance.getEntriesByType('largest-contentful-paint');
                  if (entries && entries.length) lcp = entries[entries.length-1].startTime;
                } catch(e) {}
                try {
                  performance.getEntriesByType('layout-shift').forEach(function(e){
                    if (!e.hadRecentInput) cls += e.value;
                  });
                } catch(e) {}
                var nav = performance.getEntriesByType('navigation')[0];
                var ttfb = nav ? nav.responseStart : null;
                var summary = 'LCP=' + (lcp!=null?Math.round(lcp)+'ms':'?')
                  + ' CLS=' + cls.toFixed(3)
                  + ' TTFB=' + (ttfb!=null?Math.round(ttfb)+'ms':'?');
                window.__thorIpcQueue = window.__thorIpcQueue || [];
                window.__thorIpcQueue.push(JSON.stringify({cmd:'web_vitals_result', summary: summary}));
              } catch (e) {
                window.__thorIpcQueue = window.__thorIpcQueue || [];
                window.__thorIpcQueue.push(JSON.stringify({cmd:'web_vitals_result', summary: 'vitals error'}));
              }
            })();"#;
            let _ = tabs.evaluate_active(script);
            tb.set_status("Capturing…");
            false
        }
        native_toolbar::ChromeToolMode::Idor => {
            tb.set_status("Running IDOR scan…");
            false
        }
    }
}
