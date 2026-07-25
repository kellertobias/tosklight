use super::DeskStore;
use crate::{RevisionCopySource, ShowEntry, ShowRevision, StoreError};
use chrono::Utc;
use light_core::{Revision, ShowId};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

impl DeskStore {
    pub fn library(&self) -> Result<Vec<ShowEntry>, StoreError> {
        let mut statement = self.conn.prepare(
            "SELECT id,name,path,revision,updated_at,revision_source_show_id,revision_source_show_name,revision_source_revision,revision_source_name,revision_copy_created_at FROM show_library ORDER BY name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
            ))
        })?;
        rows.map(|row| {
            let (
                id,
                name,
                path,
                revision,
                updated_at,
                source_show_id,
                source_show_name,
                source_revision,
                source_revision_name,
                copied_at,
            ) = row?;
            let revision_copy = match source_show_id {
                None => None,
                Some(source_show_id) => Some(RevisionCopySource {
                    show_id: ShowId(Uuid::parse_str(&source_show_id)?),
                    show_name: source_show_name.ok_or_else(|| {
                        StoreError::Invalid("revision copy is missing its source show name".into())
                    })?,
                    revision: source_revision.ok_or_else(|| {
                        StoreError::Invalid("revision copy is missing its source revision".into())
                    })? as u64,
                    revision_name: source_revision_name.ok_or_else(|| {
                        StoreError::Invalid(
                            "revision copy is missing its source revision name".into(),
                        )
                    })?,
                    copied_at: copied_at.ok_or_else(|| {
                        StoreError::Invalid("revision copy is missing its creation time".into())
                    })?,
                }),
            };
            Ok(ShowEntry {
                id: ShowId(Uuid::parse_str(&id)?),
                name,
                path,
                revision: revision as u64,
                updated_at,
                revision_copy,
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
        self.upsert_show_with_revision_copy(name, path, overwrite, None)
    }

    pub fn upsert_show_with_revision_copy(
        &self,
        name: &str,
        path: &str,
        overwrite: bool,
        revision_copy: Option<&RevisionCopySource>,
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
            self.conn.execute(
                "INSERT INTO show_library (id,name,path,revision,updated_at,revision_source_show_id,revision_source_show_name,revision_source_revision,revision_source_name,revision_copy_created_at) VALUES (?1,?2,?3,1,?4,?5,?6,?7,?8,?9)",
                params![
                    id.0.to_string(),
                    name,
                    path,
                    now,
                    revision_copy.map(|source| source.show_id.0.to_string()),
                    revision_copy.map(|source| source.show_name.as_str()),
                    revision_copy.map(|source| source.revision as i64),
                    revision_copy.map(|source| source.revision_name.as_str()),
                    revision_copy.map(|source| source.copied_at.as_str()),
                ],
            )?;
            id
        };
        self.show(id)?
            .ok_or_else(|| StoreError::Invalid("show index update failed".into()))
    }

    pub fn mark_show_updated(&self, id: ShowId) -> Result<ShowEntry, StoreError> {
        if self.conn.execute(
            "UPDATE show_library SET revision=revision+1,updated_at=?1 WHERE id=?2",
            params![Utc::now().to_rfc3339(), id.0.to_string()],
        )? != 1
        {
            return Err(StoreError::Invalid("show does not exist".into()));
        }
        self.show(id)?
            .ok_or_else(|| StoreError::Invalid("show index update failed".into()))
    }

    pub fn rename_show(&self, id: ShowId, name: &str, path: &str) -> Result<ShowEntry, StoreError> {
        if self.conn.execute(
            "UPDATE show_library SET name=?1,path=?2,revision=revision+1,updated_at=?3 WHERE id=?4",
            params![name, path, Utc::now().to_rfc3339(), id.0.to_string()],
        )? != 1
        {
            return Err(StoreError::Invalid("show does not exist".into()));
        }
        self.show(id)?
            .ok_or_else(|| StoreError::Invalid("show index update failed".into()))
    }

    /// Repoints an indexed show after its containing desk-data directory was moved.
    ///
    /// Relocation is metadata repair, not an operator save, so it intentionally does not
    /// increment the show's content revision or change its updated timestamp.
    pub fn relocate_show(&self, id: ShowId, path: &str) -> Result<ShowEntry, StoreError> {
        if self.conn.execute(
            "UPDATE show_library SET path=?1 WHERE id=?2",
            params![path, id.0.to_string()],
        )? != 1
        {
            return Err(StoreError::Invalid("show does not exist".into()));
        }
        self.show(id)?
            .ok_or_else(|| StoreError::Invalid("show index update failed".into()))
    }

    /// Repoints a named revision after its containing desk-data directory was moved.
    pub fn relocate_show_revision(
        &self,
        show_id: ShowId,
        revision: Revision,
        path: &str,
    ) -> Result<(), StoreError> {
        if self.conn.execute(
            "UPDATE show_revisions SET path=?1 WHERE show_id=?2 AND revision=?3",
            params![path, show_id.0.to_string(), revision as i64],
        )? != 1
        {
            return Err(StoreError::Invalid("show revision does not exist".into()));
        }
        Ok(())
    }

    pub fn remove_show(&self, id: ShowId) -> Result<bool, StoreError> {
        Ok(self
            .conn
            .execute("DELETE FROM show_library WHERE id=?1", [id.0.to_string()])?
            == 1)
    }

    pub fn show_revisions(&self, show_id: ShowId) -> Result<Vec<ShowRevision>, StoreError> {
        let mut statement = self.conn.prepare(
            "SELECT revision,name,path,created_at FROM show_revisions WHERE show_id=?1 ORDER BY revision DESC",
        )?;
        let rows = statement.query_map([show_id.0.to_string()], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (revision, name, path, created_at) = row?;
            Ok(ShowRevision {
                show_id,
                revision: revision as u64,
                name,
                path,
                created_at,
            })
        })
        .collect()
    }

    pub fn show_revision(
        &self,
        show_id: ShowId,
        revision: Revision,
    ) -> Result<Option<ShowRevision>, StoreError> {
        Ok(self
            .show_revisions(show_id)?
            .into_iter()
            .find(|candidate| candidate.revision == revision))
    }

    pub fn add_show_revision(
        &mut self,
        show_id: ShowId,
        name: &str,
        path: &str,
    ) -> Result<ShowRevision, StoreError> {
        let name = name.trim();
        if name.is_empty() || name.len() > 120 {
            return Err(StoreError::Invalid(
                "revision name must contain 1-120 characters".into(),
            ));
        }
        if self.show(show_id)?.is_none() {
            return Err(StoreError::Invalid("show does not exist".into()));
        }
        let transaction = self.conn.transaction()?;
        let revision = transaction.query_row(
            "SELECT COALESCE(MAX(revision),0)+1 FROM show_revisions WHERE show_id=?1",
            [show_id.0.to_string()],
            |row| row.get::<_, i64>(0),
        )?;
        let created_at = Utc::now().to_rfc3339();
        transaction.execute(
            "INSERT INTO show_revisions(show_id,revision,name,path,created_at) VALUES(?1,?2,?3,?4,?5)",
            params![show_id.0.to_string(), revision, name, path, created_at],
        )?;
        transaction.commit()?;
        Ok(ShowRevision {
            show_id,
            revision: revision as u64,
            name: name.to_owned(),
            path: path.to_owned(),
            created_at,
        })
    }
}
