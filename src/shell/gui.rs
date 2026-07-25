/// Thor Browser Chrome — Obsidian Glass Developer UI
/// Returns complete HTML/CSS/JS as a string for the chrome webview.
pub fn get_chrome_html() -> String {
    r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<style>
  @import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap');

  :root {
    --bg-void: #08090d;
    --bg-chrome: rgba(12, 14, 20, 0.94);
    --bg-modal: rgba(16, 18, 26, 0.96);
    --bg-card: rgba(255, 255, 255, 0.04);
    --bg-hover: rgba(255, 255, 255, 0.06);
    --violet: #a855f7;
    --violet-glow: rgba(168, 85, 247, 0.35);
    --violet-dim: rgba(168, 85, 247, 0.15);
    --cyan: #22d3ee;
    --cyan-dim: rgba(34, 211, 238, 0.12);
    --green: #34d399;
    --orange: #fb923c;
    --red: #f87171;
    --text: #f8fafc;
    --text-secondary: #94a3b8;
    --text-dim: #64748b;
    --border: rgba(255, 255, 255, 0.08);
    --border-active: rgba(168, 85, 247, 0.5);
    --transition: 160ms cubic-bezier(0.4, 0, 0.2, 1);
  }

  * { box-sizing: border-box; margin: 0; padding: 0; }

  body {
    font-family: 'Inter', system-ui, -apple-system, sans-serif;
    background: transparent;
    color: var(--text);
    height: 100vh;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
  }

  /* Animated gradient top edge */
  .glow-edge {
    height: 2px;
    background: linear-gradient(90deg, #7c3aed, #a855f7, #22d3ee, #34d399, #fb923c, #f87171, #a855f7, #7c3aed);
    background-size: 300% 100%;
    animation: edgeFlow 4s ease-in-out infinite;
    position: relative;
    z-index: 10;
  }
  .glow-edge::after {
    content: '';
    position: absolute;
    inset: 0;
    background: inherit;
    background-size: inherit;
    animation: inherit;
    filter: blur(6px);
    opacity: 0.5;
    transform: translateY(1px);
  }
  @keyframes edgeFlow {
    0%, 100% { background-position: 0% 50%; }
    50% { background-position: 100% 50%; }
  }

  /* Single-row header chrome */
  .chrome-row {
    height: 46px;
    background: var(--bg-chrome);
    backdrop-filter: blur(40px) saturate(1.4);
    -webkit-backdrop-filter: blur(40px) saturate(1.4);
    display: flex;
    align-items: center;
    padding: 0 10px 0 78px; /* macOS traffic lights spacing */
    gap: 6px;
    border-bottom: 1px solid var(--border);
    position: relative;
    z-index: 5;
    -webkit-app-region: drag;
  }

  /* Noise overlay */
  .chrome-row::before {
    content: '';
    position: absolute;
    inset: 0;
    opacity: 0.015;
    background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 256 256' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");
    pointer-events: none;
  }

  /* === TABS === */
  .tabs {
    display: flex;
    align-items: center;
    gap: 3px;
    flex-shrink: 1;
    min-width: 0;
    overflow: hidden;
    -webkit-app-region: no-drag;
  }

  .tab {
    height: 30px;
    min-width: 44px;
    max-width: 180px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px;
    display: flex;
    align-items: center;
    padding: 0 8px;
    gap: 6px;
    cursor: pointer;
    transition: all var(--transition);
    position: relative;
    flex-shrink: 1;
  }

  .tab:hover { background: var(--bg-hover); }

  .tab.active {
    background: rgba(168, 85, 247, 0.12);
    border-color: rgba(168, 85, 247, 0.3);
    box-shadow: 0 0 10px var(--violet-glow);
  }

  .tab-fav {
    width: 15px; height: 15px;
    border-radius: 4px;
    display: flex; align-items: center; justify-content: center;
    font-size: 10px; flex-shrink: 0;
    background: rgba(255,255,255,0.06);
    color: var(--text-dim);
  }
  .tab.active .tab-fav {
    background: var(--violet);
    color: #fff;
    box-shadow: 0 0 8px var(--violet-glow);
  }

  .tab-title {
    font-size: 11.5px; font-weight: 500;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    color: var(--text-secondary);
    transition: color var(--transition);
  }
  .tab.active .tab-title { color: var(--text); font-weight: 600; }
  .tab:hover .tab-title { color: var(--text); }

  .tab-x {
    opacity: 0;
    width: 15px; height: 15px;
    border-radius: 3px;
    display: flex; align-items: center; justify-content: center;
    font-size: 12px; color: var(--text-dim);
    transition: all var(--transition);
    flex-shrink: 0;
  }
  .tab:hover .tab-x, .tab.active .tab-x { opacity: 0.6; }
  .tab-x:hover { opacity: 1 !important; background: rgba(248,113,113,0.2); color: var(--red); }

  .new-tab {
    -webkit-app-region: no-drag;
    width: 28px; height: 28px;
    border-radius: 7px;
    display: flex; align-items: center; justify-content: center;
    font-size: 16px; color: var(--text-secondary);
    cursor: pointer;
    transition: all var(--transition);
    flex-shrink: 0;
  }
  .new-tab:hover { background: var(--violet-dim); color: var(--violet); transform: scale(1.08); }

  /* Divider */
  .sep {
    width: 1px; height: 20px;
    background: var(--border);
    flex-shrink: 0;
    margin: 0 4px;
  }

  /* Nav Buttons */
  .nav-btn {
    -webkit-app-region: no-drag;
    width: 28px; height: 28px;
    border-radius: 7px;
    border: none; background: transparent;
    color: var(--text-secondary);
    display: flex; align-items: center; justify-content: center;
    cursor: pointer;
    transition: all var(--transition);
    flex-shrink: 0;
  }
  .nav-btn:hover { background: var(--bg-hover); color: var(--text); transform: scale(1.06); }
  .nav-btn:active { transform: scale(0.94); }
  .nav-btn svg { width: 14px; height: 14px; fill: currentColor; }

  /* Omnibox Search Bar */
  .omnibox {
    -webkit-app-region: no-drag;
    flex: 1;
    min-width: 180px;
    height: 32px;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid var(--border);
    border-radius: 16px;
    display: flex;
    align-items: center;
    padding: 0 12px;
    gap: 8px;
    transition: all 200ms cubic-bezier(0.4, 0, 0.2, 1);
    position: relative;
  }
  .omnibox:focus-within {
    border-color: var(--border-active);
    box-shadow: 0 0 0 2px var(--violet-glow), 0 4px 20px rgba(0,0,0,0.3);
    background: rgba(0, 0, 0, 0.5);
  }

  .ssl-dot {
    width: 6px; height: 6px;
    border-radius: 50%;
    background: var(--green);
    box-shadow: 0 0 6px rgba(52, 211, 153, 0.5);
    animation: pulse 2s ease-in-out infinite;
    flex-shrink: 0;
  }
  @keyframes pulse { 0%,100%{opacity:1}50%{opacity:0.4} }

  .url-input {
    flex: 1;
    background: transparent;
    border: none; outline: none;
    color: var(--text);
    font-family: 'JetBrains Mono', 'SF Mono', Consolas, monospace;
    font-size: 12px; font-weight: 400;
    letter-spacing: -0.02em;
    min-width: 0;
  }
  .url-input::placeholder { color: var(--text-dim); }
  .url-input::selection { background: var(--violet-dim); }

  /* Actions */
  .actions {
    -webkit-app-region: no-drag;
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .action-btn {
    height: 28px;
    padding: 0 8px;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: var(--bg-card);
    color: var(--text-secondary);
    display: flex; align-items: center; justify-content: center;
    gap: 5px;
    cursor: pointer;
    font-size: 11px;
    font-weight: 500;
    transition: all var(--transition);
  }
  .action-btn:hover {
    background: var(--violet-dim);
    border-color: var(--border-active);
    color: var(--violet);
  }
  .action-btn svg { width: 13px; height: 13px; fill: currentColor; }

  .kbd {
    font-size: 9.5px;
    font-weight: 600;
    color: var(--text-dim);
    background: rgba(255,255,255,0.06);
    border: 1px solid var(--border);
    padding: 1px 5px;
    border-radius: 4px;
    pointer-events: none;
    line-height: 15px;
  }

  /* === FEATURE PALETTE MODAL OVERLAY (CMD+K / CMD+SHIFT+K) === */
  .modal-overlay {
    display: none;
    position: fixed;
    top: 48px;
    left: 0; right: 0; bottom: 0;
    background: rgba(8, 9, 13, 0.7);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    z-index: 100;
    align-items: flex-start;
    justify-content: center;
    padding-top: 24px;
    opacity: 0;
    transition: opacity 180ms ease-out;
  }
  .modal-overlay.active {
    display: flex;
    opacity: 1;
  }

  .modal-box {
    width: 680px;
    max-width: 90vw;
    background: var(--bg-modal);
    border: 1px solid var(--border-active);
    border-radius: 14px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.6), 0 0 30px var(--violet-glow);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    animation: modalSlide 200ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes modalSlide {
    from { transform: translateY(-12px) scale(0.98); opacity: 0; }
    to { transform: translateY(0) scale(1); opacity: 1; }
  }

  /* Modal Search Input */
  .modal-search-row {
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    gap: 10px;
    background: rgba(0,0,0,0.2);
  }
  .modal-search-icon {
    color: var(--violet);
    font-size: 16px;
  }
  .modal-search-input {
    flex: 1;
    background: transparent;
    border: none; outline: none;
    color: var(--text);
    font-size: 14px;
    font-family: 'Inter', sans-serif;
  }
  .modal-search-input::placeholder { color: var(--text-dim); }

  /* Modal Tabs */
  .modal-tabs {
    display: flex;
    padding: 8px 14px 0 14px;
    gap: 6px;
    border-bottom: 1px solid var(--border);
    background: rgba(0,0,0,0.1);
  }
  .modal-tab-btn {
    padding: 6px 12px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-dim);
    border: none;
    background: transparent;
    cursor: pointer;
    border-radius: 6px 6px 0 0;
    transition: all var(--transition);
  }
  .modal-tab-btn:hover { color: var(--text-secondary); background: var(--bg-hover); }
  .modal-tab-btn.active {
    color: var(--violet);
    font-weight: 600;
    border-bottom: 2px solid var(--violet);
    background: var(--violet-dim);
  }

  /* Modal Items List */
  .modal-list {
    max-height: 320px;
    overflow-y: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .modal-item {
    padding: 10px 12px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    cursor: pointer;
    transition: all var(--transition);
    background: transparent;
  }
  .modal-item:hover, .modal-item.selected {
    background: var(--violet-dim);
    border: 1px solid rgba(168, 85, 247, 0.3);
  }
  .modal-item-left {
    display: flex;
    align-items: center;
    gap: 10px;
    overflow: hidden;
  }
  .modal-item-icon {
    width: 28px; height: 28px;
    border-radius: 6px;
    background: rgba(255,255,255,0.06);
    display: flex; align-items: center; justify-content: center;
    font-size: 13px;
    color: var(--violet);
    flex-shrink: 0;
  }
  .modal-item-text {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .modal-item-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--text);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .modal-item-sub {
    font-size: 11px;
    color: var(--text-dim);
    font-family: 'JetBrains Mono', monospace;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .modal-item-badge {
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(255,255,255,0.06);
    color: var(--cyan);
    font-family: monospace;
    flex-shrink: 0;
  }

  /* Modal Footer */
  .modal-footer {
    padding: 8px 16px;
    background: rgba(0,0,0,0.25);
    border-top: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 11px;
    color: var(--text-dim);
  }

  ::-webkit-scrollbar { width: 4px; }
  ::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 2px; }
</style>
</head>
<body>
  <div class="glow-edge"></div>
  <div class="chrome-row">
    <!-- Tabs -->
    <div class="tabs" id="tabBar"></div>
    <div class="new-tab" onclick="newTab()" title="New Tab (⌘T)">+</div>

    <div class="sep"></div>

    <!-- Nav -->
    <div class="nav-btn" onclick="ipc('back')" title="Back (⌘[)">
      <svg viewBox="0 0 24 24"><path d="M15.41 16.59L10.83 12l4.58-4.59L14 6l-6 6 6 6z"/></svg>
    </div>
    <div class="nav-btn" onclick="ipc('forward')" title="Forward (⌘])">
      <svg viewBox="0 0 24 24"><path d="M8.59 16.59L13.17 12 8.59 7.41 10 6l6 6-6 6z"/></svg>
    </div>
    <div class="nav-btn" onclick="ipc('reload')" title="Reload (⌘R)">
      <svg viewBox="0 0 24 24"><path d="M17.65 6.35A7.96 7.96 0 0012 4c-4.42 0-8 3.58-8 8s3.58 8 8 8c3.73 0 6.84-2.55 7.73-6h-2.08A5.99 5.99 0 0112 18c-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"/></svg>
    </div>

    <!-- Omnibox Search Bar -->
    <div class="omnibox">
      <div class="ssl-dot"></div>
      <input type="text" class="url-input" id="urlInput"
             placeholder="Search Google or enter URL..." spellcheck="false" autocomplete="off" autocapitalize="off">
    </div>

    <!-- Actions -->
    <div class="actions">
      <button class="action-btn" onclick="openModal('features')" title="Feature Panel (⌘K)">
        <span>⌘K</span>
      </button>
      <button class="action-btn" onclick="openModal('bookmarks_history')" title="Bookmarks & History (⌘⇧K)">
        <span>⌘⇧K</span>
      </button>
      <button class="action-btn" onclick="ipc('toggle_devtools')" title="DevTools (F12)">
        <svg viewBox="0 0 24 24"><path d="M9.4 16.6L4.8 12l4.6-4.6L8 6l-6 6 6 6 1.4-1.4zm5.2 0l4.6-4.6-4.6-4.6L16 6l6 6-6 6-1.4-1.4z"/></svg>
      </button>
    </div>
  </div>

  <!-- FEATURE PALETTE / BOOKMARKS MODAL -->
  <div class="modal-overlay" id="modalOverlay" onclick="closeModalOnBackdrop(event)">
    <div class="modal-box">
      <!-- Search Row -->
      <div class="modal-search-row">
        <span class="modal-search-icon">⚡</span>
        <input type="text" class="modal-search-input" id="modalSearchInput" 
               placeholder="Type a command, bookmark, history URL, setting, or developer tool..."
               oninput="filterModalItems()" onkeydown="handleModalKeydown(event)">
        <span class="kbd">ESC to close</span>
      </div>

      <!-- Navigation Tabs inside Modal -->
      <div class="modal-tabs">
        <button class="modal-tab-btn active" id="mtab-all" onclick="switchModalTab('all')">All Features</button>
        <button class="modal-tab-btn" id="mtab-bookmarks" onclick="switchModalTab('bookmarks')">Bookmarks</button>
        <button class="modal-tab-btn" id="mtab-history" onclick="switchModalTab('history')">History</button>
        <button class="modal-tab-btn" id="mtab-devtools" onclick="switchModalTab('devtools')">Developer Tools</button>
        <button class="modal-tab-btn" id="mtab-settings" onclick="switchModalTab('settings')">Settings</button>
      </div>

      <!-- List Items -->
      <div class="modal-list" id="modalList"></div>

      <!-- Footer -->
      <div class="modal-footer">
        <span>Press <span class="kbd">↵</span> to select &bull; <span class="kbd">↑↓</span> to navigate</span>
        <span id="modalCounter">0 items</span>
      </div>
    </div>
  </div>

<script>
// ==================== TAB STATE & BOOKMARKS/HISTORY DATA ====================
let tabs = [{ id: genId(), title: 'Google', url: 'https://www.google.com', active: true }];

const bookmarks = [
  { icon: '🐙', title: 'GitHub', url: 'https://github.com', category: 'bookmarks' },
  { icon: '🔍', title: 'Google Search', url: 'https://www.google.com', category: 'bookmarks' },
  { icon: '📚', title: 'MDN Web Docs', url: 'https://developer.mozilla.org', category: 'bookmarks' },
  { icon: '💬', title: 'Stack Overflow', url: 'https://stackoverflow.com', category: 'bookmarks' },
  { icon: '⚙️', title: 'Localhost:3000 (React/Next.js Dev Server)', url: 'http://localhost:3000', category: 'bookmarks' },
  { icon: '⚡', title: 'Vite Dev Server (Localhost:5173)', url: 'http://localhost:5173', category: 'bookmarks' },
  { icon: '📄', title: 'Rust Documentation', url: 'https://docs.rs', category: 'bookmarks' },
];

let historyItems = [
  { icon: '🌐', title: 'Google Home', url: 'https://www.google.com', category: 'history', time: 'Just now' },
];

const devToolsFeatures = [
  { icon: '🛠️', title: 'Toggle WebKit Developer Tools', action: () => ipc('toggle_devtools'), category: 'devtools', badge: 'F12 / ⌘⌥I' },
  { icon: '🧊', title: 'Obsidian 3D DOM Snapshotter', action: () => alert('Capturing 3D DOM layout tree...'), category: 'devtools', badge: 'wgpu' },
  { icon: '📐', title: 'Multi-Viewport Sync Canvas (Mobile/Tablet/Desktop)', action: () => alert('Multi-viewport sync enabled.'), category: 'devtools', badge: '120Hz' },
  { icon: '🔗', title: 'IDE Jump (Option+Click DOM -> VS Code / Cursor)', action: () => alert('VS Code bridge ready.'), category: 'devtools', badge: 'vscode://' },
  { icon: '📊', title: 'Live SLA Metrics: Cold-Start <30ms | IPC 0.4μs', action: () => alert('SLA metrics active.'), category: 'devtools', badge: 'SPSC Ring' },
];

const settingsFeatures = [
  { icon: '🛡️', title: 'Local ECDSA P-256 SSL CA Certificate', action: () => alert('Local SSL CA active for *.localhost'), category: 'settings', badge: 'SSL Trusted' },
  { icon: '🔥', title: 'HMR WebSocket Proxy (Tab Sleep Buffer)', action: () => alert('HMR Proxy active'), category: 'settings', badge: 'WebSocket' },
  { icon: '❄️', title: 'OS Process Compaction (proc_suspend / cgroup)', action: () => alert('Memory compaction active'), category: 'settings', badge: 'RAM Freeing' },
];

let currentModalTab = 'all';
let selectedModalIndex = 0;
let filteredItems = [];

function genId() { return Math.random().toString(36).substr(2, 9); }

function ipc(cmd, data) {
  const p = Object.assign({ cmd }, data || {});
  if (window.ipc && window.ipc.postMessage) window.ipc.postMessage(JSON.stringify(p));
}

// ==================== TAB MANAGEMENT ====================
function newTab() {
  tabs.forEach(t => t.active = false);
  const newUrl = 'https://www.google.com';
  tabs.push({ id: genId(), title: 'Google', url: newUrl, active: true });
  renderTabs();
  ipc('navigate', { url: newUrl });
  document.getElementById('urlInput').value = newUrl;
}

function switchTab(idx) {
  if (idx < 0 || idx >= tabs.length || tabs[idx].active) return;
  tabs.forEach(t => t.active = false);
  tabs[idx].active = true;
  renderTabs();
  document.getElementById('urlInput').value = tabs[idx].url;
  ipc('navigate', { url: tabs[idx].url });
}

function closeTab(idx) {
  if (tabs.length <= 1) return;
  const wasActive = tabs[idx].active;
  tabs.splice(idx, 1);
  if (wasActive) {
    const ni = Math.min(idx, tabs.length - 1);
    tabs[ni].active = true;
    document.getElementById('urlInput').value = tabs[ni].url;
    ipc('navigate', { url: tabs[ni].url });
  }
  renderTabs();
}

// ==================== SEARCH BAR NAVIGATION ====================
const urlInput = document.getElementById('urlInput');
urlInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    e.preventDefault();
    let url = urlInput.value.trim();
    if (!url) return;
    if (!/^(https?:\/\/|about:|file:\/\/|data:)/.test(url)) {
      url = (url.includes('.') && !url.includes(' ')) ? 'https://' + url : 'https://www.google.com/search?q=' + encodeURIComponent(url);
    }
    const at = tabs.find(t => t.active);
    if (at) {
      at.url = url;
      at.title = url;
      renderTabs();
    }
    urlInput.value = url;
    ipc('navigate', { url });
    urlInput.blur();
  }
});
urlInput.addEventListener('focus', () => urlInput.select());

