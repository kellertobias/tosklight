#![forbid(unsafe_code)]
//! Versioned SQLite persistence for desk state and portable, self-contained show files.

use chrono::Utc;
use light_core::{Revision, SessionId, ShowId, UserId};
use rusqlite::{Connection, MAIN_DB, OpenFlags, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

const DESK_SCHEMA_VERSION: i64 = 4;
const SHOW_SCHEMA_VERSION: i64 = 3;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Uuid(#[from] uuid::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid data: {0}")]
    Invalid(String),
    #[error("revision conflict: expected {expected}, current {current}")]
    RevisionConflict {
        expected: Revision,
        current: Revision,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeskUser {
    pub id: UserId,
    pub name: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ControlDesk {
    pub id: Uuid,
    pub name: String,
    pub osc_alias: String,
    pub columns: u8,
    pub rows: u8,
    pub buttons: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScreenConfiguration {
    pub id: Uuid,
    pub name: String,
    pub layout: serde_json::Value,
    pub show_dock: bool,
    pub show_playbacks: bool,
    pub playback_count: u8,
    pub playback_rows: u8,
    pub first_playback_slot: u8,
    pub page_mode: String,
    pub show_page_controls: bool,
    pub desired_open: bool,
    pub display_id: Option<String>,
    pub bounds: Option<serde_json::Value>,
    pub fullscreen: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShowEntry {
    pub id: ShowId,
    pub name: String,
    pub path: String,
    pub revision: Revision,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PersistedSession {
    pub id: SessionId,
    pub user_id: UserId,
    pub token: String,
    pub programmer_json: String,
    pub connected: bool,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VersionedObject {
    pub kind: String,
    pub id: String,
    pub body: serde_json::Value,
    pub revision: Revision,
    pub updated_at: String,
}

pub struct DeskStore {
    conn: Connection,
}

impl DeskStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let mut conn = Connection::open(path)?;
        configure(&conn)?;
        migrate_desk(&mut conn)?;
        let store = Self { conn };
        if store.users()?.is_empty() {
            store.add_user("Operator")?;
        }
        Ok(store)
    }

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

    pub fn desks(&self) -> Result<Vec<ControlDesk>, StoreError> {
        let mut statement = self.conn.prepare("SELECT id,name,osc_alias,columns_count,rows_count,buttons_count FROM control_desks ORDER BY name COLLATE NOCASE")?;
        let rows = statement.query_map([], |row| Ok((row.get::<_,String>(0)?,row.get(1)?,row.get(2)?,row.get::<_,u8>(3)?,row.get::<_,u8>(4)?,row.get::<_,u8>(5)?)))?;
        rows.map(|row| { let (id,name,osc_alias,columns,rows,buttons)=row?; Ok(ControlDesk { id:Uuid::parse_str(&id)?,name,osc_alias,columns,rows,buttons }) }).collect()
    }
    pub fn control_desk(&self, id: Uuid) -> Result<Option<ControlDesk>, StoreError> { Ok(self.desks()?.into_iter().find(|desk| desk.id==id)) }
    pub fn control_desk_by_alias(&self, alias: &str) -> Result<Option<ControlDesk>, StoreError> { Ok(self.desks()?.into_iter().find(|desk| desk.osc_alias.eq_ignore_ascii_case(alias))) }
    pub fn add_desk(&self, name: &str, alias: &str) -> Result<ControlDesk, StoreError> {
        let alias = alias.trim().to_ascii_lowercase();
        if alias.is_empty() || alias.len()>40 || !alias.chars().all(|c| c.is_ascii_alphanumeric() || c=='-' || c=='_') { return Err(StoreError::Invalid("OSC alias must contain only letters, numbers, dash, or underscore".into())); }
        let desk=ControlDesk{id:Uuid::new_v4(),name:name.trim().to_owned(),osc_alias:alias,columns:8,rows:1,buttons:3};
        self.conn.execute("INSERT INTO control_desks(id,name,osc_alias,columns_count,rows_count,buttons_count) VALUES (?1,?2,?3,?4,?5,?6)",params![desk.id.to_string(),desk.name,desk.osc_alias,desk.columns,desk.rows,desk.buttons])?; Ok(desk)
    }
    pub fn update_desk(&self, id:Uuid, name:&str, alias:&str, columns:u8, rows:u8, buttons:u8)->Result<ControlDesk,StoreError>{
        let alias=alias.trim().to_ascii_lowercase(); if name.trim().is_empty() || alias.is_empty() || !alias.chars().all(|c|c.is_ascii_alphanumeric()||c=='-'||c=='_') || !(1..=32).contains(&columns)||!(1..=3).contains(&rows)||buttons>3{return Err(StoreError::Invalid("invalid control desk configuration".into()));}
        if self.conn.execute("UPDATE control_desks SET name=?1,osc_alias=?2,columns_count=?3,rows_count=?4,buttons_count=?5 WHERE id=?6",params![name.trim(),alias,columns,rows,buttons,id.to_string()])?!=1{return Err(StoreError::Invalid("control desk does not exist".into()));}
        self.control_desk(id)?.ok_or_else(||StoreError::Invalid("control desk update failed".into()))
    }
    pub fn desk_page(&self, desk: Uuid, show: ShowId) -> Result<u8, StoreError> {
        Ok(self.conn.query_row("SELECT page FROM control_desk_pages WHERE desk_id=?1 AND show_id=?2",params![desk.to_string(),show.0.to_string()],|row|row.get(0)).optional()?.unwrap_or(1))
    }
    pub fn set_desk_page(&self, desk: Uuid, show: ShowId, page: u8) -> Result<(), StoreError> {
        if !(1..=127).contains(&page) { return Err(StoreError::Invalid("page must be within 1-127".into())); }
        self.conn.execute("INSERT INTO control_desk_pages(desk_id,show_id,page) VALUES (?1,?2,?3) ON CONFLICT(desk_id,show_id) DO UPDATE SET page=excluded.page",params![desk.to_string(),show.0.to_string(),page])?; Ok(())
    }

    pub fn screens(&self) -> Result<Vec<ScreenConfiguration>, StoreError> {
        let mut statement = self.conn.prepare("SELECT id,name,layout_json,show_dock,show_playbacks,playback_count,playback_rows,first_playback_slot,page_mode,show_page_controls,desired_open,display_id,bounds_json,fullscreen FROM screens ORDER BY name COLLATE NOCASE")?;
        let rows = statement.query_map([], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,bool>(3)?,row.get::<_,bool>(4)?,row.get::<_,u8>(5)?,row.get::<_,u8>(6)?,row.get::<_,u8>(7)?,row.get::<_,String>(8)?,row.get::<_,bool>(9)?,row.get::<_,bool>(10)?,row.get::<_,Option<String>>(11)?,row.get::<_,Option<String>>(12)?,row.get::<_,bool>(13)?)))?;
        rows.map(|row| { let (id,name,layout,show_dock,show_playbacks,playback_count,playback_rows,first_playback_slot,page_mode,show_page_controls,desired_open,display_id,bounds,fullscreen)=row?; Ok(ScreenConfiguration{id:Uuid::parse_str(&id)?,name,layout:serde_json::from_str(&layout)?,show_dock,show_playbacks,playback_count,playback_rows,first_playback_slot,page_mode,show_page_controls,desired_open,display_id,bounds:bounds.map(|value|serde_json::from_str(&value)).transpose()?,fullscreen}) }).collect()
    }
    pub fn screen(&self, id: Uuid) -> Result<Option<ScreenConfiguration>, StoreError> { Ok(self.screens()?.into_iter().find(|screen|screen.id==id)) }
    pub fn put_screen(&self, mut screen: ScreenConfiguration) -> Result<ScreenConfiguration, StoreError> {
        screen.name=screen.name.trim().to_owned();
        if screen.name.is_empty() || screen.name.len()>80 || screen.page_mode!="follow_main" && screen.page_mode!="independent" || screen.playback_count==0 || screen.playback_rows==0 || screen.playback_rows>screen.playback_count || screen.first_playback_slot==0 || u16::from(screen.first_playback_slot)+u16::from(screen.playback_count)-1>127 { return Err(StoreError::Invalid("invalid screen configuration".into())); }
        self.conn.execute("INSERT INTO screens(id,name,layout_json,show_dock,show_playbacks,playback_count,playback_rows,first_playback_slot,page_mode,show_page_controls,desired_open,display_id,bounds_json,fullscreen) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14) ON CONFLICT(id) DO UPDATE SET name=excluded.name,layout_json=excluded.layout_json,show_dock=excluded.show_dock,show_playbacks=excluded.show_playbacks,playback_count=excluded.playback_count,playback_rows=excluded.playback_rows,first_playback_slot=excluded.first_playback_slot,page_mode=excluded.page_mode,show_page_controls=excluded.show_page_controls,desired_open=excluded.desired_open,display_id=excluded.display_id,bounds_json=excluded.bounds_json,fullscreen=excluded.fullscreen",params![screen.id.to_string(),screen.name,serde_json::to_string(&screen.layout)?,screen.show_dock,screen.show_playbacks,screen.playback_count,screen.playback_rows,screen.first_playback_slot,screen.page_mode,screen.show_page_controls,screen.desired_open,screen.display_id,screen.bounds.as_ref().map(serde_json::to_string).transpose()?,screen.fullscreen])?;
        self.screen(screen.id)?.ok_or_else(||StoreError::Invalid("screen update failed".into()))
    }
    pub fn delete_screen(&self,id:Uuid)->Result<(),StoreError>{self.conn.execute("DELETE FROM screens WHERE id=?1",[id.to_string()])?;Ok(())}
    pub fn screen_page(&self,screen:Uuid,show:ShowId)->Result<u8,StoreError>{Ok(self.conn.query_row("SELECT page FROM screen_pages WHERE screen_id=?1 AND show_id=?2",params![screen.to_string(),show.0.to_string()],|row|row.get(0)).optional()?.unwrap_or(1))}
    pub fn set_screen_page(&self,screen:Uuid,show:ShowId,page:u8)->Result<(),StoreError>{if !(1..=127).contains(&page){return Err(StoreError::Invalid("page must be within 1-127".into()));} self.conn.execute("INSERT INTO screen_pages(screen_id,show_id,page) VALUES(?1,?2,?3) ON CONFLICT(screen_id,show_id) DO UPDATE SET page=excluded.page",params![screen.to_string(),show.0.to_string(),page])?;Ok(())}

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

    pub fn library(&self) -> Result<Vec<ShowEntry>, StoreError> {
        let mut statement = self.conn.prepare("SELECT id,name,path,revision,updated_at FROM show_library ORDER BY name COLLATE NOCASE")?;
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
            let (id, name, path, revision, updated_at) = row?;
            Ok(ShowEntry {
                id: ShowId(Uuid::parse_str(&id)?),
                name,
                path,
                revision: revision as u64,
                updated_at,
            })
        })
        .collect()
    }

    pub fn show(&self, id: ShowId) -> Result<Option<ShowEntry>, StoreError> {
        Ok(self.library()?.into_iter().find(|entry| entry.id == id))
    }

    pub fn upsert_show(
        &self,
        name: &str,
        path: &str,
        overwrite: bool,
    ) -> Result<ShowEntry, StoreError> {
        let existing = self
            .conn
            .query_row(
                "SELECT id,revision FROM show_library WHERE name=?1 COLLATE NOCASE",
                [name],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        let now = Utc::now().to_rfc3339();
        let id = if let Some((id, revision)) = existing {
            if !overwrite {
                return Err(StoreError::Invalid(
                    "a show with that name already exists".into(),
                ));
            }
            self.conn.execute(
                "UPDATE show_library SET name=?1,path=?2,revision=?3,updated_at=?4 WHERE id=?5",
                params![name, path, revision + 1, now, id],
            )?;
            ShowId(Uuid::parse_str(&id)?)
        } else {
            let id = ShowId::new();
            self.conn.execute("INSERT INTO show_library (id,name,path,revision,updated_at) VALUES (?1,?2,?3,1,?4)", params![id.0.to_string(), name, path, now])?;
            id
        };
        self.show(id)?
            .ok_or_else(|| StoreError::Invalid("show index update failed".into()))
    }

    pub fn remove_show(&self, id: ShowId) -> Result<bool, StoreError> {
        Ok(self
            .conn
            .execute("DELETE FROM show_library WHERE id=?1", [id.0.to_string()])?
            == 1)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn.execute("INSERT INTO settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value])?;
        Ok(())
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(Into::into)
    }

    pub fn set_active_show(&self, id: Option<ShowId>) -> Result<(), StoreError> {
        match id {
            Some(id) => self.set_setting("active_show_id", &id.0.to_string()),
            None => {
                self.conn
                    .execute("DELETE FROM settings WHERE key='active_show_id'", [])?;
                Ok(())
            }
        }
    }

    pub fn active_show(&self) -> Result<Option<ShowEntry>, StoreError> {
        let Some(id) = self.setting("active_show_id")? else {
            return Ok(None);
        };
        self.show(ShowId(Uuid::parse_str(&id)?))
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

pub struct ShowStore {
    conn: Connection,
}

impl ShowStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let mut conn = Connection::open(path)?;
        configure(&conn)?;
        let version = conn
            .query_row("SELECT version FROM schema_info", [], |row| {
                row.get::<_, i64>(0)
            })
            .ok();
        if version.is_none_or(|version| version < SHOW_SCHEMA_VERSION) {
            migrate_show(&mut conn)?;
        }
        validate_show_connection(&conn)?;
        Ok(Self { conn })
    }

    pub fn create(path: impl AsRef<Path>, name: &str) -> Result<(Self, ShowId), StoreError> {
        let mut conn = Connection::open(path)?;
        configure(&conn)?;
        migrate_show(&mut conn)?;
        let id = ShowId::new();
        let transaction = conn.transaction()?;
        transaction.execute(
            "INSERT INTO metadata(key,value) VALUES ('show_id',?1)",
            [id.0.to_string()],
        )?;
        transaction.execute("INSERT INTO metadata(key,value) VALUES ('name',?1)", [name])?;
        transaction.commit()?;
        Ok((Self { conn }, id))
    }

    pub fn id(&self) -> Result<ShowId, StoreError> {
        let value: String = self.conn.query_row(
            "SELECT value FROM metadata WHERE key='show_id'",
            [],
            |row| row.get(0),
        )?;
        Ok(ShowId(Uuid::parse_str(&value)?))
    }

    pub fn name(&self) -> Result<String, StoreError> {
        self.conn
            .query_row("SELECT value FROM metadata WHERE key='name'", [], |row| {
                row.get(0)
            })
            .map_err(Into::into)
    }

    pub fn put_object(
        &self,
        kind: &str,
        id: &str,
        body: &serde_json::Value,
        expected: Revision,
    ) -> Result<Revision, StoreError> {
        let current_row = self
            .conn
            .query_row(
                "SELECT revision,body_json FROM objects WHERE kind=?1 AND id=?2",
                params![kind, id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let current = current_row.as_ref().map(|row| row.0 as u64).unwrap_or(0);
        if current != expected {
            return Err(StoreError::RevisionConflict { expected, current });
        }
        let revision = current + 1;
        if let Some((previous_revision, previous_body)) = current_row {
            self.conn.execute("INSERT INTO object_history(kind,id,revision,body_json,created_at) VALUES (?1,?2,?3,?4,?5)", params![kind,id,previous_revision,previous_body,Utc::now().to_rfc3339()])?;
        }
        self.conn.execute("INSERT INTO objects(kind,id,body_json,revision,updated_at) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(kind,id) DO UPDATE SET body_json=excluded.body_json,revision=excluded.revision,updated_at=excluded.updated_at", params![kind,id,serde_json::to_string(body)?,revision as i64,Utc::now().to_rfc3339()])?;
        Ok(revision)
    }

    pub fn undo_object(
        &self,
        kind: &str,
        id: &str,
        expected: Revision,
    ) -> Result<Revision, StoreError> {
        let current: i64 = self.conn.query_row(
            "SELECT revision FROM objects WHERE kind=?1 AND id=?2",
            params![kind, id],
            |row| row.get(0),
        )?;
        if current as u64 != expected {
            return Err(StoreError::RevisionConflict {
                expected,
                current: current as u64,
            });
        }
        let previous = self.conn.query_row("SELECT rowid,body_json FROM object_history WHERE kind=?1 AND id=?2 ORDER BY rowid DESC LIMIT 1", params![kind,id], |row| Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?))).optional()?.ok_or_else(|| StoreError::Invalid("object has no undo history".into()))?;
        let revision = expected + 1;
        self.conn.execute(
            "UPDATE objects SET body_json=?3,revision=?4,updated_at=?5 WHERE kind=?1 AND id=?2",
            params![
                kind,
                id,
                previous.1,
                revision as i64,
                Utc::now().to_rfc3339()
            ],
        )?;
        self.conn
            .execute("DELETE FROM object_history WHERE rowid=?1", [previous.0])?;
        Ok(revision)
    }

    pub fn objects(&self, kind: &str) -> Result<Vec<VersionedObject>, StoreError> {
        let mut statement = self.conn.prepare(
            "SELECT id,body_json,revision,updated_at FROM objects WHERE kind=?1 ORDER BY id",
        )?;
        let rows = statement.query_map([kind], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (id, body, revision, updated_at) = row?;
            Ok(VersionedObject {
                kind: kind.into(),
                id,
                body: serde_json::from_str(&body)?,
                revision: revision as u64,
                updated_at,
            })
        })
        .collect()
    }

    pub fn put_user_layout(
        &self,
        user_id: UserId,
        layout: &serde_json::Value,
        expected: Revision,
    ) -> Result<Revision, StoreError> {
        self.put_object("user_layout", &user_id.0.to_string(), layout, expected)
    }

    pub fn backup_to(&self, destination: impl AsRef<Path>) -> Result<(), StoreError> {
        self.conn.execute_batch("PRAGMA wal_checkpoint(FULL)")?;
        self.conn.backup(MAIN_DB, destination, None)?;
        Ok(())
    }
}

pub fn initialise_show(path: impl AsRef<Path>, name: &str) -> Result<ShowId, StoreError> {
    if path.as_ref().exists() {
        return ShowStore::open(path)?.id();
    }
    let (_, id) = ShowStore::create(path, name)?;
    Ok(id)
}

pub fn validate_show_file(path: impl AsRef<Path>) -> Result<(ShowId, String), StoreError> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    validate_show_connection(&conn)?;
    let id: String = conn.query_row(
        "SELECT value FROM metadata WHERE key='show_id'",
        [],
        |row| row.get(0),
    )?;
    let name: String =
        conn.query_row("SELECT value FROM metadata WHERE key='name'", [], |row| {
            row.get(0)
        })?;
    Ok((ShowId(Uuid::parse_str(&id)?), name))
}

fn configure(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch("PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL")
}

fn migrate_desk(conn: &mut Connection) -> Result<(), StoreError> {
    let tx = conn.transaction()?;
    tx.execute_batch(r#"CREATE TABLE IF NOT EXISTS schema_info(version INTEGER NOT NULL); INSERT INTO schema_info(version) SELECT 0 WHERE NOT EXISTS(SELECT 1 FROM schema_info);
      CREATE TABLE IF NOT EXISTS users(id TEXT PRIMARY KEY,name TEXT NOT NULL UNIQUE COLLATE NOCASE,enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN(0,1)));
      CREATE TABLE IF NOT EXISTS show_library(id TEXT PRIMARY KEY,name TEXT NOT NULL UNIQUE COLLATE NOCASE,path TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 1,updated_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS control_desks(id TEXT PRIMARY KEY,name TEXT NOT NULL,osc_alias TEXT NOT NULL UNIQUE COLLATE NOCASE,columns_count INTEGER NOT NULL DEFAULT 8,rows_count INTEGER NOT NULL DEFAULT 1,buttons_count INTEGER NOT NULL DEFAULT 3);
      CREATE TABLE IF NOT EXISTS control_desk_pages(desk_id TEXT NOT NULL,show_id TEXT NOT NULL,page INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(desk_id,show_id),FOREIGN KEY(desk_id) REFERENCES control_desks(id) ON DELETE CASCADE);
      CREATE TABLE IF NOT EXISTS screens(id TEXT PRIMARY KEY,name TEXT NOT NULL,layout_json TEXT NOT NULL DEFAULT '{"desks":[],"activeDeskId":""}',show_dock INTEGER NOT NULL DEFAULT 1,show_playbacks INTEGER NOT NULL DEFAULT 1,playback_count INTEGER NOT NULL DEFAULT 8,playback_rows INTEGER NOT NULL DEFAULT 1,first_playback_slot INTEGER NOT NULL DEFAULT 1,page_mode TEXT NOT NULL DEFAULT 'follow_main',show_page_controls INTEGER NOT NULL DEFAULT 1,desired_open INTEGER NOT NULL DEFAULT 0,display_id TEXT,bounds_json TEXT,fullscreen INTEGER NOT NULL DEFAULT 0);
      CREATE TABLE IF NOT EXISTS screen_pages(screen_id TEXT NOT NULL,show_id TEXT NOT NULL,page INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(screen_id,show_id),FOREIGN KEY(screen_id) REFERENCES screens(id) ON DELETE CASCADE);
      CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,user_id TEXT NOT NULL,token TEXT NOT NULL,programmer_json TEXT NOT NULL,connected INTEGER NOT NULL CHECK(connected IN(0,1)),updated_at TEXT NOT NULL,FOREIGN KEY(user_id) REFERENCES users(id));"#)?;
    set_schema_version(&tx, DESK_SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

fn migrate_show(conn: &mut Connection) -> Result<(), StoreError> {
    let tx = conn.transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS schema_info(version INTEGER NOT NULL); INSERT INTO schema_info(version) SELECT 0 WHERE NOT EXISTS(SELECT 1 FROM schema_info);
      CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS embedded_fixtures(id TEXT NOT NULL,revision INTEGER NOT NULL,definition_json TEXT NOT NULL,PRIMARY KEY(id,revision));
      CREATE TABLE IF NOT EXISTS objects(kind TEXT NOT NULL,id TEXT NOT NULL,body_json TEXT NOT NULL,revision INTEGER NOT NULL,updated_at TEXT NOT NULL,PRIMARY KEY(kind,id));
      CREATE TABLE IF NOT EXISTS object_history(kind TEXT NOT NULL,id TEXT NOT NULL,revision INTEGER NOT NULL,body_json TEXT NOT NULL,created_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS cues(cue_list_id TEXT NOT NULL,cue_number REAL NOT NULL,values_json TEXT NOT NULL,cue_only_restore_json TEXT,revision INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(cue_list_id,cue_number));
      CREATE INDEX IF NOT EXISTS objects_kind ON objects(kind);")?;
    set_schema_version(&tx, SHOW_SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

fn set_schema_version(tx: &Transaction<'_>, version: i64) -> Result<(), rusqlite::Error> {
    tx.execute("UPDATE schema_info SET version=?1", [version])?;
    Ok(())
}

fn validate_show_connection(conn: &Connection) -> Result<(), StoreError> {
    let integrity: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(StoreError::Invalid(format!(
            "SQLite integrity check failed: {integrity}"
        )));
    }
    let version: i64 = conn
        .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .map_err(|_| StoreError::Invalid("not a Light show file: schema_info is missing".into()))?;
    if version > SHOW_SCHEMA_VERSION {
        return Err(StoreError::Invalid(format!(
            "show schema {version} is newer than supported schema {SHOW_SCHEMA_VERSION}"
        )));
    }
    for key in ["show_id", "name"] {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM metadata WHERE key=?1)",
            [key],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(StoreError::Invalid(format!(
                "show metadata is missing {key}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temporary(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("light-{name}-{}.sqlite", Uuid::new_v4()))
    }

    #[test]
    fn screens_persist_and_keep_independent_pages_per_show() {
        let path=temporary("screens"); let store=DeskStore::open(&path).unwrap(); let show=ShowId::new(); let id=Uuid::new_v4();
        let screen=ScreenConfiguration{id,name:"Wing".into(),layout:serde_json::json!({"desks":[],"activeDeskId":""}),show_dock:false,show_playbacks:true,playback_count:12,playback_rows:2,first_playback_slot:20,page_mode:"independent".into(),show_page_controls:false,desired_open:true,display_id:Some("display".into()),bounds:Some(serde_json::json!({"x":1,"y":2,"width":800,"height":600})),fullscreen:true};
        store.put_screen(screen).unwrap(); store.set_screen_page(id,show,7).unwrap(); let restored=store.screen(id).unwrap().unwrap(); assert_eq!(restored.first_playback_slot,20);assert_eq!(restored.playback_count,12);assert_eq!(store.screen_page(id,show).unwrap(),7);drop(store);let _=fs::remove_file(path);
    }

    #[test]
    fn screen_playback_range_must_fit_page_slots() {
        let path=temporary("screen-validation");let store=DeskStore::open(&path).unwrap();let invalid=ScreenConfiguration{id:Uuid::new_v4(),name:"Bad".into(),layout:serde_json::json!({}),show_dock:true,show_playbacks:true,playback_count:9,playback_rows:1,first_playback_slot:120,page_mode:"follow_main".into(),show_page_controls:true,desired_open:false,display_id:None,bounds:None,fullscreen:false};assert!(store.put_screen(invalid).is_err());drop(store);let _=fs::remove_file(path);
    }

    #[test]
    fn desk_sessions_survive_reopen() {
        let path = temporary("desk");
        let (user, session) = {
            let desk = DeskStore::open(&path).unwrap();
            let user = desk.users().unwrap().remove(0);
            let session = PersistedSession {
                id: SessionId::new(),
                user_id: user.id,
                token: "token".into(),
                programmer_json: "{}".into(),
                connected: false,
                updated_at: Utc::now().to_rfc3339(),
            };
            desk.save_session(&session).unwrap();
            (user, session)
        };
        let desk = DeskStore::open(&path).unwrap();
        let loaded = desk.persisted_sessions().unwrap();
        assert_eq!(loaded[0].id, session.id);
        assert_eq!(loaded[0].user_id, user.id);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn control_desks_have_unique_aliases_and_per_show_pages() {
        let path=temporary("control-desks"); let desk=DeskStore::open(&path).unwrap();
        let control=desk.add_desk("Front","front-desk").unwrap(); assert!(desk.add_desk("Other","front-desk").is_err());
        let first=ShowId::new(); let second=ShowId::new(); desk.set_desk_page(control.id,first,12).unwrap();
        assert_eq!(desk.desk_page(control.id,first).unwrap(),12); assert_eq!(desk.desk_page(control.id,second).unwrap(),1); assert!(desk.set_desk_page(control.id,first,128).is_err());
        drop(desk); let _=fs::remove_file(path);
    }

    #[test]
    fn show_objects_enforce_optimistic_revisions() {
        let path = temporary("show");
        let (show, _) = ShowStore::create(&path, "Tour").unwrap();
        assert_eq!(
            show.put_object("preset", "one", &serde_json::json!({"value": 1}), 0)
                .unwrap(),
            1
        );
        assert!(matches!(
            show.put_object("preset", "one", &serde_json::json!({"value": 2}), 0),
            Err(StoreError::RevisionConflict {
                expected: 0,
                current: 1
            })
        ));
        assert_eq!(show.objects("preset").unwrap()[0].revision, 1);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn group_membership_edits_are_undoable_with_revision_protection() {
        let path = temporary("group-undo");
        let (show, _) = ShowStore::create(&path, "Template").unwrap();
        assert_eq!(
            show.put_object("group", "front", &serde_json::json!({"fixtures":[]}), 0)
                .unwrap(),
            1
        );
        assert_eq!(
            show.put_object(
                "group",
                "front",
                &serde_json::json!({"fixtures":["fixture-a"]}),
                1
            )
            .unwrap(),
            2
        );
        assert_eq!(show.undo_object("group", "front", 2).unwrap(), 3);
        let group = &show.objects("group").unwrap()[0];
        assert_eq!(group.body["fixtures"], serde_json::json!([]));
        assert!(matches!(
            show.undo_object("group", "front", 2),
            Err(StoreError::RevisionConflict { current: 3, .. })
        ));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn backup_is_a_standalone_valid_show() {
        let source = temporary("source");
        let backup = temporary("backup");
        let (show, id) = ShowStore::create(&source, "Portable").unwrap();
        show.put_object("group", "front", &serde_json::json!([1, 2, 3]), 0)
            .unwrap();
        show.backup_to(&backup).unwrap();
        assert_eq!(
            validate_show_file(&backup).unwrap(),
            (id, "Portable".into())
        );
        let reopened = ShowStore::open(&backup).unwrap();
        assert_eq!(reopened.objects("group").unwrap().len(), 1);
        let _ = fs::remove_file(source);
        let _ = fs::remove_file(backup);
    }

    #[test]
    fn concurrent_readers_do_not_rerun_show_migrations() {
        let path = temporary("concurrent");
        let (show, _) = ShowStore::create(&path, "Concurrent").unwrap();
        show.put_object("group", "front", &serde_json::json!({"fixtures":[]}), 0)
            .unwrap();
        drop(show);
        let handles = (0..8)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || ShowStore::open(path).unwrap().objects("group").unwrap())
            })
            .collect::<Vec<_>>();
        for handle in handles {
            assert_eq!(handle.join().unwrap().len(), 1);
        }
        let _ = fs::remove_file(path);
    }

    #[test]
    fn desk_always_retains_an_enabled_login_user() {
        let path = temporary("users");
        let desk = DeskStore::open(&path).unwrap();
        let operator = desk.users().unwrap().remove(0);
        assert!(desk.update_user(operator.id, "Operator", false).is_err());
        assert!(desk.delete_user(operator.id).is_err());
        let second = desk.add_user("Programmer").unwrap();
        desk.update_user(operator.id, "Operator", false).unwrap();
        assert!(!desk.user(operator.id).unwrap().unwrap().enabled);
        assert!(desk.delete_user(operator.id).unwrap());
        assert_eq!(desk.users().unwrap(), vec![second]);
        let _ = fs::remove_file(path);
    }
}
