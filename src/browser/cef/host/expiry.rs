use std::time::Instant;

use ::cef::*;

use super::{browser, with_host};

pub(super) fn schedule() {
    let deadline = with_host(|host| {
        if host.closing {
            return None;
        }
        let deadline = host.session.next_expiry()?;
        if host
            .expiry_task_deadline
            .is_some_and(|pending| pending <= deadline)
        {
            return None;
        }
        host.expiry_task_deadline = Some(deadline);
        Some(deadline)
    })
    .flatten();
    let Some(deadline) = deadline else {
        return;
    };
    let delay = deadline
        .saturating_duration_since(Instant::now())
        .as_millis() as i64
        + 1;
    let mut task = ExpiryTask::new(deadline);
    if post_delayed_task(ThreadId::UI, Some(&mut task), delay) != 1 {
        with_host(|host| {
            if host.expiry_task_deadline == Some(deadline) {
                host.expiry_task_deadline = None;
            }
        });
        crate::thor_warn!("thor::cef", "CEF rejected the page expiry timer");
    }
}

fn expire_background_pages(deadline: Instant) {
    let expired = with_host(|host| {
        if host.expiry_task_deadline != Some(deadline) {
            return Vec::new();
        }
        host.expiry_task_deadline = None;
        if host.closing {
            return Vec::new();
        }
        host.session.take_expired_tabs(Instant::now())
    })
    .unwrap_or_default();

    for id in expired {
        let still_background =
            with_host(|host| !host.closing && host.session.active_tab() != Some(id))
                .unwrap_or(false);
        if still_background {
            if let Some(host) = browser(id).and_then(|browser| browser.host()) {
                host.close_browser(0);
            }
        }
    }
    schedule();
}

wrap_task! {
    struct ExpiryTask { deadline: Instant }
    impl Task {
        fn execute(&self) {
            expire_background_pages(self.deadline);
        }
    }
}
