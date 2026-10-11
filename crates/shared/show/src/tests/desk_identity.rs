//! The installation identity an Architect binding names.
use super::temporary;
use crate::DeskStore;
use rusqlite::Connection;

#[test]
fn a_legacy_desk_database_is_refused_without_changing_identity_or_settings() {
    let path = temporary("desk-identity-legacy");
    {
        // A desk database from before identities: settings exist, the identity does not.
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO settings(key,value) VALUES('active_show_id','00000000-0000-0000-0000-000000000001');",
        )
        .unwrap();
    }
    let original = std::fs::read(&path).unwrap();
    assert!(DeskStore::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let _ = std::fs::remove_file(path);
}

#[test]
fn two_installations_never_share_an_identity() {
    let left = temporary("desk-identity-left");
    let right = temporary("desk-identity-right");
    let left_identity = DeskStore::open(&left).unwrap().desk_identity().unwrap();
    let right_identity = DeskStore::open(&right).unwrap().desk_identity().unwrap();
    assert_ne!(left_identity, right_identity);
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}
