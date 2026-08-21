mod attribute_mapping;
mod gel_catalog;
mod migration;
mod package_io;
mod profiles;

pub use attribute_mapping::*;
pub use gel_catalog::*;

use crate::FixtureError;
use rusqlite::Connection;
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
             CREATE TABLE IF NOT EXISTS fixture_package_installations(package_path TEXT PRIMARY KEY,package_digest TEXT NOT NULL,profile_id TEXT NOT NULL,installed_revision INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS fixture_attribute_mapping_preferences(source_format TEXT NOT NULL,source_attribute TEXT NOT NULL,target_attribute TEXT NOT NULL,PRIMARY KEY(source_format,source_attribute));
             CREATE TABLE IF NOT EXISTS gel_catalogs(id TEXT PRIMARY KEY,name TEXT NOT NULL,revision INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS gel_catalog_entries(catalog_id TEXT NOT NULL,entry_id TEXT NOT NULL,number TEXT NOT NULL,name TEXT NOT NULL,display_srgb TEXT NOT NULL,visualizer_srgb TEXT NOT NULL,sort_order INTEGER NOT NULL,PRIMARY KEY(catalog_id,entry_id),UNIQUE(catalog_id,number),FOREIGN KEY(catalog_id) REFERENCES gel_catalogs(id) ON DELETE CASCADE);",
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
        library.migrate_legacy_profiles()?;
        library.seed_generic_gel_catalog()?;
        Ok(library)
    }
}
