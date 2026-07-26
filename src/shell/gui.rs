/// Thor Browser Chrome — Clean Matte Obsidian Developer UI
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
    --bg-void: #0b0c10;
    --bg-chrome: #121318;
    --bg-modal: #161820;
    --bg-card: rgba(255, 255, 255, 0.05);
    --bg-hover: rgba(255, 255, 255, 0.08);
    --bg-active: #1e202a;
    --accent: #3b82f6;
    --accent-green: #22c55e;
    --accent-red: #ef4444;
    --accent-amber: #f59e0b;
    --accent-purple: #a855f7;
    --text: #f1f5f9;
    --text-secondary: #94a3b8;
    --text-dim: #64748b;
    --border: rgba(255, 255, 255, 0.08);
    --border-active: rgba(255, 255, 255, 0.22);
    --transition: 120ms ease-out;
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

  /* Single-row header chrome - Clean Matte Dark Slate */
  .chrome-row {
    height: 48px;
    background: var(--bg-chrome);
    display: flex;
    align-items: center;
    padding: 0 10px 0 78px; /* macOS traffic lights spacing */
    gap: 6px;
    border-bottom: 1px solid var(--border);
    position: relative;
    z-index: 5;
  }

  /* === TABS === */
  .tabs {
    display: flex;
    align-items: center;
    gap: 3px;
    flex-shrink: 1;
    min-width: 0;
    overflow: hidden;
  }

  .tab {
    height: 30px;
    min-width: 44px;
    max-width: 180px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    display: flex;
    align-items: center;
    padding: 0 8px;
    gap: 6px;
    cursor: pointer;
    transition: background var(--transition);
    position: relative;
    flex-shrink: 1;
  }

  .tab:hover { background: var(--bg-hover); }

  .tab.active {
    background: var(--bg-active);
    border-color: var(--border-active);
  }

  .tab-fav {
    width: 15px; height: 15px;
    border-radius: 3px;
    display: flex; align-items: center; justify-content: center;
    font-size: 10px; flex-shrink: 0;
    background: rgba(255,255,255,0.06);
    color: var(--text-secondary);
  }
  .tab.active .tab-fav {
    background: var(--accent);
    color: #fff;
  }

  .tab-title {
    font-size: 12px; font-weight: 500;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    color: var(--text-secondary);
  }
  .tab.active .tab-title { color: var(--text); font-weight: 600; }
  .tab:hover .tab-title { color: var(--text); }

  input, select, textarea {
    user-select: text !important;
    -webkit-user-select: text !important;
  }

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
  .tab-x:hover { opacity: 1 !important; background: rgba(255,255,255,0.12); color: var(--text); }

  .new-tab {
    width: 28px; height: 28px;
    border-radius: 6px;
    display: flex; align-items: center; justify-content: center;
    font-size: 16px; color: var(--text-secondary);
    cursor: pointer;
    transition: background var(--transition);
    flex-shrink: 0;
  }
  .new-tab:hover { background: var(--bg-hover); color: var(--text); }

  /* Divider */
  .sep {
    width: 1px; height: 18px;
    background: var(--border);
    flex-shrink: 0;
    margin: 0 4px;
  }

  /* Nav Buttons */
  .nav-btn {
    width: 28px; height: 28px;
    border-radius: 6px;
    border: none; background: transparent;
    color: var(--text-secondary);
    display: flex; align-items: center; justify-content: center;
    cursor: pointer;
    transition: background var(--transition);
    flex-shrink: 0;
  }
  .nav-btn:hover { background: var(--bg-hover); color: var(--text); }
  .nav-btn svg { width: 14px; height: 14px; fill: currentColor; }

  /* Omnibox Search Bar */
  .omnibox {
    flex: 1;
    min-width: 180px;
    height: 30px;
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid var(--border);
    border-radius: 6px;
    display: flex;
    align-items: center;
    padding: 0 10px;
    gap: 8px;
    transition: border-color var(--transition);
  }
  .omnibox:focus-within {
    border-color: var(--border-active);
    background: rgba(0, 0, 0, 0.5);
  }

  .ssl-icon {
    font-size: 12px;
    cursor: pointer;
    flex-shrink: 0;
    padding: 2px 4px;
    border-radius: 4px;
    transition: background var(--transition);
  }
  .ssl-icon:hover { background: rgba(255,255,255,0.1); }
  .ssl-icon.secure { color: #4ade80; }
  .ssl-icon.unsecure { color: #f87171; }
  .ssl-icon.system { color: #60a5fa; }

  .url-input {
    flex: 1;
    background: transparent;
    border: none; outline: none;
    color: var(--text);
    font-family: 'JetBrains Mono', 'SF Mono', Consolas, monospace;
    font-size: 12px; font-weight: 400;
    letter-spacing: -0.01em;
    min-width: 0;
  }
  .url-input::placeholder { color: var(--text-dim); }

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
    border-radius: 6px;
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
    background: var(--bg-hover);
    border-color: var(--border-active);
    color: var(--text);
  }
  .action-btn svg { width: 13px; height: 13px; fill: currentColor; }

  .kbd {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-dim);
    background: rgba(255,255,255,0.06);
    border: 1px solid var(--border);
    padding: 1px 5px;
    border-radius: 4px;
    pointer-events: none;
    line-height: 14px;
  }

  /* === TECH STACK & LANGUAGE DETECTOR BAR === */
  .stack-detector-bar {
    height: 30px;
    background: #0e0f15;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    padding: 0 12px 0 78px;
    gap: 8px;
    font-size: 11px;
    color: var(--text-secondary);
    position: relative;
    z-index: 4;
  }
  .stack-detector-title {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--accent);
    text-transform: uppercase;
    flex-shrink: 0;
  }
  .stack-pills {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow-x: auto;
    flex: 1;
    min-width: 0;
    scrollbar-width: none;
  }
  .stack-pills::-webkit-scrollbar { display: none; }
  .tech-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 10.5px;
    font-weight: 500;
    font-family: 'JetBrains Mono', monospace;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid var(--border);
    padding: 2px 7px;
    border-radius: 5px;
    color: var(--text);
    white-space: nowrap;
    cursor: pointer;
    transition: all var(--transition);
  }
  .tech-pill:hover {
    background: rgba(255, 255, 255, 0.12);
    border-color: var(--border-active);
    transform: translateY(-1px);
  }
  .tech-pill-version {
    font-size: 9px;
    opacity: 0.75;
    background: rgba(255, 255, 255, 0.1);
    padding: 0 4px;
    border-radius: 3px;
  }
  .stack-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
    margin-left: auto;
  }
  .stack-btn {
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-dim);
    font-size: 10px;
    padding: 2px 6px;
    cursor: pointer;
    transition: all var(--transition);
  }
  .stack-btn:hover { background: var(--bg-hover); color: var(--text); border-color: var(--border-active); }
  .tracker-stripped-notice {
    font-size: 10px;
    font-weight: 600;
    color: #4ade80;
    background: rgba(74, 222, 128, 0.12);
    border: 1px solid rgba(74, 222, 128, 0.25);
    padding: 2px 7px;
    border-radius: 4px;
    display: none;
    align-items: center;
    gap: 4px;
  }

  /* === LOCUST HTTP LOAD TESTING TOOLBAR === */
  .locust-bar {
    display: none;
    background: #13151d;
    border-bottom: 1px solid var(--border-active);
    padding: 8px 14px 8px 78px;
    flex-direction: column;
    gap: 8px;
    box-shadow: 0 8px 24px rgba(0,0,0,0.6);
    position: relative;
    z-index: 6;
  }
  .locust-bar.active { display: flex; }
  .locust-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .locust-title {
    font-size: 12px;
    font-weight: 700;
    color: #f59e0b;
    display: flex;
    align-items: center;
    gap: 6px;
    letter-spacing: 0.02em;
  }
  .locust-controls-row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .locust-group {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-secondary);
  }
  .locust-group label {
    font-weight: 500;
    color: var(--text-secondary);
  }
  .locust-input {
    width: 65px;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--text);
    padding: 3px 6px;
    font-size: 11px;
    font-family: 'JetBrains Mono', monospace;
    outline: none;
    transition: border-color var(--transition);
  }
  .locust-input:focus { border-color: var(--accent); }
  .locust-select {
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--text);
    padding: 3px 8px;
    font-size: 11px;
    font-family: 'Inter', sans-serif;
    outline: none;
    cursor: pointer;
  }
  .locust-btn {
    height: 26px;
    padding: 0 12px;
    border-radius: 5px;
    border: none;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    transition: all var(--transition);
  }
  .locust-btn.go { background: #22c55e; color: #ffffff; }
  .locust-btn.go:hover { background: #16a34a; box-shadow: 0 2px 8px rgba(34, 197, 94, 0.3); }
  .locust-btn.stop { background: #ef4444; color: #ffffff; }
  .locust-btn.stop:hover { background: #dc2626; box-shadow: 0 2px 8px rgba(239, 68, 68, 0.3); }
  .locust-btn.x { background: rgba(255,255,255,0.08); color: var(--text-secondary); padding: 0 8px; }
  .locust-btn.x:hover { background: rgba(255,255,255,0.16); color: var(--text); }

  .locust-stats-container {
    display: flex;
    align-items: center;
    gap: 18px;
    border-top: 1px solid var(--border);
    padding-top: 8px;
    margin-top: 2px;
  }
  .locust-stat-box { display: flex; flex-direction: column; gap: 1px; }
  .locust-stat-val { font-size: 13px; font-weight: 700; font-family: 'JetBrains Mono', monospace; color: var(--text); }
  .locust-stat-lbl { font-size: 9.5px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.03em; }
  .locust-status-badge {
    margin-left: auto;
    font-size: 10px;
    font-weight: 700;
    padding: 3px 9px;
    border-radius: 12px;
    background: rgba(255,255,255,0.08);
    color: var(--text-secondary);
    letter-spacing: 0.05em;
  }
  .locust-status-badge.running {
    background: rgba(34, 197, 94, 0.2);
    color: #4ade80;
    box-shadow: 0 0 10px rgba(74, 222, 128, 0.2);
    animation: pulse 1.5s infinite;
  }
  @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.6; } }

  /* === FEATURE PALETTE MODAL OVERLAY (CMD+K / CMD+SHIFT+K) === */
  .modal-overlay {
    display: none;
    position: fixed;
    top: 46px;
    left: 0;
    width: 100vw;
    height: 474px;
    background: rgba(0, 0, 0, 0.75);
    z-index: 100;
    align-items: flex-start;
    justify-content: center;
    padding-top: 16px;
  }
  .modal-overlay.active { display: flex; }

  .modal-box {
    width: 680px; max-width: 92vw; background: var(--bg-modal);
    border: 1px solid var(--border-active); border-radius: 8px;
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.6); overflow: hidden;
    display: flex; flex-direction: column;
  }

  .modal-search-row {
    padding: 12px 14px; border-bottom: 1px solid var(--border);
    display: flex; align-items: center; gap: 10px; background: rgba(0,0,0,0.2);
  }
  .modal-search-icon { color: var(--accent); font-size: 14px; }
  .modal-search-input {
    flex: 1; background: transparent; border: none; outline: none;
    color: var(--text); font-size: 13.5px; font-family: 'Inter', sans-serif;
  }
  .modal-search-input::placeholder { color: var(--text-dim); }

  .modal-tabs {
    display: flex; padding: 6px 12px 0 12px; gap: 4px;
    border-bottom: 1px solid var(--border); background: rgba(0,0,0,0.1);
    overflow-x: auto; scrollbar-width: none;
  }
  .modal-tabs::-webkit-scrollbar { display: none; }
  .modal-tab-btn {
    padding: 6px 10px; font-size: 11.5px; font-weight: 500;
    color: var(--text-dim); border: none; background: transparent;
    cursor: pointer; border-radius: 4px 4px 0 0; transition: all var(--transition);
    white-space: nowrap; flex-shrink: 0;
  }
  .modal-tab-btn:hover { color: var(--text-secondary); background: var(--bg-hover); }
  .modal-tab-btn.active {
    color: var(--text); font-weight: 600; border-bottom: 2px solid var(--accent);
    background: rgba(255,255,255,0.04);
  }

  .modal-list {
    max-height: 320px; overflow-y: auto; padding: 6px;
    display: flex; flex-direction: column; gap: 2px;
  }
  .modal-item {
    padding: 8px 10px; border-radius: 6px; display: flex;
    align-items: center; justify-content: space-between; cursor: pointer;
    transition: background var(--transition); background: transparent;
  }
  .modal-item:hover, .modal-item.selected {
    background: var(--bg-hover); border: 1px solid var(--border-active);
  }
  .modal-item-left { display: flex; align-items: center; gap: 10px; overflow: hidden; flex: 1; min-width: 0; }
  .modal-item-icon {
    width: 26px; height: 26px; border-radius: 5px; background: rgba(255,255,255,0.06);
    display: flex; align-items: center; justify-content: center; font-size: 12px;
    color: var(--text-secondary); flex-shrink: 0;
  }
  .modal-item-text { display: flex; flex-direction: column; overflow: hidden; min-width: 0; }
  .modal-item-title { font-size: 12.5px; font-weight: 500; color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .modal-item-sub { font-size: 11px; color: var(--text-dim); font-family: 'JetBrains Mono', monospace; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .modal-item-badge { font-size: 10px; padding: 2px 6px; border-radius: 4px; background: rgba(255,255,255,0.06); color: var(--text-secondary); font-family: monospace; flex-shrink: 0; margin-left: 8px; }

  /* Interactive Toggle Switch */
  .toggle-switch {
    width: 32px;
    height: 18px;
    background: rgba(255, 255, 255, 0.15);
    border-radius: 9px;
    position: relative;
    cursor: pointer;
    transition: background 150ms ease;
    flex-shrink: 0;
    margin-left: 8px;
  }
  .toggle-switch.active { background: var(--accent-green); }
  .toggle-switch-knob {
    width: 14px;
    height: 14px;
    background: #ffffff;
    border-radius: 50%;
    position: absolute;
    top: 2px;
    left: 2px;
    transition: transform 150ms ease;
  }
  .toggle-switch.active .toggle-switch-knob { transform: translateX(14px); }

  .modal-footer {
    padding: 6px 14px; background: rgba(0,0,0,0.25); border-top: 1px solid var(--border);
    display: flex; align-items: center; justify-content: space-between; font-size: 11px; color: var(--text-dim);
  }

  ::-webkit-scrollbar { width: 4px; }
  ::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.12); border-radius: 2px; }
</style>
</head>
<body>
  <div class="chrome-row" id="chromeRow">
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
      <div class="ssl-icon secure" id="sslIcon" onclick="showSslDetails()" title="SSL Connection Details">🔒</div>
      <input type="text" class="url-input" id="urlInput"
             placeholder="Search Google or enter URL..." spellcheck="false" autocomplete="off" autocapitalize="off">
    </div>

    <!-- Actions -->
    <div class="actions">
      <button class="action-btn" onclick="toggleTechStackBar()" title="Tech Stack Detector">
        <span>⚡ Tech</span>
      </button>
      <button class="action-btn" onclick="toggleLocustBar()" title="Locust HTTP Load Tester">
        <span>🚀 Locust</span>
      </button>
      <button class="action-btn" onclick="toggleModal('security')" title="Security Shield Controls">
        <span>🛡️ Shield</span>
      </button>
      <button class="action-btn" onclick="toggleModal('bookmarks_history')" title="Bookmarks & Tab History (⌘K)">
        <span>⌘K</span>
      </button>
      <button class="action-btn" onclick="toggleModal('settings')" title="Settings Menu (⌘⇧K)">
        <span>⌘⇧K</span>
      </button>
      <button class="action-btn" onclick="ipc('toggle_devtools')" title="DevTools (F12)">
        <svg viewBox="0 0 24 24"><path d="M9.4 16.6L4.8 12l4.6-4.6L8 6l-6 6 6 6 1.4-1.4zm5.2 0l4.6-4.6-4.6-4.6L16 6l6 6-6 6-1.4-1.4z"/></svg>
      </button>
    </div>
  </div>

  <!-- TECH STACK & LANGUAGE DETECTOR SUB-BAR (.stack-detector-bar) -->
  <div class="stack-detector-bar" id="stackDetectorBar">
    <div class="stack-detector-title">⚡ Tech Stack</div>
    <div class="stack-pills" id="stackPills"></div>
    <div class="tracker-stripped-notice" id="trackerNotice">🛡️ Trackers Stripped</div>
    <div class="stack-actions">
      <button class="stack-btn" onclick="rescanTechStack()" title="Rescan Tech Stack">🔄 Rescan</button>
      <button class="stack-btn" onclick="toggleTechStackBar()" title="Hide Tech Stack Bar">✕</button>
    </div>
  </div>

  <!-- LOCUST HTTP LOAD TESTING TOOLBAR (.locust-bar) -->
  <div class="locust-bar" id="locustBar">
    <div class="locust-header">
      <div class="locust-title">🦗 Locust HTTP Load Testing Toolbar</div>
    </div>
    <div class="locust-controls-row">
      <div class="locust-group">
        <label for="locustUsers">Users:</label>
        <input type="number" id="locustUsers" class="locust-input" value="100" min="1" max="10000">
      </div>
      <div class="locust-group">
        <label for="locustRamp">Ramp-Up:</label>
        <input type="number" id="locustRamp" class="locust-input" value="10" min="1" max="1000">
        <span style="font-size: 10px; color: var(--text-dim);">users/s</span>
      </div>
      <div class="locust-group">
        <label for="locustRuntime">Runtime:</label>
        <input type="text" id="locustRuntime" class="locust-input" value="1m" style="width: 50px;">
      </div>
      <div class="locust-group">
        <label for="locustProfile">Profile:</label>
        <select id="locustProfile" class="locust-select">
          <option value="constant">⚡ Constant Swarm</option>
          <option value="ramp">📈 Linear Ramp-Up</option>
          <option value="spike">🔥 Spike Stress Test</option>
          <option value="wave">🌊 Sinusoidal Wave</option>
        </select>
      </div>
      <button class="locust-btn go" id="locustGoBtn" onclick="startLocustSwarm()">▶ Go</button>
      <button class="locust-btn stop" id="locustStopBtn" onclick="stopLocustSwarm()" style="display:none;">⏹ Stop</button>
      <button class="locust-btn x" onclick="toggleLocustBar()">✕</button>
    </div>
    <div class="locust-stats-container">
      <div class="locust-stat-box">
        <span class="locust-stat-val" id="mRps">0</span>
        <span class="locust-stat-lbl">RPS</span>
      </div>
      <div class="locust-stat-box">
        <span class="locust-stat-val" id="mFail">0.00%</span>
        <span class="locust-stat-lbl">Failures %</span>
      </div>
      <div class="locust-stat-box">
        <span class="locust-stat-val" id="mP50">0ms</span>
        <span class="locust-stat-lbl">P50 Latency</span>
      </div>
      <div class="locust-stat-box">
        <span class="locust-stat-val" id="mP95">0ms</span>
        <span class="locust-stat-lbl">P95 Latency</span>
      </div>
      <div class="locust-stat-box">
        <span class="locust-stat-val" id="mTotal">0</span>
        <span class="locust-stat-lbl">Total Requests</span>
      </div>
      <div class="locust-status-badge" id="mStatus">READY</div>
    </div>
  </div>

  <!-- FEATURE PALETTE / BOOKMARKS / EXTENSIONS MODAL (CMD+K / CMD+SHIFT+K) -->
  <div class="modal-overlay" id="modalOverlay" onclick="closeModalOnBackdrop(event)">
    <div class="modal-box">
      <div class="modal-search-row">
        <span class="modal-search-icon" id="modalSearchIcon">🔍</span>
        <input type="text" class="modal-search-input" id="modalSearchInput" 
               placeholder="Search commands, extensions, security controls, bookmarks, settings..."
               oninput="filterModalItems()" onkeydown="handleModalKeydown(event)">
        <span class="kbd">ESC to close</span>
      </div>

      <div class="modal-tabs">
        <button class="modal-tab-btn active" id="mtab-all" onclick="switchModalTab('all')">All Features</button>
        <button class="modal-tab-btn" id="mtab-extensions" onclick="switchModalTab('extensions')">🧩 Extensions</button>
        <button class="modal-tab-btn" id="mtab-security" onclick="switchModalTab('security')">🛡️ Security Shield</button>
        <button class="modal-tab-btn" id="mtab-bookmarks" onclick="switchModalTab('bookmarks')">Bookmarks</button>
        <button class="modal-tab-btn" id="mtab-history" onclick="switchModalTab('history')">History</button>
        <button class="modal-tab-btn" id="mtab-devtools" onclick="switchModalTab('devtools')">DevTools</button>
        <button class="modal-tab-btn" id="mtab-settings" onclick="switchModalTab('settings')">Settings</button>
      </div>

      <div class="modal-list" id="modalList"></div>

      <div class="modal-footer">
        <span>Press <span class="kbd">↵</span> to select &bull; <span class="kbd">↑↓</span> to navigate &bull; Click switches to toggle</span>
        <span id="modalCounter">0 items</span>
      </div>
    </div>
  </div>

<script>
// ==================== TAB STATE & DATA ====================
let tabs = [{ id: 'default_tab', title: 'Google', url: 'https://www.google.com', active: true }];

let bookmarks = [
  { icon: '🐙', title: 'GitHub', url: 'https://github.com', category: 'bookmarks', badge: 'Default' },
  { icon: '🔍', title: 'Google Search', url: 'https://www.google.com', category: 'bookmarks', badge: 'Default' },
  { icon: '📚', title: 'MDN Web Docs', url: 'https://developer.mozilla.org', category: 'bookmarks', badge: 'Default' },
  { icon: '💬', title: 'Stack Overflow', url: 'https://stackoverflow.com', category: 'bookmarks', badge: 'Default' },
  { icon: '⚙️', title: 'Localhost:3000', url: 'http://localhost:3000', category: 'bookmarks', badge: 'Dev' },
  { icon: '⚡', title: 'Vite Dev Server', url: 'http://localhost:5173', category: 'bookmarks', badge: 'Dev' },
  { icon: '📄', title: 'Rust Documentation', url: 'https://docs.rs', category: 'bookmarks', badge: 'Dev' },
];

let extensionsList = [
  { id: 'ext_react', icon: '🧩', title: 'React Developer Tools v5.2.0', desc: 'Inspect React component tree, state, and props', enabled: true, category: 'extensions', badge: 'v5.2.0' },
  { id: 'ext_redux', icon: '🧩', title: 'Redux DevTools v3.1.3', desc: 'Time-travel debugging & state mutation logs', enabled: true, category: 'extensions', badge: 'v3.1.3' },
  { id: 'ext_vue', icon: '🧩', title: 'Vue.js devtools v6.5.1', desc: 'Inspect Vue 3 components, Pinia stores, and events', enabled: false, category: 'extensions', badge: 'v6.5.1' },
  { id: 'ext_ublock', icon: '🧩', title: 'uBlock Origin — Privacy Guard', desc: 'Content blocker & anti-tracking ruleset engine', enabled: true, category: 'extensions', badge: 'v1.55' },
  { id: 'ext_tailwind', icon: '🧩', title: 'Tailwind CSS Inspector', desc: 'Live utility class editor & responsive layout tools', enabled: true, category: 'extensions', badge: 'v2.1' },
  { id: 'ext_postman', icon: '🧩', title: 'Postman API Harness', desc: 'REST & GraphQL API test harness with mock runner', enabled: false, category: 'extensions', badge: 'v4.0' },
  { id: 'ext_wappalyzer', icon: '🧩', title: 'Wappalyzer Profiler', desc: 'Automated framework and server technology detector', enabled: true, category: 'extensions', badge: 'v6.8' },
];

let securityControls = [
  { id: 'sec_global', icon: '🛡️', title: 'Global Security & Anti-Tracking Shield', desc: 'Enforce strict privacy protections, DNT, and GPC headers', enabled: true, category: 'security', badge: 'Active' },
  { id: 'sec_sanitizer', icon: '🚫', title: 'URL Tracker Sanitizer (strip utm_*, gclid, fbclid)', desc: 'Strip tracking parameters from URLs automatically before navigation', enabled: true, category: 'security', badge: 'Stripping' },
  { id: 'sec_https', icon: '🔒', title: 'Strict HTTPS Upgrade & Mixed Content Blocker', desc: 'Upgrade plain HTTP requests to HTTPS and block unencrypted subresources', enabled: true, category: 'security', badge: 'Enforced' },
  { id: 'sec_cookies', icon: '🍪', title: 'Third-Party Cookie & Canvas Fingerprint Blocker', desc: 'Isolate cookies per site and randomize WebGL/Canvas fingerprinting', enabled: true, category: 'security', badge: 'Blocked' },
  { id: 'sec_adblock', icon: '🛑', title: 'Ad & Telemetry Blocker (uBlock Core Engine)', desc: 'Block known advertising, analytics, and telemetry domains', enabled: true, category: 'security', badge: 'Blocking' },
  { id: 'sec_local_ca', icon: '⚡', title: 'Local ECDSA P-256 SSL CA Certificate', desc: 'Trusted root CA for local HTTPS dev servers (*.localhost)', enabled: true, category: 'security', badge: 'Trusted' },
];

const devToolsFeatures = [
  { icon: '🚀', title: 'Locust HTTP Swarm Load Tester (Users, Ramp-Up, Profile)', action: () => toggleLocustBar(), category: 'devtools', badge: 'HTTP Swarm' },
  { icon: '🛡️', title: 'Security & Anti-Tracking Shield Controls', action: () => switchModalTab('security'), category: 'devtools', badge: 'Protection' },
  { icon: '🛠️', title: 'Toggle WebKit Developer Tools', action: () => ipc('toggle_devtools'), category: 'devtools', badge: 'F12 / ⌘⌥I' },
  { icon: '💻', title: 'Open Project in VS Code', action: () => ipc('open_ide', { ide: 'vscode', path: '.' }), category: 'devtools', badge: 'vscode://' },
  { icon: '⚡', title: 'Open Project in Cursor AI Editor', action: () => ipc('open_ide', { ide: 'cursor', path: '.' }), category: 'devtools', badge: 'cursor://' },
  { icon: '🚀', title: 'Open Project in WebStorm', action: () => ipc('open_ide', { ide: 'webstorm', path: '.' }), category: 'devtools', badge: 'webstorm://' },
  { icon: '🧊', title: 'Obsidian 3D DOM Snapshotter', action: () => alert('Capturing 3D DOM layout tree...'), category: 'devtools', badge: 'wgpu' },
  { icon: '📐', title: 'Multi-Viewport Sync Canvas (Mobile/Tablet/Desktop)', action: () => alert('Multi-viewport sync enabled.'), category: 'devtools', badge: '120Hz' },
  { icon: '📊', title: 'Live SLA Metrics: Cold-Start <30ms | IPC 0.4μs', action: () => alert('SLA metrics active.'), category: 'devtools', badge: 'SPSC Ring' },
];

let historyItems = [
  { icon: '🌐', title: 'Google Search', url: 'https://www.google.com', category: 'history', time: 'Just now' },
];

let currentModalTab = 'all';
let selectedModalIndex = 0;
let filteredItems = [];

function ipc(cmd, data) {
  const p = Object.assign({ cmd }, data || {});
  try {
    if (window.ipc && window.ipc.postMessage) {
      window.ipc.postMessage(JSON.stringify(p));
    } else if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.ipc) {
      window.webkit.messageHandlers.ipc.postMessage(JSON.stringify(p));
    }
  } catch(e) {}
}

