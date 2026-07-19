use crate::{
    PortableShowObjectUndo, RevisionCopySource, StoreError, VersionedObject, connection::configure,
    portable,
};
use light_core::{Revision, ShowId, UserId};
use rusqlite::{Connection, MAIN_DB, OpenFlags, OptionalExtension, params};
use std::path::Path;
use uuid::Uuid;

pub struct ShowStore {
    pub(crate) conn: Connection,
}

pub struct AtomicObjectWrite<'a> {
    pub kind: &'a str,
    pub id: &'a str,
    pub body: &'a serde_json::Value,
    pub expected: Revision,
}

pub struct AtomicObjectDelete<'a> {
    pub kind: &'a str,
    pub id: &'a str,
    pub expected: Revision,
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
        if version.is_none_or(|version| version < portable::SHOW_SCHEMA_VERSION) {
            portable::migrate_show(&mut conn)?;
        }
        portable::validate_show_connection(&conn)?;
        Ok(Self { conn })
    }

    pub fn create(path: impl AsRef<Path>, name: &str) -> Result<(Self, ShowId), StoreError> {
        let mut conn = Connection::open(path)?;
        configure(&conn)?;
        portable::migrate_show(&mut conn)?;
        let id = ShowId::new();
        let transaction = conn.transaction()?;
        transaction.execute(
            "INSERT INTO metadata(key,value) VALUES ('show_id',?1)",
            [id.0.to_string()],
        )?;
        transaction.execute("INSERT INTO metadata(key,value) VALUES ('name',?1)", [name])?;
        portable::initialise_revision(&transaction)?;
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

    pub fn set_identity(
        &self,
        id: ShowId,
        name: &str,
        revision_copy: Option<&RevisionCopySource>,
    ) -> Result<(), StoreError> {
        let transaction = self.conn.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO metadata(key,value) VALUES('show_id',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [id.0.to_string()],
        )?;
        transaction.execute(
            "INSERT INTO metadata(key,value) VALUES('name',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [name],
        )?;
        transaction.execute(
            "DELETE FROM metadata WHERE key IN ('revision_source_show_id','revision_source_show_name','revision_source_revision','revision_source_name','revision_copy_created_at')",
            [],
        )?;
        if let Some(source) = revision_copy {
            for (key, value) in [
                ("revision_source_show_id", source.show_id.0.to_string()),
                ("revision_source_show_name", source.show_name.clone()),
                ("revision_source_revision", source.revision.to_string()),
                ("revision_source_name", source.revision_name.clone()),
                ("revision_copy_created_at", source.copied_at.clone()),
            ] {
                transaction.execute(
                    "INSERT INTO metadata(key,value) VALUES(?1,?2)",
                    params![key, value],
                )?;
            }
        }
        portable::bump_revision(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn revision_copy_source(&self) -> Result<Option<RevisionCopySource>, StoreError> {
        let value = |key: &str| -> Result<Option<String>, StoreError> {
            self.conn
                .query_row("SELECT value FROM metadata WHERE key=?1", [key], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(Into::into)
        };
        let Some(show_id) = value("revision_source_show_id")? else {
            return Ok(None);
        };
        let required = |key: &str| -> Result<String, StoreError> {
            value(key)?.ok_or_else(|| {
                StoreError::Invalid(format!("revision copy metadata is missing {key}"))
            })
        };
        Ok(Some(RevisionCopySource {
            show_id: ShowId(Uuid::parse_str(&show_id)?),
            show_name: required("revision_source_show_name")?,
            revision: required("revision_source_revision")?.parse().map_err(|_| {
                StoreError::Invalid("revision copy source revision is invalid".into())
            })?,
            revision_name: required("revision_source_name")?,
            copied_at: required("revision_copy_created_at")?,
        }))
    }

    pub fn put_object(
        &self,
        kind: &str,
        id: &str,
        body: &serde_json::Value,
        expected: Revision,
    ) -> Result<Revision, StoreError> {
        portable::put_legacy_object(&self.conn, kind, id, body, expected)
    }

    /// Applies related versioned object writes/deletes in one SQLite transaction. Playback slot
    /// assignment uses this so a definition and its page identity can never be partially saved.
    pub fn mutate_objects_atomically(
        &self,
        writes: &[AtomicObjectWrite<'_>],
        deletes: &[AtomicObjectDelete<'_>],
    ) -> Result<Vec<Revision>, StoreError> {
        portable::mutate_legacy_objects(&self.conn, writes, deletes)
    }

    pub fn undo_object(
        &self,
        kind: &str,
        id: &str,
        expected: Revision,
    ) -> Result<Revision, StoreError> {
        portable::undo_legacy_object(&self.conn, kind, id, expected)
    }

    /// Reads the exact previous raw body and its compare-and-pop history condition.
    pub fn prepare_object_undo(
        &self,
        kind: &str,
        id: &str,
        expected: Revision,
    ) -> Result<PortableShowObjectUndo, StoreError> {
        portable::prepare_undo(&self.conn, kind, id, expected)
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

    pub fn delete_object(&self, kind: &str, id: &str) -> Result<bool, StoreError> {
        portable::delete_legacy_object(&self.conn, kind, id)
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
    portable::validate_show_connection(&conn)?;
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
