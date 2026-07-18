use super::{
    FixtureProfileRevision, FixtureProfileRevisionId, canonical_fixture_profile_json,
    profile_conflict,
};
use crate::{
    ShowStore, StoreError,
    portable::{
        PortableShowRevision, bump_revision, repository::immediate_transaction,
        store::current_revision,
    },
};
use light_core::FixtureId;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureProfileRevisionInsertStatus {
    Inserted,
    AlreadyPresent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixtureProfileRevisionInsertResult {
    status: FixtureProfileRevisionInsertStatus,
    show_revision: PortableShowRevision,
}

impl FixtureProfileRevisionInsertResult {
    pub const fn status(self) -> FixtureProfileRevisionInsertStatus {
        self.status
    }

    pub const fn show_revision(self) -> PortableShowRevision {
        self.show_revision
    }
}

impl ShowStore {
    pub fn insert_fixture_profile_revision(
        &self,
        profile: &FixtureProfileRevision,
    ) -> Result<FixtureProfileRevisionInsertResult, StoreError> {
        let tx = immediate_transaction(&self.conn)?;
        let status = insert_fixture_profile_revision_in(&tx, profile)?;
        let show_revision = revision_after_insert(&tx, status)?;
        tx.commit()?;
        Ok(FixtureProfileRevisionInsertResult {
            status,
            show_revision,
        })
    }

    pub fn resolve_fixture_profile_revision(
        &self,
        profile_id: FixtureId,
        revision: light_core::Revision,
    ) -> Result<Option<FixtureProfileRevision>, StoreError> {
        load_fixture_profile_revision(&self.conn, profile_id, revision)
    }

    pub fn list_fixture_profile_revisions(
        &self,
    ) -> Result<Vec<FixtureProfileRevision>, StoreError> {
        load_fixture_profile_revisions(&self.conn)
    }

    pub fn list_fixture_profile_revisions_for(
        &self,
        profile_id: FixtureId,
    ) -> Result<Vec<FixtureProfileRevision>, StoreError> {
        load_fixture_profile_revisions_for(&self.conn, profile_id)
    }
}

pub(crate) fn insert_fixture_profile_revision_in(
    tx: &Transaction<'_>,
    candidate: &FixtureProfileRevision,
) -> Result<FixtureProfileRevisionInsertStatus, StoreError> {
    if let Some(existing) = load_by_id(tx, candidate.id())? {
        return matching_insert_status(&existing, candidate);
    }
    let profile_json = canonical_fixture_profile_json(candidate.profile())?;
    tx.execute(
        "INSERT INTO fixture_profile_revisions(profile_id,revision,content_digest,profile_json) VALUES (?1,?2,?3,?4)",
        params![
            candidate.id().profile_id().0.to_string(),
            candidate.id().revision() as i64,
            candidate.digest().as_str(),
            profile_json
        ],
    )?;
    Ok(FixtureProfileRevisionInsertStatus::Inserted)
}

pub(crate) fn load_fixture_profile_revisions(
    conn: &Connection,
) -> Result<Vec<FixtureProfileRevision>, StoreError> {
    load_list(
        conn,
        "SELECT profile_id,revision,content_digest,profile_json FROM fixture_profile_revisions ORDER BY profile_id,revision",
        [],
    )
}

fn load_fixture_profile_revisions_for(
    conn: &Connection,
    profile_id: FixtureId,
) -> Result<Vec<FixtureProfileRevision>, StoreError> {
    let profile_id = profile_id.0.to_string();
    load_list(
        conn,
        "SELECT profile_id,revision,content_digest,profile_json FROM fixture_profile_revisions WHERE profile_id=?1 ORDER BY revision",
        [profile_id],
    )
}

fn load_fixture_profile_revision(
    conn: &Connection,
    profile_id: FixtureId,
    revision: light_core::Revision,
) -> Result<Option<FixtureProfileRevision>, StoreError> {
    let id = FixtureProfileRevisionId::new(profile_id, revision)?;
    load_by_id(conn, &id)
}

fn load_by_id(
    conn: &Connection,
    id: &FixtureProfileRevisionId,
) -> Result<Option<FixtureProfileRevision>, StoreError> {
    let row = conn
        .query_row(
            "SELECT profile_id,revision,content_digest,profile_json FROM fixture_profile_revisions WHERE profile_id=?1 AND revision=?2",
            params![id.profile_id().0.to_string(), id.revision() as i64],
            profile_row,
        )
        .optional()?;
    row.map(decode_row).transpose()
}

fn load_list<P>(
    conn: &Connection,
    query: &str,
    params: P,
) -> Result<Vec<FixtureProfileRevision>, StoreError>
where
    P: rusqlite::Params,
{
    let mut statement = conn.prepare(query)?;
    let rows = statement.query_map(params, profile_row)?;
    rows.map(|row| decode_row(row?)).collect()
}

type ProfileRow = (String, i64, String, String);

fn profile_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProfileRow> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}

fn decode_row(row: ProfileRow) -> Result<FixtureProfileRevision, StoreError> {
    let (profile_id, revision, digest, profile_json) = row;
    if revision < 0 {
        return Err(StoreError::Invalid(
            "fixture profile revision is negative".into(),
        ));
    }
    FixtureProfileRevision::from_stored(
        profile_id,
        revision as u64,
        digest,
        serde_json::from_str(&profile_json)?,
    )
}

fn matching_insert_status(
    existing: &FixtureProfileRevision,
    candidate: &FixtureProfileRevision,
) -> Result<FixtureProfileRevisionInsertStatus, StoreError> {
    if existing.digest() == candidate.digest() {
        Ok(FixtureProfileRevisionInsertStatus::AlreadyPresent)
    } else {
        Err(profile_conflict(existing, candidate))
    }
}

fn revision_after_insert(
    tx: &Transaction<'_>,
    status: FixtureProfileRevisionInsertStatus,
) -> Result<PortableShowRevision, StoreError> {
    match status {
        FixtureProfileRevisionInsertStatus::Inserted => bump_revision(tx),
        FixtureProfileRevisionInsertStatus::AlreadyPresent => current_revision(tx),
    }
}