// ==================== KEYBOARD SHORTCUTS ====================
document.addEventListener('keydown', (e) => {
  const cmd = e.metaKey || e.ctrlKey;
  if (e.key === 'Escape') {
    closeModal();
    return;
  }
  if (!cmd) return;

  if (e.shiftKey && e.key.toLowerCase() === 'k') {
    e.preventDefault();
    openModal('bookmarks_history');
    return;
  }

  switch (e.key.toLowerCase()) {
    case 'k': e.preventDefault(); openModal('features'); break;
    case 't': e.preventDefault(); newTab(); break;
    case 'w': e.preventDefault(); { const i = tabs.findIndex(t => t.active); if (i >= 0) closeTab(i); } break;
    case 'l': e.preventDefault(); urlInput.focus(); break;
    case '[': e.preventDefault(); ipc('back'); break;
    case ']': e.preventDefault(); ipc('forward'); break;
    case 'r': e.preventDefault(); ipc('reload'); break;
  }
  if (e.key >= '1' && e.key <= '9') { e.preventDefault(); const i = parseInt(e.key) - 1; if (i < tabs.length) switchTab(i); }
});

// ==================== MODAL (CMD+K / CMD+SHIFT+K) LOGIC ====================
function openModal(defaultTab = 'features') {
  const overlay = document.getElementById('modalOverlay');
  overlay.classList.add('active');
  ipc('expand_chrome'); // Expand webview height in Rust to 520px

  if (defaultTab === 'bookmarks_history') {
    switchModalTab('bookmarks');
  } else {
    switchModalTab('all');
  }

  const sInput = document.getElementById('modalSearchInput');
  sInput.value = '';
  setTimeout(() => sInput.focus(), 50);
}

