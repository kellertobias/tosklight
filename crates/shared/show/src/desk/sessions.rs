//! The desk's persisted Programmer sessions.
//!
//! This file used to also own a `users` table: a row per operator, with no password and no
//! credential of any kind. A login named one, the lookup could only ever find the single seeded
//! `Operator`, and nothing else chose between them. The session and its token carry the desk's
//! authority; the row named nobody.

use super::DeskStore;
use crate::{PersistedSession, StoreError};
use light_core::SessionId;
use rusqlite::params;
use uuid::Uuid;

impl DeskStore {
    pub fn save_session(&self, session: &PersistedSession) -> Result<(), StoreError> {
        self.conn.execute("INSERT INTO sessions(id,token,programmer_json,connected,updated_at) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET token=excluded.token,programmer_json=excluded.programmer_json,connected=excluded.connected,updated_at=excluded.updated_at", params![session.id.0.to_string(), session.token, session.programmer_json, i64::from(session.connected), session.updated_at])?;
        Ok(())
    }

    pub fn persisted_sessions(&self) -> Result<Vec<PersistedSession>, StoreError> {
        let mut statement = self.conn.prepare("SELECT id,token,programmer_json,connected,updated_at FROM sessions ORDER BY updated_at")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (id, token, programmer_json, connected, updated_at) = row?;
            Ok(PersistedSession {
                id: SessionId(Uuid::parse_str(&id)?),
                token,
                programmer_json,
                connected: connected != 0,
                updated_at,
            })
        })
        .collect()
    }

    pub fn delete_session(&self, id: SessionId) -> Result<bool, StoreError> {
        Ok(self
            .conn
            .execute("DELETE FROM sessions WHERE id=?1", [id.0.to_string()])?
            == 1)
    }
}
