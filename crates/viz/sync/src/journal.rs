//! The durable journal of show-edit intents not yet confirmed by the desk.
//!
//! One SQLite file per association, opened with `synchronous=FULL`, so an entry the Architect
//! reported as "saved on this computer" survives a power cut. Entries keep their request identity
//! for life: a retry after a lost reply, a restart or a reconnect sends the same identity and the
//! desk applies it once. Conflicted and rejected entries stay until the operator resolves them —
//! they are the recoverable drafts.

use crate::intent::{PendingOperation, ShowEditIntent};
use light_wire::v2::show_sync::{ShowSyncConflict, ShowSyncTransactionOutcome};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

type Answer<T> = Result<T, String>;

/// Where one journal entry stands.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryState {
    /// Waiting to be sent, or sent without a confirmed answer.
    Pending,
    /// Applied by the desk.
    Accepted,
    /// Partly applied; the conflicting fields await the operator.
    Conflict,
    /// Refused by the desk; the draft is kept for the operator.
    Rejected,
    /// Resolved by the operator, kept as history.
    Superseded,
}

impl EntryState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Conflict => "conflict",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
        }
    }

    fn parse(value: &str) -> Answer<Self> {
        Ok(match value {
            "pending" | "in_flight" => Self::Pending,
            "accepted" => Self::Accepted,
            "conflict" => Self::Conflict,
            "rejected" => Self::Rejected,
            "superseded" => Self::Superseded,
            other => return Err(format!("unknown journal entry state {other:?}")),
        })
    }
}

/// The desk's answer to an entry, with the drafts a conflict must keep recoverable.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct EntryOutcome {
    #[serde(default)]
    pub show_revision: u64,
    #[serde(default)]
    pub conflicts: Vec<ShowSyncConflict>,
    /// The Architect's whole body of each conflicted object when the conflict was reported,
    /// keyed `kind/id`, so a deleted object can be recreated from the draft.
    #[serde(default)]
    pub drafts: BTreeMap<String, Value>,
    #[serde(default)]
    pub error: Option<String>,
    /// `true` when the conflict was found locally, not by the desk: a draft recovered after the
    /// confirmed mirror had to be rebuilt.
    #[serde(default)]
    pub recovered: bool,
}

