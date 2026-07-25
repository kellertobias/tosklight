use super::DeskStore;
use crate::{DeskUser, PersistedSession, StoreError};
use light_core::{SessionId, UserId};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

impl DeskStore {
    pub fn add_user(&self, name: &str) -> Result<DeskUser, StoreError> {
        let name = name.trim();
        if name.is_empty() || name.len() > 80 {
            return Err(StoreError::Invalid(
                "user name must contain 1-80 characters".into(),
            ));
        }
        let user = DeskUser {
            id: UserId::new(),
            name: name.to_owned(),
            enabled: true,
        };
        self.conn.execute(
            "INSERT INTO users (id,name,enabled) VALUES (?1,?2,1)",
            params![user.id.0.to_string(), user.name],
        )?;
        Ok(user)
    }

    pub fn users(&self) -> Result<Vec<DeskUser>, StoreError> {
        let mut statement = self
            .conn
            .prepare("SELECT id,name,enabled FROM users ORDER BY name COLLATE NOCASE")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (id, name, enabled) = row?;
            Ok(DeskUser {
                id: UserId(Uuid::parse_str(&id)?),
                name,
                enabled: enabled != 0,
            })
        })
        .collect()
    }

    pub fn find_user(&self, name: &str) -> Result<Option<DeskUser>, StoreError> {
        let row = self
            .conn
            .query_row(
                "SELECT id,name,enabled FROM users WHERE name=?1 COLLATE NOCASE",
                [name],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(id, name, enabled)| {
            Ok(DeskUser {
                id: UserId(Uuid::parse_str(&id)?),
                name,
                enabled: enabled != 0,
            })
        })
        .transpose()
    }

    pub fn user(&self, id: UserId) -> Result<Option<DeskUser>, StoreError> {
        let row = self
            .conn
            .query_row(
                "SELECT id,name,enabled FROM users WHERE id=?1",
                [id.0.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(id, name, enabled)| {
            Ok(DeskUser {
                id: UserId(Uuid::parse_str(&id)?),
                name,
                enabled: enabled != 0,
            })
        })
        .transpose()
    }

    pub fn update_user(
        &self,
        id: UserId,
        name: &str,
        enabled: bool,
    ) -> Result<DeskUser, StoreError> {
        let name = name.trim();
        if name.is_empty() || name.len() > 80 {
            return Err(StoreError::Invalid(
                "user name must contain 1-80 characters".into(),
            ));
        }
        if !enabled {
            let enabled_count: i64 =
                self.conn
                    .query_row("SELECT COUNT(*) FROM users WHERE enabled=1", [], |row| {
                        row.get(0)
                    })?;
            if enabled_count <= 1 && self.user(id)?.is_some_and(|user| user.enabled) {
                return Err(StoreError::Invalid(
                    "the last enabled desk user cannot be disabled".into(),
                ));
            }
        }
        if self.conn.execute(
            "UPDATE users SET name=?1,enabled=?2 WHERE id=?3",
            params![name, i64::from(enabled), id.0.to_string()],
        )? != 1
        {
            return Err(StoreError::Invalid("user does not exist".into()));
        }
        self.user(id)?
            .ok_or_else(|| StoreError::Invalid("user update failed".into()))
    }

    pub fn delete_user(&self, id: UserId) -> Result<bool, StoreError> {
        let Some(user) = self.user(id)? else {
            return Ok(false);
        };
        if user.enabled {
            let count: i64 =
                self.conn
                    .query_row("SELECT COUNT(*) FROM users WHERE enabled=1", [], |row| {
                        row.get(0)
                    })?;
            if count <= 1 {
                return Err(StoreError::Invalid(
                    "the last enabled desk user cannot be deleted".into(),
                ));
            }
        }
        Ok(self
            .conn
            .execute("DELETE FROM users WHERE id=?1", [id.0.to_string()])?
            == 1)
    }

    pub fn save_session(&self, session: &PersistedSession) -> Result<(), StoreError> {
        self.conn.execute("INSERT INTO sessions(id,user_id,token,programmer_json,connected,updated_at) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET user_id=excluded.user_id,token=excluded.token,programmer_json=excluded.programmer_json,connected=excluded.connected,updated_at=excluded.updated_at", params![session.id.0.to_string(), session.user_id.0.to_string(), session.token, session.programmer_json, i64::from(session.connected), session.updated_at])?;
        Ok(())
    }

    pub fn persisted_sessions(&self) -> Result<Vec<PersistedSession>, StoreError> {
        let mut statement = self.conn.prepare("SELECT id,user_id,token,programmer_json,connected,updated_at FROM sessions ORDER BY updated_at")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        rows.map(|row| {
            let (id, user_id, token, programmer_json, connected, updated_at) = row?;
            Ok(PersistedSession {
                id: SessionId(Uuid::parse_str(&id)?),
                user_id: UserId(Uuid::parse_str(&user_id)?),
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