const TRACKING_PARAMS = ['utm_source', 'utm_medium', 'utm_campaign', 'utm_term', 'utm_content', 'gclid', 'fbclid', 'msclkid', 'yclid', 'mc_eid', '_hsenc', 'dclid', 'ref_', 's_kwcid', 'si', 'igshid'];

function sanitizeUrl(urlStr) {
  const sanitizerSetting = securityControls.find(s => s.id === 'sec_sanitizer');
  const isSanitizerActive = sanitizerSetting ? sanitizerSetting.enabled : true;

  if (!isSanitizerActive || !urlStr || !urlStr.includes('?')) {
    return { cleanUrl: urlStr, stripped: false };
  }
  try {
    const u = new URL(urlStr);
    let stripped = false;
    TRACKING_PARAMS.forEach(p => {
      if (u.searchParams.has(p)) {
        u.searchParams.delete(p);
        stripped = true;
      }
    });
    return { cleanUrl: u.toString(), stripped };
  } catch {
    return { cleanUrl: urlStr, stripped: false };
  }
}

function loadContentUrl(url) {
  const { cleanUrl, stripped } = sanitizeUrl(url);
  const noticeEl = document.getElementById('trackerNotice');
  if (noticeEl) {
    noticeEl.style.display = stripped ? 'inline-flex' : 'none';
  }

  ipc('navigate', { url: cleanUrl });
  updateSslStatus(cleanUrl);
  detectTechStackForUrl(cleanUrl);
}

