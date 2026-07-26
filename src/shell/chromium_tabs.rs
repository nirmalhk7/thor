//! Chromium (CDP) content tabs for the dual-window M2 shell.
//! AppKit owns chrome; pages live in one headful Chromium process.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::engine::{
    discover_chromium_path, ChromiumResult, ChromiumRuntime, ChromiumRuntimeConfig, TabSession,
};
use crate::shell::engine_status;

use super::{sanitize_and_record, TabMeta};

pub struct ChromiumTabs {
    pub(crate) runtime: Arc<ChromiumRuntime>,
    pub(crate) sessions: HashMap<String, TabSession>,
    pub(crate) meta: HashMap<String, TabMeta>,
    pub(crate) active: String,
    hotkey_script: String,
}

impl ChromiumTabs {
    pub fn launch(hotkey_script: &str) -> anyhow::Result<Self> {
        if discover_chromium_path().is_none() {
            anyhow::bail!(
                "No Chrome/Chromium found. Install Google Chrome or set THOR_CHROMIUM_PATH to the binary."
            );
        }
        let runtime = ChromiumRuntime::launch_blocking(ChromiumRuntimeConfig {
            headful: true,
            prewarm_pool_size: 2,
            ..Default::default()
        })
        .map_err(|e| anyhow::anyhow!("Chromium launch failed: {e}"))?;

        println!(
            "⚡ [Thor] Chromium ready — {} (CDP {})",
            runtime.executable_path().display(),
            runtime.websocket_url()
        );

        Ok(Self {
            runtime,
            sessions: HashMap::new(),
            meta: HashMap::new(),
            active: String::new(),
            hotkey_script: hotkey_script.to_string(),
        })
    }

    pub fn touch(&mut self, tab_id: &str) {
        if let Some(meta) = self.meta.get_mut(tab_id) {
            meta.last_active = Instant::now();
        }
    }

    pub fn set_url_title(&mut self, tab_id: &str, url: &str, title: &str) {
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

    pub fn active_url(&self) -> String {
        self.meta
            .get(&self.active)
            .map(|m| m.url.clone())
            .unwrap_or_default()
    }

    pub fn resolved_url(&self, tab_id: &str) -> String {
        if let Some(session) = self.sessions.get(tab_id) {
            if let Ok(Some(live)) = session.url_blocking() {
                let live = live.trim().to_string();
                if !live.is_empty() && live != "about:blank" {
                    return live;
                }
            }
        }
        self.meta
            .get(tab_id)
            .map(|m| m.url.clone())
            .unwrap_or_default()
    }

    pub fn open_tab_palette_refs(&self) -> Vec<(&str, &str, &str)> {
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

    fn reinject(&self, session: &TabSession) {
        inject_thor_bridge(session, &self.hotkey_script);
    }

    pub fn create_tab(&mut self, tab_id: &str, url: &str) -> ChromiumResult<()> {
        let clean = sanitize_and_record(tab_id, url, "");
        let session = self.runtime.new_tab_blocking(Some(&clean))?;
        self.reinject(&session);
        self.meta.insert(
            tab_id.to_string(),
            TabMeta {
                last_active: Instant::now(),
                url: clean,
                title: "New Tab".into(),
            },
        );
        self.sessions.insert(tab_id.to_string(), session);
        self.active = tab_id.to_string();
        self.show_only(tab_id);
        Ok(())
    }

    pub fn show_only(&self, target_id: &str) {
        for (id, session) in self.sessions.iter() {
            if id == target_id {
                let _ = session.soft_unfreeze_blocking();
                let _ = session.bring_to_front_blocking();
            } else {
                let _ = session.soft_freeze_blocking();
            }
        }
    }

    pub fn navigate_active(&mut self, url: &str) -> ChromiumResult<String> {
        let id = self.active.clone();
        let clean = sanitize_and_record(&id, url, "");
        if let Some(session) = self.sessions.get(&id) {
            session.navigate_blocking(&clean)?;
            self.reinject(session);
            if let Ok(title) = session.title_blocking() {
                self.set_url_title(&id, &clean, &title);
            } else {
                self.set_url_title(&id, &clean, "");
            }
        }
        Ok(clean)
    }

    pub fn reload_active(&self) {
        if let Some(session) = self.sessions.get(&self.active) {
            let _ = session.reload_blocking();
            self.reinject(session);
        }
    }

    pub fn back_active(&self) {
        if let Some(session) = self.sessions.get(&self.active) {
            let _ = session.back_blocking();
        }
    }

    pub fn forward_active(&self) {
        if let Some(session) = self.sessions.get(&self.active) {
            let _ = session.forward_blocking();
        }
    }

    pub fn close_tab(&mut self, tab_id: &str) {
        crate::app_state::AppState::global()
            .tab_history()
            .clear_tab_history(tab_id);
        self.meta.remove(tab_id);
        if let Some(session) = self.sessions.remove(tab_id) {
            let _ = session.close_blocking();
        }
        if self.active == tab_id {
            let next = self
                .meta
                .iter()
                .max_by_key(|(_, m)| m.last_active)
                .map(|(id, _)| id.clone())
                .or_else(|| self.sessions.keys().next().cloned())
                .unwrap_or_default();
            self.active = next;
            if !self.active.is_empty() {
                let active = self.active.clone();
                self.show_only(&active);
                self.touch(&active);
            }
        }
    }

    pub fn switch_tab(&mut self, target_id: &str) -> Option<String> {
        if !self.sessions.contains_key(target_id) {
            return None;
        }
        self.active = target_id.to_string();
        self.show_only(target_id);
        self.touch(target_id);
        let url = self.resolved_url(target_id);
        if !url.is_empty() {
            self.set_url_title(target_id, &url, "");
        }
        Some(url)
    }

    pub fn open_devtools_active(&self) {
        if let Some(session) = self.sessions.get(&self.active) {
            match session.open_devtools_blocking() {
                Ok(()) => println!("⚡ [Thor] Chromium DevTools opened"),
                Err(e) => eprintln!("⚡ [Thor] DevTools failed: {e}"),
            }
        }
    }

    pub fn evaluate_active(&self, js: &str) -> Option<serde_json::Value> {
        self.sessions
            .get(&self.active)
            .and_then(|s| s.evaluate_js_blocking(js).ok())
    }

    pub fn drain_ipc_queue(&self) -> Vec<String> {
        let raw = self.evaluate_active(
            r#"(function(){
              var q = window.__thorIpcQueue || [];
              window.__thorIpcQueue = [];
              return q;
            })()"#,
        );
        match raw {
            Some(serde_json::Value::Array(items)) => items
                .into_iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn gc_idle(&mut self, idle: std::time::Duration) {
        let now = Instant::now();
        let victims: Vec<String> = self
            .meta
            .iter()
            .filter(|(id, meta)| {
                self.sessions.len() > 1
                    && now.duration_since(meta.last_active) >= idle
                    && (*id != &self.active || self.sessions.len() > 1)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in victims {
            if self.sessions.len() <= 1 {
                break;
            }
            println!("⚡ [Thor GC] Closing idle Chromium tab (≥24h): {id}");
            self.close_tab(&id);
        }
    }
}

pub fn inject_thor_bridge(session: &TabSession, hotkey_script: &str) {
    let status = engine_status::engine_status_script();
    let _ = session.evaluate_js_blocking(&status);
    if !hotkey_script.is_empty() {
        let _ = session.evaluate_js_blocking(hotkey_script);
    }
}
