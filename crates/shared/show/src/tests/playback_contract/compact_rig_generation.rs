//! Explicit repository asset regeneration; this is never called by a show reader.
//! Run after artifact-path initialization:
//! LIGHT_REGENERATE_COMPACT_RIG=1 cargo test -p light-show regenerate_compact_rig_with_canonical_writer -- --ignored
use crate::{ShowStore, validate_show_playback_contract};
use rusqlite::{Connection, OpenFlags};
use serde_json::json;
use std::{fs, path::Path};

#[test]
#[ignore = "explicit, audited publication of the owned compact-rig fixture"]
fn regenerate_compact_rig_with_canonical_writer() {
    assert_eq!(
        std::env::var("LIGHT_REGENERATE_COMPACT_RIG").as_deref(),
        Ok("1")
    );
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/fixtures/compact-rig.show");
    let original = fs::read(&source).unwrap();
    let reader = ShowStore {
        conn: Connection::open_with_flags(&source, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap(),
    };
    for table in ["embedded_fixtures", "cues", "object_history"] {
        let count: u64 = reader
            .conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "no authored {table} may be omitted");
    }
    let redo_exists: bool = reader
        .conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='object_redo')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    if redo_exists {
        let count: u64 = reader
            .conn
            .query_row("SELECT count(*) FROM object_redo", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
    let authored = reader.portable_document().unwrap();
    assert_eq!(authored.objects().len(), 24);
    assert_eq!(authored.fixture_profile_revisions().len(), 2);
    assert_eq!(authored.objects_of_kind("group").count(), 4);
    let scratch =
        Path::new(&std::env::var("LIGHT_TMP_DIR").expect("initialize canonical artifact paths"))
            .join(format!("compact-rig-canonical-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&scratch).unwrap();
    let fresh_path = scratch.join("fresh.show");
    let publish_path = scratch.join("publish.show");
    let (writer, _) = ShowStore::create(&fresh_path, authored.name()).unwrap();
    let mut transaction = writer.portable_document().unwrap().transaction();
    for (key, value) in authored.metadata() {
        transaction.set_metadata(key, value);
    }
    transaction.set_metadata(crate::PLAYBACK_CONTRACT_METADATA_KEY, "1");
    for object in authored.objects() {
        let mut body = object.body().clone();
        if object.key().kind() == "group" {
            assert!(matches!(object.key().id(), "1" | "2" | "3" | "4"));
            let fields = body.as_object_mut().unwrap();
            assert_eq!(
                fields.remove("master").and_then(|value| value.as_f64()),
                Some(1.0)
            );
            assert_eq!(
                fields.remove("playback_fader"),
                Some(json!(object.key().id().parse::<u16>().unwrap()))
            );
        }
        transaction.put(object.key().kind(), object.key().id(), body);
    }
    for profile in authored.fixture_profile_revisions() {
        transaction
            .put_fixture_profile_revision(profile.clone())
            .unwrap();
    }
    writer.apply_portable_transaction(transaction).unwrap();
    writer.checkpoint_for_replacement().unwrap();
    drop(writer);
    let reopened = ShowStore::open(&fresh_path).unwrap();
    let canonical = reopened.portable_document().unwrap();
    assert_eq!(canonical.id(), authored.id());
    assert_eq!(canonical.name(), authored.name());
    assert_eq!(
        canonical.fixture_profile_revisions(),
        authored.fixture_profile_revisions()
    );
    assert_eq!(canonical.objects().len(), authored.objects().len());
    for object in authored.objects() {
        let mut expected = object.body().clone();
        if object.key().kind() == "group" {
            expected.as_object_mut().unwrap().remove("master");
            expected.as_object_mut().unwrap().remove("playback_fader");
        }
        assert_eq!(
            canonical
                .object(object.key().kind(), object.key().id())
                .unwrap()
                .body(),
            &expected
        );
    }
    for (key, value) in authored.metadata() {
        if !matches!(
            key.as_str(),
            "light.portable_show_revision" | "light.patch_revision"
        ) {
            assert_eq!(canonical.metadata().get(key), Some(value));
        }
    }
    let schema: u16 = reopened
        .conn
        .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .unwrap();
    assert_eq!(schema, 10);
    assert_eq!(
        canonical.metadata()[crate::PLAYBACK_CONTRACT_METADATA_KEY],
        "1"
    );
    reopened.backup_to(&publish_path).unwrap();
    validate_show_playback_contract(&publish_path).unwrap();
    drop(reopened);
    drop(reader);
    assert_eq!(
        fs::read(&source).unwrap(),
        original,
        "source untouched until audited publication"
    );
    fs::rename(&publish_path, &source).unwrap();
}
