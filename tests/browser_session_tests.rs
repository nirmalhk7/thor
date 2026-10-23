use thor::browser::session::{BrowserSession, SessionError};

#[test]
fn closing_a_tab_erases_its_history_without_removing_persistent_identity() {
    let mut session = BrowserSession::default();
    let tab = session.open_tab(0).unwrap();
    session
        .record_navigation(tab, "https://app.test/login", "Login")
        .unwrap();
    session
        .record_navigation(tab, "https://app.test/private", "Private")
        .unwrap();
    assert_eq!(session.history(tab).unwrap().len(), 2);
    session.close_tab(tab).unwrap();
    assert_eq!(session.history(tab).unwrap_err(), SessionError::TabNotFound);
    assert!(!session.identities()[0].disposable);
}

#[test]
fn disposable_identity_lives_until_its_last_tab_closes() {
    let mut session = BrowserSession::default();
    let identity = session.add_identity("Anonymous", true).unwrap();
    let first = session.open_tab(identity).unwrap();
    let second = session.open_tab(identity).unwrap();
    assert_eq!(session.close_tab(first).unwrap(), None);
    assert_eq!(session.close_tab(second).unwrap(), Some(identity));
    assert_eq!(
        session.open_tab(identity).unwrap_err(),
        SessionError::IdentityNotFound
    );
}

#[test]
fn navigation_invalidates_a_previously_selected_target() {
    let mut session = BrowserSession::default();
    let tab = session.open_tab(0).unwrap();
    let selected = session.target(tab).unwrap();
    session
        .record_navigation(tab, "https://app.test/next", "Next")
        .unwrap();
    assert_eq!(
        session.validate_target(selected),
        Err(SessionError::StaleTarget)
    );
    assert!(session
        .validate_target(session.target(tab).unwrap())
        .is_ok());
}

#[test]
fn title_changes_do_not_invalidate_a_selected_document() {
    let mut session = BrowserSession::default();
    let tab = session.open_tab(0).unwrap();
    session
        .record_navigation(tab, "https://app.test/", "Loading")
        .unwrap();
    let target = session.target(tab).unwrap();
    session.update_title(tab, "Application").unwrap();
    assert!(session.validate_target(target).is_ok());
    assert_eq!(
        session.history(tab).unwrap().back().unwrap().title,
        "Application"
    );
}

#[test]
fn closing_active_tab_selects_a_surviving_tab_and_ids_are_not_reused() {
    let mut session = BrowserSession::default();
    let first = session.open_tab(0).unwrap();
    let second = session.open_tab(0).unwrap();
    session.activate(first).unwrap();
    session.close_tab(first).unwrap();
    assert_eq!(session.active_tab(), Some(second));
    let third = session.open_tab(0).unwrap();
    assert_ne!(third, first);
}

#[test]
fn passive_history_is_bounded_and_isolated_per_tab() {
    let mut session = BrowserSession::default();
    let first = session.open_tab(0).unwrap();
    let second = session.open_tab(0).unwrap();
    for index in 0..150 {
        session
            .record_navigation(first, &format!("https://app.test/{index}"), "Page")
            .unwrap();
    }
    let history = session.history(first).unwrap();
    assert_eq!(history.len(), 100);
    assert_eq!(history.front().unwrap().url, "https://app.test/50");
    assert!(session.history(second).unwrap().is_empty());
}
