use rusqlite::{Connection, Transaction};

pub(crate) fn configure(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL",
    )
}

pub(crate) fn set_schema_version(
    tx: &Transaction<'_>,
    version: i64,
) -> Result<(), rusqlite::Error> {
    tx.execute("UPDATE schema_info SET version=?1", [version])?;
    Ok(())
}
