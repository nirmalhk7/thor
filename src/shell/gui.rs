pub fn get_chrome_html() -> String {
    r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Thor Chrome</title>
    <style>
        :root {
            --bg-obsidian: #12141a;
            --panel-bg: rgba(26, 29, 38, 0.85);
            --accent-violet: #a855f7;
            --accent-cyan: #06b6d4;
            --accent-green: #22c55e;
            --text-main: #e2e8f0;
            --text-muted: #94a3b8;
            --border-subtle: rgba(255, 255, 255, 0.06);
            --hover-bg: rgba(255, 255, 255, 0.05);
        }

        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }

        body {
            font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background: linear-gradient(135deg, var(--bg-obsidian) 0%, #0d0f14 100%);
            color: var(--text-main);
            height: 90px;
            overflow: hidden;
            user-select: none;
            display: flex;
            flex-direction: column;
        }

        .chrome-container {
            flex: 1;
            display: flex;
            flex-direction: column;
            background: var(--panel-bg);
            backdrop-filter: blur(20px);
            -webkit-backdrop-filter: blur(20px);
            border-bottom: 1px solid var(--border-subtle);
            height: 100%;
        }

        /* Tab Bar Row (Drag Region) */
        .tab-bar-row {
            height: 36px;
            display: flex;
            align-items: flex-end;
            padding: 0 12px;
            gap: 4px;
            -webkit-app-region: drag;
            /* Extra padding on left for native window controls (traffic lights) */
            padding-left: 75px; 
        }

        .tab {
            -webkit-app-region: no-drag;
            height: 30px;
            min-width: 120px;
            max-width: 200px;
            background: transparent;
            border: 1px solid transparent;
            border-bottom: none;
            border-top-left-radius: 6px;
            border-top-right-radius: 6px;
            display: flex;
            align-items: center;
            padding: 0 10px;
            gap: 8px;
            cursor: pointer;
            transition: background 150ms ease-out;
            position: relative;
        }

        .tab:hover {
            background: var(--hover-bg);
        }

        .tab.active {
            background: rgba(255, 255, 255, 0.03);
            border: 1px solid var(--border-subtle);
            border-bottom: 2px solid var(--accent-violet);
        }

        .tab-icon {
            font-size: 14px;
            display: flex;
            align-items: center;
            justify-content: center;
        }

        .tab-title {
            flex: 1;
            font-size: 12px;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
            color: var(--text-muted);
        }

        .tab.active .tab-title {
            color: var(--text-main);
            font-weight: 500;
        }

        .tab-close {
            opacity: 0;
            width: 18px;
            height: 18px;
            border-radius: 4px;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 14px;
            color: var(--text-muted);
            transition: all 150ms ease-out;
        }

        .tab:hover .tab-close, .tab.active .tab-close {
            opacity: 1;
        }

        .tab-close:hover {
            background: rgba(255, 255, 255, 0.1);
            color: var(--text-main);
        }

        .new-tab-btn {
            -webkit-app-region: no-drag;
            width: 28px;
            height: 28px;
            margin-bottom: 2px;
            border-radius: 6px;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 18px;
            color: var(--text-muted);
            cursor: pointer;
            transition: all 150ms ease-out;
        }

        .new-tab-btn:hover {
            background: var(--hover-bg);
            color: var(--text-main);
        }

        /* Address Bar Row */
        .address-bar-row {
            height: 54px;
            display: flex;
            align-items: center;
            padding: 0 16px;
            gap: 12px;
            -webkit-app-region: no-drag;
        }

        .nav-buttons {
            display: flex;
            gap: 4px;
        }

        .icon-btn {
            width: 32px;
            height: 32px;
            border-radius: 6px;
            border: none;
            background: transparent;
            color: var(--text-muted);
            display: flex;
            align-items: center;
            justify-content: center;
            cursor: pointer;
            transition: all 150ms ease-out;
        }

        .icon-btn:hover {
            background: var(--hover-bg);
            color: var(--text-main);
            transform: scale(1.05);
        }

        .icon-btn svg {
            width: 16px;
            height: 16px;
            fill: currentColor;
        }

        .omnibox {
            flex: 1;
            height: 32px;
            background: rgba(0, 0, 0, 0.2);
            border: 1px solid var(--border-subtle);
            border-radius: 8px;
            display: flex;
            align-items: center;
            padding: 0 12px;
            gap: 8px;
            transition: all 150ms ease-out;
            box-shadow: inset 0 1px 3px rgba(0,0,0,0.1);
        }

        .omnibox:focus-within {
            border-color: var(--accent-violet);
            box-shadow: 0 0 0 1px var(--accent-violet), inset 0 1px 3px rgba(0,0,0,0.1);
            background: rgba(0, 0, 0, 0.4);
        }

        .ssl-indicator {
            color: var(--accent-green);
            font-size: 12px;
            display: flex;
            align-items: center;
        }

        .url-input {
            flex: 1;
            background: transparent;
            border: none;
            outline: none;
            color: var(--accent-cyan);
            font-family: "SF Mono", "Fira Code", Consolas, monospace;
            font-size: 13px;
        }

        .url-input::placeholder {
            color: rgba(6, 182, 212, 0.4);
        }

        .actions {
            display: flex;
            align-items: center;
            gap: 8px;
        }

        .shortcut-hint {
            font-size: 11px;
            color: var(--text-muted);
            background: rgba(255, 255, 255, 0.05);
            padding: 2px 6px;
            border-radius: 4px;
            border: 1px solid var(--border-subtle);
            font-family: system-ui, sans-serif;
            letter-spacing: 0.5px;
            pointer-events: none;
        }
    </style>
</head>
<body>
    <div class="chrome-container">
        <!-- Tab Bar -->
        <div class="tab-bar-row" id="tab-bar">
            <!-- Tabs injected by JS -->
            <div class="new-tab-btn" onclick="sendCmd('new_tab')" title="New Tab (Cmd+T)">+</div>
        </div>

        <!-- Address Bar -->
        <div class="address-bar-row">
            <div class="nav-buttons">
                <button class="icon-btn" onclick="sendCmd('back')" title="Back (Cmd+[)">
                    <svg viewBox="0 0 24 24"><path d="M15.41 16.59L10.83 12l4.58-4.59L14 6l-6 6 6 6 1.41-1.41z"/></svg>
                </button>
                <button class="icon-btn" onclick="sendCmd('forward')" title="Forward (Cmd+])">
                    <svg viewBox="0 0 24 24"><path d="M8.59 16.59L13.17 12 8.59 7.41 10 6l6 6-6 6-1.41-1.41z"/></svg>
                </button>
                <button class="icon-btn" onclick="sendCmd('reload')" title="Reload (Cmd+R)">
                    <svg viewBox="0 0 24 24"><path d="M17.65 6.35A7.958 7.958 0 0012 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08A5.99 5.99 0 0112 18c-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"/></svg>
                </button>
            </div>

            <div class="omnibox">
                <div class="ssl-indicator" title="Secure">🔒</div>
                <input type="text" class="url-input" id="url-input" placeholder="Enter URL or search query..." spellcheck="false" autocomplete="off">
            </div>

            <div class="actions">
                <div class="shortcut-hint">⌘K</div>
                <button class="icon-btn" onclick="sendCmd('toggle_devtools')" title="Toggle DevTools">
                    <svg viewBox="0 0 24 24"><path d="M22.7 19l-9.1-9.1c.9-2.3.4-5-1.5-6.9-2-2-5-2.4-7.4-1.1L7.6 4.8 5.4 7 8.3 9.9c-1.3 2.4-.9 5.4 1.1 7.4 1.9 1.9 4.6 2.4 6.9 1.5l9.1 9.1c.4.4 1 .4 1.4 0l.9-.9c.4-.4.4-1.1-.1-1.4zM9 13.9c-1.5-1.5-1.7-3.9-.6-5.6l5.2 5.2c-1.7 1.1-4.1.9-5.6-.6z"/></svg>
                </button>
            </div>
        </div>
    </div>

    <script>
        let tabs = [
            { title: "New Tab", url: "about:blank", active: true }
        ];

        function sendCmd(cmd, data = {}) {
            if (window.ipc && window.ipc.postMessage) {
                window.ipc.postMessage(JSON.stringify(Object.assign({ cmd: cmd }, data)));
            } else {
                console.log("IPC Mock:", cmd, data);
            }
        }

        const urlInput = document.getElementById("url-input");
        const tabBar = document.getElementById("tab-bar");
        const newTabBtn = document.querySelector(".new-tab-btn");

        urlInput.addEventListener("keydown", (e) => {
            if (e.key === "Enter") {
                let url = urlInput.value.trim();
                if (url) {
                    if (!url.startsWith('http://') && !url.startsWith('https://') && !url.startsWith('about:') && !url.startsWith('file://')) {
                        if (url.includes('.') && !url.includes(' ')) {
                            url = 'https://' + url;
                        } else {
                            url = 'https://google.com/search?q=' + encodeURIComponent(url);
                        }
                    }
                    sendCmd('navigate', { url: url });
                }
            }
        });

        urlInput.addEventListener("focus", () => urlInput.select());

        document.addEventListener("keydown", (e) => {
            const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
            const cmdOrCtrl = isMac ? e.metaKey : e.ctrlKey;
            
            if (cmdOrCtrl) {
                switch(e.key.toLowerCase()) {
                    case 't':
                        e.preventDefault();
                        sendCmd('new_tab');
                        break;
                    case 'w':
                        e.preventDefault();
                        const activeIdx = tabs.findIndex(t => t.active);
                        if (activeIdx >= 0) sendCmd('close_tab', { idx: activeIdx });
                        break;
                    case 'l':
                        e.preventDefault();
                        urlInput.focus();
                        break;
                    case '[':
                        e.preventDefault();
                        sendCmd('back');
                        break;
                    case ']':
                        e.preventDefault();
                        sendCmd('forward');
                        break;
                    case 'r':
                        e.preventDefault();
                        sendCmd('reload');
                        break;
                    case 'k':
                        e.preventDefault();
                        urlInput.focus();
                        urlInput.value = '';
                        break;
                }
            }
        });

        window.updateUrl = function(url) {
            urlInput.value = url;
            const activeTab = tabs.find(t => t.active);
            if(activeTab) activeTab.url = url;
        };

        window.updateTitle = function(title) {
            const activeTab = tabs.find(t => t.active);
            if(activeTab) {
                activeTab.title = title;
                renderTabs();
            }
        };

        window.updateTabs = function(tabsJson) {
            try {
                if (typeof tabsJson === 'string') {
                    tabs = JSON.parse(tabsJson);
                } else {
                    tabs = tabsJson; 
                }
                renderTabs();
                
                const activeTab = tabs.find(t => t.active);
                if (activeTab && document.activeElement !== urlInput) {
                    urlInput.value = activeTab.url || "";
                }
            } catch (e) {
                console.error("Failed to parse tabs JSON", e);
            }
        };

        function renderTabs() {
            const elements = tabBar.querySelectorAll('.tab');
            elements.forEach(el => el.remove());

            tabs.forEach((tab, index) => {
                const tabEl = document.createElement('div');
                tabEl.className = 'tab' + (tab.active ? ' active' : '');
                tabEl.onclick = (e) => {
                    if (!e.target.closest('.tab-close')) {
                        sendCmd('switch_tab', { idx: index });
                    }
                };

                const icon = document.createElement('div');
                icon.className = 'tab-icon';
                icon.innerHTML = '⚡';

                const title = document.createElement('div');
                title.className = 'tab-title';
                title.title = tab.title || "New Tab";
                title.textContent = tab.title || "New Tab";

                const closeBtn = document.createElement('div');
                closeBtn.className = 'tab-close';
                closeBtn.innerHTML = '×';
                closeBtn.title = 'Close Tab (Cmd+W)';
                closeBtn.onclick = (e) => {
                    e.stopPropagation();
                    sendCmd('close_tab', { idx: index });
                };

                tabEl.appendChild(icon);
                tabEl.appendChild(title);
                tabEl.appendChild(closeBtn);
                
                tabBar.insertBefore(tabEl, newTabBtn);
            });
        }

        renderTabs();
    </script>
</body>
</html>
    "#
    .to_string()
}
