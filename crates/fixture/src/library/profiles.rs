use super::{FixtureLibrary, LegacyFixtureProfileSource};
use crate::profile::FIXTURE_PROFILE_SCHEMA_VERSION;
use crate::{FixtureDefinition, FixtureError, FixtureProfile};
use light_core::FixtureId;
use rusqlite::{OptionalExtension, params};

impl FixtureLibrary {
    pub fn import_json(&self, json: &str) -> Result<FixtureDefinition, FixtureError> {
        self.import_json_with_source(json, None)
    }
    pub fn import_json_with_source(
        &self,
        json: &str,
        source_gdtf: Option<&[u8]>,
    ) -> Result<FixtureDefinition, FixtureError> {
        let fixture: FixtureDefinition = serde_json::from_str(json)?;
        fixture.validate()?;
        self.conn.execute("INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id,revision) DO UPDATE SET manufacturer=excluded.manufacturer,model=excluded.model,mode=excluded.mode,definition_json=excluded.definition_json,source_gdtf=COALESCE(excluded.source_gdtf,fixture_definitions.source_gdtf)",params![fixture.id.0.to_string(),fixture.revision,fixture.manufacturer,fixture.model,fixture.mode,json,source_gdtf])?;
        self.migrate_legacy_profiles()?;
        Ok(fixture)
    }