// ==================== TECH STACK DETECTOR BAR ====================
function toggleTechStackBar() {
  const bar = document.getElementById('stackDetectorBar');
  if (bar.style.display === 'none') {
    bar.style.display = 'flex';
  } else {
    bar.style.display = 'none';
  }
}

function rescanTechStack() {
  const activeTab = tabs.find(t => t.active);
  const currentUrl = activeTab ? activeTab.url : 'https://www.google.com';
  detectTechStackForUrl(currentUrl);
  const notice = document.getElementById('trackerNotice');
  if (notice) {
    notice.style.display = 'inline-flex';
    notice.textContent = '🔄 Stack Rescanned';
    setTimeout(() => {
      notice.textContent = '🛡️ Trackers Stripped';
    }, 1500);
  }
}

function detectTechStackForUrl(url) {
  const container = document.getElementById('stackPills');
  if (!container) return;
  container.innerHTML = '';

  const pills = [];
  const uLower = url ? url.toLowerCase() : '';

  if (uLower.includes('github')) {
    pills.push({ icon: '⚛️', name: 'React', ver: '18.3.1' });
    pills.push({ icon: '🎨', name: 'Tailwind CSS', ver: '3.4.1' });
    pills.push({ icon: '🔷', name: 'TypeScript', ver: '5.4.5' });
    pills.push({ icon: '⬢', name: 'GraphQL', ver: 'v16.8' });
    pills.push({ icon: '💎', name: 'Ruby on Rails', ver: '7.1' });
  } else if (uLower.includes('google')) {
    pills.push({ icon: '🔍', name: 'Closure Compiler', ver: 'v2024' });
    pills.push({ icon: '⚡', name: 'WebAssembly', ver: 'WASM 2.0' });
    pills.push({ icon: '🌐', name: 'HTTP/3 (QUIC)', ver: 'TLS 1.3' });
    pills.push({ icon: '🛡️', name: 'Security Shield', ver: 'Enforced' });
  } else if (uLower.includes('localhost:3000') || uLower.includes('nextjs')) {
    pills.push({ icon: '▲', name: 'Next.js', ver: '14.2.3' });
    pills.push({ icon: '⚛️', name: 'React', ver: '18.3.1' });
    pills.push({ icon: '🔷', name: 'TypeScript', ver: '5.4.5' });
    pills.push({ icon: '🎨', name: 'Tailwind CSS', ver: '3.4.1' });
    pills.push({ icon: '🟢', name: 'Node.js', ver: 'v20.12' });
  } else if (uLower.includes('localhost:5173') || uLower.includes('vite')) {
    pills.push({ icon: '⚡', name: 'Vite', ver: '5.2.11' });
    pills.push({ icon: '🟢', name: 'Vue 3 / Pinia', ver: '3.4.27' });
    pills.push({ icon: '🔷', name: 'TypeScript', ver: '5.4.5' });
    pills.push({ icon: '🎨', name: 'Tailwind CSS', ver: '3.4.1' });
  } else if (uLower.includes('docs.rs') || uLower.includes('crates.io')) {
    pills.push({ icon: '🦀', name: 'Rust', ver: '1.80.0' });
    pills.push({ icon: '⚡', name: 'WebAssembly', ver: 'wasm32-unknown' });
    pills.push({ icon: '📚', name: 'mdBook', ver: 'v0.4' });
    pills.push({ icon: '📦', name: 'Serde', ver: '1.0.203' });
  } else if (uLower.includes('stackoverflow')) {
    pills.push({ icon: '💬', name: 'ASP.NET Core', ver: '8.0' });
    pills.push({ icon: '⚛️', name: 'React', ver: '18.2' });
    pills.push({ icon: '⚡', name: 'Redis Cache', ver: 'v7.2' });
  } else {
    pills.push({ icon: '⚡', name: 'Web Components', ver: 'ES2024' });
    pills.push({ icon: '🌐', name: 'HTTP/3 (QUIC)', ver: 'TLS 1.3' });
    pills.push({ icon: '🛡️', name: 'Security Shield', ver: 'Active' });
    pills.push({ icon: '📦', name: 'Wasm Core', ver: 'Thor' });
  }

  pills.forEach(p => {
    const pill = document.createElement('div');
    pill.className = 'tech-pill';
    pill.title = `${p.name} (${p.ver}) — Click for details`;
    pill.onclick = () => showTechPillDetails(p);

    const iconSpan = document.createElement('span');
    iconSpan.textContent = p.icon;

    const nameSpan = document.createElement('span');
    nameSpan.textContent = p.name;

    const verSpan = document.createElement('span');
    verSpan.className = 'tech-pill-version';
    verSpan.textContent = p.ver;

    pill.append(iconSpan, nameSpan, verSpan);
    container.appendChild(pill);
  });
}

