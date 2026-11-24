use std::collections::{BTreeMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const BACKGROUND_TIMEOUT: Duration = Duration::from_secs(30 * 60);

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
    expires_at: Option<Instant>,
    protected_frames: HashSet<String>,
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
        self.open_tab_at(identity, Instant::now())
    }

    fn open_tab_at(&mut self, identity: u64, now: Instant) -> Result<u64, SessionError> {
        self.identity(identity)?;
        let id = self.next_tab;
        self.next_tab = id.checked_add(1).ok_or(SessionError::IdExhausted)?;
        self.tabs.insert(
            id,
            Tab {
                identity,
                generation: 1,
                history: VecDeque::new(),
                expires_at: None,
                protected_frames: HashSet::new(),
            },
        );
        self.activate_at(id, now)?;
        Ok(id)
    }

    pub fn activate(&mut self, id: u64) -> Result<(), SessionError> {
        self.activate_at(id, Instant::now())
    }

    fn activate_at(&mut self, id: u64, now: Instant) -> Result<(), SessionError> {
        self.tabs
            .get_mut(&id)
            .ok_or(SessionError::TabNotFound)?
            .expires_at = None;
        let previous = self.active.replace(id);
        if let Some(previous) = previous.filter(|previous| *previous != id) {
            if let Some(tab) = self.tabs.get_mut(&previous) {
                tab.expires_at = Some(now + BACKGROUND_TIMEOUT);
            }
        }
        Ok(())
    }

    pub(crate) fn next_expiry(&self) -> Option<Instant> {
        self.tabs
            .values()
            .filter(|tab| tab.protected_frames.is_empty())
            .filter_map(|tab| tab.expires_at)
            .min()
    }

    /// Each background period can request one close. Actual removal belongs
    /// to CEF's close callback, including when a page handles beforeunload.
    pub(crate) fn take_expired_tabs(&mut self, now: Instant) -> Vec<u64> {
        let mut expired = Vec::new();
        for (&id, tab) in &mut self.tabs {
            if tab.protected_frames.is_empty()
                && tab.expires_at.is_some_and(|deadline| deadline <= now)
            {
                tab.expires_at = None;
                expired.push(id);
            }
        }
        expired
    }

    pub(crate) fn set_frame_protection(&mut self, id: u64, frame: &str, protected: bool) {
        if let Some(tab) = self.tabs.get_mut(&id) {
            if protected {
                tab.protected_frames.insert(frame.to_owned());
            } else {
                tab.protected_frames.remove(frame);
            }
        }
    }

    pub(crate) fn clear_protection(&mut self, id: u64) {
        if let Some(tab) = self.tabs.get_mut(&id) {
            tab.protected_frames.clear();
        }
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
            if let Some(replacement) = self.active.and_then(|active| self.tabs.get_mut(&active)) {
                replacement.expires_at = None;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_page_expires_at_thirty_minutes_and_is_removed_only_on_close() {
        let mut session = BrowserSession::default();
        let start = Instant::now();
        let background = session.open_tab_at(0, start).unwrap();
        session
            .record_navigation(background, "https://example.test/", "Example")
            .unwrap();
        let left = start + Duration::from_secs(10);
        let foreground = session.open_tab_at(0, left).unwrap();

        assert_eq!(
            session.next_expiry(),
            Some(left + Duration::from_secs(1800))
        );
        assert!(session
            .take_expired_tabs(left + Duration::from_secs(1799))
            .is_empty());
        assert_eq!(
            session.take_expired_tabs(left + Duration::from_secs(1800)),
            vec![background]
        );
        assert!(session
            .take_expired_tabs(left + Duration::from_secs(1801))
            .is_empty());
        assert_eq!(session.active_tab(), Some(foreground));
        assert_eq!(session.history(background).unwrap().len(), 1);
        session.close_tab(background).unwrap();
        assert_eq!(
            session.history(background).unwrap_err(),
            SessionError::TabNotFound
        );
    }

    #[test]
    fn returning_restarts_the_clock_only_when_the_page_goes_back_into_the_background() {
        let mut session = BrowserSession::default();
        let start = Instant::now();
        let first = session.open_tab_at(0, start).unwrap();
        let second = session.open_tab_at(0, start).unwrap();
        let returned = start + Duration::from_secs(1799);
        session.activate_at(first, returned).unwrap();

        assert!(session
            .take_expired_tabs(start + Duration::from_secs(1800))
            .is_empty());
        let left_again = start + Duration::from_secs(1900);
        session.activate_at(second, left_again).unwrap();
        assert_eq!(
            session.next_expiry(),
            Some(left_again + Duration::from_secs(1800))
        );
        assert!(session
            .take_expired_tabs(left_again + Duration::from_secs(1799))
            .is_empty());
        assert_eq!(
            session.take_expired_tabs(left_again + Duration::from_secs(1800)),
            vec![first]
        );
    }

    #[test]
    fn expired_page_waits_for_the_last_protected_frame_and_keeps_its_deadline() {
        let mut session = BrowserSession::default();
        let start = Instant::now();
        let page = session.open_tab_at(0, start).unwrap();
        session.open_tab_at(0, start).unwrap();
        session.set_frame_protection(page, "draft", true);
        session.set_frame_protection(page, "call", true);

        let expired_at = start + Duration::from_secs(1800);
        assert!(session.take_expired_tabs(expired_at).is_empty());
        assert_eq!(session.next_expiry(), None);
        session.set_frame_protection(page, "draft", false);
        assert!(session.take_expired_tabs(expired_at).is_empty());
        session.set_frame_protection(page, "call", false);
        assert_eq!(session.next_expiry(), Some(expired_at));
        assert_eq!(session.take_expired_tabs(expired_at), vec![page]);
    }

    #[test]
    fn closing_the_active_page_cancels_its_replacement_page_expiry() {
        let mut session = BrowserSession::default();
        let start = Instant::now();
        let replacement = session.open_tab_at(0, start).unwrap();
        let active = session.open_tab_at(0, start).unwrap();
        session.close_tab(active).unwrap();

        assert_eq!(session.active_tab(), Some(replacement));
        assert!(session
            .take_expired_tabs(start + Duration::from_secs(1800))
            .is_empty());
    }
}
