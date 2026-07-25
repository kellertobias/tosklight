use super::FixtureLibrary;
use crate::{FixtureDefinition, FixtureError, FixtureProfile};
use light_core::FixtureId;
use rusqlite::{Connection, Transaction, params};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone)]
struct StoredLegacyRow {
    id: String,
    revision: u32,
    json: String,
    source: Option<Vec<u8>>,
}

#[derive(Clone)]
struct LegacyRow {
    stored: StoredLegacyRow,
    definition: FixtureDefinition,
}

type LegacyFamilies = BTreeMap<String, Vec<LegacyRow>>;

impl FixtureLibrary {
    pub(super) fn migrate_legacy_profiles(&self) -> Result<usize, FixtureError> {
        let valid_rows = self.read_valid_legacy_rows()?;
        if valid_rows.is_empty() {
            return Ok(0);
        }
        let (families, family_counts) = group_legacy_families(valid_rows)?;
        let transaction = self.conn.unchecked_transaction()?;
        let mut migrated = 0;
        for (family_key, rows) in families {
            let family = base_family_key(&family_key);
            if migrate_family(&transaction, &rows, family_counts[&family] > 1)? {
                migrated += 1;
            }
        }
        transaction.execute(
            "INSERT INTO library_metadata(key,value) VALUES('fixture_profile_schema','2') ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [],
        )?;
        transaction.commit()?;
        Ok(migrated)
    }

    fn read_valid_legacy_rows(&self) -> Result<Vec<LegacyRow>, FixtureError> {
        let mut valid_rows = Vec::new();
        for stored in read_pending_legacy_rows(&self.conn)? {
            match serde_json::from_str(&stored.json) {
                Ok(definition) => valid_rows.push(LegacyRow { stored, definition }),
                Err(error) => self.record_legacy_parse_failure(&stored, &error)?,
            }
        }
        Ok(valid_rows)
    }

    fn record_legacy_parse_failure(
        &self,
        row: &StoredLegacyRow,
        error: &serde_json::Error,
    ) -> Result<(), FixtureError> {
        let message = format!(
            "Legacy fixture {} revision {} could not be migrated: {error}. The original definition and GDTF source were retained.",
            row.id, row.revision
        );
        self.conn.execute(
            "INSERT OR REPLACE INTO fixture_profile_migration_failures(legacy_id,legacy_revision,error) VALUES(?1,?2,?3)",
            params![row.id, row.revision, error.to_string()],
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO fixture_library_warnings(message) VALUES(?1)",
            [message],
        )?;
        Ok(())
    }
}

