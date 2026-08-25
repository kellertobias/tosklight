use super::DESK_SCHEMA_VERSION;
use crate::{StoreError, connection::set_schema_version};
use rusqlite::{Connection, OptionalExtension, Transaction};

pub(super) fn migrate_desk(conn: &mut Connection) -> Result<(), StoreError> {
    let tx = conn.transaction()?;
    tx.execute_batch(
        r#"CREATE TABLE IF NOT EXISTS schema_info(version INTEGER NOT NULL); INSERT INTO schema_info(version) SELECT 0 WHERE NOT EXISTS(SELECT 1 FROM schema_info);
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
      CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,token TEXT NOT NULL,programmer_json TEXT NOT NULL,connected INTEGER NOT NULL CHECK(connected IN(0,1)),updated_at TEXT NOT NULL);"#,
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
    // Before the desks: collapsing them deletes each superseded desk's own lock setting, and a
    // desk that was locked must not come back unlocked because its row was the one superseded.
    collapse_desk_locks(conn)?;
    collapse_control_desks(conn)?;
    drop_desk_users(conn)?;
    Ok(())
}

/// Fold the per-desk Desk Locks into the desk's one lock.
///
/// A lock used to be stored under `desk_lock:<desk id>`, one per control desk. There is one desk
/// and one lock. A desk that was locked must not come back unlocked, so any locked one carries
/// over; the rest are equivalent, and an installation that already holds the singleton keeps it.
///
/// This ran on every read until now, which made a migration look like a read path.
fn collapse_desk_locks(conn: &mut Connection) -> Result<(), StoreError> {
    let legacy = {
        let mut statement =
            conn.prepare("SELECT key,value FROM settings WHERE key LIKE 'desk_lock:%'")?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    if legacy.is_empty() {
        return Ok(());
    }
    let tx = conn.transaction()?;
    let already_collapsed = tx
        .query_row("SELECT 1 FROM settings WHERE key='desk_lock'", [], |_| {
            Ok(())
        })
        .optional()?
        .is_some();
    if !already_collapsed {
        // A locked desk wins. `"locked":true` is the serialized shape of the flag, read without
        // depending on the rest of the configuration this store does not own.
        let kept = legacy
            .iter()
            .find(|(_, value)| value.contains("\"locked\":true"))
            .or_else(|| legacy.first());
        if let Some((_, value)) = kept {
            tx.execute(
                "INSERT INTO settings(key,value) VALUES('desk_lock',?1)",
                [value],
            )?;
        }
    }
    for (key, _) in &legacy {
        tx.execute("DELETE FROM settings WHERE key=?1", [key])?;
    }
    tx.commit()?;
    Ok(())
}

/// Retire the desk's users.
///
/// The `users` table held an id, a name and an enabled flag — no password and no credential of any
/// kind. A login named one, the lookup could only ever find the single seeded `Operator`, and
/// nothing chose between them. The session and its token carry the desk's authority.
///
/// `sessions.user_id` is `NOT NULL` with a foreign key into that table, so the table is rebuilt
/// rather than the column dropped: SQLite will not drop a column a constraint refers to.
fn drop_desk_users(conn: &mut Connection) -> Result<(), StoreError> {
    if !column_exists(conn, "sessions", "user_id")? {
        return Ok(());
    }
    conn.execute_batch("PRAGMA foreign_keys=OFF")?;
    let result = rebuild_sessions(conn);
    conn.execute_batch("PRAGMA foreign_keys=ON")?;
    result
}

fn rebuild_sessions(conn: &mut Connection) -> Result<(), StoreError> {
    let tx = conn.transaction()?;
    tx.execute_batch(
        r#"CREATE TABLE sessions_without_user(id TEXT PRIMARY KEY,token TEXT NOT NULL,programmer_json TEXT NOT NULL,connected INTEGER NOT NULL CHECK(connected IN(0,1)),updated_at TEXT NOT NULL);
      INSERT INTO sessions_without_user(id,token,programmer_json,connected,updated_at) SELECT id,token,programmer_json,connected,updated_at FROM sessions;
      DROP TABLE sessions;
      ALTER TABLE sessions_without_user RENAME TO sessions;
      DROP TABLE IF EXISTS users;"#,
    )?;
    tx.commit()?;
    Ok(())
}

/// Leave one control desk behind.
///
/// Opening a second window used to create a second control desk, so an installation from before
/// the collapse holds one row per window that ever connected. Every client resolves to the first
/// desk by name now, which leaves the rest addressing nothing while still appearing wherever the
/// operator is shown their desks. The kept desk is the one clients already resolve to, so nothing
/// an operator is looking at moves; the others take their own page, playback selection, lock and
/// per-desk settings with them.
fn collapse_control_desks(conn: &mut Connection) -> Result<(), StoreError> {
    let superseded = {
        let mut statement =
            conn.prepare("SELECT id FROM control_desks ORDER BY name COLLATE NOCASE, id")?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .skip(1)
            .collect::<Result<Vec<_>, _>>()?
    };
    if superseded.is_empty() {
        return Ok(());
    }
    let tx = conn.transaction()?;
    super::control_desks::delete_desks(&tx, &superseded)?;
    tx.commit()?;
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
