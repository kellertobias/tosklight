use super::{FixtureLibrary, FixturePackageLoadReport};
use crate::{
    FIXTURE_PACKAGE_EXTENSION, FixtureError, FixtureProfile, read_fixture_package,
    write_fixture_package,
};
use light_core::FixtureId;
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

impl FixtureLibrary {
    /// Imports the exact same portable archive used for desk-to-desk transfer. Stable profile IDs
    /// are retained; changed content becomes a new local revision of the same fixture family.
    pub fn import_fixture_package(&self, bytes: &[u8]) -> Result<FixtureProfile, FixtureError> {
        let mut profile = read_fixture_package(bytes)
            .map_err(|error| FixtureError::Invalid(error.to_string()))?;
        profile.reserved_source = None;
        let current = self.latest_profile_for_id(profile.id)?;
        if let Some(existing) = &current {
            ensure_same_fixture_family(existing, &profile)?;
            if normalized_profile_json(existing)? == normalized_profile_json(&profile)? {
                return Ok(existing.clone());
            }
        }
        self.save_profile(profile, current.map_or(0, |profile| profile.revision))
    }

    pub fn export_fixture_package(
        &self,
        id: FixtureId,
        revision: u32,
    ) -> Result<Option<Vec<u8>>, FixtureError> {
        self.profile(id, revision)?
            .map(|profile| {
                write_fixture_package(&profile)
                    .map_err(|error| FixtureError::Invalid(error.to_string()))
            })
            .transpose()
    }

    /// Loads a shipped directory of normal transferable packages. Package upgrades are applied
    /// only while the installed revision is still current; a later operator revision always wins.
    pub fn load_fixture_package_directory(
        &self,
        directory: impl AsRef<Path>,
    ) -> Result<FixturePackageLoadReport, FixtureError> {
        let directory = directory.as_ref();
        let mut paths = fs::read_dir(directory)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path.extension().is_some_and(|extension| {
                        extension.eq_ignore_ascii_case(FIXTURE_PACKAGE_EXTENSION)
                    })
            })
            .collect::<Vec<_>>();
        paths.sort();
        let mut report = FixturePackageLoadReport::default();
        for path in paths {
            let bytes = fs::read(&path)?;
            let digest = format!("{:x}", Sha256::digest(&bytes));
            let package_key = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    FixtureError::Invalid("fixture package filename is not UTF-8".into())
                })?
                .to_owned();
            let mut incoming = read_fixture_package(&bytes)
                .map_err(|error| FixtureError::Invalid(format!("{}: {error}", path.display())))?;
            incoming.reserved_source = None;
            let installation = self
                .conn
                .query_row(
                    "SELECT package_digest,profile_id,installed_revision FROM fixture_package_installations WHERE package_path=?1",
                    [&package_key],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, u32>(2)?)),
                )
                .optional()?;
            if installation.as_ref().is_some_and(
                |(installed_digest, installed_id, installed_revision)| {
                    installed_digest == &digest
                        && installed_id == &incoming.id.0.to_string()
                        && self
                            .profile(incoming.id, *installed_revision)
                            .ok()
                            .flatten()
                            .is_some()
                },
            ) {
                report.unchanged += 1;
                continue;
            }
            let current = self.latest_profile_for_id(incoming.id)?;
            let legacy_packaged_profile = installation.is_none()
                && incoming.manufacturer.eq_ignore_ascii_case("Generic")
                && self.conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM fixture_profile_legacy_sources WHERE profile_id=?1)",
                    [incoming.id.0.to_string()],
                    |row| row.get::<_, bool>(0),
                )?;
            let stored = match current {
                None => {
                    report.installed += 1;
                    self.save_profile(incoming, 0)?
                }
                Some(existing) => {
                    ensure_same_fixture_family(&existing, &incoming)?;
                    if normalized_profile_json(&existing)? == normalized_profile_json(&incoming)? {
                        report.unchanged += 1;
                        existing
                    } else if legacy_packaged_profile
                        || installation
                            .as_ref()
                            .is_some_and(|(_, installed_id, revision)| {
                                installed_id == &incoming.id.0.to_string()
                                    && *revision == existing.revision
                            })
                    {
                        report.updated += 1;
                        self.save_profile(incoming, existing.revision)?
                    } else {
                        report.preserved_operator_revisions += 1;
                        self.conn.execute(
                            "INSERT OR IGNORE INTO fixture_library_warnings(message) VALUES(?1)",
                            [format!(
                                "Shipped fixture package {package_key} was not applied because {} {} has an operator revision.",
                                existing.manufacturer, existing.name
                            )],
                        )?;
                        continue;
                    }
                }
            };
            if legacy_packaged_profile {
                self.retire_packaged_legacy_sources(stored.id)?;
            }
            self.conn.execute(
                "INSERT INTO fixture_package_installations(package_path,package_digest,profile_id,installed_revision) VALUES(?1,?2,?3,?4) ON CONFLICT(package_path) DO UPDATE SET package_digest=excluded.package_digest,profile_id=excluded.profile_id,installed_revision=excluded.installed_revision",
                params![package_key, digest, stored.id.0.to_string(), stored.revision],
            )?;
        }
        Ok(report)
    }

    fn retire_packaged_legacy_sources(&self, profile_id: FixtureId) -> Result<(), FixtureError> {
        let transaction = self.conn.unchecked_transaction()?;
        let legacy_ids = {
            let mut statement = transaction.prepare(
                "SELECT DISTINCT legacy_id FROM fixture_profile_legacy_sources WHERE profile_id=?1",
            )?;
            statement
                .query_map([profile_id.0.to_string()], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        transaction.execute(
            "DELETE FROM fixture_profile_legacy_sources WHERE profile_id=?1",
            [profile_id.0.to_string()],
        )?;
        transaction.execute(
            "DELETE FROM fixture_profile_legacy_map WHERE profile_id=?1",
            [profile_id.0.to_string()],
        )?;
        for legacy_id in legacy_ids {
            transaction.execute(
                "DELETE FROM fixture_profile_migration_failures WHERE legacy_id=?1",
                [&legacy_id],
            )?;
            transaction.execute("DELETE FROM fixture_definitions WHERE id=?1", [&legacy_id])?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn latest_profile_for_id(&self, id: FixtureId) -> Result<Option<FixtureProfile>, FixtureError> {
        self.conn
            .query_row(
                "SELECT profile_json FROM fixture_profiles WHERE id=?1 ORDER BY revision DESC LIMIT 1",
                [id.0.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(FixtureError::from))
            .transpose()
    }
}

fn normalized_profile_json(profile: &FixtureProfile) -> Result<String, FixtureError> {
    let mut profile = profile.clone();
    profile.revision = 0;
    profile.reserved_source = None;
    Ok(serde_json::to_string(&profile)?)
}

fn ensure_same_fixture_family(
    existing: &FixtureProfile,
    incoming: &FixtureProfile,
) -> Result<(), FixtureError> {
    if existing
        .manufacturer
        .trim()
        .eq_ignore_ascii_case(incoming.manufacturer.trim())
        && existing
            .name
            .trim()
            .eq_ignore_ascii_case(incoming.name.trim())
    {
        Ok(())
    } else {
        Err(FixtureError::Invalid(format!(
            "fixture package ID {} belongs to {} {}, not {} {}",
            incoming.id.0,
            existing.manufacturer,
            existing.name,
            incoming.manufacturer,
            incoming.name
        )))
    }
}