fn read_pending_legacy_rows(conn: &Connection) -> Result<Vec<StoredLegacyRow>, FixtureError> {
    let mut statement = conn.prepare(
        "SELECT f.id,f.revision,f.definition_json,f.source_gdtf FROM fixture_definitions f JOIN (SELECT id,MAX(revision) revision FROM fixture_definitions GROUP BY id) latest ON latest.id=f.id AND latest.revision=f.revision LEFT JOIN fixture_profile_legacy_map m ON m.legacy_id=f.id AND m.legacy_revision=f.revision LEFT JOIN fixture_profile_migration_failures x ON x.legacy_id=f.id AND x.legacy_revision=f.revision WHERE m.legacy_id IS NULL AND x.legacy_id IS NULL ORDER BY f.manufacturer COLLATE NOCASE,f.model COLLATE NOCASE,f.mode COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(StoredLegacyRow {
            id: row.get(0)?,
            revision: row.get(1)?,
            json: row.get(2)?,
            source: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn group_legacy_families(
    rows: Vec<LegacyRow>,
) -> Result<(LegacyFamilies, HashMap<String, usize>), FixtureError> {
    let mut families = LegacyFamilies::new();
    for row in rows {
        let metadata = fixture_level_metadata(&row.definition)?;
        let family = definition_family_key(&row.definition);
        families
            .entry(format!("{family}\0{metadata}"))
            .or_default()
            .push(row);
    }
    let mut family_counts = HashMap::new();
    for key in families.keys() {
        *family_counts.entry(base_family_key(key)).or_default() += 1;
    }
    Ok((families, family_counts))
}

fn fixture_level_metadata(definition: &FixtureDefinition) -> Result<String, FixtureError> {
    Ok(serde_json::to_string(&serde_json::json!({
        "device_type": definition.device_type,
        "name": definition.name,
        "physical": definition.physical,
        "model_asset": definition.model_asset,
        "icon_asset": definition.icon_asset,
        "hazardous": definition.hazardous,
        "direct_control_protocols": definition.direct_control_protocols,
        "signal_loss_policy": definition.signal_loss_policy,
    }))?)
}

fn definition_family_key(definition: &FixtureDefinition) -> String {
    format!(
        "{}\0{}",
        definition.manufacturer.to_lowercase(),
        definition.model.to_lowercase()
    )
}

fn base_family_key(key: &str) -> String {
    key.split('\0').take(2).collect::<Vec<_>>().join("\0")
}

fn migrate_family(
    transaction: &Transaction<'_>,
    rows: &[LegacyRow],
    conflicting_metadata: bool,
) -> Result<bool, FixtureError> {
    let definitions = rows
        .iter()
        .map(|row| row.definition.clone())
        .collect::<Vec<_>>();
    let mut profile = match FixtureProfile::from_legacy_modes(&definitions) {
        Ok(profile) => profile,
        Err(error) => {
            record_family_failure(transaction, rows, &error)?;
            return Ok(false);
        }
    };
    assign_available_profile_id(transaction, &mut profile)?;
    insert_profile(transaction, &profile)?;
    insert_legacy_sources(transaction, rows, profile.id)?;
    if conflicting_metadata {
        record_metadata_conflict(transaction, rows)?;
    }
    Ok(true)
}

fn record_family_failure(
    transaction: &Transaction<'_>,
    rows: &[LegacyRow],
    error: &impl std::fmt::Display,
) -> Result<(), FixtureError> {
    let first = &rows[0].definition;
    let message = format!(
        "Legacy fixture family {} {} could not be migrated: {error}. Original rows and GDTF sources were retained.",
        first.manufacturer, first.model
    );
    transaction.execute(
        "INSERT OR IGNORE INTO fixture_library_warnings(message) VALUES(?1)",
        [message],
    )?;
    for row in rows {
        transaction.execute(
            "INSERT OR REPLACE INTO fixture_profile_migration_failures(legacy_id,legacy_revision,error) VALUES(?1,?2,?3)",
            params![row.stored.id, row.stored.revision, error.to_string()],
        )?;
    }
    Ok(())
}

fn assign_available_profile_id(
    transaction: &Transaction<'_>,
    profile: &mut FixtureProfile,
) -> Result<(), FixtureError> {
    while transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM fixture_profiles WHERE id=?1 AND revision=1)",
        [profile.id.0.to_string()],
        |row| row.get::<_, bool>(0),
    )? {
        profile.id = FixtureId::new();
    }
    Ok(())
}

fn insert_profile(
    transaction: &Transaction<'_>,
    profile: &FixtureProfile,
) -> Result<(), FixtureError> {
    let profile_json = serde_json::to_string(profile)?;
    transaction.execute(
        "INSERT INTO fixture_profiles(id,revision,manufacturer,name,profile_json,reserved_source) VALUES(?1,1,?2,?3,?4,NULL)",
        params![profile.id.0.to_string(), profile.manufacturer, profile.name, profile_json],
    )?;
    Ok(())
}

fn insert_legacy_sources(
    transaction: &Transaction<'_>,
    rows: &[LegacyRow],
    profile_id: FixtureId,
) -> Result<(), FixtureError> {
    for row in rows {
        transaction.execute(
            "INSERT INTO fixture_profile_legacy_sources(profile_id,profile_revision,legacy_id,legacy_revision,definition_json,source_gdtf) VALUES(?1,1,?2,?3,?4,?5)",
            params![profile_id.0.to_string(), row.stored.id, row.stored.revision, row.stored.json, row.stored.source],
        )?;
        transaction.execute(
            "INSERT INTO fixture_profile_legacy_map(legacy_id,legacy_revision,profile_id,profile_revision) VALUES(?1,?2,?3,1)",
            params![row.stored.id, row.stored.revision, profile_id.0.to_string()],
        )?;
    }
    Ok(())
}

fn record_metadata_conflict(
    transaction: &Transaction<'_>,
    rows: &[LegacyRow],
) -> Result<(), FixtureError> {
    let first = &rows[0].definition;
    transaction.execute(
        "INSERT OR IGNORE INTO fixture_library_warnings(message) VALUES(?1)",
        [format!(
            "{} {} contained conflicting fixture-level metadata; its legacy modes were retained as separate profiles",
            first.manufacturer, first.model
        )],
    )?;
    Ok(())
}