    /// Latest complete profile revisions, one atomic record per manufacturer fixture family.
    pub fn profiles(&self) -> Result<Vec<FixtureProfile>, FixtureError> {
        let mut statement = self.conn.prepare(
            "SELECT p.profile_json FROM fixture_profiles p JOIN (SELECT id,MAX(revision) revision FROM fixture_profiles GROUP BY id) latest ON latest.id=p.id AND latest.revision=p.revision ORDER BY p.manufacturer COLLATE NOCASE,p.name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }

    /// Resolves every ordered mode in each latest profile to the portable definition snapshot
    /// consumed by patching. Legacy rows whose migration failed remain patchable while the
    /// corresponding warning explains how to recover or repair them.
    pub fn patchable_definitions(&self) -> Result<Vec<FixtureDefinition>, FixtureError> {
        let mut definitions = Vec::new();
        for profile in self.profiles()? {
            for mode in &profile.modes {
                definitions.push(
                    profile
                        .resolved_definition(mode.id)
                        .map_err(|error| FixtureError::Invalid(error.to_string()))?,
                );
            }
        }
        let mut statement = self.conn.prepare(
            "SELECT f.definition_json FROM fixture_definitions f JOIN fixture_profile_migration_failures x ON x.legacy_id=f.id AND x.legacy_revision=f.revision ORDER BY f.manufacturer COLLATE NOCASE,f.model COLLATE NOCASE,f.mode COLLATE NOCASE",
        )?;
        let failures = statement.query_map([], |row| row.get::<_, String>(0))?;
        for json in failures {
            definitions.push(serde_json::from_str(&json?)?);
        }
        definitions.sort_by(|left, right| {
            (
                left.manufacturer.to_lowercase(),
                left.name.to_lowercase(),
                left.mode.to_lowercase(),
            )
                .cmp(&(
                    right.manufacturer.to_lowercase(),
                    right.name.to_lowercase(),
                    right.mode.to_lowercase(),
                ))
        });
        Ok(definitions)
    }

    pub fn profile(
        &self,
        id: FixtureId,
        revision: u32,
    ) -> Result<Option<FixtureProfile>, FixtureError> {
        self.conn
            .query_row(
                "SELECT profile_json FROM fixture_profiles WHERE id=?1 AND revision=?2",
                params![id.0.to_string(), revision],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(FixtureError::from))
            .transpose()
    }

    pub fn profile_revisions(&self, id: FixtureId) -> Result<Vec<u32>, FixtureError> {
        let mut statement = self
            .conn
            .prepare("SELECT revision FROM fixture_profiles WHERE id=?1 ORDER BY revision")?;
        Ok(statement
            .query_map([id.0.to_string()], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }

    /// Deletes one immutable fixture-profile revision. Patched shows remain unaffected because
    /// they carry their own profile/mode snapshot rather than consulting the live library.
    pub fn delete_profile(&self, id: FixtureId, revision: u32) -> Result<bool, FixtureError> {
        self.conn.execute(
            "DELETE FROM fixture_profile_sources WHERE profile_id=?1 AND profile_revision=?2",
            params![id.0.to_string(), revision],
        )?;
        Ok(self.conn.execute(
            "DELETE FROM fixture_profiles WHERE id=?1 AND revision=?2",
            params![id.0.to_string(), revision],
        )? == 1)
    }

    /// Stores a whole profile as one immutable revision. The server/library assigns the revision;
    /// clients can only state which current revision they edited.
    pub fn save_profile(
        &self,
        mut profile: FixtureProfile,
        expected_revision: u32,
    ) -> Result<FixtureProfile, FixtureError> {
        let current = self.conn.query_row(
            "SELECT COALESCE(MAX(revision),0) FROM fixture_profiles WHERE id=?1",
            [profile.id.0.to_string()],
            |row| row.get::<_, u32>(0),
        )?;
        if current != expected_revision {
            return Err(FixtureError::RevisionConflict {
                expected: expected_revision,
                current,
            });
        }
        profile.revision = current + 1;
        profile.schema_version = FIXTURE_PROFILE_SCHEMA_VERSION;
        profile.reserved_source = None;
        profile
            .validate()
            .map_err(|error| FixtureError::Invalid(error.to_string()))?;
        let json = serde_json::to_string(&profile)?;
        self.conn.execute(
            "INSERT INTO fixture_profiles(id,revision,manufacturer,name,profile_json,reserved_source) VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                profile.id.0.to_string(),
                profile.revision,
                profile.manufacturer,
                profile.name,
                json,
                profile.reserved_source,
            ],
        )?;
        if current > 0 {
            self.conn.execute(
                "INSERT OR IGNORE INTO fixture_profile_sources(profile_id,profile_revision,source_gdtf) SELECT profile_id,?2,source_gdtf FROM fixture_profile_sources WHERE profile_id=?1 AND profile_revision=?3",
                params![profile.id.0.to_string(), profile.revision, current],
            )?;
        }
        Ok(profile)
    }

    /// Retain the original GDTF archive independently from the normalized editable profile.
    pub fn set_profile_source_gdtf(
        &self,
        id: FixtureId,
        revision: u32,
        source: &[u8],
    ) -> Result<bool, FixtureError> {
        let exists = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM fixture_profiles WHERE id=?1 AND revision=?2)",
            params![id.0.to_string(), revision],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Ok(false);
        }
        self.conn.execute(
            "INSERT INTO fixture_profile_sources(profile_id,profile_revision,source_gdtf) VALUES(?1,?2,?3) ON CONFLICT(profile_id,profile_revision) DO UPDATE SET source_gdtf=excluded.source_gdtf",
            params![id.0.to_string(), revision, source],
        )?;
        Ok(true)
    }

    pub fn profile_source_gdtf(
        &self,
        id: FixtureId,
        revision: u32,
    ) -> Result<Option<Vec<u8>>, FixtureError> {
        self.conn
            .query_row(
                "SELECT source_gdtf FROM fixture_profile_sources WHERE profile_id=?1 AND profile_revision=?2",
                params![id.0.to_string(), revision],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn migration_warnings(&self) -> Result<Vec<String>, FixtureError> {
        let mut statement = self
            .conn
            .prepare("SELECT message FROM fixture_library_warnings ORDER BY id")?;
        Ok(statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }

    pub fn profile_legacy_sources(
        &self,
        id: FixtureId,
        revision: u32,
    ) -> Result<Vec<LegacyFixtureProfileSource>, FixtureError> {
        let mut statement = self.conn.prepare(
            "SELECT legacy_id,definition_json,source_gdtf FROM fixture_profile_legacy_sources WHERE profile_id=?1 AND profile_revision=?2 ORDER BY legacy_id,legacy_revision",
        )?;
        Ok(statement
            .query_map(params![id.0.to_string(), revision], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?)
    }

    pub fn source_gdtf(
        &self,
        id: FixtureId,
        revision: u32,
    ) -> Result<Option<Vec<u8>>, FixtureError> {
        if let Some(source) = self.profile_source_gdtf(id, revision)? {
            return Ok(Some(source));
        }
        self.conn
            .query_row(
                "SELECT source_gdtf FROM fixture_definitions WHERE id=?1 AND revision=?2",
                params![id.0.to_string(), revision],
                |row| row.get(0),
            )
            .optional()
            .map(|value| value.flatten())
            .map_err(Into::into)
    }
    pub fn export_json(
        &self,
        id: FixtureId,
        revision: u32,
    ) -> Result<Option<String>, FixtureError> {
        self.conn
            .query_row(
                "SELECT definition_json FROM fixture_definitions WHERE id=?1 AND revision=?2",
                params![id.0.to_string(), revision],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }
    pub fn revisions(&self, id: FixtureId) -> Result<Vec<u32>, FixtureError> {
        let mut statement = self
            .conn
            .prepare("SELECT revision FROM fixture_definitions WHERE id=?1 ORDER BY revision")?;
        Ok(statement
            .query_map([id.0.to_string()], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }
    pub fn definitions(&self) -> Result<Vec<FixtureDefinition>, FixtureError> {
        let mut statement = self.conn.prepare(
            "SELECT f.definition_json FROM fixture_definitions f JOIN (SELECT id,MAX(revision) revision FROM fixture_definitions GROUP BY id) latest ON latest.id=f.id AND latest.revision=f.revision ORDER BY f.manufacturer COLLATE NOCASE, f.model COLLATE NOCASE, f.mode COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }
    pub fn delete(&self, id: FixtureId, revision: u32) -> Result<bool, FixtureError> {
        Ok(self.conn.execute(
            "DELETE FROM fixture_definitions WHERE id=?1 AND revision=?2",
            params![id.0.to_string(), revision],
        )? == 1)
    }
}