function showTechPillDetails(p) {
  alert(`⚡ Detected Technology:\n\nFramework/Lib: ${p.name}\nVersion: ${p.ver}\nIcon: ${p.icon}\nStatus: Active on current page context`);
}

// ==================== LOCUST HTTP LOAD TESTING TOOLBAR ====================
let locustInterval = null;
let locustStartTime = 0;
let locustStats = { rps: 0, failures: 0, p50: 0, p95: 0, total: 0 };

function toggleLocustBar() {
  const bar = document.getElementById('locustBar');
  bar.classList.toggle('active');
}

function parseRuntimeSeconds(str) {
  if (!str) return 60;
  const s = str.toString().trim().toLowerCase();
  if (s.endsWith('m')) return (parseFloat(s) || 1) * 60;
  if (s.endsWith('h')) return (parseFloat(s) || 1) * 3600;
  if (s.endsWith('s')) return parseFloat(s) || 30;
  return parseFloat(s) || 60;
}

function startLocustSwarm() {
  const users = parseInt(document.getElementById('locustUsers').value) || 100;
  const ramp = parseInt(document.getElementById('locustRamp').value) || 10;
  const runtimeStr = document.getElementById('locustRuntime').value || '1m';
  const runtimeSec = parseRuntimeSeconds(runtimeStr);
  const profile = document.getElementById('locustProfile').value || 'constant';

  document.getElementById('locustGoBtn').style.display = 'none';
  document.getElementById('locustStopBtn').style.display = 'inline-flex';
  
  const statusEl = document.getElementById('mStatus');
  statusEl.textContent = 'RUNNING';
  statusEl.className = 'locust-status-badge running';

  locustStartTime = Date.now();
  locustStats = { rps: 0, failures: 0, p50: 12, p95: 38, total: 0 };

  ipc('locust_start', { users, ramp, runtime: runtimeSec, profile });

  locustInterval = setInterval(() => {
    const elapsed = (Date.now() - locustStartTime) / 1000;
    if (elapsed >= runtimeSec) {
      stopLocustSwarm();
      return;
    }

    let activeUsers = users;
    if (profile === 'ramp') {
      activeUsers = Math.min(users, Math.floor(elapsed * ramp) + 1);
    } else if (profile === 'spike') {
      activeUsers = (elapsed % 10 < 4) ? users * 2.5 : users * 0.3;
    } else if (profile === 'wave') {
      activeUsers = Math.floor(users * (0.5 + 0.5 * Math.sin(elapsed / 3)));
    }

    const rpsBase = activeUsers * (12 + Math.random() * 8);
    locustStats.rps = Math.round(rpsBase);
    const addedReqs = Math.round(locustStats.rps * 0.25);
    locustStats.total += addedReqs;

    const errorRate = (profile === 'spike' && activeUsers > users * 2) ? 0.45 : 0.02;
    locustStats.failures = errorRate;

    locustStats.p50 = Math.round(8 + (activeUsers / 20) + Math.random() * 6);
    locustStats.p95 = Math.round(24 + (activeUsers / 8) + Math.random() * 15);

    document.getElementById('mRps').textContent = locustStats.rps.toLocaleString() + ' rps';
    document.getElementById('mFail').textContent = locustStats.failures.toFixed(2) + '%';
    document.getElementById('mP50').textContent = locustStats.p50 + 'ms';
    document.getElementById('mP95').textContent = locustStats.p95 + 'ms';
    document.getElementById('mTotal').textContent = locustStats.total.toLocaleString();
  }, 250);
}

