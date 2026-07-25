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
    --bg-chrome: rgba(12, 14, 20, 0.88);
    --bg-panel: rgba(22, 25, 34, 0.7);
    --bg-hover: rgba(255, 255, 255, 0.04);
    --bg-active: rgba(255, 255, 255, 0.06);
    --violet: #a855f7;
    --violet-glow: rgba(168, 85, 247, 0.25);
    --violet-dim: rgba(168, 85, 247, 0.12);
    --cyan: #22d3ee;
    --cyan-dim: rgba(34, 211, 238, 0.1);
    --green: #34d399;
    --orange: #fb923c;
    --red: #f87171;
    --text: #e2e8f0;
    --text-secondary: #94a3b8;
    --text-dim: #475569;
    --border: rgba(255, 255, 255, 0.05);
    --border-active: rgba(168, 85, 247, 0.4);
    --radius: 10px;
    --radius-pill: 20px;
    --transition: 180ms cubic-bezier(0.4, 0, 0.2, 1);
  }

  * { box-sizing: border-box; margin: 0; padding: 0; }

  body {
    font-family: 'Inter', system-ui, -apple-system, sans-serif;
    background: transparent;
    color: var(--text);
    height: 96px;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
  }

  /* === ANIMATED TOP GRADIENT EDGE === */
  .glow-edge {
    height: 2px;
    background: linear-gradient(90deg, 
      #7c3aed, #a855f7, #22d3ee, #34d399, #fb923c, #f87171, #a855f7, #7c3aed);
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
    filter: blur(8px);
    opacity: 0.6;
    transform: translateY(2px);
  }

  @keyframes edgeFlow {
    0%, 100% { background-position: 0% 50%; }
    50% { background-position: 100% 50%; }
  }

  /* === CHROME CONTAINER === */
  .chrome {
    height: 94px;
    background: var(--bg-chrome);
    backdrop-filter: blur(40px) saturate(1.4);
    -webkit-backdrop-filter: blur(40px) saturate(1.4);
    display: flex;
    flex-direction: column;
    position: relative;
    border-bottom: 1px solid var(--border);
  }

  /* Noise texture overlay */
  .chrome::before {
    content: '';
    position: absolute;
    inset: 0;
    opacity: 0.015;
    background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 256 256' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noise'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noise)'/%3E%3C/svg%3E");
    pointer-events: none;
  }

  /* === TAB BAR === */
  .tab-bar {
    height: 40px;
    display: flex;
    align-items: flex-end;
    padding: 0 10px 0 78px; /* macOS traffic lights offset */
    gap: 3px;
    -webkit-app-region: drag;
    position: relative;
    z-index: 2;
  }

  .tab {
    -webkit-app-region: no-drag;
    height: 32px;
    min-width: 48px;
    max-width: 220px;
    background: transparent;
    border: 1px solid transparent;
    border-bottom: none;
    border-radius: 8px 8px 0 0;
    display: flex;
    align-items: center;
    padding: 0 8px;
    gap: 6px;
    cursor: pointer;
    transition: all var(--transition);
    position: relative;
    flex-shrink: 1;
  }

  .tab:hover {
    background: var(--bg-hover);
  }

  .tab.active {
    background: linear-gradient(180deg, rgba(168, 85, 247, 0.08) 0%, rgba(22, 25, 34, 0.5) 100%);
    border-color: var(--border);
  }

  .tab.active::after {
    content: '';
    position: absolute;
    bottom: -1px;
    left: 8px;
    right: 8px;
    height: 2px;
    background: var(--violet);
    border-radius: 2px 2px 0 0;
    box-shadow: 0 0 10px var(--violet-glow), 0 0 20px var(--violet-glow);
  }

  .tab-favicon {
    width: 14px;
    height: 14px;
    border-radius: 3px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    flex-shrink: 0;
    background: var(--violet-dim);
    color: var(--violet);
  }

  .tab.active .tab-favicon {
    background: var(--violet);
    color: white;
    box-shadow: 0 0 8px var(--violet-glow);
  }

  .tab-title {
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-dim);
    letter-spacing: -0.01em;
    transition: color var(--transition);
  }

  .tab.active .tab-title { color: var(--text); font-weight: 600; }
  .tab:hover .tab-title { color: var(--text-secondary); }

  .tab-close {
    opacity: 0;
    width: 16px;
    height: 16px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 13px;
    color: var(--text-dim);
    transition: all var(--transition);
    flex-shrink: 0;
    margin-left: -2px;
  }

  .tab:hover .tab-close, .tab.active .tab-close { opacity: 0.7; }
  .tab-close:hover { opacity: 1 !important; background: rgba(248,113,113,0.15); color: var(--red); transform: scale(1.1); }

  .new-tab-btn {
    -webkit-app-region: no-drag;
    width: 28px;
    height: 28px;
    margin-bottom: 4px;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 16px;
    color: var(--text-dim);
    cursor: pointer;
    transition: all var(--transition);
    flex-shrink: 0;
  }

  .new-tab-btn:hover {
    background: var(--violet-dim);
    color: var(--violet);
    transform: scale(1.08);
  }

  /* === ADDRESS BAR === */
  .address-row {
    height: 48px;
    display: flex;
    align-items: center;
    padding: 0 12px;
    gap: 6px;
    -webkit-app-region: no-drag;
    position: relative;
    z-index: 2;
  }

  .nav-btn {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    border: none;
    background: transparent;
    color: var(--text-dim);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all var(--transition);
    flex-shrink: 0;
  }

  .nav-btn:hover {
    background: var(--bg-hover);
    color: var(--text);
    transform: scale(1.08);
  }

  .nav-btn:active { transform: scale(0.95); }

  .nav-btn svg { width: 15px; height: 15px; fill: currentColor; }

  /* Omnibox — centered pill address bar */
  .omnibox-wrapper {
    flex: 1;
    display: flex;
    justify-content: center;
    min-width: 0;
  }

  .omnibox {
    width: 100%;
    max-width: 720px;
    height: 34px;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    display: flex;
    align-items: center;
    padding: 0 14px;
    gap: 8px;
    transition: all 250ms cubic-bezier(0.4, 0, 0.2, 1);
    position: relative;
    overflow: hidden;
  }

  .omnibox:focus-within {
    border-color: var(--border-active);
    box-shadow: 0 0 0 2px var(--violet-glow), 0 4px 24px rgba(0,0,0,0.3);
    background: rgba(0, 0, 0, 0.4);
    max-width: 780px;
  }

  /* Loading shimmer */
  .omnibox.loading::before {
    content: '';
    position: absolute;
    top: 0; left: -100%; right: 0; bottom: 0;
    background: linear-gradient(90deg, transparent 0%, var(--violet-dim) 50%, transparent 100%);
    animation: shimmer 1.5s ease-in-out infinite;
  }

  @keyframes shimmer {
    0% { transform: translateX(0); }
    100% { transform: translateX(200%); }
  }

  .ssl-badge {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .ssl-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--green);
    box-shadow: 0 0 6px rgba(52, 211, 153, 0.4);
    animation: pulse 2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  .url-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text);
    font-family: 'JetBrains Mono', 'SF Mono', Consolas, monospace;
    font-size: 12.5px;
    font-weight: 400;
    letter-spacing: -0.02em;
    min-width: 0;
  }

  .url-input::placeholder { color: var(--text-dim); }
  .url-input::selection { background: var(--violet-dim); }

  /* Right actions cluster */
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .action-btn {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    border: none;
    background: transparent;
    color: var(--text-dim);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all var(--transition);
    position: relative;
  }

  .action-btn:hover {
    background: var(--bg-hover);
    color: var(--text);
  }

  .action-btn svg { width: 15px; height: 15px; fill: currentColor; }

  .kbd {
    font-size: 10px;
    font-family: 'Inter', system-ui, sans-serif;
    font-weight: 600;
    color: var(--text-dim);
    background: rgba(255,255,255,0.04);
    border: 1px solid var(--border);
    padding: 1px 5px;
    border-radius: 4px;
    letter-spacing: 0.5px;
    pointer-events: none;
    line-height: 16px;
  }

  /* Dev badge (console errors, etc) */
  .dev-badge {
    position: absolute;
    top: 3px;
    right: 3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    font-size: 7px;
    display: none;
    align-items: center;
    justify-content: center;
  }

  .dev-badge.error { display: flex; background: var(--red); box-shadow: 0 0 6px rgba(248,113,113,0.4); }

  /* === BOTTOM MICRO STATUS === */
  .status-strip {
    display: none; /* Hidden by default, shown when relevant */
    height: 6px;
    background: var(--bg-void);
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
  }

  /* === SCROLLBAR === */
  ::-webkit-scrollbar { display: none; }

  /* Smooth all transitions */
  *, *::before, *::after {
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }
</style>
</head>
<body>
  <div class="glow-edge"></div>
  <div class="chrome">
    <div class="tab-bar" id="tabBar">
      <div class="new-tab-btn" onclick="newTab()" title="New Tab (⌘T)">+</div>
    </div>
    <div class="address-row">
      <div class="nav-btn" onclick="ipc('back')" title="Back (⌘[)">
        <svg viewBox="0 0 24 24"><path d="M15.41 16.59L10.83 12l4.58-4.59L14 6l-6 6 6 6z"/></svg>
      </div>
      <div class="nav-btn" onclick="ipc('forward')" title="Forward (⌘])">
        <svg viewBox="0 0 24 24"><path d="M8.59 16.59L13.17 12 8.59 7.41 10 6l6 6-6 6z"/></svg>
      </div>
      <div class="nav-btn" onclick="ipc('reload')" title="Reload (⌘R)">
        <svg viewBox="0 0 24 24"><path d="M17.65 6.35A7.96 7.96 0 0012 4c-4.42 0-8 3.58-8 8s3.58 8 8 8c3.73 0 6.84-2.55 7.73-6h-2.08A5.99 5.99 0 0112 18c-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"/></svg>
      </div>

      <div class="omnibox-wrapper">
        <div class="omnibox" id="omnibox">
          <div class="ssl-badge"><div class="ssl-dot"></div></div>
          <input type="text" class="url-input" id="urlInput" 
                 placeholder="Search or enter URL..."
                 spellcheck="false" autocomplete="off" autocapitalize="off">
        </div>
      </div>

      <div class="actions">
        <span class="kbd">⌘K</span>
        <button class="action-btn" onclick="ipc('toggle_devtools')" title="DevTools (F12)">
          <svg viewBox="0 0 24 24"><path d="M9.4 16.6L4.8 12l4.6-4.6L8 6l-6 6 6 6 1.4-1.4zm5.2 0l4.6-4.6-4.6-4.6L16 6l6 6-6 6-1.4-1.4z"/></svg>
        </button>
      </div>
    </div>
  </div>

