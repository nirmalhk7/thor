//! Process / page freeze hooks for Thor (OS signals + CDP lifecycle).

use chromiumoxide::cdp::browser_protocol::page::SetWebLifecycleStateParams;
use chromiumoxide::Page;

use super::chromium_runtime::ChromiumResult;

/// OS-level suspend / resume of a Chromium process.
///
/// On Unix this uses `SIGSTOP` / `SIGCONT`. Windows currently best-effort-logs
/// (EmptyWorkingSet-style suspend requires more privileges / NtSuspendProcess).
pub struct ProcessFreezer;

impl ProcessFreezer {
    pub fn freeze_process(pid: u32) -> Result<(), String> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            // SAFETY: kill with SIGSTOP on a known Chromium pid we spawned.
            let rc = unsafe { libc::kill(pid as libc::pid_t, libc::SIGSTOP) };
            if rc != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            // Best-effort placeholder: full NtSuspendProcess wiring is deferred.
            let _ = pid;
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = pid;
            Err("Unsupported OS for freezing".to_string())
        }
    }

    pub fn unfreeze_process(pid: u32) -> Result<(), String> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let rc = unsafe { libc::kill(pid as libc::pid_t, libc::SIGCONT) };
            if rc != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            let _ = pid;
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = pid;
            Err("Unsupported OS for unfreezing".to_string())
        }
    }
}

/// CDP `Page.setWebLifecycleState` helpers (frozen / active).
///
/// Some Chromium builds reject the experimental command; callers should treat
/// errors as soft failures and fall back to [`ProcessFreezer`].
pub struct CdpPageFreezer;

impl CdpPageFreezer {
    pub async fn freeze_page(page: &Page) -> ChromiumResult<()> {
        use chromiumoxide::cdp::browser_protocol::page::SetWebLifecycleStateState;
        let params = SetWebLifecycleStateParams::new(SetWebLifecycleStateState::Frozen);
        page.execute(params).await?;
        Ok(())
    }

    pub async fn unfreeze_page(page: &Page) -> ChromiumResult<()> {
        use chromiumoxide::cdp::browser_protocol::page::SetWebLifecycleStateState;
        let params = SetWebLifecycleStateParams::new(SetWebLifecycleStateState::Active);
        page.execute(params).await?;
        Ok(())
    }
}

/// Prefetch pages into RAM on hover (madvise WILLNEED).
pub struct WarmUpThread;

impl WarmUpThread {
    pub fn prefetch_ram_on_hover(address: *mut libc::c_void, length: usize) {
        let addr_val = address as usize;
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let ptr = addr_val as *mut libc::c_void;
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            {
                unsafe {
                    libc::madvise(ptr, length, libc::MADV_WILLNEED);
                }
            }
            #[cfg(target_os = "windows")]
            {
                let _ = (ptr, length);
            }
        });
    }
}

use super::tab_session::TabLifecycle;

/// State machine manager for 4-Stage Tab Lifecycle
/// (Active -> Frozen -> Discarded -> Expired).
pub struct TabLifecycleManager;

impl TabLifecycleManager {
    /// Validate state transition according to 4-Stage Lifecycle rules
    pub fn can_transition(from: TabLifecycle, to: TabLifecycle) -> bool {
        match (from, to) {
            (a, b) if a == b => true,
            (TabLifecycle::Expired, _) => false,
            (TabLifecycle::Active, TabLifecycle::Frozen)
            | (TabLifecycle::Active, TabLifecycle::Discarded)
            | (TabLifecycle::Active, TabLifecycle::Expired) => true,
            (TabLifecycle::Frozen, TabLifecycle::Active)
            | (TabLifecycle::Frozen, TabLifecycle::Discarded)
            | (TabLifecycle::Frozen, TabLifecycle::Expired) => true,
            (TabLifecycle::Discarded, TabLifecycle::Active)
            | (TabLifecycle::Discarded, TabLifecycle::Expired) => true,
            _ => false,
        }
    }

    /// Perform validated state transition
    pub fn transition(current: TabLifecycle, next: TabLifecycle) -> Result<TabLifecycle, String> {
        if Self::can_transition(current, next) {
            Ok(next)
        } else {
            Err(format!("Invalid 4-Stage Lifecycle transition from {:?} to {:?}", current, next))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freeze_invalid_pid_errors_on_unix() {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            // PID 1 may or may not allow SIGSTOP depending on privileges; use absurd pid.
            let err = ProcessFreezer::freeze_process(u32::MAX - 2);
            assert!(err.is_err());
        }
    }

    #[test]
    fn test_four_stage_lifecycle_transitions() {
        assert!(TabLifecycleManager::can_transition(TabLifecycle::Active, TabLifecycle::Frozen));
        assert!(TabLifecycleManager::can_transition(TabLifecycle::Frozen, TabLifecycle::Discarded));
        assert!(TabLifecycleManager::can_transition(TabLifecycle::Discarded, TabLifecycle::Expired));
        assert!(TabLifecycleManager::can_transition(TabLifecycle::Discarded, TabLifecycle::Active));
        assert!(!TabLifecycleManager::can_transition(TabLifecycle::Expired, TabLifecycle::Active));

        let state = TabLifecycleManager::transition(TabLifecycle::Active, TabLifecycle::Frozen).unwrap();
        assert_eq!(state, TabLifecycle::Frozen);
    }
}