impl EntryOutcome {
    pub fn from_wire(outcome: &ShowSyncTransactionOutcome) -> Self {
        Self {
            show_revision: outcome.show_revision,
            conflicts: outcome.conflicts.clone(),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct JournalEntry {
    pub seq: i64,
    pub request_id: String,
    pub created_at_millis: u64,
    pub base_show_revision: u64,
    pub operations: Vec<PendingOperation>,
    pub state: EntryState,
    pub outcome: Option<EntryOutcome>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct JournalCounts {
    pub pending: usize,
    pub conflicts: usize,
    pub rejected: usize,
}

pub struct Journal {
    connection: Connection,
    path: PathBuf,
}

/// A journal file that could not be read. The file is moved aside, never deleted.
#[derive(Debug)]
pub struct DamagedJournal {
    pub set_aside: Option<PathBuf>,
    pub reason: String,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS entries(
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id TEXT NOT NULL UNIQUE,
    created_at_millis INTEGER NOT NULL,
    base_show_revision INTEGER NOT NULL,
    operations TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('pending','in_flight','accepted','conflict','rejected','superseded')),
    outcome TEXT
);
CREATE INDEX IF NOT EXISTS entries_by_state ON entries(state, seq);
";

impl Journal {
    /// Opens the journal at `path`. A damaged file is moved beside itself and a fresh journal is
    /// started; the caller reports the damage and recovers the document's unconfirmed edits.
    pub fn open(path: &Path) -> Result<(Self, Option<DamagedJournal>), String> {
        match Self::open_checked(path) {
            Ok(journal) => Ok((journal, None)),
            Err(reason) => {
                let set_aside = set_aside(path);
                let journal = Self::open_checked(path)?;
                Ok((journal, Some(DamagedJournal { set_aside, reason })))
            }
        }
    }

    fn open_checked(path: &Path) -> Answer<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let connection = Connection::open(path).map_err(|e| e.to_string())?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA busy_timeout=2000;",
            )
            .map_err(|e| e.to_string())?;
        let check: String = connection
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if check != "ok" {
            return Err(format!("integrity check failed: {check}"));
        }
        connection
            .execute_batch(SCHEMA)
            .map_err(|e| e.to_string())?;
        let journal = Self {
            connection,
            path: path.to_path_buf(),
        };
        // Reading every entry proves the rows decode, not just that the pages are intact.
        journal.entries_where("1=1")?;
        Ok(journal)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Records one gesture durably. Returns the entry the engine will send.
    pub fn append(&self, intent: ShowEditIntent, base_show_revision: u64) -> Answer<JournalEntry> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let operations = intent.into_operations();
        let created_at_millis = now_millis();
        self.connection
            .execute(
                "INSERT INTO entries(request_id,created_at_millis,base_show_revision,operations,state)
                 VALUES(?1,?2,?3,?4,'pending')",
                params![
                    request_id,
                    created_at_millis as i64,
                    base_show_revision as i64,
                    serde_json::to_string(&operations).map_err(|e| e.to_string())?,
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(JournalEntry {
            seq: self.connection.last_insert_rowid(),
            request_id,
            created_at_millis,
            base_show_revision,
            operations,
            state: EntryState::Pending,
            outcome: None,
        })
    }

    /// A conflict entry found locally, carrying no edits of its own: the drafts recovered when
    /// the confirmed mirror had to be rebuilt.
    pub fn append_recovered(&self, base_show_revision: u64) -> Answer<i64> {
        self.connection
            .execute(
                "INSERT INTO entries(request_id,created_at_millis,base_show_revision,operations,state)
                 VALUES(?1,?2,?3,'[]','conflict')",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    now_millis() as i64,
                    base_show_revision as i64,
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(self.connection.last_insert_rowid())
    }

    /// Every entry still owed to the desk, oldest first.
    pub fn pending(&self) -> Answer<Vec<JournalEntry>> {
        self.entries_where("state IN ('pending','in_flight')")
    }

    /// Entries that shape the working document or need the operator: pending, conflicted,
    /// rejected, and accepted ones whose commit the mirror may not have seen yet.
    pub fn unresolved(&self) -> Answer<Vec<JournalEntry>> {
        self.entries_where("state IN ('pending','in_flight','accepted','conflict','rejected')")
    }

    pub fn entry(&self, seq: i64) -> Answer<Option<JournalEntry>> {
        Ok(self
            .entries_where(&format!("seq={seq}"))?
            .into_iter()
            .next())
    }

    pub fn record(&self, seq: i64, state: EntryState, outcome: &EntryOutcome) -> Answer<()> {
        self.connection
            .execute(
                "UPDATE entries SET state=?2, outcome=?3 WHERE seq=?1",
                params![
                    seq,
                    state.as_str(),
                    serde_json::to_string(outcome).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_state(&self, seq: i64, state: EntryState) -> Answer<()> {
        self.connection
            .execute(
                "UPDATE entries SET state=?2 WHERE seq=?1",
                params![seq, state.as_str()],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Forgets accepted entries the mirror already holds, keeping the newest `keep` for history.
    pub fn prune_accepted(&self, mirrored_revision: u64, keep: usize) -> Answer<()> {
        let accepted = self.entries_where("state='accepted'")?;
        let removable = accepted
            .iter()
            .filter(|entry| {
                entry
                    .outcome
                    .as_ref()
                    .is_some_and(|outcome| outcome.show_revision <= mirrored_revision)
            })
            .count()
            .saturating_sub(keep);
        for entry in accepted.iter().take(removable) {
            self.connection
                .execute("DELETE FROM entries WHERE seq=?1", [entry.seq])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn entries_where(&self, condition: &str) -> Answer<Vec<JournalEntry>> {
        let mut statement = self
            .connection
            .prepare(&format!(
                "SELECT seq,request_id,created_at_millis,base_show_revision,operations,state,outcome
                 FROM entries WHERE {condition} ORDER BY seq"
            ))
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut entries = Vec::new();
        for row in rows {
            let (seq, request_id, created, base, operations, state, outcome) =
                row.map_err(|e| e.to_string())?;
            entries.push(JournalEntry {
                seq,
                request_id,
                created_at_millis: created.max(0) as u64,
                base_show_revision: base.max(0) as u64,
                operations: serde_json::from_str(&operations)
                    .map_err(|e| format!("journal entry {seq} is unreadable: {e}"))?,
                state: EntryState::parse(&state)?,
                outcome: outcome
                    .map(|outcome| serde_json::from_str(&outcome))
                    .transpose()
                    .map_err(|e| format!("journal entry {seq} outcome is unreadable: {e}"))?,
            });
        }
        Ok(entries)
    }

    /// How many entries await the desk, how many conflicts await the operator, and how many
    /// entries the desk refused.
    pub fn counts(&self) -> Answer<JournalCounts> {
        let mut counts = JournalCounts::default();
        for entry in self.entries_where("state IN ('pending','in_flight','conflict','rejected')")? {
            match entry.state {
                EntryState::Pending => counts.pending += 1,
                EntryState::Conflict => {
                    counts.conflicts += entry
                        .outcome
                        .as_ref()
                        .map_or(1, |outcome| outcome.conflicts.len().max(1));
                }
                EntryState::Rejected => counts.rejected += 1,
                _ => {}
            }
        }
        Ok(counts)
    }

    /// Whether the journal holds `request_id`, for recognising the desk's echo of an entry.
    pub fn holds(&self, request_id: &str) -> Answer<bool> {
        self.connection
            .query_row(
                "SELECT 1 FROM entries WHERE request_id=?1",
                [request_id],
                |_| Ok(()),
            )
            .optional()
            .map(|found| found.is_some())
            .map_err(|e| e.to_string())
    }
}

/// Moves a damaged SQLite file and its WAL companions beside themselves.
pub(crate) fn set_aside(path: &Path) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("journal");
    let target = path.with_file_name(format!("{stem}.damaged-{}.sqlite", now_millis()));
    std::fs::rename(path, &target).ok()?;
    for suffix in ["-wal", "-shm"] {
        let companion = PathBuf::from(format!("{}{suffix}", path.display()));
        if companion.exists() {
            let _ = std::fs::rename(
                &companion,
                PathBuf::from(format!("{}{suffix}", target.display())),
            );
        }
    }
    Some(target)
}

pub(crate) fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
#[path = "journal_tests.rs"]
mod tests;