<script>
// ==================== TAB STATE ====================
let tabs = [{ id: genId(), title: 'Google', url: 'https://www.google.com', active: true }];

function genId() { return Math.random().toString(36).substr(2, 9); }

// ==================== IPC ====================
function ipc(cmd, data) {
  const payload = Object.assign({ cmd }, data || {});
  if (window.ipc && window.ipc.postMessage) {
    window.ipc.postMessage(JSON.stringify(payload));
  }
}

// ==================== TAB MANAGEMENT ====================
function newTab() {
  tabs.forEach(t => t.active = false);
  tabs.push({ id: genId(), title: 'New Tab', url: 'about:blank', active: true });
  renderTabs();
  ipc('navigate', { url: 'about:blank' });
  document.getElementById('urlInput').value = '';
  setTimeout(() => document.getElementById('urlInput').focus(), 50);
}

function switchTab(idx) {
  if (idx < 0 || idx >= tabs.length || tabs[idx].active) return;
  tabs.forEach(t => t.active = false);
  tabs[idx].active = true;
  renderTabs();
  document.getElementById('urlInput').value = tabs[idx].url === 'about:blank' ? '' : tabs[idx].url;
  ipc('navigate', { url: tabs[idx].url });
}

function closeTab(idx) {
  if (tabs.length <= 1) return; // Don't close last tab
  const wasActive = tabs[idx].active;
  tabs.splice(idx, 1);
  if (wasActive) {
    const newIdx = Math.min(idx, tabs.length - 1);
    tabs[newIdx].active = true;
    document.getElementById('urlInput').value = tabs[newIdx].url === 'about:blank' ? '' : tabs[newIdx].url;
    ipc('navigate', { url: tabs[newIdx].url });
  }
  renderTabs();
}

