use super::DESK_SCHEMA_VERSION;
use crate::{StoreError, connection::set_schema_version};
use rusqlite::{Connection, Transaction};

pub(super) fn migrate_desk(conn: &mut Connection) -> Result<(), StoreError> {
    let tx = conn.transaction()?;
    tx.execute_batch(
        r#"CREATE TABLE IF NOT EXISTS schema_info(version INTEGER NOT NULL); INSERT INTO schema_info(version) SELECT 0 WHERE NOT EXISTS(SELECT 1 FROM schema_info);
      CREATE TABLE IF NOT EXISTS users(id TEXT PRIMARY KEY,name TEXT NOT NULL UNIQUE COLLATE NOCASE,enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN(0,1)));
      CREATE TABLE IF NOT EXISTS show_library(id TEXT PRIMARY KEY,name TEXT NOT NULL UNIQUE COLLATE NOCASE,path TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 1,updated_at TEXT NOT NULL,revision_source_show_id TEXT,revision_source_show_name TEXT,revision_source_revision INTEGER,revision_source_name TEXT,revision_copy_created_at TEXT);
      CREATE TABLE IF NOT EXISTS show_revisions(show_id TEXT NOT NULL,revision INTEGER NOT NULL,name TEXT NOT NULL,path TEXT NOT NULL,created_at TEXT NOT NULL,PRIMARY KEY(show_id,revision),FOREIGN KEY(show_id) REFERENCES show_library(id) ON DELETE CASCADE);
      CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS control_desks(id TEXT PRIMARY KEY,name TEXT NOT NULL,osc_alias TEXT NOT NULL UNIQUE COLLATE NOCASE,columns_count INTEGER NOT NULL DEFAULT 8,rows_count INTEGER NOT NULL DEFAULT 1,buttons_count INTEGER NOT NULL DEFAULT 3,playback_layout_json TEXT,client_id TEXT,last_connected_at TEXT);
      CREATE TABLE IF NOT EXISTS control_desk_pages(desk_id TEXT NOT NULL,show_id TEXT NOT NULL,page INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(desk_id,show_id),FOREIGN KEY(desk_id) REFERENCES control_desks(id) ON DELETE CASCADE);
      CREATE TABLE IF NOT EXISTS control_desk_selections(desk_id TEXT NOT NULL,show_id TEXT NOT NULL,playback INTEGER NOT NULL,PRIMARY KEY(desk_id,show_id),FOREIGN KEY(desk_id) REFERENCES control_desks(id) ON DELETE CASCADE);
      CREATE TABLE IF NOT EXISTS screens(id TEXT PRIMARY KEY,name TEXT NOT NULL,layout_json TEXT NOT NULL DEFAULT '{"desks":[],"activeDeskId":""}',show_dock INTEGER NOT NULL DEFAULT 1,show_playbacks INTEGER NOT NULL DEFAULT 1,playback_count INTEGER NOT NULL DEFAULT 8,playback_rows INTEGER NOT NULL DEFAULT 1,first_playback_slot INTEGER NOT NULL DEFAULT 1,page_mode TEXT NOT NULL DEFAULT 'follow_main',show_page_controls INTEGER NOT NULL DEFAULT 1,desired_open INTEGER NOT NULL DEFAULT 0,display_id TEXT,bounds_json TEXT,fullscreen INTEGER NOT NULL DEFAULT 0,playback_layout_json TEXT);
      CREATE TABLE IF NOT EXISTS screen_pages(screen_id TEXT NOT NULL,show_id TEXT NOT NULL,page INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(screen_id,show_id),FOREIGN KEY(screen_id) REFERENCES screens(id) ON DELETE CASCADE);
      CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,user_id TEXT NOT NULL,token TEXT NOT NULL,programmer_json TEXT NOT NULL,connected INTEGER NOT NULL CHECK(connected IN(0,1)),updated_at TEXT NOT NULL,FOREIGN KEY(user_id) REFERENCES users(id));"#,
    )?;
    add_column_if_missing(
        &tx,
        "show_library",
        "revision_source_show_id",
        "revision_source_show_id TEXT",
    )?;
    add_column_if_missing(
        &tx,
        "control_desks",
        "playback_layout_json",
        "playback_layout_json TEXT",
    )?;
    add_column_if_missing(&tx, "control_desks", "client_id", "client_id TEXT")?;
    add_column_if_missing(
        &tx,
        "control_desks",
        "last_connected_at",
        "last_connected_at TEXT",
    )?;
    tx.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS control_desks_client_id ON control_desks(client_id) WHERE client_id IS NOT NULL",
        [],
    )?;
    add_column_if_missing(
        &tx,
        "screens",
        "playback_layout_json",
        "playback_layout_json TEXT",
    )?;
    add_column_if_missing(
        &tx,
        "show_library",
        "revision_source_show_name",
        "revision_source_show_name TEXT",
    )?;
    add_column_if_missing(
        &tx,
        "show_library",
        "revision_source_revision",
        "revision_source_revision INTEGER",
    )?;
    add_column_if_missing(
        &tx,
        "show_library",
        "revision_source_name",
        "revision_source_name TEXT",
    )?;
    add_column_if_missing(
        &tx,
        "show_library",
        "revision_copy_created_at",
        "revision_copy_created_at TEXT",
    )?;
    set_schema_version(&tx, DESK_SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

fn add_column_if_missing(
    tx: &Transaction<'_>,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), rusqlite::Error> {
    let mut statement = tx.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(1)? == column {
            return Ok(());
        }
    }
    drop(rows);
    drop(statement);
    tx.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {definition}"))?;
    Ok(())
}
