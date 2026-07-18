use super::{PortableShowObjectKey, bump_revision};
use crate::{AtomicObjectDelete, AtomicObjectWrite, StoreError};
use chrono::Utc;
use light_core::Revision;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::Value;

struct StoredObject {
    revision: Revision,
    body_json: String,
}

pub(crate) fn immediate_transaction(conn: &Connection) -> Result<Transaction<'_>, StoreError> {
    Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(Into::into)
}

pub(crate) fn put_legacy_object(
    conn: &Connection,
    kind: &str,
    id: &str,
    body: &Value,
    expected: Revision,
) -> Result<Revision, StoreError> {
    let tx = immediate_transaction(conn)?;
    let revision = write_checked(&tx, kind, id, body, expected, &timestamp())?;
    bump_revision(&tx)?;
    tx.commit()?;
    Ok(revision)
}

pub(crate) fn mutate_legacy_objects(
    conn: &Connection,
    writes: &[AtomicObjectWrite<'_>],
    deletes: &[AtomicObjectDelete<'_>],
) -> Result<Vec<Revision>, StoreError> {
    let tx = immediate_transaction(conn)?;
    let timestamp = timestamp();
    let revisions = apply_checked_writes(&tx, writes, &timestamp)?;
    let deleted = apply_checked_deletes(&tx, deletes)?;
    if !writes.is_empty() || deleted {
        bump_revision(&tx)?;
    }
    tx.commit()?;
    Ok(revisions)
}

pub(crate) fn undo_legacy_object(
    conn: &Connection,
    kind: &str,
    id: &str,
    expected: Revision,
) -> Result<Revision, StoreError> {
    let tx = immediate_transaction(conn)?;
    let current = current_object(&tx, kind, id)?
        .ok_or_else(|| StoreError::Sql(rusqlite::Error::QueryReturnedNoRows))?;
    ensure_expected(Some(&current), expected)?;
    let previous = previous_history(&tx, kind, id)?;
    let revision = next_revision(expected)?;
    restore_previous(&tx, kind, id, &previous, revision)?;
    bump_revision(&tx)?;
    tx.commit()?;
    Ok(revision)
}

pub(crate) fn delete_legacy_object(
    conn: &Connection,
    kind: &str,
    id: &str,
) -> Result<bool, StoreError> {
    let tx = immediate_transaction(conn)?;
    let deleted = delete_current(&tx, &PortableShowObjectKey::new(kind, id))?;
    if deleted {
        bump_revision(&tx)?;
    }
    tx.commit()?;
    Ok(deleted)
}

pub(crate) fn write_current(
    tx: &Transaction<'_>,
    key: &PortableShowObjectKey,
    body: &Value,
    updated_at: &str,
) -> Result<Revision, StoreError> {
    let current = current_object(tx, key.kind(), key.id())?;
    let expected = current.as_ref().map_or(0, |object| object.revision);
    write_stored(
        tx,
        key.kind(),
        key.id(),
        body,
        current,
        expected,
        updated_at,
    )
}

pub(crate) fn delete_current(
    tx: &Transaction<'_>,
    key: &PortableShowObjectKey,
) -> Result<bool, StoreError> {
    Ok(tx.execute(
        "DELETE FROM objects WHERE kind=?1 AND id=?2",
        params![key.kind(), key.id()],
    )? == 1)
}

fn apply_checked_writes(
    tx: &Transaction<'_>,
    writes: &[AtomicObjectWrite<'_>],
    updated_at: &str,
) -> Result<Vec<Revision>, StoreError> {
    writes
        .iter()
        .map(|write| {
            write_checked(
                tx,
                write.kind,
                write.id,
                write.body,
                write.expected,
                updated_at,
            )
        })
        .collect()
}

fn apply_checked_deletes(
    tx: &Transaction<'_>,
    deletes: &[AtomicObjectDelete<'_>],
) -> Result<bool, StoreError> {
    let mut changed = false;
    for delete in deletes {
        changed |= delete_checked(tx, delete.kind, delete.id, delete.expected)?;
    }
    Ok(changed)
}

fn write_checked(
    tx: &Transaction<'_>,
    kind: &str,
    id: &str,
    body: &Value,
    expected: Revision,
    updated_at: &str,
) -> Result<Revision, StoreError> {
    let current = current_object(tx, kind, id)?;
    ensure_expected(current.as_ref(), expected)?;
    write_stored(tx, kind, id, body, current, expected, updated_at)
}

fn write_stored(
    tx: &Transaction<'_>,
    kind: &str,
    id: &str,
    body: &Value,
    current: Option<StoredObject>,
    expected: Revision,
    updated_at: &str,
) -> Result<Revision, StoreError> {
    let body_json = serde_json::to_string(body)?;
    let revision = next_revision(expected)?;
    if let Some(current) = current {
        archive_current(tx, kind, id, current)?;
    }
    upsert_object(tx, kind, id, &body_json, revision, updated_at)?;
    Ok(revision)
}

fn current_object(
    conn: &Connection,
    kind: &str,
    id: &str,
) -> Result<Option<StoredObject>, StoreError> {
    conn.query_row(
        "SELECT revision,body_json FROM objects WHERE kind=?1 AND id=?2",
        params![kind, id],
        |row| {
            Ok(StoredObject {
                revision: row.get::<_, i64>(0)? as u64,
                body_json: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn ensure_expected(current: Option<&StoredObject>, expected: Revision) -> Result<(), StoreError> {
    let current = current.map_or(0, |object| object.revision);
    if current == expected {
        Ok(())
    } else {
        Err(StoreError::RevisionConflict { expected, current })
    }
}

fn archive_current(
    tx: &Transaction<'_>,
    kind: &str,
    id: &str,
    current: StoredObject,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO object_history(kind,id,revision,body_json,created_at) VALUES (?1,?2,?3,?4,?5)",
        params![
            kind,
            id,
            current.revision as i64,
            current.body_json,
            timestamp()
        ],
    )?;
    Ok(())
}

fn upsert_object(
    tx: &Transaction<'_>,
    kind: &str,
    id: &str,
    body_json: &str,
    revision: Revision,
    updated_at: &str,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO objects(kind,id,body_json,revision,updated_at) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(kind,id) DO UPDATE SET body_json=excluded.body_json,revision=excluded.revision,updated_at=excluded.updated_at",
        params![kind, id, body_json, revision as i64, updated_at],
    )?;
    Ok(())
}

fn delete_checked(
    tx: &Transaction<'_>,
    kind: &str,
    id: &str,
    expected: Revision,
) -> Result<bool, StoreError> {
    let current = current_object(tx, kind, id)?;
    ensure_expected(current.as_ref(), expected)?;
    delete_current(tx, &PortableShowObjectKey::new(kind, id))
}

fn previous_history(
    tx: &Transaction<'_>,
    kind: &str,
    id: &str,
) -> Result<(i64, String), StoreError> {
    tx.query_row(
        "SELECT rowid,body_json FROM object_history WHERE kind=?1 AND id=?2 ORDER BY rowid DESC LIMIT 1",
        params![kind, id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()?
    .ok_or_else(|| StoreError::Invalid("object has no undo history".into()))
}

fn restore_previous(
    tx: &Transaction<'_>,
    kind: &str,
    id: &str,
    previous: &(i64, String),
    revision: Revision,
) -> Result<(), StoreError> {
    let updated = tx.execute(
        "UPDATE objects SET body_json=?3,revision=?4,updated_at=?5 WHERE kind=?1 AND id=?2",
        params![kind, id, previous.1, revision as i64, timestamp()],
    )?;
    if updated != 1 {
        return Err(StoreError::Invalid("object disappeared during undo".into()));
    }
    tx.execute("DELETE FROM object_history WHERE rowid=?1", [previous.0])?;
    Ok(())
}

fn next_revision(current: Revision) -> Result<Revision, StoreError> {
    current
        .checked_add(1)
        .filter(|revision| *revision <= i64::MAX as u64)
        .ok_or_else(|| StoreError::Invalid("object revision overflow".into()))
}

fn timestamp() -> String {
    Utc::now().to_rfc3339()
}
