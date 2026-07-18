use super::{
    LegacyInlineProfileSnapshot, insert_fixture_profile_revision_in,
    visit_legacy_inline_profile_snapshots,
};
use crate::{StoreError, portable::PortableShowObjectKey};
use rusqlite::Transaction;
use serde_json::Value;

pub(crate) fn materialize_legacy_fixture_profile_revisions(
    tx: &Transaction<'_>,
) -> Result<(), StoreError> {
    let mut statement = tx.prepare("SELECT kind,id,body_json FROM objects ORDER BY kind,id")?;
    let rows = statement.query_map([], legacy_object_row)?;
    for row in rows {
        let (owner, body) = decode_legacy_object(row?)?;
        materialize_object(tx, &owner, &body)?;
    }
    Ok(())
}

type LegacyObjectRow = (String, String, String);

fn legacy_object_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LegacyObjectRow> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
}

fn decode_legacy_object(
    row: LegacyObjectRow,
) -> Result<(PortableShowObjectKey, Value), StoreError> {
    let (kind, id, body_json) = row;
    let body = serde_json::from_str(&body_json).map_err(|error| {
        StoreError::Invalid(format!(
            "invalid portable object {kind}/{id} during fixture profile migration: {error}"
        ))
    })?;
    Ok((PortableShowObjectKey::new(kind, id), body))
}

fn materialize_object(
    tx: &Transaction<'_>,
    owner: &PortableShowObjectKey,
    body: &Value,
) -> Result<(), StoreError> {
    visit_legacy_inline_profile_snapshots(owner, body, &mut |snapshot| {
        insert_snapshot(tx, &snapshot)
    })
}

fn insert_snapshot(
    tx: &Transaction<'_>,
    snapshot: &LegacyInlineProfileSnapshot,
) -> Result<(), StoreError> {
    insert_fixture_profile_revision_in(tx, snapshot.profile())?;
    Ok(())
}
