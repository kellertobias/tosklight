mod migration;
mod package_io;
mod profiles;

use crate::FixtureError;
use rusqlite::{Connection, params};
use std::path::Path;

pub struct FixtureLibrary {
    conn: Connection,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FixturePackageLoadReport {
    pub installed: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub preserved_operator_revisions: usize,
}

pub type LegacyFixtureProfileSource = (String, String, Option<Vec<u8>>);

// Removed code-owned catalogs used these markers. They remain only for a one-time data migration
// that releases old catalog rows before loading the same fixtures from transferable packages.
const LEGACY_GENERIC_CATALOG_SOURCE: &str = "builtin:generic-catalog";
const LEGACY_VENDOR_CATALOG_SOURCE: &str = "builtin:vendor-catalog";

impl FixtureLibrary {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, FixtureError> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS fixture_definitions(id TEXT NOT NULL,revision INTEGER NOT NULL,manufacturer TEXT NOT NULL,model TEXT NOT NULL,mode TEXT NOT NULL,definition_json TEXT NOT NULL,source_gdtf BLOB,PRIMARY KEY(id,revision));
             CREATE TABLE IF NOT EXISTS fixture_profiles(id TEXT NOT NULL,revision INTEGER NOT NULL,manufacturer TEXT NOT NULL,name TEXT NOT NULL,profile_json TEXT NOT NULL,reserved_source TEXT,PRIMARY KEY(id,revision));
             CREATE TABLE IF NOT EXISTS fixture_profile_sources(profile_id TEXT NOT NULL,profile_revision INTEGER NOT NULL,source_gdtf BLOB NOT NULL,PRIMARY KEY(profile_id,profile_revision));
             CREATE TABLE IF NOT EXISTS fixture_profile_legacy_sources(profile_id TEXT NOT NULL,profile_revision INTEGER NOT NULL,legacy_id TEXT NOT NULL,legacy_revision INTEGER NOT NULL,definition_json TEXT NOT NULL,source_gdtf BLOB,PRIMARY KEY(profile_id,profile_revision,legacy_id,legacy_revision));
             CREATE TABLE IF NOT EXISTS fixture_profile_legacy_map(legacy_id TEXT NOT NULL,legacy_revision INTEGER NOT NULL,profile_id TEXT NOT NULL,profile_revision INTEGER NOT NULL,PRIMARY KEY(legacy_id,legacy_revision));
             CREATE TABLE IF NOT EXISTS fixture_profile_migration_failures(legacy_id TEXT NOT NULL,legacy_revision INTEGER NOT NULL,error TEXT NOT NULL,PRIMARY KEY(legacy_id,legacy_revision));
             CREATE TABLE IF NOT EXISTS fixture_library_warnings(id INTEGER PRIMARY KEY AUTOINCREMENT,message TEXT NOT NULL UNIQUE);
             CREATE TABLE IF NOT EXISTS library_metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS fixture_package_installations(package_path TEXT PRIMARY KEY,package_digest TEXT NOT NULL,profile_id TEXT NOT NULL,installed_revision INTEGER NOT NULL);",
        )?;
        if conn
            .prepare("SELECT source_gdtf FROM fixture_definitions LIMIT 0")
            .is_err()
        {
            conn.execute(
                "ALTER TABLE fixture_definitions ADD COLUMN source_gdtf BLOB",
                [],
            )?;
        }
        let library = Self { conn };
        library.remove_legacy_code_owned_catalogs()?;
        library.migrate_legacy_profiles()?;
        Ok(library)
    }

    fn remove_legacy_code_owned_catalogs(&self) -> Result<(), FixtureError> {
        let transaction = self.conn.unchecked_transaction()?;
        let owned_profile_ids = {
            let mut statement = transaction.prepare(
                "SELECT DISTINCT id FROM fixture_profiles WHERE reserved_source IN (?1,?2)",
            )?;
            statement
                .query_map(
                    params![LEGACY_GENERIC_CATALOG_SOURCE, LEGACY_VENDOR_CATALOG_SOURCE],
                    |row| row.get::<_, String>(0),
                )?
                .collect::<Result<Vec<_>, _>>()?
        };
        let legacy_definition_ids = {
            let mut statement = transaction.prepare(
                "SELECT DISTINCT m.legacy_id FROM fixture_profile_legacy_map m JOIN fixture_profiles p ON p.id=m.profile_id AND p.revision=m.profile_revision WHERE p.reserved_source IN (?1,?2)",
            )?;
            statement
                .query_map(
                    params![LEGACY_GENERIC_CATALOG_SOURCE, LEGACY_VENDOR_CATALOG_SOURCE],
                    |row| row.get::<_, String>(0),
                )?
                .collect::<Result<Vec<_>, _>>()?
        };
        for profile_id in owned_profile_ids {
            transaction.execute(
                "DELETE FROM fixture_profile_sources WHERE profile_id=?1",
                [&profile_id],
            )?;
            transaction.execute(
                "DELETE FROM fixture_profile_legacy_sources WHERE profile_id=?1",
                [&profile_id],
            )?;
            transaction.execute(
                "DELETE FROM fixture_profile_legacy_map WHERE profile_id=?1",
                [&profile_id],
            )?;
            transaction.execute("DELETE FROM fixture_profiles WHERE id=?1", [&profile_id])?;
        }
        for legacy_id in legacy_definition_ids {
            transaction.execute(
                "DELETE FROM fixture_profile_migration_failures WHERE legacy_id=?1",
                [&legacy_id],
            )?;
            transaction.execute("DELETE FROM fixture_definitions WHERE id=?1", [&legacy_id])?;
        }
        transaction.execute(
            "DELETE FROM library_metadata WHERE key IN ('generic_catalog_version','generic_catalog_profile_count','vendor_catalog_version','vendor_catalog_profile_count')",
            [],
        )?;
        transaction.commit()?;
        Ok(())
    }
}
