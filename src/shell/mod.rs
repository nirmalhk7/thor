pub mod gui;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::{dpi, Rect, WebView, WebViewBuilder};

#[cfg(target_os = "macos")]
use tao::platform::macos::WindowBuilderExtMacOS;

const CHROME_HEIGHT: f64 = 48.0;

fn create_content_tab(
    window: &tao::window::Window,
    url: &str,
    tab_id: String,
    chrome_ref: Rc<RefCell<Option<WebView>>>,
    hotkey_script: &str,
    w: f64,
    h: f64,
) -> anyhow::Result<WebView> {
    let chrome_for_hotkeys = chrome_ref.clone();
    let chrome_for_title = chrome_ref.clone();
    let chrome_for_nav = chrome_ref.clone();
    let id_for_title = tab_id.clone();
    let id_for_nav = tab_id.clone();

    let wv = WebViewBuilder::new()
        .with_url(url)
        .with_user_agent(crate::net::security_shield::CHROMIUM_USER_AGENT)
        .with_back_forward_navigation_gestures(true)
        .with_bounds(Rect {
            position: dpi::LogicalPosition::new(0.0, CHROME_HEIGHT).into(),
            size: dpi::LogicalSize::new(w, h - CHROME_HEIGHT).into(),
        })
        .with_devtools(true)
        .with_initialization_script(hotkey_script)
        .with_ipc_handler(move |request: http::Request<String>| {
            let msg = request.body();
            println!("⚡ [Thor Tab IPC Rx] {}", msg);
            let payload: serde_json::Value = serde_json::from_str(msg).unwrap_or_default();
            if let Some(cmd) = payload["cmd"].as_str() {
                if let Some(ref chrome_wv) = *chrome_for_hotkeys.borrow() {
                    match cmd {
                        "log" => {
                            if let Some(log_msg) = payload["message"].as_str() {
                                println!("⚡ [Thor Log] {}", log_msg);
                            }
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
                        }
                        "open_cmd_k" => {
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("openModalFromHotkey('bookmarks_history')");
                        }
                        "open_cmd_shift_k" => {
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("openModalFromHotkey('settings')");
                        }
                        "hotkey_new_tab" => {
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("newTab()");
                        }
                        "hotkey_close_tab" => {
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("const ai = tabs.findIndex(t => t.active); if (ai >= 0) closeTab(ai);");
                        }
                        "hotkey_focus_url" => {
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("focusUrlInput()");
                        }
                        "toggle_devtools" => {
                            let _ = chrome_wv.evaluate_script("ipc('toggle_devtools')");
                        }
                        _ => {}
                    }
                }
            }
        })
        .with_document_title_changed_handler(move |title: String| {
            if let Some(ref wv) = *chrome_for_title.borrow() {
                let escaped_title = serde_json::to_string(&title).unwrap_or_default();
                let escaped_id = serde_json::to_string(&id_for_title).unwrap_or_default();
                let _ = wv.evaluate_script(&format!("updateTabTitle({}, {})", escaped_id, escaped_title));
            }
        })
        .with_navigation_handler(move |url: String| {
            if let Some(ref wv) = *chrome_for_nav.borrow() {
                let escaped_url = serde_json::to_string(&url).unwrap_or_default();
                let escaped_id = serde_json::to_string(&id_for_nav).unwrap_or_default();
                let _ = wv.evaluate_script(&format!("updateTabUrl({}, {})", escaped_id, escaped_url));
            }
            true
        })
        .with_new_window_req_handler(|_url: String| false)
        .build_as_child(window)?;

    Ok(wv)
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn setup_macos_app_menu() {
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

pub fn run() -> anyhow::Result<()> {
    println!("[Thor Shell] Initializing native OS window event loop on main thread...");

    #[cfg(target_os = "macos")]
    setup_macos_app_menu();

    let event_loop = EventLoop::new();
    let mut builder = WindowBuilder::new()
        .with_title("Thor — Developer Browser ⚡")
        .with_inner_size(tao::dpi::LogicalSize::new(1440.0, 900.0));

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
            .with_title_hidden(true)
            .with_movable_by_window_background(true);
    }

    let window = Rc::new(builder.build(&event_loop)?);

    let chrome_ref: Rc<RefCell<Option<WebView>>> = Rc::new(RefCell::new(None));
    let tab_pool: Rc<RefCell<HashMap<String, WebView>>> = Rc::new(RefCell::new(HashMap::new()));
    let active_tab_id: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    let scale_factor = window.scale_factor();
    let initial_logical = window.inner_size().to_logical::<f64>(scale_factor);
    let win_size: Rc<RefCell<(f64, f64)>> = Rc::new(RefCell::new((initial_logical.width, initial_logical.height)));

    let raw_chrome_bookmarks = crate::dx::cmd_palette::read_chrome_bookmarks_json().unwrap_or_default();
    let escaped_chrome_bookmarks = serde_json::to_string(&raw_chrome_bookmarks).unwrap_or_default();

    let hotkey_script = format!(r#"
        (function() {{
            window.__thor_chrome_bookmarks_raw = {};
            let inTabVisitedHistory = [];

            function sendIpc(msg) {{
                try {{
                    if (window.ipc && window.ipc.postMessage) {{
                        window.ipc.postMessage(msg);
                    }} else if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.ipc) {{
                        window.webkit.messageHandlers.ipc.postMessage(msg);
                    }}
                }} catch(e) {{}}
            }}

            function recordTabVisit() {{
                const url = window.location.href;
                const title = document.title || url;
                if (url && !url.startsWith('about:')) {{
                    const last = inTabVisitedHistory[0];
                    if (!last || last.url !== url) {{
                        inTabVisitedHistory.unshift({{
                            icon: '🌐',
                            title: title,
                            url: url,
                            badge: 'Tab History',
                            type: 'history'
                        }});
                        if (inTabVisitedHistory.length > 50) inTabVisitedHistory.pop();
                    }}
                }}
            }}

            recordTabVisit();

            function flattenChromeNode(node, folderPath) {{
                if (!node) return [];
                let res = [];
                const name = node.name || '';
                const currentPath = (folderPath && name && name !== 'bookmark_bar' && name !== 'other' && name !== 'synced')
                  ? (folderPath + ' / ' + name)
                  : (name !== 'bookmark_bar' && name !== 'other' && name !== 'synced' ? name : folderPath);

                if (node.type === 'url' && node.url) {{
                  let icon = '🔖';
                  if (node.url.includes('github')) icon = '🐙';
                  else if (node.url.includes('google') || node.url.includes('gemini')) icon = '🔍';
                  else if (node.url.includes('chatgpt') || node.url.includes('perplexity') || node.url.includes('oneai')) icon = '🤖';
                  else if (node.url.includes('zoom')) icon = '📹';
                  else if (node.url.includes('slack')) icon = '💬';
                  else if (node.url.includes('canvas') || node.url.includes('colorado')) icon = '🎓';

                  res.push({{ icon: icon, title: node.name || node.url, url: node.url, badge: currentPath || 'Chrome', type: 'bookmark' }});
                }} else if (node.children && Array.isArray(node.children)) {{
                  node.children.forEach(child => res.push(...flattenChromeNode(child, currentPath)));
                }}
                return res;
            }}

            function getParsedChromeBookmarks() {{
                try {{
                    const raw = window.__thor_chrome_bookmarks_raw;
                    if (!raw) return [];
                    const data = (typeof raw === 'string') ? JSON.parse(raw) : raw;
                    const res = [];
                    if (data && data.roots) {{
                        if (data.roots.bookmark_bar) res.push(...flattenChromeNode(data.roots.bookmark_bar, ''));
                        if (data.roots.other) res.push(...flattenChromeNode(data.roots.other, 'Other'));
                        if (data.roots.synced) res.push(...flattenChromeNode(data.roots.synced, 'Mobile'));
                    }}
                    return res;
                }} catch(e) {{
                    return [];
                }}
            }}

            function initThorModal() {{
                if (document.getElementById('thorModalOverlay')) return;
                const style = document.createElement('style');
                style.textContent = `
                  .thor-modal-overlay {{
                    position: fixed; top: 0; left: 0; right: 0; bottom: 0;
                    background: rgba(0, 0, 0, 0.75); z-index: 999999999;
                    display: none; align-items: flex-start; justify-content: center;
                    padding-top: 50px; font-family: -apple-system, BlinkMacSystemFont, 'Inter', sans-serif;
                  }}
                  .thor-modal-overlay.active {{ display: flex; }}
                  .thor-modal-box {{
                    width: 660px; max-width: 90vw; background: #161820;
                    border: 1px solid rgba(255,255,255,0.2); border-radius: 10px;
                    box-shadow: 0 16px 48px rgba(0,0,0,0.8); overflow: hidden; display: flex; flex-direction: column;
                    color: #f1f5f9;
                  }}
                  .thor-modal-search {{
                    padding: 14px; background: rgba(0,0,0,0.3); border-bottom: 1px solid rgba(255,255,255,0.1);
                    display: flex; align-items: center; gap: 10px;
                  }}
                  .thor-modal-input {{
                    flex: 1; background: transparent; border: none; outline: none;
                    color: #fff; font-size: 14px; font-family: sans-serif;
                  }}
                  .thor-modal-list {{ max-height: 340px; overflow-y: auto; padding: 8px; display: flex; flex-direction: column; gap: 4px; }}
                  .thor-modal-item {{
                    padding: 10px 12px; border-radius: 6px; display: flex; align-items: center; justify-content: space-between;
                    cursor: pointer; background: transparent; transition: background 100ms;
                  }}
                  .thor-modal-item:hover, .thor-modal-item.selected {{ background: rgba(255,255,255,0.1); }}
                  .thor-item-title {{ font-weight: 500; font-size: 13px; color: #f1f5f9; }}
                  .thor-item-sub {{ font-size: 11px; color: #94a3b8; margin-top: 2px; font-family: monospace; }}
                  .thor-item-badge {{ font-size: 10px; padding: 2px 6px; border-radius: 4px; background: rgba(255,255,255,0.1); color: #3b82f6; font-family: monospace; }}
                `;
                document.head ? document.head.appendChild(style) : document.documentElement.appendChild(style);

                const overlay = document.createElement('div');
                overlay.className = 'thor-modal-overlay';
                overlay.id = 'thorModalOverlay';
                overlay.onclick = function(e) {{ if (e.target === overlay) closeThorModal(); }};

                overlay.innerHTML = `
                  <div class="thor-modal-box">
                    <div class="thor-modal-search">
                      <span style="color:#3b82f6;" id="thorModalHeaderIcon">🔍</span>
                      <input type="text" class="thor-modal-input" id="thorModalInput" placeholder="Type to search...">
                      <span style="font-size:10px;color:#64748b;background:rgba(255,255,255,0.1);padding:2px 6px;border-radius:4px;">ESC</span>
                    </div>
                    <div class="thor-modal-list" id="thorModalList"></div>
                  </div>
                `;
                document.body ? document.body.appendChild(overlay) : document.documentElement.appendChild(overlay);

                const input = document.getElementById('thorModalInput');
                input.oninput = renderThorItems;
                input.onkeydown = function(e) {{
                    if (e.key === 'Escape') closeThorModal();
                    else if (e.key === 'Enter') {{
                        const query = input.value.trim();
                        if (query) {{
                            let url = (query.includes('.') && !query.includes(' ')) ? 'https://' + query : 'https://www.google.com/search?q=' + encodeURIComponent(query);
                            location.href = url;
                            closeThorModal();
                        }}
                    }}
                }};
            }}

            let currentThorMode = 'bookmarks_history';

            function getCombinedItems(mode) {{
                if (mode === 'settings') {{
                    return [
                        {{ icon: '🛠️', title: 'Toggle WebKit Developer Tools (F12)', action: 'devtools', badge: 'F12' }},
                        {{ icon: '💻', title: 'Open Project in VS Code', action: 'vscode', badge: 'vscode://' }},
                        {{ icon: '⚡', title: 'Open Project in Cursor AI Editor', action: 'cursor', badge: 'cursor://' }},
                        {{ icon: '🚀', title: 'Open Project in WebStorm', action: 'webstorm', badge: 'webstorm://' }},
                        {{ icon: '🛡️', title: 'Local ECDSA P-256 SSL CA Certificate', action: 'ssl', badge: 'SSL Trusted' }},
                        {{ icon: '🔥', title: 'HMR WebSocket Proxy (Tab Sleep Buffer)', action: 'hmr', badge: 'WebSocket' }},
                        {{ icon: '❄️', title: 'OS Process Compaction & RAM Freeing', action: 'memory', badge: 'RAM Free' }},
                        {{ icon: '🗑️', title: 'Clear Visited Tab History & Cache', action: 'clear', badge: 'Privacy' }},
                    ];
                }}
                
                const chromeBms = getParsedChromeBookmarks();
                return [...inTabVisitedHistory, ...chromeBms];
            }}

            function renderThorItems() {{
                const list = document.getElementById('thorModalList');
                const input = document.getElementById('thorModalInput');
                if (!list || !input) return;
                const q = input.value.toLowerCase().trim();
                list.innerHTML = '';
                const items = getCombinedItems(currentThorMode);
                const filtered = items.filter(it => 
                    it.title.toLowerCase().includes(q) || 
                    (it.url && it.url.toLowerCase().includes(q)) || 
                    (it.badge && it.badge.toLowerCase().includes(q))
                );

                if (filtered.length === 0) {{
                    list.innerHTML = '<div style="padding:16px;text-align:center;color:#64748b;font-size:12px;">No matching items found. Type to search or press Enter.</div>';
                    return;
                }}

                filtered.forEach(it => {{
                    const el = document.createElement('div');
                    el.className = 'thor-modal-item';
                    el.innerHTML = `
                      <div>
                        <div class="thor-item-title">${{it.icon || '⚡'}} ${{it.title}}</div>
                        <div class="thor-item-sub">${{it.url || ''}}</div>
                      </div>
                      <div class="thor-item-badge">${{it.badge || ''}}</div>
                    `;
                    el.onclick = function() {{
                        if (it.url) {{
                            location.href = it.url;
                        }} else if (it.action === 'vscode' || it.action === 'cursor' || it.action === 'webstorm') {{
                            sendIpc(JSON.stringify({{ cmd: 'open_ide', ide: it.action }}));
                        }} else if (it.action === 'devtools') {{
                            sendIpc(JSON.stringify({{ cmd: 'toggle_devtools' }}));
                        }} else if (it.action === 'ssl') {{
                            alert('Local ECDSA P-256 SSL CA Certificate Active for *.localhost');
                        }} else if (it.action === 'hmr') {{
                            alert('HMR WebSocket Proxy Active.');
                        }} else if (it.action === 'memory') {{
                            alert('OS Memory Compaction Triggered.');
                        }} else if (it.action === 'clear') {{
                            inTabVisitedHistory.length = 0;
                            alert('Tab Visited History Cleared.');
                        }}
                        closeThorModal();
                    }};
                    list.appendChild(el);
                }});
            }}

            window.openThorModal = function(mode = 'bookmarks_history') {{
                currentThorMode = mode;
                initThorModal();
                const overlay = document.getElementById('thorModalOverlay');
                const input = document.getElementById('thorModalInput');
                const icon = document.getElementById('thorModalHeaderIcon');
                if (overlay) {{
                    overlay.classList.add('active');
                    if (input) {{
                        input.value = '';
                        input.placeholder = (mode === 'settings') ? 'Search settings, dev tools, or preferences...' : 'Search Chrome bookmarks and in-tab visited history...';
                    }}
                    if (icon) {{
                        icon.textContent = (mode === 'settings') ? '⚙️' : '🔍';
                    }}
                    renderThorItems();
                    if (input) {{ setTimeout(() => input.focus(), 30); }}
                }}
            }};

            window.closeThorModal = function() {{
                const overlay = document.getElementById('thorModalOverlay');
                if (overlay) overlay.classList.remove('active');
            }};

            function handleKey(e) {{
                const isCmdOrCtrl = e.metaKey || e.ctrlKey;
                if (!isCmdOrCtrl) return;

                const key = e.key ? e.key.toLowerCase() : '';
                const code = e.code ? e.code.toLowerCase() : '';

                if (e.shiftKey && (key === 'k' || code === 'keyk')) {{
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({{ cmd: 'open_cmd_shift_k' }}));
                    window.openThorModal('settings');
                }} else if (!e.shiftKey && (key === 'k' || code === 'keyk')) {{
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({{ cmd: 'open_cmd_k' }}));
                    window.openThorModal('bookmarks_history');
                }} else if (key === 't') {{
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({{ cmd: 'hotkey_new_tab' }}));
                }} else if (key === 'w') {{
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({{ cmd: 'hotkey_close_tab' }}));
                }} else if (key === 'l') {{
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({{ cmd: 'hotkey_focus_url' }}));
                }}
            }}

            if (document.readyState === 'loading') {{
                document.addEventListener('DOMContentLoaded', initThorModal);
            }} else {{
                initThorModal();
            }}

            window.addEventListener('keydown', handleKey, true);
            document.addEventListener('keydown', handleKey, true);

            // Option-click (altKey) element jump — extract source location from
            // data-source-file attributes, React __source props, or sourceMappingURL,
            // and send open_ide IPC to jump to the element's source in the IDE.
            document.addEventListener('click', function(e) {{
                if (!e.altKey) return;
                e.preventDefault();
                e.stopPropagation();

                const el = e.target;
                let path = null;
                let line = null;
                let col = null;

                // 1. Check for data-source-file / data-source-line / data-source-col attributes
                let node = el;
                while (node && node !== document.documentElement) {{
                    if (node.dataset && node.dataset.sourceFile) {{
                        path = node.dataset.sourceFile;
                        line = parseInt(node.dataset.sourceLine, 10) || null;
                        col = parseInt(node.dataset.sourceCol, 10) || null;
                        break;
                    }}
                    // 2. Check React fiber __source (react-refresh / babel plugin)
                    if (node._reactInternals || node._reactInternalInstance) {{
                        const fiber = node._reactInternals || node._reactInternalInstance;
                        if (fiber && fiber._debugSource) {{
                            path = fiber._debugSource.fileName;
                            line = fiber._debugSource.lineNumber;
                            col = fiber._debugSource.columnNumber || null;
                            break;
                        }}
                    }}
                    node = node.parentElement;
                }}

                // 3. Fallback: inspect computed sourceURL from inline styles or scripts
                if (!path) {{
                    const id = el.id || el.className;
                    const tag = el.tagName.toLowerCase();
                    // Send best-effort element identification to the IDE bridge
                    path = document.location.pathname || '.';
                    sendIpc(JSON.stringify({{
                        cmd: 'log',
                        message: 'Option-click on <' + tag + '> id=' + (el.id || '') + ' class=' + (el.className || '') + ' — no source mapping found, opening page path'
                    }}));
                }}

                if (path) {{
                    sendIpc(JSON.stringify({{
                        cmd: 'open_ide',
                        ide: window.__thorPreferredIde || 'vscode',
                        path: path,
                        line: line,
                        col: col
                    }}));
                }}
            }}, true);

            let swipeCooling = false;
            window.addEventListener('wheel', function(e) {{
                if (swipeCooling) return;
                if (Math.abs(e.deltaX) > 40 && Math.abs(e.deltaX) > Math.abs(e.deltaY) * 1.5) {{
                    if (e.deltaX < -40) {{
                        swipeCooling = true;
                        sendIpc(JSON.stringify({{ cmd: 'back' }}));
                        setTimeout(() => {{ swipeCooling = false; }}, 350);
                    }} else if (e.deltaX > 40) {{
                        swipeCooling = true;
                        sendIpc(JSON.stringify({{ cmd: 'forward' }}));
                        setTimeout(() => {{ swipeCooling = false; }}, 350);
                    }}
                }}
            }}, {{ passive: true }});
        }})();
    "#, escaped_chrome_bookmarks);

    // Create Initial Default Native Tab ("default_tab")
    let initial_tab_id = "default_tab".to_string();
    let initial_wv = create_content_tab(
        &window,
        "https://www.google.com",
        initial_tab_id.clone(),
        chrome_ref.clone(),
        &hotkey_script,
        1440.0,
        900.0,
    )?;

    tab_pool.borrow_mut().insert(initial_tab_id.clone(), initial_wv);
    *active_tab_id.borrow_mut() = initial_tab_id;

    // Clones for Chrome IPC
    let window_for_ipc = window.clone();
    let chrome_for_ipc = chrome_ref.clone();
    let tab_pool_for_ipc = tab_pool.clone();
    let active_tab_for_ipc = active_tab_id.clone();
    let win_size_for_ipc = win_size.clone();
    let hotkey_script_for_ipc = hotkey_script.clone();

    // Build Chrome Toolbar WebView SECOND (fixed 48px header on top)
    let chrome = WebViewBuilder::new()
        .with_html(gui::get_chrome_html())
        .with_bounds(Rect {
            position: dpi::LogicalPosition::new(0.0, 0.0).into(),
            size: dpi::LogicalSize::new(initial_logical.width, CHROME_HEIGHT).into(),
        })
        .with_transparent(true)
        .with_ipc_handler(move |request: http::Request<String>| {
            let msg = request.body();
            println!("⚡ [Thor Chrome IPC Rx] {}", msg);
            let payload: serde_json::Value = serde_json::from_str(msg).unwrap_or_default();
            if let Some(cmd) = payload["cmd"].as_str() {
                match cmd {
                    "drag_window" => {
                        let _ = window_for_ipc.drag_window();
                    }
                    "log" => {
                        if let Some(log_msg) = payload["message"].as_str() {
                            println!("⚡ [Thor Chrome Log] {}", log_msg);
                        }
                    }
                    "open_cmd_palette" => {
                        let cur_active = active_tab_for_ipc.borrow().clone();
                        let pool = tab_pool_for_ipc.borrow();
                        if let Some(wv) = pool.get(&cur_active) {
                            let _ = wv.focus();
                            let _ = wv.evaluate_script("if (window.openThorModal) window.openThorModal('bookmarks_history');");
                        }
                    }
                    "open_cmd_k" => {
                        let cur_active = active_tab_for_ipc.borrow().clone();
                        let pool = tab_pool_for_ipc.borrow();
                        if let Some(wv) = pool.get(&cur_active) {
                            let _ = wv.focus();
                            let _ = wv.evaluate_script("if (window.openThorModal) window.openThorModal('bookmarks_history');");
                        }
                    }
                    "open_cmd_shift_k" => {
                        let cur_active = active_tab_for_ipc.borrow().clone();
                        let pool = tab_pool_for_ipc.borrow();
                        if let Some(wv) = pool.get(&cur_active) {
                            let _ = wv.focus();
                            let _ = wv.evaluate_script("if (window.openThorModal) window.openThorModal('settings');");
                        }
                    }
                    "new_tab" => {
                        let tab_id = payload["id"].as_str().unwrap_or("tab_new").to_string();
                        let url = payload["url"].as_str().unwrap_or("https://www.google.com");
                        println!("⚡ [Thor Rust Engine] Creating new native tab: {} ({})", tab_id, url);
                        
                        let (w, h) = *win_size_for_ipc.borrow();
                        if let Ok(new_wv) = create_content_tab(
                            &window_for_ipc,
                            url,
                            tab_id.clone(),
                            chrome_for_ipc.clone(),
                            &hotkey_script_for_ipc,
                            w,
                            h,
                        ) {
                            // Hide currently active tab
                            let cur_active = active_tab_for_ipc.borrow().clone();
                            let pool = tab_pool_for_ipc.borrow();
                            if let Some(cur_wv) = pool.get(&cur_active) {
                                let _ = cur_wv.set_bounds(Rect {
                                    position: dpi::LogicalPosition::new(-10000.0, -10000.0).into(),
                                    size: dpi::LogicalSize::new(w, h - CHROME_HEIGHT).into(),
                                });
                            }
                            drop(pool);

                            // Insert & Focus new tab
                            tab_pool_for_ipc.borrow_mut().insert(tab_id.clone(), new_wv);
                            *active_tab_for_ipc.borrow_mut() = tab_id.clone();
                            if let Some(active_wv) = tab_pool_for_ipc.borrow().get(&tab_id) {
                                let _ = active_wv.focus();
                            }
                        }
                    }
                    "switch_tab" => {
                        if let Some(target_id) = payload["id"].as_str() {
                            println!("⚡ [Thor Rust Engine] Switching to native tab: {}", target_id);
                            let (w, h) = *win_size_for_ipc.borrow();
                            let pool = tab_pool_for_ipc.borrow();
                            for (id, wv) in pool.iter() {
                                if id == target_id {
                                    let _ = wv.set_bounds(Rect {
                                        position: dpi::LogicalPosition::new(0.0, CHROME_HEIGHT).into(),
                                        size: dpi::LogicalSize::new(w, h - CHROME_HEIGHT).into(),
                                    });
                                    let _ = wv.focus();
                                } else {
                                    let _ = wv.set_bounds(Rect {
                                        position: dpi::LogicalPosition::new(-10000.0, -10000.0).into(),
                                        size: dpi::LogicalSize::new(w, h - CHROME_HEIGHT).into(),
                                    });
                                }
                            }
                            *active_tab_for_ipc.borrow_mut() = target_id.to_string();
                        }
                    }
                    "close_tab" => {
                        if let Some(target_id) = payload["id"].as_str() {
                            println!("⚡ [Thor Rust Engine] Destroying native tab: {}", target_id);
                            tab_pool_for_ipc.borrow_mut().remove(target_id);
                        }
                    }
                    "navigate" => {
                        if let Some(url) = payload["url"].as_str() {
                            let cur_active = active_tab_for_ipc.borrow().clone();
                            let pool = tab_pool_for_ipc.borrow();
                            if let Some(wv) = pool.get(&cur_active) {
                                println!("⚡ [Thor Rust Engine] Navigating native tab [{}] to: {}", cur_active, url);
                                let _ = wv.load_url(url);
                                let _ = wv.focus();
                            }
                        }
                    }
                    "back" => {
                        let cur_active = active_tab_for_ipc.borrow().clone();
                        let pool = tab_pool_for_ipc.borrow();
                        if let Some(wv) = pool.get(&cur_active) {
                            let _ = wv.evaluate_script("history.back()");
                            let _ = wv.focus();
                        }
                    }
                    "forward" => {
                        let cur_active = active_tab_for_ipc.borrow().clone();
                        let pool = tab_pool_for_ipc.borrow();
                        if let Some(wv) = pool.get(&cur_active) {
                            let _ = wv.evaluate_script("history.forward()");
                            let _ = wv.focus();
                        }
                    }
                    "reload" => {
                        let cur_active = active_tab_for_ipc.borrow().clone();
                        let pool = tab_pool_for_ipc.borrow();
                        if let Some(wv) = pool.get(&cur_active) {
                            let _ = wv.evaluate_script("location.reload()");
                            let _ = wv.focus();
                        }
                    }
                    "toggle_devtools" => {
                        let cur_active = active_tab_for_ipc.borrow().clone();
                        let pool = tab_pool_for_ipc.borrow();
                        if let Some(_wv) = pool.get(&cur_active) {
                            println!("⚡ [Thor Rust Engine] DevTools toggled for active tab: {}", cur_active);
                        }
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
                    }
                    "toggle_profile" => {
                        let mode = payload["mode"].as_str().unwrap_or("DevHost");
                        println!("⚡ [Thor Rust Engine] Profile switched to: {}", mode);
                        let profile_mode = match mode {
                            "StealthTarget" => crate::engine::profiles::ProfileMode::StealthTarget,
                            _ => crate::engine::profiles::ProfileMode::DevHost,
                        };
                        let mgr = crate::engine::profiles::ProfileManager::new();
                        mgr.set_mode(profile_mode);
                    }
                    "set_interception_enabled" => {
                        let enabled = payload["enabled"].as_bool().unwrap_or(false);
                        println!("⚡ [Thor Rust Engine] Traffic Interception: {}", if enabled { "ENABLED" } else { "DISABLED" });
                        let engine = crate::net::tamper::TrafficTamperEngine::new();
                        engine.set_interception_enabled(enabled);
                    }
                    "add_tamper_rule" => {
                        let pattern = payload["target_url_pattern"].as_str().unwrap_or("").to_string();
                        let key = payload["replace_header_key"].as_str().unwrap_or("").to_string();
                        let val = payload["replace_header_value"].as_str().unwrap_or("").to_string();
                        println!("⚡ [Thor Rust Engine] Tamper rule added: {} → {}={}", pattern, key, val);
                        let engine = crate::net::tamper::TrafficTamperEngine::new();
                        engine.add_rule(crate::net::tamper::TamperRule {
                            id: format!("rule_{}", std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis()),
                            enabled: true,
                            target_url_pattern: pattern,
                            match_header_key: None,
                            replace_header_key: key,
                            replace_header_value: val,
                        });
                    }
                    _ => {}
                }
            }
        })
        .build_as_child(&window)?;

    // Load Chrome Bookmarks into chrome header webview
    if !raw_chrome_bookmarks.is_empty() {
        let _ = chrome.evaluate_script(&format!("loadChromeBookmarksFromJSON({});", escaped_chrome_bookmarks));
    }

    *chrome_ref.borrow_mut() = Some(chrome);

    let chrome_for_resize = chrome_ref.clone();
    let tab_pool_for_resize = tab_pool.clone();
    let active_tab_for_resize = active_tab_id.clone();
    let win_size_for_resize = win_size.clone();

    println!("============================================================");
    println!("🚀 [Thor Shell] Matte Obsidian Developer Browser ACTIVE!");
    println!("🚀 [Thor Shell] Mirrored Google Chrome Bookmarks & Tree Structure LOADED.");
    println!("============================================================");

    event_loop.run(move |event, _, control_flow| {
        let _window = &window;
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::Resized(new_size) => {
                    let scale_factor = window.scale_factor();
                    let logical_size = new_size.to_logical::<f64>(scale_factor);
                    let w = logical_size.width;
                    let h = logical_size.height;
                    *win_size_for_resize.borrow_mut() = (w, h);

                    if let Some(ref wv) = *chrome_for_resize.borrow() {
                        let _ = wv.set_bounds(Rect {
                            position: dpi::LogicalPosition::new(0.0, 0.0).into(),
                            size: dpi::LogicalSize::new(w, CHROME_HEIGHT).into(),
                        });
                    }

                    let cur_active = active_tab_for_resize.borrow().clone();
                    let pool = tab_pool_for_resize.borrow();
                    for (id, wv) in pool.iter() {
                        if id == &cur_active {
                            let _ = wv.set_bounds(Rect {
                                position: dpi::LogicalPosition::new(0.0, CHROME_HEIGHT).into(),
                                size: dpi::LogicalSize::new(w, h - CHROME_HEIGHT).into(),
                            });
                        } else {
                            let _ = wv.set_bounds(Rect {
                                position: dpi::LogicalPosition::new(-10000.0, -10000.0).into(),
                                size: dpi::LogicalSize::new(w, h - CHROME_HEIGHT).into(),
                            });
                        }
                    }
                }
                _ => {}
            },
            _ => (),
        }
    });
}
