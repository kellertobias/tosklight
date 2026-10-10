//! Canonicalize a supported portable document before enabling planning edits.
use crate::DocumentError;
use light_core::{ShowId, programming::SUPPORTED_PROGRAMMING_CONTRACT};
use light_show::{ShowStore, StoreError};
use std::path::Path;

pub(crate) fn open_canonical(path: &Path) -> Result<ShowId, DocumentError> {
    if !path.is_file() {
        return Err(DocumentError::Store(format!(
            "Show file does not exist: {}",
            path.display()
        )));
    }
    // Match the desk's read-only pre-open compatibility gate: invalid programming must not
    // cause even a SQLite schema upgrade. Other read errors use the ordinary store recovery.
    if let Err(StoreError::Invalid(message)) =
        light_show::validate_show_programming_contract(path, SUPPORTED_PROGRAMMING_CONTRACT)
    {
        return Err(DocumentError::Store(message));
    }
    let store = ShowStore::open(path)?;
    let document = store.portable_document()?;
    let prepared = light_application::prepare_show_candidate(&document, document.transaction())?;
    let (transaction, snapshot) = prepared.into_parts();
    light_fixture::validate_patch_for_planning(&snapshot.fixtures)
        .map_err(|error| DocumentError::Fixture(error.to_string()))?;
    transaction
        .check_programming_contract(SUPPORTED_PROGRAMMING_CONTRACT)
        .map_err(|error| DocumentError::Action(error.message))?;
    commit_migration(&store, path, transaction)?;
    Ok(document.id())
}

fn commit_migration(
    store: &ShowStore,
    path: &Path,
    transaction: light_show::PortableShowTransaction,
) -> Result<(), DocumentError> {
    if !transaction.is_empty() {
        // A fresh recovery file never replaces an older backup or the operator's source.
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".pre-canonical-{}.show", uuid::Uuid::new_v4()));
        let backup = path.with_file_name(name);
        std::fs::OpenOptions::new().write(true).create_new(true).open(&backup)
            .map_err(|error| DocumentError::Store(format!(
                "Cannot reserve migration backup {}: {error}. The portable show was not migrated.",
                backup.display()
            )))?;
        store.backup_to(&backup).map_err(|error| {
            DocumentError::Store(format!(
                "Cannot preserve the show before migration at {}: {error}. The portable show was not migrated.",
                backup.display()
            ))
        })?;
        store
            .apply_portable_transaction(transaction)
            .map_err(|error| {
                DocumentError::Store(format!(
                    "Cannot migrate the show: {error}. Original portable data is retained in {}.",
                    backup.display()
                ))
            })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_migration_commit_retains_backup_and_current_source() {
        let directory = std::path::PathBuf::from(std::env::var_os("LIGHT_TMP_DIR").unwrap())
            .join(format!("viz-migration-conflict-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("source.show");
        let (store, _) = ShowStore::create(&path, "Source").unwrap();
        store
            .put_object("preset", "2.1", &serde_json::json!({"family":"Color"}), 0)
            .unwrap();
        let source = store.portable_document().unwrap();
        let (migration, _) =
            light_application::prepare_show_candidate(&source, source.transaction())
                .unwrap()
                .into_parts();
        store
            .put_object(
                "future_vendor",
                "concurrent",
                &serde_json::json!({"preserve":true}),
                0,
            )
            .unwrap();
        let before = store.portable_document().unwrap();
        let error = commit_migration(&store, &path, migration)
            .unwrap_err()
            .to_string();
        let after = store.portable_document().unwrap();
        assert_eq!(after.revision(), before.revision());
        assert!(after.object("preset", "2.1").unwrap().body()["instance_id"].is_null());
        assert_eq!(
            after.object("future_vendor", "concurrent").unwrap().body(),
            before.object("future_vendor", "concurrent").unwrap().body()
        );
        let backups: Vec<_> = std::fs::read_dir(&directory)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains(".pre-canonical-")
            })
            .map(|entry| entry.path())
            .collect();
        assert_eq!(backups.len(), 1);
        assert!(error.contains(&backups[0].display().to_string()));
        assert_eq!(
            ShowStore::open(&backups[0])
                .unwrap()
                .portable_document()
                .unwrap()
                .revision(),
            before.revision()
        );
        drop(store);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
