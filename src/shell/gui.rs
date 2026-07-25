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
    --bg-chrome: rgba(12, 14, 20, 0.92);
    --bg-hover: rgba(255, 255, 255, 0.05);
    --violet: #a855f7;
    --violet-glow: rgba(168, 85, 247, 0.25);
    --violet-dim: rgba(168, 85, 247, 0.12);
    --cyan: #22d3ee;
    --green: #34d399;
    --red: #f87171;
    --text: #e2e8f0;
    --text-secondary: #94a3b8;
    --text-dim: #475569;
    --border: rgba(255, 255, 255, 0.05);
    --border-active: rgba(168, 85, 247, 0.4);
    --transition: 160ms cubic-bezier(0.4, 0, 0.2, 1);
  }

  * { box-sizing: border-box; margin: 0; padding: 0; }

  body {
    font-family: 'Inter', system-ui, -apple-system, sans-serif;
    background: transparent;
    color: var(--text);
    height: 48px;
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

  /* Single-row chrome */
  .chrome-row {
    height: 46px;
    background: var(--bg-chrome);
    backdrop-filter: blur(40px) saturate(1.4);
    -webkit-backdrop-filter: blur(40px) saturate(1.4);
    display: flex;
    align-items: center;
    padding: 0 10px 0 78px; /* macOS traffic lights */
    gap: 6px;
    border-bottom: 1px solid var(--border);
    position: relative;
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
    gap: 2px;
    flex-shrink: 1;
    min-width: 0;
    overflow: hidden;
    -webkit-app-region: no-drag;
  }

  .tab {
    height: 30px;
    min-width: 36px;
    max-width: 180px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px;
    display: flex;
    align-items: center;
    padding: 0 8px;
    gap: 5px;
    cursor: pointer;
    transition: all var(--transition);
    position: relative;
    flex-shrink: 1;
  }

  .tab:hover { background: var(--bg-hover); }

  .tab.active {
    background: rgba(168, 85, 247, 0.08);
    border-color: rgba(168, 85, 247, 0.2);
    box-shadow: 0 0 8px var(--violet-glow);
  }

  .tab-fav {
    width: 14px; height: 14px;
    border-radius: 3px;
    display: flex; align-items: center; justify-content: center;
    font-size: 9px; flex-shrink: 0;
    background: rgba(255,255,255,0.05);
    color: var(--text-dim);
  }
  .tab.active .tab-fav {
    background: var(--violet);
    color: #fff;
    box-shadow: 0 0 6px var(--violet-glow);
  }

  .tab-title {
    font-size: 11.5px; font-weight: 500;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    color: var(--text-dim);
    transition: color var(--transition);
  }
  .tab.active .tab-title { color: var(--text); font-weight: 600; }
  .tab:hover .tab-title { color: var(--text-secondary); }

  .tab-x {
    opacity: 0;
    width: 14px; height: 14px;
    border-radius: 3px;
    display: flex; align-items: center; justify-content: center;
    font-size: 12px; color: var(--text-dim);
    transition: all var(--transition);
    flex-shrink: 0;
  }
  .tab:hover .tab-x, .tab.active .tab-x { opacity: 0.6; }
  .tab-x:hover { opacity: 1 !important; background: rgba(248,113,113,0.15); color: var(--red); }

  .new-tab {
    -webkit-app-region: no-drag;
    width: 26px; height: 26px;
    border-radius: 6px;
    display: flex; align-items: center; justify-content: center;
    font-size: 15px; color: var(--text-dim);
    cursor: pointer;
    transition: all var(--transition);
    flex-shrink: 0;
  }
  .new-tab:hover { background: var(--violet-dim); color: var(--violet); transform: scale(1.08); }

  /* Divider between tabs and nav */
  .sep {
    width: 1px;
    height: 20px;
    background: var(--border);
    flex-shrink: 0;
    margin: 0 4px;
  }

  /* === NAV BUTTONS === */
  .nav-btn {
    -webkit-app-region: no-drag;
    width: 28px; height: 28px;
    border-radius: 6px;
    border: none; background: transparent;
    color: var(--text-dim);
    display: flex; align-items: center; justify-content: center;
    cursor: pointer;
    transition: all var(--transition);
    flex-shrink: 0;
  }
  .nav-btn:hover { background: var(--bg-hover); color: var(--text); transform: scale(1.06); }
  .nav-btn:active { transform: scale(0.94); }
  .nav-btn svg { width: 14px; height: 14px; fill: currentColor; }

  /* === OMNIBOX === */
  .omnibox {
    -webkit-app-region: no-drag;
    flex: 1;
    min-width: 180px;
    height: 30px;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid var(--border);
    border-radius: 16px;
    display: flex;
    align-items: center;
    padding: 0 12px;
    gap: 7px;
    transition: all 200ms cubic-bezier(0.4, 0, 0.2, 1);
    position: relative;
    overflow: hidden;
  }
  .omnibox:focus-within {
    border-color: var(--border-active);
    box-shadow: 0 0 0 2px var(--violet-glow), 0 2px 16px rgba(0,0,0,0.25);
    background: rgba(0, 0, 0, 0.4);
  }

  .ssl-dot {
    width: 6px; height: 6px;
    border-radius: 50%;
    background: var(--green);
    box-shadow: 0 0 5px rgba(52, 211, 153, 0.4);
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

  /* === ACTIONS === */
  .actions {
    -webkit-app-region: no-drag;
    display: flex;
    align-items: center;
    gap: 3px;
    flex-shrink: 0;
  }

  .action-btn {
    width: 28px; height: 28px;
    border-radius: 6px;
    border: none; background: transparent;
    color: var(--text-dim);
    display: flex; align-items: center; justify-content: center;
    cursor: pointer;
    transition: all var(--transition);
  }
  .action-btn:hover { background: var(--bg-hover); color: var(--text); }
  .action-btn svg { width: 14px; height: 14px; fill: currentColor; }

  .kbd {
    font-size: 9.5px;
    font-weight: 600;
    color: var(--text-dim);
    background: rgba(255,255,255,0.04);
    border: 1px solid var(--border);
    padding: 1px 5px;
    border-radius: 4px;
    pointer-events: none;
    line-height: 15px;
  }

  ::-webkit-scrollbar { display: none; }
  *, *::before, *::after {
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }
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
    <div class="nav-btn" onclick="ipc('back')" title="Back">
      <svg viewBox="0 0 24 24"><path d="M15.41 16.59L10.83 12l4.58-4.59L14 6l-6 6 6 6z"/></svg>
    </div>
    <div class="nav-btn" onclick="ipc('forward')" title="Forward">
      <svg viewBox="0 0 24 24"><path d="M8.59 16.59L13.17 12 8.59 7.41 10 6l6 6-6 6z"/></svg>
    </div>
    <div class="nav-btn" onclick="ipc('reload')" title="Reload">
      <svg viewBox="0 0 24 24"><path d="M17.65 6.35A7.96 7.96 0 0012 4c-4.42 0-8 3.58-8 8s3.58 8 8 8c3.73 0 6.84-2.55 7.73-6h-2.08A5.99 5.99 0 0112 18c-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"/></svg>
    </div>

    <!-- Omnibox -->
    <div class="omnibox">
      <div class="ssl-dot"></div>
      <input type="text" class="url-input" id="urlInput"
             placeholder="Search or enter URL..." spellcheck="false" autocomplete="off" autocapitalize="off">
    </div>

    <!-- Actions -->
    <div class="actions">
      <span class="kbd">⌘K</span>
      <button class="action-btn" onclick="ipc('toggle_devtools')" title="DevTools">
        <svg viewBox="0 0 24 24"><path d="M9.4 16.6L4.8 12l4.6-4.6L8 6l-6 6 6 6 1.4-1.4zm5.2 0l4.6-4.6-4.6-4.6L16 6l6 6-6 6-1.4-1.4z"/></svg>
      </button>
    </div>
  </div>

<script>
let tabs = [{ id: genId(), title: 'Google', url: 'https://www.google.com', active: true }];
function genId() { return Math.random().toString(36).substr(2, 9); }

function ipc(cmd, data) {
  const p = Object.assign({ cmd }, data || {});
  if (window.ipc && window.ipc.postMessage) window.ipc.postMessage(JSON.stringify(p));
}

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
  if (tabs.length <= 1) return;
  const wasActive = tabs[idx].active;
  tabs.splice(idx, 1);
  if (wasActive) {
    const ni = Math.min(idx, tabs.length - 1);
    tabs[ni].active = true;
    document.getElementById('urlInput').value = tabs[ni].url === 'about:blank' ? '' : tabs[ni].url;
    ipc('navigate', { url: tabs[ni].url });
  }
  renderTabs();
}

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
    if (at) at.url = url;
    urlInput.value = url;
    ipc('navigate', { url });
    urlInput.blur();
  }
});
urlInput.addEventListener('focus', () => urlInput.select());