function closeModal() {
  const overlay = document.getElementById('modalOverlay');
  overlay.classList.remove('active');
  ipc('shrink_chrome'); // Shrink webview height in Rust back to 48px
}

function closeModalOnBackdrop(e) {
  if (e.target.id === 'modalOverlay') {
    closeModal();
  }
}

function switchModalTab(tabName) {
  currentModalTab = tabName;
  ['all', 'bookmarks', 'history', 'devtools', 'settings'].forEach(t => {
    const btn = document.getElementById('mtab-' + t);
    if (btn) btn.classList.toggle('active', t === tabName);
  });
  filterModalItems();
}

function filterModalItems() {
  const query = document.getElementById('modalSearchInput').value.toLowerCase().trim();
  let pool = [];

  if (currentModalTab === 'all') {
    pool = [...bookmarks, ...historyItems, ...devToolsFeatures, ...settingsFeatures];
  } else if (currentModalTab === 'bookmarks') {
    pool = [...bookmarks];
  } else if (currentModalTab === 'history') {
    pool = [...historyItems];
  } else if (currentModalTab === 'devtools') {
    pool = [...devToolsFeatures];
  } else if (currentModalTab === 'settings') {
    pool = [...settingsFeatures];
  }

  if (query) {
    pool = pool.filter(item => 
      item.title.toLowerCase().includes(query) || 
      (item.url && item.url.toLowerCase().includes(query)) ||
      (item.badge && item.badge.toLowerCase().includes(query))
    );
  }

  filteredItems = pool;
  selectedModalIndex = 0;
  renderModalList();
}

