use super::*;
use serde_json::json;

fn deletion() -> PendingOperation {
    PendingOperation::DeleteObject {
        key: ObjectKey::new("patched_fixture", "fixture"),
        base_revision: Some(5),
    }
}
fn conflict() -> ShowSyncConflict {
    ShowSyncConflict {
        kind: "patched_fixture".into(),
        id: "fixture".into(),
        path: String::new(),
        base: None,
        mine: None,
        theirs: None,
        theirs_revision: Some(6),
        reason: ShowSyncConflictReason::ObjectModified,
    }
}
#[test]
fn legacy_fixture_delete_conflict_retains_delete_at_current_revision() {
    let operations = use_mine(&[conflict()], &BTreeMap::new(), &[deletion()], false).unwrap();
    assert_eq!(
        operations,
        vec![PendingOperation::DeleteObject {
            key: ObjectKey::new("patched_fixture", "fixture"),
            base_revision: Some(6)
        }]
    );
}
#[test]
fn rich_fixture_delete_conflict_retains_typed_intent() {
    let mut current = conflict();
    current.theirs = Some(json!({"name":"Control rename"}));
    let operations = use_mine(&[current], &BTreeMap::new(), &[deletion()], false).unwrap();
    assert!(matches!(
        &operations[0],
        PendingOperation::DeleteObject {
            base_revision: Some(6),
            ..
        }
    ));
}
#[test]
fn legacy_nulls_require_matching_typed_delete_and_explicit_revision() {
    let wrong = PendingOperation::DeleteObject {
        key: ObjectKey::new("patch_layer", "fixture"),
        base_revision: Some(5),
    };
    let update = PendingOperation::UpdateObject {
        key: ObjectKey::new("patched_fixture", "fixture"),
        fields: vec![],
    };
    for operations in [vec![], vec![wrong], vec![update]] {
        assert!(use_mine(&[conflict()], &BTreeMap::new(), &operations, false).is_err());
        let mut rich = conflict();
        rich.theirs = Some(json!({"name":"Control"}));
        assert!(use_mine(&[rich], &BTreeMap::new(), &operations, false).is_err());
    }
    let mut current = conflict();
    current.theirs_revision = None;
    assert!(use_mine(&[current], &BTreeMap::new(), &[deletion()], false).is_err());
}
#[test]
fn recovered_whole_body_delete_still_resolves_without_original_operation() {
    let mut current = conflict();
    current.theirs = Some(json!({"name":"recovered"}));
    assert!(matches!(
        &use_mine(&[current], &BTreeMap::new(), &[], true).unwrap()[0],
        PendingOperation::DeleteObject {
            base_revision: Some(6),
            ..
        }
    ));
}

