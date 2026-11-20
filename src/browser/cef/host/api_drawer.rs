use std::cell::{Cell, RefCell};

use ::cef::*;

use super::{palette, queue_action, theme, with_host, Action, BrowserKeys};
use crate::workbench::{API_URL, HOME_URL, SECURITY_URL};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_drawer_only_hosts_its_own_document() {
        assert!(drawer_document(API_URL));
        assert!(drawer_document("thor://workbench/api#saved"));
        assert!(!drawer_document(SECURITY_URL));
        assert!(!drawer_document("https://workbench/api"));
        assert!(!drawer_document("thor://workbench@other/api"));
    }
}