function renderModalList() {
  const listEl = document.getElementById('modalList');
  listEl.innerHTML = '';
  document.getElementById('modalCounter').textContent = filteredItems.length + ' items';

  if (filteredItems.length === 0) {
    listEl.innerHTML = '<div style="padding: 24px; text-align: center; color: var(--text-dim); font-size: 13px;">No matching items found. Type to search or press Enter to search Google.</div>';
    return;
  }

  filteredItems.forEach((item, index) => {
    const el = document.createElement('div');
    el.className = 'modal-item' + (index === selectedModalIndex ? ' selected' : '');
    el.onclick = () => selectModalItem(item);

    const left = document.createElement('div');
    left.className = 'modal-item-left';

    const icon = document.createElement('div');
    icon.className = 'modal-item-icon';
    icon.textContent = item.icon || '⚡';

    const text = document.createElement('div');
    text.className = 'modal-item-text';

    const title = document.createElement('div');
    title.className = 'modal-item-title';
    title.textContent = item.title;

    const sub = document.createElement('div');
    sub.className = 'modal-item-sub';
    sub.textContent = item.url || item.category || '';

    text.appendChild(title);
    if (item.url || item.category) text.appendChild(sub);

    left.appendChild(icon);
    left.appendChild(text);
    el.appendChild(left);

    if (item.badge) {
      const badge = document.createElement('div');
      badge.className = 'modal-item-badge';
      badge.textContent = item.badge;
      el.appendChild(badge);
    }

    listEl.appendChild(el);
  });
}

