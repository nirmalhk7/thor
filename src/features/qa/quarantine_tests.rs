use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct DataDir {
    path: PathBuf,
    cleanup: bool,
}

impl DataDir {
    fn enter(case: &str) -> Option<Self> {
        if std::env::var("THOR_QA_TEST_CASE").as_deref() == Ok(case) {
            return Some(Self {
                path: PathBuf::from(std::env::var_os("THOR_DATA_DIR").unwrap()),
                cleanup: false,
            });
        }
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "thor-qa-state-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        let data = Self {
            path,
            cleanup: true,
        };
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(format!("features::qa::quarantine_tests::{case}"))
            .args(["--nocapture", "--test-threads=1"])
            .env("THOR_QA_TEST_CASE", case)
            .env("THOR_DATA_DIR", &data.path)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("running 1 test"),
            "isolated test not selected: {stdout}"
        );
        assert!(
            output.status.success(),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        None
    }
}

impl Drop for DataDir {
    fn drop(&mut self) {
        if self.cleanup {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[test]
fn ingestion_preserves_unknown_actions_and_uses_last_complete_duplicate() {
    let Some(_data) =
        DataDir::enter("ingestion_preserves_unknown_actions_and_uses_last_complete_duplicate")
    else {
        return;
    };
    let engine = QaEngine::new();
    engine.ingest_quarantine_yaml("action: skip\nreason: ignored\n- test: 'first'\n  action: strange\n  reason: 'why'\n- test: missing-action\n  reason: discarded\n- test: first\n  action: keep\n  reason: last\n- test: unknown\n  action: custom\n  reason: \"a\\\"b\"\n");
    let first = engine.quarantine_status("first").unwrap();
    assert_eq!(
        (first.action.as_str(), first.reason.as_str()),
        ("keep", "last")
    );
    assert!(first.persist_ok);
    assert!(first.persist_err.is_none());
    assert!(engine.quarantine_status("missing-action").is_none());
    let unknown = engine.quarantine_status("unknown").unwrap();
    assert_eq!(unknown.action, "custom");
    assert_eq!(unknown.reason, "a\\\"b");
    assert!(unknown.quarantine_yaml.contains("action: custom"));
}

#[test]
fn persistence_merges_disk_filters_actions_and_removes_expired_entries() {
    let Some(_data) =
        DataDir::enter("persistence_merges_disk_filters_actions_and_removes_expired_entries")
    else {
        return;
    };
    let engine = QaEngine::new();
    std::fs::write(QaEngine::quarantine_path(), "quarantine:\n- test: retained\n action: skip\n reason: disk\n- test: retained\n action: custom\n reason: invalid-must-not-overwrite\n- test: expire-me\n action: quarantine\n reason: old\n- test: unknown-only\n action: custom\n reason: ignored\n").unwrap();
    engine.ingest_quarantine_yaml("- test: expire-me\n action: expire\n reason: remove\n- test: patched\n action: keep\n reason: memory\n");
    let path = engine.persist_quarantine().unwrap();
    assert_eq!(path, QaEngine::quarantine_path());
    let yaml = std::fs::read_to_string(path).unwrap();
    assert!(yaml.find("patched").unwrap() < yaml.find("retained").unwrap());
    assert!(yaml.contains("memory"));
    assert!(yaml.contains("disk"));
    assert!(!yaml.contains("expire-me"));
    assert!(!yaml.contains("unknown-only"));
    assert!(!yaml.contains("invalid-must-not-overwrite"));
    let hydrated = QaEngine::new();
    assert_eq!(
        hydrated.quarantine_status("retained").unwrap().action,
        "skip"
    );
    assert_eq!(
        hydrated.quarantine_status("patched").unwrap().reason,
        "memory"
    );
}

#[test]
fn persistence_reports_directory_and_write_failures_without_losing_memory_rows() {
    let Some(data) = DataDir::enter(
        "persistence_reports_directory_and_write_failures_without_losing_memory_rows",
    ) else {
        return;
    };
    let engine = QaEngine::new();
    engine.ingest_quarantine_yaml("- test: live\n action: keep\n reason: memory\n");
    let blocked = data.path.join("blocked");
    std::fs::write(&blocked, "file").unwrap();
    std::env::set_var("THOR_DATA_DIR", &blocked);
    assert!(engine
        .persist_quarantine()
        .unwrap_err()
        .starts_with("create quarantine dir: "));
    std::env::set_var("THOR_DATA_DIR", &data.path);
    std::fs::create_dir(QaEngine::quarantine_path()).unwrap();
    assert!(engine
        .persist_quarantine()
        .unwrap_err()
        .starts_with("write quarantine.yaml: "));
    assert_eq!(engine.quarantine_status("live").unwrap().reason, "memory");
}

#[test]
fn empty_quarantine_persistence_writes_empty_list() {
    let Some(_data) = DataDir::enter("empty_quarantine_persistence_writes_empty_list") else {
        return;
    };
    let engine = QaEngine::new();
    engine.ingest_quarantine_yaml("- test: ignored\n action: custom\n reason: not-persistent\n");
    let path = engine.persist_quarantine().unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "quarantine: []\n");
}
