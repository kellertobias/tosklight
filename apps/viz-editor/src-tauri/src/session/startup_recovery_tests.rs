use super::*;

fn scratch() -> PathBuf {
    let base =
        PathBuf::from(std::env::var_os("LIGHT_TMP_DIR").expect("canonical temporary directory"));
    let path = base.join(format!("architect-startup-recovery-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn recent_session(directory: &Path, path: &Path) -> Session {
    let recent = RecentShow::at(directory.join("recent-show"));
    recent.remember(path);
    let session = Session::default();
    session.set_recent_store(recent);
    session
}

#[test]
fn startup_recovery_healthy_and_no_recent_have_no_notice() {
    let empty = Session::default();
    empty.reopen_recent();
    assert!(empty.startup_recovery().is_none());
    let directory = scratch();
    let path = directory.join("supported.show");
    drop(PlanningDocument::create(&path, "Supported").unwrap());
    let session = recent_session(&directory, &path);
    session.reopen_recent();
    assert_eq!(session.document_name().as_deref(), Some("Supported"));
    assert!(session.startup_recovery().is_none());
    drop(session);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn startup_recovery_preserves_malformed_and_incompatible_files_and_identifies_fallback() {
    for malformed in [false, true] {
        let directory = scratch();
        let path = directory.join("refused.show");
        if malformed {
            std::fs::write(&path, b"not a SQLite show").unwrap();
        } else {
            drop(PlanningDocument::create(&path, "Older show").unwrap());
            let store = light_show::ShowStore::open(&path).unwrap();
            let fixture = Uuid::new_v4().to_string();
            store.put_object("preset", "3.1", &serde_json::json!({
                "family":"Position", "values":{fixture:{"pan":{"kind":"normalized","value":0.5}}}
            }), 0).unwrap();
        }
        let original = std::fs::read(&path).unwrap();
        let session = recent_session(&directory, &path);
        session.reopen_recent();
        assert!(session.document_name().is_none());
        let refused = session.startup_recovery().unwrap();
        assert_eq!(refused.path, path.to_string_lossy());
        assert!(!refused.reason.is_empty());
        if !malformed {
            assert!(refused.reason.contains("pan"), "{}", refused.reason);
        }
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let fallback_path = directory.join("separate-demo.show");
        drop(PlanningDocument::create(&fallback_path, "Demo fallback").unwrap());
        let fallback = session.open(&fallback_path);
        session.complete_startup_fallback(&fallback);
        let late_mount = session.startup_recovery().unwrap();
        let identified = late_mount.fallback.unwrap();
        assert_eq!(identified.name, "Demo fallback");
        assert_eq!(identified.path, fallback_path.to_string_lossy());
        assert_eq!(late_mount.reason, refused.reason);
        assert_eq!(std::fs::read(&path).unwrap(), original);
        // Snapshot reads do not consume a startup failure before a later window mounts.
        assert!(session.startup_recovery().is_some());
        drop(session);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn startup_recovery_retains_fallback_failure_without_opening_a_document() {
    let directory = scratch();
    let path = directory.join("malformed.show");
    std::fs::write(&path, b"invalid show").unwrap();
    let session = recent_session(&directory, &path);
    session.reopen_recent();
    session.complete_startup_fallback(&Err("packaged Demo Show is missing".into()));
    let recovery = session.startup_recovery().unwrap();
    assert!(recovery.fallback.is_none());
    assert_eq!(
        recovery.fallback_error.as_deref(),
        Some("packaged Demo Show is missing")
    );
    assert!(session.document_name().is_none());
    assert_eq!(std::fs::read(path).unwrap(), b"invalid show");
    std::fs::remove_dir_all(directory).unwrap();
}