document.addEventListener('keydown', (e) => {
  const cmd = e.metaKey || e.ctrlKey;
  if (!cmd) return;
  switch (e.key.toLowerCase()) {
    case 't': e.preventDefault(); newTab(); break;
    case 'w': e.preventDefault(); { const i = tabs.findIndex(t => t.active); if (i >= 0) closeTab(i); } break;
    case 'l': e.preventDefault(); urlInput.focus(); break;
    case 'k': e.preventDefault(); urlInput.focus(); urlInput.value = ''; break;
    case '[': e.preventDefault(); ipc('back'); break;
    case ']': e.preventDefault(); ipc('forward'); break;
    case 'r': e.preventDefault(); ipc('reload'); break;
  }
  if (e.key >= '1' && e.key <= '9') { e.preventDefault(); const i = parseInt(e.key) - 1; if (i < tabs.length) switchTab(i); }
});

window.updateUrl = function(url) {
  const at = tabs.find(t => t.active);
  if (at) at.url = url;
  if (document.activeElement !== urlInput) urlInput.value = (url === 'about:blank') ? '' : url;
};
window.updateTitle = function(title) {
  const at = tabs.find(t => t.active);
  if (at && title) { at.title = title; renderTabs(); }
};
window.updateTabs = function(data) {
  try { tabs = typeof data === 'string' ? JSON.parse(data) : data; renderTabs(); } catch(e) {}
};

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
    title.textContent = tab.title || 'New Tab';

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
    if (h.includes('github')) return '⌥';
    if (h.includes('localhost')) return '⚙';
    return h.charAt(0).toUpperCase();
  } catch { return '⚡'; }
}

renderTabs();
</script>
</body>
</html>"##.to_string()
}