struct TestHost(parking_lot::Mutex<viz_document::PlanningDocument>);
impl super::super::DocumentHost for TestHost {
    fn apply_remote(
        &self,
        edit: &mut dyn FnMut(&viz_document::PlanningDocument) -> Result<bool, String>,
    ) -> Result<(), String> {
        edit(&self.0.lock()).map(|_| ())
    }
    fn status_changed(&self, _: &crate::status::SyncStatus) {}
}
fn shared(directory: &std::path::Path) -> Shared {
    use super::super::Shadow;
    use crate::{
        binding::{SyncBinding, SyncBindingStore},
        desk::DeskClient,
        journal::Journal,
        mirror::Mirror,
        state::{DocumentReader, VersionedState},
        status::StatusFacts,
    };
    use parking_lot::Mutex;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    };
    let document =
        viz_document::PlanningDocument::create(directory.join("rig.show"), "Rig").unwrap();
    let mut reader = DocumentReader::default();
    let versioned = reader.read(&document).unwrap();
    let (journal, _) = Journal::open(&directory.join("journal.sqlite")).unwrap();
    let (mut mirror, _) = Mirror::open(&directory.join("mirror.sqlite")).unwrap();
    mirror
        .replace(VersionedState {
            show_revision: 811,
            ..VersionedState::default()
        })
        .unwrap();
    Shared {
        binding: Mutex::new(SyncBinding::new(
            None,
            uuid::Uuid::new_v4(),
            "http://127.0.0.1:1".into(),
            "Test".into(),
            811,
        )),
        store: SyncBindingStore::at(directory.join("bindings")),
        directory: directory.to_owned(),
        journal: Mutex::new(journal),
        mirror: Mutex::new(mirror),
        shadow: Mutex::new(Shadow { versioned, reader }),
        facts: Mutex::new(StatusFacts::default()),
        last_status: Mutex::new(None),
        host: Arc::new(TestHost(Mutex::new(document))),
        client: DeskClient::new("http://127.0.0.1:1").unwrap(),
        wake: tokio::sync::Notify::new(),
        stopped: AtomicBool::new(true),
        paused: AtomicBool::new(true),
        lose_next_reply: AtomicBool::new(false),
        snapshot_reads: AtomicU64::new(0),
        rebuild_mirror: AtomicBool::new(false),
        known_revisions: Mutex::new(BTreeMap::new()),
    }
}
fn directory() -> std::path::PathBuf {
    let directory = std::path::PathBuf::from(std::env::var_os("LIGHT_TMP_DIR").unwrap())
        .join(format!("tl823-resolve-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    directory
}
fn record_conflict(shared: &Shared, operation: PendingOperation) -> crate::journal::JournalEntry {
    let journal = shared.journal.lock();
    let old = journal
        .append(
            ShowEditIntent::from_operations(vec![operation]).unwrap(),
            811,
        )
        .unwrap();
    journal
        .record(
            old.seq,
            EntryState::Conflict,
            &crate::journal::EntryOutcome {
                show_revision: 811,
                conflicts: vec![conflict()],
                ..Default::default()
            },
        )
        .unwrap();
    old
}
#[test]
fn real_resolution_appends_fresh_request_before_superseding_and_survives_reopen() {
    use crate::journal::Journal;
    let directory = directory();
    let shared = shared(&directory);
    let old = record_conflict(&shared, deletion());
    shared.resolve(old.seq, Resolution::UseMine).unwrap();
    let pending = shared.journal.lock().pending().unwrap();
    assert_eq!(pending.len(), 1);
    let new = &pending[0];
    assert_ne!(old.request_id, new.request_id);
    assert_eq!(
        new.operations,
        vec![PendingOperation::DeleteObject {
            key: ObjectKey::new("patched_fixture", "fixture"),
            base_revision: Some(6)
        }]
    );
    assert!(
        shared.resolve(old.seq, Resolution::UseMine).is_err(),
        "no duplicate resolution"
    );
    drop(shared);
    let (journal, _) = Journal::open(&directory.join("journal.sqlite")).unwrap();
    assert_eq!(journal.pending().unwrap(), pending);
    let history = journal.entry(old.seq).unwrap().unwrap();
    assert_eq!(history.state, EntryState::Superseded);
    assert_eq!(history.operations, old.operations);
    drop(journal);
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn keep_control_does_not_queue_delete_and_invalid_legacy_intent_stays_unresolved() {
    let directory = directory();
    let shared = shared(&directory);
    let old = record_conflict(&shared, deletion());
    shared.resolve(old.seq, Resolution::KeepControl).unwrap();
    assert!(shared.journal.lock().pending().unwrap().is_empty());
    let old = record_conflict(
        &shared,
        PendingOperation::UpdateObject {
            key: ObjectKey::new("patched_fixture", "fixture"),
            fields: vec![],
        },
    );
    assert!(shared.resolve(old.seq, Resolution::UseMine).is_err());
    assert_eq!(
        shared.journal.lock().entry(old.seq).unwrap().unwrap().state,
        EntryState::Conflict
    );
    drop(shared);
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn failed_append_leaves_original_conflict_recoverable() {
    let directory = directory();
    let shared = shared(&directory);
    let old = record_conflict(&shared, deletion());
    let connection = rusqlite::Connection::open(directory.join("journal.sqlite")).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_append BEFORE INSERT ON entries BEGIN SELECT RAISE(ABORT, 'test append refused'); END;").unwrap();
    assert!(shared.resolve(old.seq, Resolution::UseMine).is_err());
    assert_eq!(
        shared.journal.lock().entry(old.seq).unwrap().unwrap().state,
        EntryState::Conflict
    );
    assert!(shared.journal.lock().pending().unwrap().is_empty());
    drop(connection);
    drop(shared);
    std::fs::remove_dir_all(directory).unwrap();
}