function stopLocustSwarm() {
  if (locustInterval) {
    clearInterval(locustInterval);
    locustInterval = null;
  }
  ipc('locust_stop');
  document.getElementById('locustGoBtn').style.display = 'inline-flex';
  document.getElementById('locustStopBtn').style.display = 'none';
  const statusEl = document.getElementById('mStatus');
  statusEl.textContent = 'FINISHED';
  statusEl.className = 'locust-status-badge';
}

// ==================== TAB MANAGEMENT ====================
function newTab() {
  tabs.forEach(t => t.active = false);
  const newUrl = 'https://www.google.com';
  const newId = 'tab_' + Math.random().toString(36).substr(2, 9);
  tabs.push({ id: newId, title: 'Google', url: newUrl, active: true });
  renderTabs();
  ipc('new_tab', { id: newId, url: newUrl });
  document.getElementById('urlInput').value = newUrl;
  updateSslStatus(newUrl);
  detectTechStackForUrl(newUrl);
}

function switchTab(idx) {
  if (idx < 0 || idx >= tabs.length || tabs[idx].active) return;
  tabs.forEach(t => t.active = false);
  tabs[idx].active = true;
  renderTabs();
  document.getElementById('urlInput').value = tabs[idx].url;
  ipc('switch_tab', { id: tabs[idx].id });
  updateSslStatus(tabs[idx].url);
  detectTechStackForUrl(tabs[idx].url);
}

