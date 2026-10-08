//! The confirmed mirror: the desk's synchronized content at the show revision this Architect last
//! confirmed. Only the desk's commits and snapshots advance it; the Architect's own edits reach it
//! only once the desk has committed them.

use crate::journal::set_aside;
use crate::state::{ObjectKey, ProfileKey, VersionedState};
use rusqlite::{Connection, params};
use serde_json::Value;
use std::path::{Path, PathBuf};

type Answer<T> = Result<T, String>;

pub struct Mirror {
    connection: Connection,
    state: VersionedState,
    /// `false` until the mirror holds a snapshot of the desk: a new binding before its first
    /// confirmation, or a damaged mirror that was set aside.
    trusted: bool,
}

/// The mirror file could not be read and was moved aside.
#[derive(Debug)]
pub struct DamagedMirror {
    pub set_aside: Option<PathBuf>,
    pub reason: String,
}

/// One change the desk committed, as the mirror records it.
#[derive(Clone, Debug, PartialEq)]
pub enum MirrorChange {
    Object {
        key: ObjectKey,
        revision: u64,
        body: Value,
    },
    Removed(ObjectKey),
    Metadata {
        key: String,
        value: Option<String>,
    },
    Profile(ProfileKey),
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS facts(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS objects(kind TEXT NOT NULL, id TEXT NOT NULL, revision INTEGER NOT NULL,
    body TEXT NOT NULL, PRIMARY KEY(kind, id));
CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS profiles(profile_id TEXT NOT NULL, revision INTEGER NOT NULL,
    PRIMARY KEY(profile_id, revision));
";

impl Mirror {
    pub fn open(path: &Path) -> Answer<(Self, Option<DamagedMirror>)> {
        match Self::open_checked(path) {
            Ok(mirror) => Ok((mirror, None)),
            Err(reason) => {
                let set_aside = set_aside(path);
                let mirror = Self::open_checked(path)?;
                Ok((mirror, Some(DamagedMirror { set_aside, reason })))
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
        let mut mirror = Self {
            connection,
            state: VersionedState::default(),
            trusted: false,
        };
        mirror.load()?;
        Ok(mirror)
    }

    fn load(&mut self) -> Answer<()> {
        let fact = |key: &str| -> Answer<Option<String>> {
            let mut statement = self
                .connection
                .prepare("SELECT value FROM facts WHERE key=?1")
                .map_err(|e| e.to_string())?;
            let mut rows = statement.query([key]).map_err(|e| e.to_string())?;
            Ok(match rows.next().map_err(|e| e.to_string())? {
                Some(row) => Some(row.get(0).map_err(|e| e.to_string())?),
                None => None,
            })
        };
        self.trusted = fact("trusted")?.as_deref() == Some("1");
        self.state.show_revision = fact("show_revision")?
            .map(|value| value.parse().map_err(|_| "mirror revision is unreadable"))
            .transpose()?
            .unwrap_or(0);
        let mut statement = self
            .connection
            .prepare("SELECT kind,id,revision,body FROM objects")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (kind, id, revision, body) = row.map_err(|e| e.to_string())?;
            let key = ObjectKey::new(kind, id);
            let body: Value = serde_json::from_str(&body)
                .map_err(|e| format!("mirror object {}/{} is unreadable: {e}", key.kind, key.id))?;
            self.state
                .revisions
                .insert(key.clone(), revision.max(0) as u64);
            self.state.state.objects.insert(key, body);
        }
        drop(statement);
        let mut statement = self
            .connection
            .prepare("SELECT key,value FROM metadata")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (key, value) = row.map_err(|e| e.to_string())?;
            self.state.state.metadata.insert(key, value);
        }
        drop(statement);
        let mut statement = self
            .connection
            .prepare("SELECT profile_id,revision FROM profiles")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (id, revision) = row.map_err(|e| e.to_string())?;
            let id = id
                .parse()
                .map_err(|_| "mirror profile identity is unreadable")?;
            self.state.profiles.insert((id, revision.max(0) as u64));
        }
        Ok(())
    }

    pub fn state(&self) -> &VersionedState {
        &self.state
    }

    pub fn trusted(&self) -> bool {
        self.trusted
    }

    pub fn show_revision(&self) -> u64 {
        self.state.show_revision
    }

    /// Replaces the whole mirror with a snapshot of the desk, in one commit.
    pub fn replace(&mut self, snapshot: VersionedState) -> Answer<()> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        transaction
            .execute_batch("DELETE FROM objects; DELETE FROM metadata; DELETE FROM profiles;")
            .map_err(|e| e.to_string())?;
        for (key, body) in &snapshot.state.objects {
            let revision = snapshot.revisions.get(key).copied().unwrap_or(0);
            insert_object(&transaction, key, revision, body)?;
        }
        for (key, value) in &snapshot.state.metadata {
            transaction
                .execute(
                    "INSERT INTO metadata(key,value) VALUES(?1,?2)",
                    params![key, value],
                )
                .map_err(|e| e.to_string())?;
        }
        for (id, revision) in &snapshot.profiles {
            insert_profile(&transaction, (*id, *revision))?;
        }
        write_facts(&transaction, snapshot.show_revision)?;
        transaction.commit().map_err(|e| e.to_string())?;
        self.state = snapshot;
        self.trusted = true;
        Ok(())
    }