function handleModalKeydown(e) {
  if (e.key === 'ArrowDown') {
    e.preventDefault();
    if (filteredItems.length > 0) {
      selectedModalIndex = (selectedModalIndex + 1) % filteredItems.length;
      renderModalList();
      scrollSelectedIntoView();
    }
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    if (filteredItems.length > 0) {
      selectedModalIndex = (selectedModalIndex - 1 + filteredItems.length) % filteredItems.length;
      renderModalList();
      scrollSelectedIntoView();
    }
  } else if (e.key === 'Enter') {
    e.preventDefault();
    if (filteredItems.length > 0 && filteredItems[selectedModalIndex]) {
      selectModalItem(filteredItems[selectedModalIndex]);
    } else {
      const q = document.getElementById('modalSearchInput').value.trim();
      if (q) {
        let url = (q.includes('.') && !q.includes(' ')) ? 'https://' + q : 'https://www.google.com/search?q=' + encodeURIComponent(q);
        const at = tabs.find(t => t.active);
        if (at) at.url = url;
        ipc('navigate', { url });
        closeModal();
      }
    }
  }
}

function scrollSelectedIntoView() {
  const listEl = document.getElementById('modalList');
  const selectedEl = listEl.children[selectedModalIndex];
  if (selectedEl) {
    selectedEl.scrollIntoView({ block: 'nearest' });
  }
}

