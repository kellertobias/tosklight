use super::profile_revision::materialize_legacy_fixture_profile_revisions;
use crate::{StoreError, set_schema_version};
use rusqlite::{Connection, TransactionBehavior};

pub(crate) const SHOW_SCHEMA_VERSION: i64 = 8;

pub(crate) fn migrate_show(conn: &mut Connection) -> Result<(), StoreError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch(SHOW_SCHEMA)?;
    if schema_version(&tx)? < 4 {
        materialize_legacy_fixture_profile_revisions(&tx)?;
    }
    if schema_version(&tx)? < 8 {
        retire_superseded_demo_venue_objects(&tx)?;
    }
    set_schema_version(&tx, SHOW_SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

/// Drops the standalone `venue` records the original demo generator wrote, once the show carries
/// its scenery as Venue fixtures.
///
/// Nothing but that generator ever wrote a `venue` record, and every show holding its records also
/// holds the same scenery as visual-only patched fixtures — whole truss runs where the records
/// were cut into two-metre pieces. There the records are only a stale second copy that no screen
/// can remove, so they go. A show whose patch has no such fixture keeps them, and any record not
/// written by the generator is left alone. Schema 8 runs it once.
fn retire_superseded_demo_venue_objects(conn: &Connection) -> Result<(), StoreError> {
    conn.execute(
        "DELETE FROM objects
          WHERE kind='venue' AND id LIKE 'planned-demo-venue-%'
            AND EXISTS (
              SELECT 1 FROM objects fixture
                LEFT JOIN fixture_profile_revisions profile
                  ON profile.profile_id = json_extract(fixture.body_json, '$.profile_id')
                 AND profile.revision = json_extract(fixture.body_json, '$.profile_revision')
               WHERE fixture.kind='patched_fixture'
                 AND COALESCE(
                       json_extract(profile.profile_json, '$.patch_policy'),
                       json_extract(fixture.body_json, '$.definition.profile_snapshot.patch_policy')
                     ) = 'visual_only'
                 AND COALESCE(
                       json_extract(profile.profile_json, '$.crowd'),
                       json_extract(fixture.body_json, '$.definition.profile_snapshot.crowd')
                     ) IS NULL)",
        [],
    )?;
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
  CREATE TABLE IF NOT EXISTS object_redo(kind TEXT NOT NULL,id TEXT NOT NULL,revision INTEGER NOT NULL,body_json TEXT NOT NULL,created_at TEXT NOT NULL);
  CREATE TABLE IF NOT EXISTS cues(cue_list_id TEXT NOT NULL,cue_number REAL NOT NULL,values_json TEXT NOT NULL,cue_only_restore_json TEXT,revision INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(cue_list_id,cue_number));
  CREATE TABLE IF NOT EXISTS cue_thumbnails(cue_id TEXT PRIMARY KEY,image BLOB NOT NULL,state_hash TEXT NOT NULL,width INTEGER NOT NULL,height INTEGER NOT NULL,updated_at TEXT NOT NULL);
  CREATE TABLE IF NOT EXISTS schedule_occurrences(sequence INTEGER PRIMARY KEY AUTOINCREMENT,schedule_id TEXT NOT NULL,occurrence_id TEXT NOT NULL,scheduled_for TEXT NOT NULL,target_action_json TEXT NOT NULL,status TEXT NOT NULL CHECK(status IN ('claimed','completed','failed','interrupted','skipped')),recorded_at TEXT NOT NULL,resolved_at TEXT,result_detail TEXT,UNIQUE(schedule_id,occurrence_id));
  CREATE INDEX IF NOT EXISTS objects_kind ON objects(kind);
  CREATE INDEX IF NOT EXISTS schedule_occurrences_history ON schedule_occurrences(schedule_id,sequence DESC);";
