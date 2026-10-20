//! TL-714 exercises the real startup path, preserving incompatible original SQLite files.
use super::{StartupState, startup_options};
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn directory() -> PathBuf {
    PathBuf::from(std::env::var_os("LIGHT_TMP_DIR").expect("canonical test temporary directory"))
        .join(format!("startup-playback-contract-{}", Uuid::new_v4()))
}
fn options(path: &Path) -> startup_options::StartupOptions {
    startup_options::StartupOptions {
        data_dir: path.into(),
        show_file: None,
        fixture_package_dir: None,
        extensions_dir: Some(path.join("extensions")),
        bind: "127.0.0.1:0".parse().unwrap(),
        test_bench: true,
        visualizer_preview: false,
        osc_bind_override: Some("127.0.0.1:0".parse().unwrap()),
        output_bind_override: Some("127.0.0.1".parse().unwrap()),
    }
}

#[test]
fn incompatible_desk_starts_in_separate_recovery_without_changing_original_database_or_wal() {
    let dir = directory();
    std::fs::create_dir_all(&dir).unwrap();
    let original = dir.join("desk.sqlite");
    let connection = rusqlite::Connection::open(&original).unwrap();
    connection.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE schema_info(version INTEGER NOT NULL); INSERT INTO schema_info VALUES(16); CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL); INSERT INTO settings VALUES('operator-setting','preserve');").unwrap();
    let database = std::fs::read(&original).unwrap();
    let wal_path = dir.join("desk.sqlite-wal");
    let wal = std::fs::read(&wal_path).unwrap();
    let started = StartupState::load(options(&dir)).unwrap();
    assert_eq!(
        started.persistent.data_dir,
        dir.join("recovery/desk-contract-17")
    );
    assert!(
        started
            .active_show_error
            .as_deref()
            .unwrap()
            .contains("original database")
    );
    assert_eq!(std::fs::read(&original).unwrap(), database);
    assert_eq!(std::fs::read(&wal_path).unwrap(), wal);
    drop(started);
    drop(connection);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn incompatible_active_default_and_explicit_show_enter_recovery_before_identity_rewrite() {
    let dir = directory();
    let initial = StartupState::load(options(&dir)).unwrap();
    let active = initial.persistent.active_show.as_ref().unwrap().clone();
    drop(initial);
    let connection = rusqlite::Connection::open(&active.path).unwrap();
    connection.execute("INSERT INTO objects(kind,id,revision,body_json,updated_at) VALUES('playback','legacy',1,'{\"number\":7}','test')", []).unwrap();
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(connection);
    let original = std::fs::read(&active.path).unwrap();
    let restarted = StartupState::load(options(&dir)).unwrap();
    assert!(
        restarted
            .active_show_error
            .as_deref()
            .unwrap()
            .contains("before TL-714")
    );
    drop(restarted);
    assert_eq!(std::fs::read(&active.path).unwrap(), original);
    let mut explicit = options(&dir);
    explicit.show_file = Some(active.path.clone().into());
    let explicit = StartupState::load(explicit).unwrap();
    assert!(
        explicit
            .active_show_error
            .as_deref()
            .unwrap()
            .contains("before TL-714")
    );
    drop(explicit);
    assert_eq!(std::fs::read(&active.path).unwrap(), original);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn venue_reader_capability_preserves_legacy_and_authored_show_startup_and_future_recovery() {
    let dir = directory();
    let initial = StartupState::load(options(&dir)).unwrap();
    assert!(
        initial.active_show_error.is_none(),
        "{:?}",
        initial.active_show_error
    );
    let active = initial.persistent.active_show.as_ref().unwrap().clone();
    drop(initial);
    let conn = rusqlite::Connection::open(&active.path).unwrap();
    let version: i64 = conn
        .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 10);
    conn.execute("UPDATE schema_info SET version=11", [])
        .unwrap();
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(conn);
    let started = StartupState::load(options(&dir)).unwrap();
    assert!(
        started.active_show_error.is_none(),
        "{:?}",
        started.active_show_error
    );
    drop(started);
    let conn = rusqlite::Connection::open(&active.path).unwrap();
    assert_eq!(
        conn.query_row("SELECT version FROM schema_info", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        11
    );
    conn.execute("UPDATE schema_info SET version=12", [])
        .unwrap();
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(conn);
    let before = std::fs::read(&active.path).unwrap();
    let started = StartupState::load(options(&dir)).unwrap();
    assert!(
        started
            .active_show_error
            .as_deref()
            .unwrap()
            .contains("newer than supported schema 11")
    );
    drop(started);
    assert_eq!(std::fs::read(&active.path).unwrap(), before);
    let mut explicit = options(&dir);
    explicit.show_file = Some(active.path.clone().into());
    let started = StartupState::load(explicit).unwrap();
    assert!(
        started
            .active_show_error
            .as_deref()
            .unwrap()
            .contains("newer than supported schema 11")
    );
    drop(started);
    assert_eq!(std::fs::read(&active.path).unwrap(), before);
    std::fs::remove_dir_all(dir).unwrap();
}
