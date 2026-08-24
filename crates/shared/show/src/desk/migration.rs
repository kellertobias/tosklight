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
      CREATE TABLE IF NOT EXISTS control_desks(id TEXT PRIMARY KEY,name TEXT NOT NULL,columns_count INTEGER NOT NULL DEFAULT 8,rows_count INTEGER NOT NULL DEFAULT 1,buttons_count INTEGER NOT NULL DEFAULT 3,playback_layout_json TEXT,client_id TEXT,last_connected_at TEXT);
      CREATE TABLE IF NOT EXISTS desk_clients(client_id TEXT PRIMARY KEY,last_connected_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS control_desk_pages(desk_id TEXT NOT NULL,show_id TEXT NOT NULL,page INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(desk_id,show_id),FOREIGN KEY(desk_id) REFERENCES control_desks(id) ON DELETE CASCADE);
      CREATE TABLE IF NOT EXISTS control_desk_selections(desk_id TEXT NOT NULL,show_id TEXT NOT NULL,playback INTEGER NOT NULL,PRIMARY KEY(desk_id,show_id),FOREIGN KEY(desk_id) REFERENCES control_desks(id) ON DELETE CASCADE);
      CREATE TABLE IF NOT EXISTS screens(id TEXT PRIMARY KEY,name TEXT NOT NULL,layout_json TEXT NOT NULL DEFAULT '{"desks":[],"activeDeskId":""}',show_dock INTEGER NOT NULL DEFAULT 1,show_playbacks INTEGER NOT NULL DEFAULT 1,playback_count INTEGER NOT NULL DEFAULT 8,playback_rows INTEGER NOT NULL DEFAULT 1,first_playback_slot INTEGER NOT NULL DEFAULT 1,page_mode TEXT NOT NULL DEFAULT 'follow_main',show_page_controls INTEGER NOT NULL DEFAULT 1,desired_open INTEGER NOT NULL DEFAULT 0,display_id TEXT,bounds_json TEXT,fullscreen INTEGER NOT NULL DEFAULT 0,playback_layout_json TEXT,content_json TEXT NOT NULL DEFAULT '{"type":"desktop"}',show_programmer INTEGER NOT NULL DEFAULT 0,not_editable INTEGER NOT NULL DEFAULT 0);
      CREATE TABLE IF NOT EXISTS screen_pages(screen_id TEXT NOT NULL,show_id TEXT NOT NULL,page INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(screen_id,show_id),FOREIGN KEY(screen_id) REFERENCES screens(id) ON DELETE CASCADE);
      CREATE TABLE IF NOT EXISTS programmer_control_surface(singleton INTEGER PRIMARY KEY CHECK(singleton=1),owner_screen_id TEXT,visible_encoders INTEGER NOT NULL DEFAULT 6 CHECK(visible_encoders IN(4,6)),FOREIGN KEY(owner_screen_id) REFERENCES screens(id) ON DELETE SET NULL);
      INSERT OR IGNORE INTO programmer_control_surface(singleton,owner_screen_id,visible_encoders) VALUES(1,NULL,6);
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
        "screens",
        "content_json",
        r#"content_json TEXT NOT NULL DEFAULT '{"type":"desktop"}'"#,
    )?;
    add_column_if_missing(
        &tx,
        "screens",
        "show_programmer",
        "show_programmer INTEGER NOT NULL DEFAULT 0",
    )?;
    // Screens that existed before the capability split kept programming, because that is what
    // they could do. A screen only becomes Not Editable when an operator says so.
    add_column_if_missing(
        &tx,
        "screens",
        "not_editable",
        "not_editable INTEGER NOT NULL DEFAULT 0",
    )?;
    // A client is a window that connected, not a desk. It used to be recorded on the desk row,
    // which is why every window got a control desk of its own — and why two browser screens ended
    // up unable to see each other's command line. Carry the existing ones across so a client that
    // has connected before is still known.
    tx.execute(
        "CREATE TABLE IF NOT EXISTS desk_clients(client_id TEXT PRIMARY KEY,last_connected_at TEXT NOT NULL)",
        [],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO desk_clients(client_id,last_connected_at) \
         SELECT client_id, COALESCE(last_connected_at, '') FROM control_desks \
         WHERE client_id IS NOT NULL",
        [],
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
    drop_desk_osc_alias(conn)?;
    Ok(())
}

/// Retire the per-desk OSC alias from installations that still carry it.
///
/// An alias used to pick which desk a wing was plugged into. There is one desk now, and the OSC
/// path says what a surface may do rather than which desk it reaches, so the column selects
/// nothing — and being `NOT NULL UNIQUE` it would refuse every insert that no longer supplies one.
/// The unique constraint is why this rebuilds the table instead of dropping the column: SQLite
/// will not drop an indexed column.
fn drop_desk_osc_alias(conn: &mut Connection) -> Result<(), StoreError> {
    if !column_exists(conn, "control_desks", "osc_alias")? {
        return Ok(());
    }
    conn.execute_batch("PRAGMA foreign_keys=OFF")?;
    let result = rebuild_control_desks(conn);
    conn.execute_batch("PRAGMA foreign_keys=ON")?;
    result
}

fn rebuild_control_desks(conn: &mut Connection) -> Result<(), StoreError> {
    let tx = conn.transaction()?;
    tx.execute_batch(
        r#"CREATE TABLE control_desks_without_alias(id TEXT PRIMARY KEY,name TEXT NOT NULL,columns_count INTEGER NOT NULL DEFAULT 8,rows_count INTEGER NOT NULL DEFAULT 1,buttons_count INTEGER NOT NULL DEFAULT 3,playback_layout_json TEXT,client_id TEXT,last_connected_at TEXT);
      INSERT INTO control_desks_without_alias(id,name,columns_count,rows_count,buttons_count,playback_layout_json,client_id,last_connected_at) SELECT id,name,columns_count,rows_count,buttons_count,playback_layout_json,client_id,last_connected_at FROM control_desks;
      DROP TABLE control_desks;
      ALTER TABLE control_desks_without_alias RENAME TO control_desks;
      CREATE UNIQUE INDEX IF NOT EXISTS control_desks_client_id ON control_desks(client_id) WHERE client_id IS NOT NULL;"#,
    )?;
    tx.commit()?;
    Ok(())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool, StoreError> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(1)? == column {
            return Ok(true);
        }
    }
    Ok(false)
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
