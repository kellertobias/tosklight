use super::profile_revision::materialize_legacy_fixture_profile_revisions;
use crate::{StoreError, set_schema_version};
use rusqlite::{Connection, TransactionBehavior};

pub(crate) const SHOW_SCHEMA_VERSION: i64 = 4;

pub(crate) fn migrate_show(conn: &mut Connection) -> Result<(), StoreError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch(SHOW_SCHEMA)?;
    if schema_version(&tx)? < SHOW_SCHEMA_VERSION {
        materialize_legacy_fixture_profile_revisions(&tx)?;
    }
    set_schema_version(&tx, SHOW_SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

fn schema_version(conn: &Connection) -> Result<i64, StoreError> {
    conn.query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .map_err(Into::into)
}

pub(crate) fn validate_show_connection(conn: &Connection) -> Result<(), StoreError> {
    validate_integrity(conn)?;
    validate_schema_version(conn)?;
    validate_identity(conn)
}

fn validate_integrity(conn: &Connection) -> Result<(), StoreError> {
    let integrity: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if integrity == "ok" {
        Ok(())
    } else {
        Err(StoreError::Invalid(format!(
            "SQLite integrity check failed: {integrity}"
        )))
    }
}

fn validate_schema_version(conn: &Connection) -> Result<(), StoreError> {
    let version: i64 = conn
        .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .map_err(|_| StoreError::Invalid("not a Light show file: schema_info is missing".into()))?;
    if version <= SHOW_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(StoreError::Invalid(format!(
            "show schema {version} is newer than supported schema {SHOW_SCHEMA_VERSION}"
        )))
    }
}

fn validate_identity(conn: &Connection) -> Result<(), StoreError> {
    for key in ["show_id", "name"] {
        if !metadata_exists(conn, key)? {
            return Err(StoreError::Invalid(format!(
                "show metadata is missing {key}"
            )));
        }
    }
    Ok(())
}

fn metadata_exists(conn: &Connection, key: &str) -> Result<bool, StoreError> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM metadata WHERE key=?1)",
        [key],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

const SHOW_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS schema_info(version INTEGER NOT NULL);
  INSERT INTO schema_info(version) SELECT 0 WHERE NOT EXISTS(SELECT 1 FROM schema_info);
  CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
  CREATE TABLE IF NOT EXISTS embedded_fixtures(id TEXT NOT NULL,revision INTEGER NOT NULL,definition_json TEXT NOT NULL,PRIMARY KEY(id,revision));
  CREATE TABLE IF NOT EXISTS fixture_profile_revisions(profile_id TEXT NOT NULL CHECK(length(profile_id)>0),revision INTEGER NOT NULL CHECK(revision>=0),content_digest TEXT NOT NULL,profile_json TEXT NOT NULL,PRIMARY KEY(profile_id,revision));
  CREATE TABLE IF NOT EXISTS objects(kind TEXT NOT NULL,id TEXT NOT NULL,body_json TEXT NOT NULL,revision INTEGER NOT NULL,updated_at TEXT NOT NULL,PRIMARY KEY(kind,id));
  CREATE TABLE IF NOT EXISTS object_history(kind TEXT NOT NULL,id TEXT NOT NULL,revision INTEGER NOT NULL,body_json TEXT NOT NULL,created_at TEXT NOT NULL);
  CREATE TABLE IF NOT EXISTS cues(cue_list_id TEXT NOT NULL,cue_number REAL NOT NULL,values_json TEXT NOT NULL,cue_only_restore_json TEXT,revision INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(cue_list_id,cue_number));
  CREATE INDEX IF NOT EXISTS objects_kind ON objects(kind);";
