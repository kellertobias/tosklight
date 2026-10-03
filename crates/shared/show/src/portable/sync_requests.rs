//! Durable exactly-once identities for Control ↔ Architect sync transactions.
//!
//! A sync transaction's request identity is written in the same SQLite transaction as the edit it
//! carries, so "the edit committed" and "the request is known" can never disagree — not across a
//! lost reply, a retry from a new session, or a Control restart. The rows live in the portable
//! show because the edit they guard lives there too.

use crate::{ShowStore, StoreError};
use chrono::Utc;
use rusqlite::{OptionalExtension, Transaction, params};
use uuid::Uuid;

/// How many applied requests one show keeps. Older identities are pruned in the transaction that
/// adds a new one; a client whose journal holds a request this old has long since reconciled.
pub const SYNC_APPLIED_REQUEST_RETENTION: usize = 10_000;

/// One request to record alongside the edit it carried.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncRequestRecord {
    pub association_id: Uuid,
    pub request_id: String,
    /// Digest of the request content, so a reused identity carrying different edits is refused.
    pub signature: String,
    /// The outcome returned to the first delivery, returned again to every retry.
    pub outcome: serde_json::Value,
}

/// A request already applied to this show.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncAppliedRequest {
    pub association_id: Uuid,
    pub request_id: String,
    pub signature: String,
    pub outcome: serde_json::Value,
    /// Whole-show revision the carrying transaction committed.
    pub show_revision: u64,
    pub applied_at: String,
}

/// The table, created where it is first needed. It is also part of the base schema for new files,
/// but a show written before it existed keeps schema version 9 — so an older build still opens it
/// — and gains the table with its first sync write instead of with a schema bump.
pub(super) const SYNC_APPLIED_REQUESTS_TABLE: &str = "CREATE TABLE IF NOT EXISTS sync_applied_requests(association_id TEXT NOT NULL,request_id TEXT NOT NULL,signature TEXT NOT NULL,outcome_json TEXT NOT NULL,show_revision INTEGER NOT NULL,applied_at TEXT NOT NULL,PRIMARY KEY(association_id,request_id))";

pub(super) fn insert_sync_request(
    tx: &Transaction<'_>,
    record: &SyncRequestRecord,
    show_revision: u64,
) -> Result<(), StoreError> {
    tx.execute(SYNC_APPLIED_REQUESTS_TABLE, [])?;
    let outcome = serde_json::to_string(&record.outcome)?;
    tx.execute(
        "INSERT INTO sync_applied_requests(association_id,request_id,signature,outcome_json,show_revision,applied_at) VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            record.association_id.to_string(),
            record.request_id,
            record.signature,
            outcome,
            i64::try_from(show_revision)
                .map_err(|_| StoreError::Invalid("show revision exceeds SQLite range".into()))?,
            Utc::now().to_rfc3339(),
        ],
    )?;
    tx.execute(
        "DELETE FROM sync_applied_requests WHERE rowid <= (SELECT rowid FROM sync_applied_requests ORDER BY rowid DESC LIMIT 1 OFFSET ?1)",
        [i64::try_from(SYNC_APPLIED_REQUEST_RETENTION).unwrap_or(i64::MAX)],
    )?;
    Ok(())
}

impl ShowStore {
    /// Carries `source`'s applied sync requests into this file, keeping `source`'s row where both
    /// hold one. A whole-file replacement of a show uses it so the desk's request identities
    /// survive and a retry after the replacement still applies once.
    pub fn adopt_sync_applied_requests(&self, source: &Self) -> Result<usize, StoreError> {
        if !source.has_sync_applied_requests_table()? {
            return Ok(0);
        }
        let mut statement = source.conn.prepare(
            "SELECT association_id,request_id,signature,outcome_json,show_revision,applied_at FROM sync_applied_requests ORDER BY rowid",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(SYNC_APPLIED_REQUESTS_TABLE, [])?;
        for row in &rows {
            tx.execute(
                "INSERT OR REPLACE INTO sync_applied_requests(association_id,request_id,signature,outcome_json,show_revision,applied_at) VALUES(?1,?2,?3,?4,?5,?6)",
                params![row.0, row.1, row.2, row.3, row.4, row.5],
            )?;
        }
        tx.commit()?;
        Ok(rows.len())
    }

    fn has_sync_applied_requests_table(&self) -> Result<bool, StoreError> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='sync_applied_requests')",
                [],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    /// The stored outcome of a sync request this show has already applied, if any.
    pub fn sync_applied_request(
        &self,
        association_id: Uuid,
        request_id: &str,
    ) -> Result<Option<SyncAppliedRequest>, StoreError> {
        if !self.has_sync_applied_requests_table()? {
            return Ok(None);
        }
        let row = self
            .conn
            .query_row(
                "SELECT signature,outcome_json,show_revision,applied_at FROM sync_applied_requests WHERE association_id=?1 AND request_id=?2",
                params![association_id.to_string(), request_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(signature, outcome, show_revision, applied_at)| {
            Ok(SyncAppliedRequest {
                association_id,
                request_id: request_id.to_owned(),
                signature,
                outcome: serde_json::from_str(&outcome)?,
                show_revision: u64::try_from(show_revision).map_err(|_| {
                    StoreError::Invalid("stored sync request revision is negative".into())
                })?,
                applied_at,
            })
        })
        .transpose()
    }
}