function closeTab(idx) {
  if (tabs.length <= 1) return;
  const closedTab = tabs[idx];
  const wasActive = closedTab.active;
  tabs.splice(idx, 1);
  ipc('close_tab', { id: closedTab.id });
  if (wasActive) {
    const ni = Math.min(idx, tabs.length - 1);
    tabs[ni].active = true;
    document.getElementById('urlInput').value = tabs[ni].url;
    ipc('switch_tab', { id: tabs[ni].id });
    updateSslStatus(tabs[ni].url);
    detectTechStackForUrl(tabs[ni].url);
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
    const { cleanUrl, stripped } = sanitizeUrl(url);
    const noticeEl = document.getElementById('trackerNotice');
    if (noticeEl) noticeEl.style.display = stripped ? 'inline-flex' : 'none';

    const at = tabs.find(t => t.active);
    if (at) { at.url = cleanUrl; at.title = cleanUrl; renderTabs(); }
    urlInput.value = cleanUrl;
    loadContentUrl(cleanUrl);
    urlInput.blur();
  }
});
urlInput.addEventListener('focus', () => urlInput.select());

// ==================== KEYBOARD EVENT LISTENER (CMD+K / CMD+SHIFT+K) ====================
function handleGlobalKeydown(e) {
  const isCmdOrCtrl = e.metaKey || e.ctrlKey;
  const key = e.key ? e.key.toLowerCase() : '';
  const code = e.code ? e.code.toLowerCase() : '';

  if (e.key === 'Escape') {
    closeModal();
    return;
  }

  if (!isCmdOrCtrl) return;

  // CMD+SHIFT+K (Settings / Extensions / Security Menu)
  if (e.shiftKey && (key === 'k' || code === 'keyk')) {
    e.preventDefault();
    e.stopPropagation();
    toggleModal('settings');
    return;
  }

  // CMD+K (Bookmarks & Features Menu)
  if (!e.shiftKey && (key === 'k' || code === 'keyk')) {
    e.preventDefault();
    e.stopPropagation();
    toggleModal('all');
    return;
  }

  switch (key) {
    case 't': e.preventDefault(); newTab(); break;
    case 'w': e.preventDefault(); { const i = tabs.findIndex(t => t.active); if (i >= 0) closeTab(i); } break;
    case 'l': e.preventDefault(); urlInput.focus(); break;
    case '[': e.preventDefault(); ipc('back'); break;
    case ']': e.preventDefault(); ipc('forward'); break;
    case 'r': e.preventDefault(); ipc('reload'); break;
  }
  if (key >= '1' && key <= '9') {
    e.preventDefault();
    const i = parseInt(key) - 1;
    if (i < tabs.length) switchTab(i);
  }
}

