use super::{
    PortableShowDocument, PortableShowObject, PortableShowObjectKey, PortableShowRevision,
    profile_revision::load_fixture_profile_revisions,
};
use crate::{ShowStore, StoreError};
use light_core::ShowId;
use rusqlite::{Connection, OptionalExtension, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

pub(crate) const REVISION_METADATA_KEY: &str = "light.portable_show_revision";

impl ShowStore {
    /// Loads every portable object as raw JSON together with unknown metadata.
    pub fn portable_document(&self) -> Result<PortableShowDocument, StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        let document = load_document(&tx)?;
        tx.commit()?;
        Ok(document)
    }

    /// Reads the O(1) whole-document revision used by portable transactions.
    pub fn portable_revision(&self) -> Result<PortableShowRevision, StoreError> {
        current_revision(&self.conn)
    }
}

pub(crate) fn load_document(conn: &Connection) -> Result<PortableShowDocument, StoreError> {
    let metadata = load_metadata(conn)?;
    let revision = revision_from_metadata(&metadata)?;
    let id = required_metadata(&metadata, "show_id")?;
    let name = required_metadata(&metadata, "name")?;
    let objects = load_objects(conn)?;
    let profile_revisions = load_fixture_profile_revisions(conn)?;
    Ok(PortableShowDocument::new(
        ShowId(Uuid::parse_str(id)?),
        name.to_owned(),
        revision,
        metadata,
        objects,
        profile_revisions,
    ))
}

pub(crate) fn current_revision(conn: &Connection) -> Result<PortableShowRevision, StoreError> {
    let value = conn
        .query_row(
            "SELECT value FROM metadata WHERE key=?1",
            [REVISION_METADATA_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    parse_revision(value.as_deref())
}

pub(crate) fn initialise_revision(tx: &Transaction<'_>) -> Result<(), StoreError> {
    tx.execute(
        "INSERT OR IGNORE INTO metadata(key,value) VALUES (?1,'0')",
        [REVISION_METADATA_KEY],
    )?;
    Ok(())
}

pub(crate) fn bump_revision(tx: &Transaction<'_>) -> Result<PortableShowRevision, StoreError> {
    let next = current_revision(tx)?
        .value()
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("portable show revision overflow".into()))?;
    tx.execute(
        "INSERT INTO metadata(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        (REVISION_METADATA_KEY, next.to_string()),
    )?;
    Ok(PortableShowRevision::new(next))
}

fn load_metadata(conn: &Connection) -> Result<BTreeMap<String, String>, StoreError> {
    let mut statement = conn.prepare("SELECT key,value FROM metadata ORDER BY key")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}

fn load_objects(conn: &Connection) -> Result<Vec<PortableShowObject>, StoreError> {
    let mut statement =
        conn.prepare("SELECT kind,id,body_json,revision,updated_at FROM objects ORDER BY kind,id")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    rows.map(decode_object).collect()
}

fn decode_object(
    row: Result<(String, String, String, i64, String), rusqlite::Error>,
) -> Result<PortableShowObject, StoreError> {
    let (kind, id, body, revision, updated_at) = row?;
    Ok(PortableShowObject::new(
        PortableShowObjectKey::new(kind, id),
        serde_json::from_str(&body)?,
        revision as u64,
        updated_at,
    ))
}

fn required_metadata<'a>(
    metadata: &'a BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str, StoreError> {
    metadata
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| StoreError::Invalid(format!("show metadata is missing {key}")))
}

fn revision_from_metadata(
    metadata: &BTreeMap<String, String>,
) -> Result<PortableShowRevision, StoreError> {
    parse_revision(metadata.get(REVISION_METADATA_KEY).map(String::as_str))
}

fn parse_revision(value: Option<&str>) -> Result<PortableShowRevision, StoreError> {
    let value = value
        .unwrap_or("0")
        .parse()
        .map_err(|_| StoreError::Invalid("portable show revision metadata is invalid".into()))?;
    Ok(PortableShowRevision::new(value))
}