// ==================== NAVIGATION ====================
const urlInput = document.getElementById('urlInput');

urlInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    e.preventDefault();
    let url = urlInput.value.trim();
    if (!url) return;

    // Smart URL handling
    if (!/^(https?:\/\/|about:|file:\/\/|data:)/.test(url)) {
      if (url.includes('.') && !url.includes(' ')) {
        url = 'https://' + url;
      } else {
        url = 'https://www.google.com/search?q=' + encodeURIComponent(url);
      }
    }

    const activeTab = tabs.find(t => t.active);
    if (activeTab) activeTab.url = url;
    urlInput.value = url;
    ipc('navigate', { url });
    urlInput.blur();
  }
});

urlInput.addEventListener('focus', () => { urlInput.select(); });

// ==================== KEYBOARD SHORTCUTS ====================
document.addEventListener('keydown', (e) => {
  const cmd = e.metaKey || e.ctrlKey;
  if (!cmd) return;

  switch (e.key.toLowerCase()) {
    case 't':
      e.preventDefault(); newTab(); break;
    case 'w':
      e.preventDefault();
      const ai = tabs.findIndex(t => t.active);
      if (ai >= 0) closeTab(ai);
      break;
    case 'l':
      e.preventDefault(); urlInput.focus(); break;
    case 'k':
      e.preventDefault(); urlInput.focus(); urlInput.value = ''; break;
    case '[':
      e.preventDefault(); ipc('back'); break;
    case ']':
      e.preventDefault(); ipc('forward'); break;
    case 'r':
      e.preventDefault(); ipc('reload'); break;
  }

  // Tab switching: Cmd+1..9
  if (e.key >= '1' && e.key <= '9') {
    e.preventDefault();
    const idx = parseInt(e.key) - 1;
    if (idx < tabs.length) switchTab(idx);
  }
});