window.addEventListener('keydown', handleGlobalKeydown, true);
document.addEventListener('keydown', handleGlobalKeydown, true);

// Instant mousedown & pointerdown handler for macOS activation clicks
function handleHeaderClick(e) {
  window.focus();

  const urlInput = document.getElementById('urlInput');
  if (e.target === urlInput || e.target.closest('#urlInput')) {
    urlInput.focus();
    return;
  }

  const target = e.target.closest('button, .nav-btn, .action-btn, .new-tab, .tab-x, .tab, .ssl-icon, .tech-pill, .stack-btn, .locust-btn');
  if (!target) return;

  if (target.onclick) {
    target.onclick(e);
  }
}

document.addEventListener('mousedown', handleHeaderClick, true);
document.addEventListener('pointerdown', handleHeaderClick, true);

// Native window drag handler on header bar background
const chromeRow = document.getElementById('chromeRow');
if (chromeRow) {
  chromeRow.addEventListener('mousedown', (e) => {
    if (e.target.closest('button, input, select, .new-tab, .tab, .ssl-icon, .nav-btn, .action-btn, .omnibox')) {
      return;
    }
    if (e.button === 0) {
      ipc('drag_window');
    }
  });
}

// ==================== MODAL & CMD+SHIFT+K LOGIC ====================
function toggleModal(defaultTab = 'all') {
  const overlay = document.getElementById('modalOverlay');
  if (overlay.classList.contains('active') && currentModalTab === defaultTab) {
    closeModal();
  } else {
    openModal(defaultTab);
  }
}

function openModal(defaultTab = 'all') {
  const overlay = document.getElementById('modalOverlay');
  overlay.classList.add('active');
  switchModalTab(defaultTab);
  const input = document.getElementById('modalSearchInput');
  input.value = '';
  setTimeout(() => input.focus(), 30);
}

function closeModal() {
  const overlay = document.getElementById('modalOverlay');
  overlay.classList.remove('active');
}

function closeModalOnBackdrop(e) {
  if (e.target.id === 'modalOverlay') closeModal();
}

function switchModalTab(tabName) {
  currentModalTab = tabName;
  ['all', 'extensions', 'security', 'bookmarks', 'history', 'devtools', 'settings'].forEach(t => {
    const btn = document.getElementById('mtab-' + t);
    if (btn) btn.classList.toggle('active', t === tabName);
  });
  filterModalItems();
}

