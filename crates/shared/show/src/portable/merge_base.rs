//! The common ancestor an offline show copy is merged against (TL-858).
//!
//! "Take Offline Copy" writes the source show's objects, exactly as they were when the copy was
//! taken, into the copy itself. A later merge then compares three versions — the base, the copy
//! and the show it merges into — on any desk, without needing the source desk's revision store.
//! The base travels with the file: a Save As of an offline copy is still mergeable, a Save As of
//! an ordinary show is not.
//!
//! The table is created on demand like `sync_applied_requests`, so writing a base never changes
//! the schema version and an older build still opens the copy.

use crate::{ShowStore, StoreError};
use light_core::ShowId;
use rusqlite::params;
use uuid::Uuid;

const MERGE_BASE_TABLE: &str = "CREATE TABLE IF NOT EXISTS merge_base(kind TEXT NOT NULL,id TEXT NOT NULL,body_json TEXT NOT NULL,PRIMARY KEY(kind,id))";

const SOURCE_SHOW_ID: &str = "light.merge_base.source_show_id";
const SOURCE_SHOW_NAME: &str = "light.merge_base.source_show_name";
const SOURCE_REVISION: &str = "light.merge_base.source_revision";
const FORK_ID: &str = "light.merge_base.fork_id";
const TAKEN_AT: &str = "light.merge_base.taken_at";

/// Where an offline copy came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeLineage {
    pub source_show_id: ShowId,
    pub source_show_name: String,
    /// The named revision of the source saved when the copy was taken.
    pub source_revision: u64,
    /// Shared by every copy taken in one action and by nothing else.
    pub fork_id: Uuid,
    pub taken_at: String,
}

/// One source object as it was when the copy was taken.
#[derive(Clone, Debug, PartialEq)]
pub struct MergeBaseObject {
    pub kind: String,
    pub id: String,
    pub body: serde_json::Value,
}

impl ShowStore {
    /// Records this file as an offline copy of `lineage`'s source, with its current objects as the
    /// merge base. Run on the freshly copied file, so its objects are the source's objects.
    pub fn record_merge_base(&self, lineage: &MergeLineage) -> Result<usize, StoreError> {
        let transaction = self.conn.unchecked_transaction()?;
        transaction.execute(MERGE_BASE_TABLE, [])?;
        transaction.execute("DELETE FROM merge_base", [])?;
        let copied = transaction.execute(
            "INSERT INTO merge_base(kind,id,body_json) SELECT kind,id,body_json FROM objects",
            [],
        )?;
        for (key, value) in [
            (SOURCE_SHOW_ID, lineage.source_show_id.0.to_string()),
            (SOURCE_SHOW_NAME, lineage.source_show_name.clone()),
            (SOURCE_REVISION, lineage.source_revision.to_string()),
            (FORK_ID, lineage.fork_id.to_string()),
            (TAKEN_AT, lineage.taken_at.clone()),
        ] {
            transaction.execute(
                "INSERT INTO metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![key, value],
            )?;
        }
        transaction.commit()?;
        Ok(copied)
    }

    /// Where this file was copied from, when it is an offline copy with a complete base.
    pub fn merge_lineage(&self) -> Result<Option<MergeLineage>, StoreError> {
        let values = self.metadata_with_prefixes(&["light.merge_base."])?;
        let (Some(show_id), Some(name), Some(revision), Some(fork_id), Some(taken_at)) = (
            values.get(SOURCE_SHOW_ID),
            values.get(SOURCE_SHOW_NAME),
            values.get(SOURCE_REVISION),
            values.get(FORK_ID),
            values.get(TAKEN_AT),
        ) else {
            return Ok(None);
        };
        if !self.has_merge_base_table()? {
            return Ok(None);
        }
        let invalid = |field: &str| StoreError::Invalid(format!("invalid merge base {field}"));
        Ok(Some(MergeLineage {
            source_show_id: ShowId(Uuid::parse_str(show_id).map_err(|_| invalid("show id"))?),
            source_show_name: name.clone(),
            source_revision: revision.parse().map_err(|_| invalid("revision"))?,
            fork_id: Uuid::parse_str(fork_id).map_err(|_| invalid("fork id"))?,
            taken_at: taken_at.clone(),
        }))
    }

    /// The objects of the source show when this copy was taken. Empty for an ordinary show.
    pub fn merge_base_objects(&self) -> Result<Vec<MergeBaseObject>, StoreError> {
        if !self.has_merge_base_table()? {
            return Ok(Vec::new());
        }
        let mut statement = self
            .conn
            .prepare("SELECT kind,id,body_json FROM merge_base ORDER BY kind,id")?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(kind, id, body)| {
                Ok(MergeBaseObject {
                    kind,
                    id,
                    body: serde_json::from_str(&body)?,
                })
            })
            .collect()
    }

    fn has_merge_base_table(&self) -> Result<bool, StoreError> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='merge_base')",
                [],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }
}