// ==================== RUST → JS CALLBACKS ====================
window.updateUrl = function(url) {
  const activeTab = tabs.find(t => t.active);
  if (activeTab) activeTab.url = url;
  if (document.activeElement !== urlInput) {
    urlInput.value = url === 'about:blank' ? '' : url;
  }
};

window.updateTitle = function(title) {
  const activeTab = tabs.find(t => t.active);
  if (activeTab && title) {
    activeTab.title = title;
    renderTabs();
  }
};

window.updateTabs = function(data) {
  try {
    tabs = typeof data === 'string' ? JSON.parse(data) : data;
    renderTabs();
  } catch(e) {}
};

// ==================== RENDER TABS ====================
function renderTabs() {
  const bar = document.getElementById('tabBar');
  const btn = bar.querySelector('.new-tab-btn');

  // Remove old tabs
  bar.querySelectorAll('.tab').forEach(el => el.remove());

  tabs.forEach((tab, i) => {
    const el = document.createElement('div');
    el.className = 'tab' + (tab.active ? ' active' : '');
    el.onclick = (e) => { if (!e.target.closest('.tab-close')) switchTab(i); };

    // Favicon
    const fav = document.createElement('div');
    fav.className = 'tab-favicon';
    fav.textContent = getFavicon(tab);

    // Title
    const title = document.createElement('div');
    title.className = 'tab-title';
    title.textContent = tab.title || 'New Tab';
    title.title = tab.title || 'New Tab';

    // Close
    const close = document.createElement('div');
    close.className = 'tab-close';
    close.innerHTML = '×';
    close.onclick = (e) => { e.stopPropagation(); closeTab(i); };

    el.appendChild(fav);
    el.appendChild(title);
    el.appendChild(close);
    bar.insertBefore(el, btn);
  });
}

function getFavicon(tab) {
  if (!tab.url || tab.url === 'about:blank') return '⚡';
  try {
    const host = new URL(tab.url).hostname;
    if (host.includes('google')) return 'G';
    if (host.includes('github')) return '⌥';
    if (host.includes('stackoverflow')) return '⧖';
    if (host.includes('localhost')) return '⚙';
    if (host.includes('youtube')) return '▶';
    if (host.includes('twitter') || host.includes('x.com')) return '𝕏';
    if (host.includes('reddit')) return '⬡';
    return host.charAt(0).toUpperCase();
  } catch { return '⚡'; }
}

// Initial render
renderTabs();
</script>
</body>
</html>"##.to_string()
}