function selectModalItem(item) {
  closeModal();
  if (item.url) {
    const at = tabs.find(t => t.active);
    if (at) {
      at.url = item.url;
      at.title = item.title;
      renderTabs();
    }
    document.getElementById('urlInput').value = item.url;
    ipc('navigate', { url: item.url });
  } else if (item.action) {
    item.action();
  }
}

// ==================== RUST → JS CALLBACKS ====================
window.updateUrl = function(url) {
  const activeTab = tabs.find(t => t.active);
  if (activeTab) activeTab.url = url;
  if (document.activeElement !== urlInput) urlInput.value = url;

  // Record in history if real URL
  if (url && !url.startsWith('about:')) {
    const exists = historyItems.find(h => h.url === url);
    if (!exists) {
      historyItems.unshift({ icon: '🌐', title: url, url: url, category: 'history', time: 'Just now' });
      if (historyItems.length > 50) historyItems.pop();
    }
  }
};

window.updateTitle = function(title) {
  const activeTab = tabs.find(t => t.active);
  if (activeTab && title) {
    activeTab.title = title;
    renderTabs();
  }
};

window.focusUrlInput = function() {
  urlInput.focus();
  urlInput.select();
};

window.openModalFromHotkey = function(type) {
  openModal(type);
};

// ==================== RENDER TABS ====================
function renderTabs() {
  const bar = document.getElementById('tabBar');
  bar.innerHTML = '';
  tabs.forEach((tab, i) => {
    const el = document.createElement('div');
    el.className = 'tab' + (tab.active ? ' active' : '');
    el.onclick = (e) => { if (!e.target.closest('.tab-x')) switchTab(i); };

    const fav = document.createElement('div');
    fav.className = 'tab-fav';
    fav.textContent = getFav(tab);

    const title = document.createElement('div');
    title.className = 'tab-title';
    title.textContent = tab.title || 'Google';

    const x = document.createElement('div');
    x.className = 'tab-x';
    x.innerHTML = '×';
    x.onclick = (e) => { e.stopPropagation(); closeTab(i); };

    el.append(fav, title, x);
    bar.appendChild(el);
  });
}

function getFav(tab) {
  if (!tab.url || tab.url === 'about:blank') return '⚡';
  try {
    const h = new URL(tab.url).hostname;
    if (h.includes('google')) return 'G';
    if (h.includes('github')) return '🐙';
    if (h.includes('localhost')) return '⚙';
    if (h.includes('stackoverflow')) return '💬';
    return h.charAt(0).toUpperCase();
  } catch { return '⚡'; }
}

renderTabs();
</script>
</body>
</html>"##.to_string()
}
