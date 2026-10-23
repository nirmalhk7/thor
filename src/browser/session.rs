use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    #[error("Tab not found")]
    TabNotFound,
    #[error("Identity not found")]
    IdentityNotFound,
    #[error("Navigation invalidated the selected target")]
    StaleTarget,
    #[error("Identity name is empty")]
    EmptyIdentityName,
    #[error("Browser identifier space exhausted")]
    IdExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub id: u64,
    pub name: String,
    pub disposable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabRef {
    pub id: u64,
    pub generation: u64,
}

#[derive(Debug, Clone)]
pub struct Navigation {
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TabSummary {
    pub target: TabRef,
    pub identity_id: u64,
    pub url: String,
    pub title: String,
}

#[derive(Debug)]
struct Tab {
    identity: u64,
    generation: u64,
    history: VecDeque<Navigation>,
}

/// Deliberately has no serialization implementation: browsing never becomes a
/// project file, session restore archive, or crash-recovery database.
pub struct BrowserSession {
    identities: BTreeMap<u64, Identity>,
    tabs: BTreeMap<u64, Tab>,
    active: Option<u64>,
    next_tab: u64,
    next_identity: u64,
}

impl Default for BrowserSession {
    fn default() -> Self {
        let identity = Identity {
            id: 0,
            name: "Default".into(),
            disposable: false,
        };
        Self {
            identities: BTreeMap::from([(0, identity)]),
            tabs: BTreeMap::new(),
            active: None,
            next_tab: 1,
            next_identity: 1,
        }
    }
}

impl BrowserSession {
    pub fn identities(&self) -> Vec<&Identity> {
        self.identities.values().collect()
    }

    pub fn identity(&self, id: u64) -> Result<&Identity, SessionError> {
        self.identities
            .get(&id)
            .ok_or(SessionError::IdentityNotFound)
    }

    pub fn add_identity(&mut self, name: &str, disposable: bool) -> Result<u64, SessionError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(SessionError::EmptyIdentityName);
        }
        let id = self.next_identity;
        self.next_identity = id.checked_add(1).ok_or(SessionError::IdExhausted)?;
        self.identities.insert(
            id,
            Identity {
                id,
                name: name.into(),
                disposable,
            },
        );
        Ok(id)
    }

    pub fn open_tab(&mut self, identity: u64) -> Result<u64, SessionError> {
        self.identity(identity)?;
        let id = self.next_tab;
        self.next_tab = id.checked_add(1).ok_or(SessionError::IdExhausted)?;
        self.tabs.insert(
            id,
            Tab {
                identity,
                generation: 1,
                history: VecDeque::new(),
            },
        );
        self.active = Some(id);
        Ok(id)
    }

    pub fn activate(&mut self, id: u64) -> Result<(), SessionError> {
        self.target(id)?;
        self.active = Some(id);
        Ok(())
    }

    pub fn active_tab(&self) -> Option<u64> {
        self.active
    }

    pub fn tab_ids(&self) -> Vec<u64> {
        self.tabs.keys().copied().collect()
    }

    pub fn summaries(&self) -> Vec<TabSummary> {
        self.tabs
            .iter()
            .map(|(id, tab)| {
                let current = tab.history.back();
                TabSummary {
                    target: TabRef {
                        id: *id,
                        generation: tab.generation,
                    },
                    identity_id: tab.identity,
                    url: current.map(|entry| entry.url.clone()).unwrap_or_default(),
                    title: current.map(|entry| entry.title.clone()).unwrap_or_default(),
                }
            })
            .collect()
    }

    pub fn tab_identity(&self, id: u64) -> Result<u64, SessionError> {
        self.tabs
            .get(&id)
            .map(|tab| tab.identity)
            .ok_or(SessionError::TabNotFound)
    }

    pub fn target(&self, id: u64) -> Result<TabRef, SessionError> {
        let tab = self.tabs.get(&id).ok_or(SessionError::TabNotFound)?;
        Ok(TabRef {
            id,
            generation: tab.generation,
        })
    }

    pub fn validate_target(&self, target: TabRef) -> Result<(), SessionError> {
        if self.target(target.id)? != target {
            return Err(SessionError::StaleTarget);
        }
        Ok(())
    }

    pub fn record_navigation(
        &mut self,
        id: u64,
        url: &str,
        title: &str,
    ) -> Result<(), SessionError> {
        let tab = self.tabs.get_mut(&id).ok_or(SessionError::TabNotFound)?;
        tab.generation = tab
            .generation
            .checked_add(1)
            .ok_or(SessionError::IdExhausted)?;
        if let Some(last) = tab.history.back_mut().filter(|entry| entry.url == url) {
            last.title = title.into();
            return Ok(());
        }
        if tab.history.len() == 100 {
            tab.history.pop_front();
        }
        tab.history.push_back(Navigation {
            url: url.into(),
            title: title.into(),
        });
        Ok(())
    }

    pub fn history(&self, id: u64) -> Result<&VecDeque<Navigation>, SessionError> {
        self.tabs
            .get(&id)
            .map(|tab| &tab.history)
            .ok_or(SessionError::TabNotFound)
    }

    pub fn update_title(&mut self, id: u64, title: &str) -> Result<(), SessionError> {
        let tab = self.tabs.get_mut(&id).ok_or(SessionError::TabNotFound)?;
        if let Some(entry) = tab.history.back_mut() {
            entry.title = title.into();
        }
        Ok(())
    }

    /// Return the disposable identity whose final tab closed. The CEF owner
    /// releases that request context only after its browser has really closed.
    pub fn close_tab(&mut self, id: u64) -> Result<Option<u64>, SessionError> {
        let tab = self.tabs.remove(&id).ok_or(SessionError::TabNotFound)?;
        if self.active == Some(id) {
            self.active = self.tabs.keys().next_back().copied();
        }
        let identity = self.identity(tab.identity)?;
        if identity.disposable
            && !self
                .tabs
                .values()
                .any(|other| other.identity == tab.identity)
        {
            self.identities.remove(&tab.identity);
            return Ok(Some(tab.identity));
        }
        Ok(None)
    }
}
