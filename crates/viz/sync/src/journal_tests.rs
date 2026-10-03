use super::*;
use crate::intent::ShowEditIntent;
use crate::state::{ObjectKey, SyncState, VersionedState};
use serde_json::json;

fn temporary(name: &str) -> PathBuf {
    let base = PathBuf::from(
        std::env::var_os("LIGHT_TMP_DIR").expect("canonical test temporary directory"),
    );
    let directory = base.join(format!("viz-sync-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn intent(id: &str) -> ShowEditIntent {
    let mut after = SyncState::default();
    after
        .objects
        .insert(ObjectKey::new("cad_annotation", id), json!({"text": id}));
    ShowEditIntent::capture(
        &SyncState::default(),
        &after,
        &VersionedState::default(),
        &|_| None,
    )
    .unwrap()
}

#[test]
fn entries_survive_reopening_in_order_with_their_identity() {
    let path = temporary("journal-order").join("journal.sqlite");
    let (journal, damaged) = Journal::open(&path).unwrap();
    assert!(damaged.is_none());
    let first = journal.append(intent("a"), 3).unwrap();
    let second = journal.append(intent("b"), 3).unwrap();
    drop(journal);
    let (journal, _) = Journal::open(&path).unwrap();
    let pending = journal.pending().unwrap();
    assert_eq!(
        pending
            .iter()
            .map(|entry| entry.request_id.clone())
            .collect::<Vec<_>>(),
        [first.request_id, second.request_id]
    );
    assert_eq!(journal.counts().unwrap().pending, 2);
}

#[test]
fn an_outcome_moves_an_entry_out_of_the_pending_queue() {
    let path = temporary("journal-outcome").join("journal.sqlite");
    let (journal, _) = Journal::open(&path).unwrap();
    let entry = journal.append(intent("a"), 3).unwrap();
    journal
        .record(
            entry.seq,
            EntryState::Accepted,
            &EntryOutcome {
                show_revision: 4,
                ..EntryOutcome::default()
            },
        )
        .unwrap();
    assert!(journal.pending().unwrap().is_empty());
    assert!(journal.holds(&entry.request_id).unwrap());
    journal.prune_accepted(4, 0).unwrap();
    assert!(!journal.holds(&entry.request_id).unwrap());
}

#[test]
fn a_damaged_journal_is_set_aside_not_deleted() {
    let directory = temporary("journal-damaged");
    let path = directory.join("journal.sqlite");
    std::fs::write(&path, b"this is not a database").unwrap();
    let (journal, damaged) = Journal::open(&path).unwrap();
    let damaged = damaged.expect("the damage is reported");
    let aside = damaged.set_aside.expect("the damaged file is kept");
    assert_eq!(std::fs::read(aside).unwrap(), b"this is not a database");
    assert!(journal.pending().unwrap().is_empty());
}

#[test]
fn the_mirror_survives_reopening_and_a_damaged_mirror_is_untrusted() {
    use crate::mirror::{Mirror, MirrorChange};
    let directory = temporary("mirror");
    let path = directory.join("mirror.sqlite");
    let (mut mirror, _) = Mirror::open(&path).unwrap();
    assert!(!mirror.trusted(), "a new mirror holds no desk snapshot yet");
    mirror.replace(VersionedState::default()).unwrap();
    let key = ObjectKey::new("cad_annotation", "a");
    mirror
        .apply(
            5,
            &[
                MirrorChange::Object {
                    key: key.clone(),
                    revision: 2,
                    body: json!({"text": "a"}),
                },
                MirrorChange::Metadata {
                    key: "previs.show_version".into(),
                    value: Some("2".into()),
                },
            ],
        )
        .unwrap();
    drop(mirror);
    let (mirror, damaged) = Mirror::open(&path).unwrap();
    assert!(damaged.is_none());
    assert!(mirror.trusted());
    assert_eq!(mirror.show_revision(), 5);
    assert_eq!(mirror.state().revisions[&key], 2);
    assert_eq!(mirror.state().state.metadata["previs.show_version"], "2");
    drop(mirror);
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
    std::fs::write(&path, b"garbage").unwrap();
    let (mirror, damaged) = Mirror::open(&path).unwrap();
    assert!(damaged.is_some());
    assert!(!mirror.trusted());
}
