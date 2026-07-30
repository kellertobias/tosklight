use crate::{ShowStore, StoreError};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::repository::immediate_transaction;

const HISTORY_LIMIT: usize = 100;
const MAX_ID_BYTES: usize = 512;
const MAX_RESULT_DETAIL_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleOccurrenceStatus {
    Claimed,
    Completed,
    Failed,
    Interrupted,
    Skipped,
}

impl ScheduleOccurrenceStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Claimed => "claimed",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
            Self::Skipped => "skipped",
        }
    }

    fn decode(value: &str) -> Result<Self, StoreError> {
        match value {
            "claimed" => Ok(Self::Claimed),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "interrupted" => Ok(Self::Interrupted),
            "skipped" => Ok(Self::Skipped),
            _ => Err(StoreError::Invalid(format!(
                "unknown Schedule occurrence status {value}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleOccurrenceClaim {
    pub schedule_id: String,
    pub occurrence_id: String,
    pub scheduled_for: DateTime<Utc>,
    pub target_action: Value,
    pub claimed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkippedScheduleOccurrence {
    pub schedule_id: String,
    pub occurrence_id: String,
    pub scheduled_for: DateTime<Utc>,
    pub target_action: Value,
    pub skipped_at: DateTime<Utc>,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScheduleOccurrenceResolution {
    Completed,
    Failed { reason: String },
    Interrupted { reason: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleOccurrenceRecord {
    pub sequence: u64,
    pub schedule_id: String,
    pub occurrence_id: String,
    pub scheduled_for: DateTime<Utc>,
    pub target_action: Value,
    pub status: ScheduleOccurrenceStatus,
    pub recorded_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub result_detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScheduleOccurrenceClaimResult {
    Claimed(ScheduleOccurrenceRecord),
    AlreadyRecorded(ScheduleOccurrenceRecord),
}

impl ShowStore {
    /// Durably claims one occurrence before action dispatch.
    ///
    /// A repeated identity returns its existing row and never refreshes or reopens the claim.
    pub fn claim_schedule_occurrence(
        &self,
        claim: &ScheduleOccurrenceClaim,
    ) -> Result<ScheduleOccurrenceClaimResult, StoreError> {
        validate_identity("schedule_id", &claim.schedule_id)?;
        validate_identity("occurrence_id", &claim.occurrence_id)?;
        let target_action_json = serde_json::to_string(&claim.target_action)?;
        let tx = immediate_transaction(&self.conn)?;
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO schedule_occurrences(schedule_id,occurrence_id,scheduled_for,target_action_json,status,recorded_at) VALUES(?1,?2,?3,?4,'claimed',?5)",
            params![
                claim.schedule_id,
                claim.occurrence_id,
                timestamp(claim.scheduled_for),
                target_action_json,
                timestamp(claim.claimed_at),
            ],
        )? == 1;
        if inserted {
            prune_history(&tx, &claim.schedule_id)?;
        }
        let record = occurrence(&tx, &claim.schedule_id, &claim.occurrence_id)?
            .expect("inserted or ignored Schedule occurrence has a stored row");
        tx.commit()?;
        Ok(if inserted {
            ScheduleOccurrenceClaimResult::Claimed(record)
        } else {
            ScheduleOccurrenceClaimResult::AlreadyRecorded(record)
        })
    }

    /// Records a deliberately skipped occurrence without creating a dispatch claim.
    pub fn record_skipped_schedule_occurrence(
        &self,
        skipped: &SkippedScheduleOccurrence,
    ) -> Result<ScheduleOccurrenceRecord, StoreError> {
        validate_identity("schedule_id", &skipped.schedule_id)?;
        validate_identity("occurrence_id", &skipped.occurrence_id)?;
        validate_detail(&skipped.reason)?;
        let target_action_json = serde_json::to_string(&skipped.target_action)?;
        let tx = immediate_transaction(&self.conn)?;
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO schedule_occurrences(schedule_id,occurrence_id,scheduled_for,target_action_json,status,recorded_at,resolved_at,result_detail) VALUES(?1,?2,?3,?4,'skipped',?5,?5,?6)",
            params![
                skipped.schedule_id,
                skipped.occurrence_id,
                timestamp(skipped.scheduled_for),
                target_action_json,
                timestamp(skipped.skipped_at),
                skipped.reason,
            ],
        )? == 1;
        if inserted {
            prune_history(&tx, &skipped.schedule_id)?;
        }
        let record = occurrence(&tx, &skipped.schedule_id, &skipped.occurrence_id)?
            .expect("inserted or ignored skipped Schedule occurrence has a stored row");
        tx.commit()?;
        Ok(record)
    }

    pub fn resolve_schedule_occurrence(
        &self,
        schedule_id: &str,
        occurrence_id: &str,
        resolution: ScheduleOccurrenceResolution,
        resolved_at: DateTime<Utc>,
    ) -> Result<ScheduleOccurrenceRecord, StoreError> {
        validate_identity("schedule_id", schedule_id)?;
        validate_identity("occurrence_id", occurrence_id)?;
        let (status, detail) = resolution_parts(&resolution);
        if let Some(detail) = detail {
            validate_detail(detail)?;
        }
        let tx = immediate_transaction(&self.conn)?;
        let updated = tx.execute(
            "UPDATE schedule_occurrences SET status=?3,resolved_at=?4,result_detail=?5 WHERE schedule_id=?1 AND occurrence_id=?2 AND status='claimed'",
            params![
                schedule_id,
                occurrence_id,
                status.as_str(),
                timestamp(resolved_at),
                detail,
            ],
        )? == 1;
        let record = occurrence(&tx, schedule_id, occurrence_id)?.ok_or_else(|| {
            StoreError::Invalid("Schedule occurrence claim does not exist".into())
        })?;
        if !updated && record.status != status {
            return Err(StoreError::Invalid(format!(
                "Schedule occurrence is already {}",
                record.status.as_str()
            )));
        }
        tx.commit()?;
        Ok(record)
    }

    /// Marks every dispatch claim left unfinished by an earlier process as interrupted.
    ///
    /// The rows remain terminal, so startup can never replay them.
    pub fn interrupt_claimed_schedule_occurrences(
        &self,
        interrupted_at: DateTime<Utc>,
        reason: &str,
    ) -> Result<usize, StoreError> {
        validate_detail(reason)?;
        let tx = immediate_transaction(&self.conn)?;
        let updated = tx.execute(
            "UPDATE schedule_occurrences SET status='interrupted',resolved_at=?1,result_detail=?2 WHERE status='claimed'",
            params![timestamp(interrupted_at), reason],
        )?;
        tx.commit()?;
        Ok(updated)
    }

    pub fn schedule_occurrence(
        &self,
        schedule_id: &str,
        occurrence_id: &str,
    ) -> Result<Option<ScheduleOccurrenceRecord>, StoreError> {
        validate_identity("schedule_id", schedule_id)?;
        validate_identity("occurrence_id", occurrence_id)?;
        occurrence(&self.conn, schedule_id, occurrence_id)
    }

    pub fn latest_schedule_occurrence(
        &self,
        schedule_id: &str,
    ) -> Result<Option<ScheduleOccurrenceRecord>, StoreError> {
        validate_identity("schedule_id", schedule_id)?;
        self.conn
            .query_row(
                "SELECT sequence,schedule_id,occurrence_id,scheduled_for,target_action_json,status,recorded_at,resolved_at,result_detail FROM schedule_occurrences WHERE schedule_id=?1 ORDER BY sequence DESC LIMIT 1",
                [schedule_id],
                decode_row,
            )
            .optional()?
            .map(StoredOccurrence::decode)
            .transpose()
    }

    /// Returns newest-first history, bounded to the repository retention contract.
    pub fn schedule_occurrence_history(
        &self,
        schedule_id: &str,
    ) -> Result<Vec<ScheduleOccurrenceRecord>, StoreError> {
        validate_identity("schedule_id", schedule_id)?;
        let mut statement = self.conn.prepare(
            "SELECT sequence,schedule_id,occurrence_id,scheduled_for,target_action_json,status,recorded_at,resolved_at,result_detail FROM schedule_occurrences WHERE schedule_id=?1 ORDER BY sequence DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![schedule_id, HISTORY_LIMIT], decode_row)?;
        rows.map(|row| row?.decode()).collect()
    }
}

fn resolution_parts(
    resolution: &ScheduleOccurrenceResolution,
) -> (ScheduleOccurrenceStatus, Option<&str>) {
    match resolution {
        ScheduleOccurrenceResolution::Completed => (ScheduleOccurrenceStatus::Completed, None),
        ScheduleOccurrenceResolution::Failed { reason } => {
            (ScheduleOccurrenceStatus::Failed, Some(reason))
        }
        ScheduleOccurrenceResolution::Interrupted { reason } => {
            (ScheduleOccurrenceStatus::Interrupted, Some(reason))
        }
    }
}

fn validate_identity(field: &str, value: &str) -> Result<(), StoreError> {
    if value.trim().is_empty() || value.len() > MAX_ID_BYTES {
        return Err(StoreError::Invalid(format!(
            "{field} must contain 1-{MAX_ID_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_detail(value: &str) -> Result<(), StoreError> {
    if value.trim().is_empty() || value.len() > MAX_RESULT_DETAIL_BYTES {
        return Err(StoreError::Invalid(format!(
            "Schedule occurrence detail must contain 1-{MAX_RESULT_DETAIL_BYTES} bytes"
        )));
    }
    Ok(())
}

fn prune_history(tx: &Transaction<'_>, schedule_id: &str) -> Result<(), StoreError> {
    tx.execute(
        "DELETE FROM schedule_occurrences WHERE schedule_id=?1 AND sequence NOT IN (SELECT sequence FROM schedule_occurrences WHERE schedule_id=?1 ORDER BY sequence DESC LIMIT ?2)",
        params![schedule_id, HISTORY_LIMIT],
    )?;
    Ok(())
}

fn occurrence(
    conn: &rusqlite::Connection,
    schedule_id: &str,
    occurrence_id: &str,
) -> Result<Option<ScheduleOccurrenceRecord>, StoreError> {
    conn.query_row(
        "SELECT sequence,schedule_id,occurrence_id,scheduled_for,target_action_json,status,recorded_at,resolved_at,result_detail FROM schedule_occurrences WHERE schedule_id=?1 AND occurrence_id=?2",
        params![schedule_id, occurrence_id],
        decode_row,
    )
    .optional()?
    .map(StoredOccurrence::decode)
    .transpose()
}

struct StoredOccurrence {
    sequence: i64,
    schedule_id: String,
    occurrence_id: String,
    scheduled_for: String,
    target_action_json: String,
    status: String,
    recorded_at: String,
    resolved_at: Option<String>,
    result_detail: Option<String>,
}

impl StoredOccurrence {
    fn decode(self) -> Result<ScheduleOccurrenceRecord, StoreError> {
        Ok(ScheduleOccurrenceRecord {
            sequence: u64::try_from(self.sequence).map_err(|_| {
                StoreError::Invalid("Schedule occurrence sequence is negative".into())
            })?,
            schedule_id: self.schedule_id,
            occurrence_id: self.occurrence_id,
            scheduled_for: decode_timestamp("scheduled_for", &self.scheduled_for)?,
            target_action: serde_json::from_str(&self.target_action_json)?,
            status: ScheduleOccurrenceStatus::decode(&self.status)?,
            recorded_at: decode_timestamp("recorded_at", &self.recorded_at)?,
            resolved_at: self
                .resolved_at
                .as_deref()
                .map(|value| decode_timestamp("resolved_at", value))
                .transpose()?,
            result_detail: self.result_detail,
        })
    }
}

fn decode_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredOccurrence> {
    Ok(StoredOccurrence {
        sequence: row.get(0)?,
        schedule_id: row.get(1)?,
        occurrence_id: row.get(2)?,
        scheduled_for: row.get(3)?,
        target_action_json: row.get(4)?,
        status: row.get(5)?,
        recorded_at: row.get(6)?,
        resolved_at: row.get(7)?,
        result_detail: row.get(8)?,
    })
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

fn decode_timestamp(field: &str, value: &str) -> Result<DateTime<Utc>, StoreError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| {
            StoreError::Invalid(format!(
                "invalid Schedule occurrence {field} timestamp: {error}"
            ))
        })
}
