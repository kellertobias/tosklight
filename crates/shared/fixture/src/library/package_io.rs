use super::{FixtureLibrary, FixturePackageLoadReport};
use crate::{
    FIXTURE_PACKAGE_EXTENSION, FixtureError, FixtureProfile, read_fixture_package,
    write_fixture_package,
};
use light_core::FixtureId;
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

/// Shipped packages that were withdrawn, each with the profile it installed.
///
/// Loading only looks at the packages that are there, so a package deleted from the shipped
/// directory would otherwise leave its profile in every library that ever installed it. The
/// fifteen decks on fixed legs — one package per platform size and leg height — were withdrawn
/// when a deck on regular feet became one profile generated at the height it is placed, so each
/// is taken out of the library it was installed into. A revision the operator made of one is
/// theirs and stays, and no show is touched: a patched fixture carries its own profile snapshot.
const WITHDRAWN_PACKAGES: &[(&str, &str)] = &[
    (
        "venue--stage-deck-1-0-5-m-legs-0-2-m.toskfixture",
        "c1d26de5-4fe2-594d-99e2-812145650314",
    ),
    (
        "venue--stage-deck-1-0-5-m-legs-0-4-m.toskfixture",
        "6476799e-fc6f-5ddc-b4f6-fa1325881aa2",
    ),
    (
        "venue--stage-deck-1-0-5-m-legs-0-6-m.toskfixture",
        "64e0ee52-d22e-5073-991c-1c93272ea285",
    ),
    (
        "venue--stage-deck-1-0-5-m-legs-0-8-m.toskfixture",
        "b2bf9af3-36f3-5bf4-8a19-0f58e9c034f1",
    ),
    (
        "venue--stage-deck-1-0-5-m-legs-1-m.toskfixture",
        "811e02d4-82e0-5962-85a7-3efd81a226ff",
    ),
    (
        "venue--stage-deck-1-1-m-legs-0-2-m.toskfixture",
        "bf818699-247f-5db9-b5b7-daad4137e57c",
    ),
    (
        "venue--stage-deck-1-1-m-legs-0-4-m.toskfixture",
        "115d33ff-1189-5f89-a9eb-3c19993f491a",
    ),
    (
        "venue--stage-deck-1-1-m-legs-0-6-m.toskfixture",
        "e3fdb557-e013-5c40-8efa-1d24cb1a7714",
    ),
    (
        "venue--stage-deck-1-1-m-legs-0-8-m.toskfixture",
        "7edb68c9-efcc-547c-80f3-bdcb1fa03003",
    ),
    (
        "venue--stage-deck-1-1-m-legs-1-m.toskfixture",
        "fe6992bc-9981-52e4-916d-9b1b8fb17c1e",
    ),
    (
        "venue--stage-deck-2-1-m-legs-0-2-m.toskfixture",
        "f5cb3a55-4e4f-5dfd-8c0e-43cf7924b096",
    ),
    (
        "venue--stage-deck-2-1-m-legs-0-4-m.toskfixture",
        "3cf7a16e-95e8-5cf3-bd54-e65743883acf",
    ),
    (
        "venue--stage-deck-2-1-m-legs-0-6-m.toskfixture",
        "9f510d06-6bb8-5dd1-bd7c-6774226d1586",
    ),
    (
        "venue--stage-deck-2-1-m-legs-0-8-m.toskfixture",
        "a1b0a402-7953-562c-8c57-6346df823ce3",
    ),
    (
        "venue--stage-deck-2-1-m-legs-1-m.toskfixture",
        "6541286a-f448-55c5-98ef-9e707b8e5a36",
    ),
];

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
        let mut report = FixturePackageLoadReport {
            retired: self.retire_withdrawn_packages()?,
            ..Default::default()
        };
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
                    let taxonomy_migration = self.is_shipped_tosklight_taxonomy_migration(
                        &existing,
                        &incoming,
                        &package_key,
                    )?;
                    if !taxonomy_migration {
                        ensure_same_fixture_family(&existing, &incoming)?;
                    }
                    if normalized_profile_json(&existing)? == normalized_profile_json(&incoming)? {
                        report.unchanged += 1;
                        existing
                    } else if taxonomy_migration
                        || legacy_packaged_profile
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

    fn is_shipped_tosklight_taxonomy_migration(
        &self,
        existing: &FixtureProfile,
        incoming: &FixtureProfile,
        package_key: &str,
    ) -> Result<bool, FixtureError> {
        let old_package = match (
            existing.manufacturer.as_str(),
            existing.name.as_str(),
            incoming.manufacturer.as_str(),
            incoming.name.as_str(),
            package_key,
        ) {
            (
                "Generic",
                "Visualizer Camera",
                "ToskLight",
                "Visualizer Camera",
                "tosklight--visualizer-camera.toskfixture",
            ) => Some("generic--visualizer-camera.toskfixture"),
            (
                "Generic",
                "Laser",
                "ToskLight",
                "Visualizer Laser",
                "tosklight--visualizer-laser.toskfixture",
            ) => Some("generic--laser.toskfixture"),
            _ => None,
        };
        let Some(old_package) = old_package else {
            return Ok(false);
        };
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM fixture_package_installations WHERE package_path=?1 AND profile_id=?2 AND installed_revision=?3)",
                params![old_package, existing.id.0.to_string(), existing.revision],
                |row| row.get(0),
            )
            .map_err(FixtureError::from)
    }

    /// Takes every [`WITHDRAWN_PACKAGES`] profile this library installed back out of it.
    ///
    /// Only a profile still at the revision its package installed is retired; once an operator has
    /// revised one it is their fixture, and it is left alone with its installation record.
    fn retire_withdrawn_packages(&self) -> Result<usize, FixtureError> {
        let mut retired = 0;
        for (package, profile_id) in WITHDRAWN_PACKAGES {
            let Some(installed_revision) = self
                .conn
                .query_row(
                    "SELECT installed_revision FROM fixture_package_installations WHERE package_path=?1 AND profile_id=?2",
                    params![package, profile_id],
                    |row| row.get::<_, u32>(0),
                )
                .optional()?
            else {
                continue;
            };
            let newest: u32 = self.conn.query_row(
                "SELECT COALESCE(MAX(revision),0) FROM fixture_profiles WHERE id=?1",
                [profile_id],
                |row| row.get(0),
            )?;
            if newest > installed_revision {
                continue;
            }
            let transaction = self.conn.unchecked_transaction()?;
            transaction.execute(
                "DELETE FROM fixture_profile_sources WHERE profile_id=?1",
                [profile_id],
            )?;
            transaction.execute("DELETE FROM fixture_profiles WHERE id=?1", [profile_id])?;
            transaction.execute(
                "DELETE FROM fixture_package_installations WHERE package_path=?1",
                [package],
            )?;
            transaction.commit()?;
            retired += 1;
        }
        Ok(retired)
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