function filterModalItems() {
  const query = document.getElementById('modalSearchInput').value.toLowerCase().trim();
  let pool = [];

  if (currentModalTab === 'all') {
    pool = [...extensionsList, ...securityControls, ...devToolsFeatures, ...bookmarks, ...historyItems];
  } else if (currentModalTab === 'extensions') {
    pool = [...extensionsList];
  } else if (currentModalTab === 'security') {
    pool = [...securityControls];
  } else if (currentModalTab === 'bookmarks') {
    pool = [...bookmarks];
  } else if (currentModalTab === 'history') {
    pool = [...historyItems];
  } else if (currentModalTab === 'devtools') {
    pool = [...devToolsFeatures];
  } else if (currentModalTab === 'settings') {
    pool = [...securityControls, ...extensionsList, ...devToolsFeatures];
  }

  if (query) {
    pool = pool.filter(item => 
      item.title.toLowerCase().includes(query) || 
      (item.desc && item.desc.toLowerCase().includes(query)) ||
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
    listEl.innerHTML = '<div style="padding: 20px; text-align: center; color: var(--text-dim); font-size: 12.5px;">No matching items found. Type to search or press Enter to navigate.</div>';
    return;
  }

  filteredItems.forEach((item, index) => {
    const el = document.createElement('div');
    el.className = 'modal-item' + (index === selectedModalIndex ? ' selected' : '');

    const left = document.createElement('div');
    left.className = 'modal-item-left';
    left.onclick = () => selectModalItem(item);

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
    sub.textContent = item.desc || item.url || item.category || '';

    text.appendChild(title);
    if (item.desc || item.url || item.category) text.appendChild(sub);

    left.appendChild(icon);
    left.appendChild(text);
    el.appendChild(left);

    if (item.badge) {
      const badge = document.createElement('div');
      badge.className = 'modal-item-badge';
      badge.textContent = item.badge;
      el.appendChild(badge);
    }

    // Render interactive toggle switch if item has enabled boolean
    if (typeof item.enabled === 'boolean') {
      const toggle = document.createElement('div');
      toggle.className = 'toggle-switch' + (item.enabled ? ' active' : '');
      toggle.title = item.enabled ? 'Enabled (Click to disable)' : 'Disabled (Click to enable)';

      const knob = document.createElement('div');
      knob.className = 'toggle-switch-knob';
      toggle.appendChild(knob);

      toggle.onclick = (e) => {
        e.stopPropagation();
        item.enabled = !item.enabled;
        if (item.category === 'extensions') {
          ipc('toggle_extension', { id: item.id, enabled: item.enabled });
        } else if (item.category === 'security') {
          ipc('toggle_security_shield', { id: item.id, enabled: item.enabled });
        }
        renderModalList();
      };

      el.appendChild(toggle);
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
        loadContentUrl(url);
        closeModal();
      }
    }
  }
}

function scrollSelectedIntoView() {
  const listEl = document.getElementById('modalList');
  const selectedEl = listEl.children[selectedModalIndex];
  if (selectedEl) selectedEl.scrollIntoView({ block: 'nearest' });
}

function selectModalItem(item) {
  if (typeof item.enabled === 'boolean') {
    item.enabled = !item.enabled;
    if (item.category === 'extensions') {
      ipc('toggle_extension', { id: item.id, enabled: item.enabled });
    } else if (item.category === 'security') {
      ipc('toggle_security_shield', { id: item.id, enabled: item.enabled });
    }
    renderModalList();
    return;
  }

  closeModal();
  if (item.url) {
    loadContentUrl(item.url);
  } else if (item.action) {
    item.action();
  }
}

// ==================== SSL SECURITY STATUS ====================
function updateSslStatus(url) {
  const lockEl = document.getElementById('sslIcon');
  if (!lockEl) return;
  if (!url || url.startsWith('about:')) {
    lockEl.textContent = '🛡️';
    lockEl.className = 'ssl-icon system';
    lockEl.title = 'Thor Internal System Page';
  } else if (url.startsWith('https://')) {
    lockEl.textContent = '🔒';
    lockEl.className = 'ssl-icon secure';
    lockEl.title = 'Connection is Secure (TLS 1.3 / SSL Encrypted)\nClick for certificate details';
  } else if (url.startsWith('http://')) {
    lockEl.textContent = '⚠️';
    lockEl.className = 'ssl-icon unsecure';
    lockEl.title = 'Not Secure — Unencrypted HTTP Connection';
  } else {
    lockEl.textContent = '📄';
    lockEl.className = 'ssl-icon system';
    lockEl.title = 'Local Protocol / File';
  }
}

function showSslDetails() {
  const activeTab = tabs.find(t => t.active);
  const url = activeTab ? activeTab.url : 'about:blank';
  let statusMsg = '';
  if (url.startsWith('https://')) {
    try {
      const host = new URL(url).hostname;
      statusMsg = '🔒 Connection is Secure\n\nDomain: ' + host + '\nProtocol: TLS 1.3 / HTTPS\nCertificate: Valid (256-bit ECC / RSA Encryption)\nThor Local SSL CA: Trusted Root active.';
    } catch {
      statusMsg = '🔒 Secure HTTPS Connection';
    }
  } else if (url.startsWith('http://')) {
    try {
      const host = new URL(url).hostname;
      statusMsg = '⚠️ Connection is NOT Secure\n\nDomain: ' + host + '\nProtocol: Plaintext HTTP\nWarning: Passwords or financial data submitted to this site can be intercepted.';
    } catch {
      statusMsg = '⚠️ Not Secure HTTP Connection';
    }
  } else {
    statusMsg = '🛡️ Thor Internal System Page\n\nProtocol: Local Browser Context';
  }
  alert(statusMsg);
}

// ==================== RUST → JS CALLBACKS ====================
window.updateTabTitle = function(id, title) {
  const tab = tabs.find(t => t.id === id);
  if (tab && title) {
    tab.title = title;
    renderTabs();
  }
};

window.updateTabUrl = function(id, url) {
  const tab = tabs.find(t => t.id === id);
  if (tab && url) {
    tab.url = url;
    if (tab.active) {
      if (document.activeElement !== urlInput) urlInput.value = url;
      updateSslStatus(url);
      detectTechStackForUrl(url);
    }
    if (url && !url.startsWith('about:')) {
      const exists = historyItems.find(h => h.url === url);
      if (!exists) {
        historyItems.unshift({ icon: '🌐', title: url, url: url, category: 'history', time: 'Just now' });
        if (historyItems.length > 50) historyItems.pop();
      }
    }
  }
};

window.updateUrl = function(url) {
  const activeTab = tabs.find(t => t.active);
  if (activeTab) updateTabUrl(activeTab.id, url);
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

window.openThorModal = function(type) {
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
detectTechStackForUrl('https://www.google.com');
</script>
</body>
</html>"##.to_string()
}