    /// Applies one desk commit and advances the confirmed revision, in one commit.
    pub fn apply(&mut self, show_revision: u64, changes: &[MirrorChange]) -> Answer<()> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        for change in changes {
            match change {
                MirrorChange::Object {
                    key,
                    revision,
                    body,
                } => insert_object(&transaction, key, *revision, body)?,
                MirrorChange::Removed(key) => {
                    transaction
                        .execute(
                            "DELETE FROM objects WHERE kind=?1 AND id=?2",
                            params![key.kind, key.id],
                        )
                        .map_err(|e| e.to_string())?;
                }
                MirrorChange::Metadata { key, value } => match value {
                    Some(value) if !value.is_empty() => {
                        transaction
                            .execute(
                                "INSERT OR REPLACE INTO metadata(key,value) VALUES(?1,?2)",
                                params![key, value],
                            )
                            .map_err(|e| e.to_string())?;
                    }
                    _ => {
                        transaction
                            .execute("DELETE FROM metadata WHERE key=?1", [key])
                            .map_err(|e| e.to_string())?;
                    }
                },
                MirrorChange::Profile(profile) => insert_profile(&transaction, *profile)?,
            }
        }
        write_facts(&transaction, show_revision)?;
        transaction.commit().map_err(|e| e.to_string())?;
        for change in changes {
            match change.clone() {
                MirrorChange::Object {
                    key,
                    revision,
                    body,
                } => {
                    self.state.revisions.insert(key.clone(), revision);
                    self.state.state.objects.insert(key, body);
                }
                MirrorChange::Removed(key) => {
                    self.state.revisions.remove(&key);
                    self.state.state.objects.remove(&key);
                }
                MirrorChange::Metadata { key, value } => match value {
                    Some(value) if !value.is_empty() => {
                        self.state.state.metadata.insert(key, value);
                    }
                    _ => {
                        self.state.state.metadata.remove(&key);
                    }
                },
                MirrorChange::Profile(profile) => {
                    self.state.profiles.insert(profile);
                }
            }
        }
        self.state.show_revision = show_revision;
        Ok(())
    }
}

fn insert_object(
    transaction: &rusqlite::Transaction<'_>,
    key: &ObjectKey,
    revision: u64,
    body: &Value,
) -> Answer<()> {
    transaction
        .execute(
            "INSERT OR REPLACE INTO objects(kind,id,revision,body) VALUES(?1,?2,?3,?4)",
            params![key.kind, key.id, revision as i64, body.to_string()],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn insert_profile(transaction: &rusqlite::Transaction<'_>, profile: ProfileKey) -> Answer<()> {
    transaction
        .execute(
            "INSERT OR IGNORE INTO profiles(profile_id,revision) VALUES(?1,?2)",
            params![profile.0.to_string(), profile.1 as i64],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn write_facts(transaction: &rusqlite::Transaction<'_>, show_revision: u64) -> Answer<()> {
    transaction
        .execute(
            "INSERT OR REPLACE INTO facts(key,value) VALUES('show_revision',?1),('trusted','1')",
            [show_revision.to_string()],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}
