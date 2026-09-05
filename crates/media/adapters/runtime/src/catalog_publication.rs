//! Import completion and operator edits publish one serial catalog history.

use std::sync::{Arc, Mutex};

use media_domain::{MediaAddress, catalog::CatalogSnapshot};

use crate::presentation::SharedCatalog;

pub(crate) type CatalogEdits = Arc<Mutex<()>>;

/// Slots whose files an edit may change. Include empty destinations: a conversion may be
/// preparing a new clip that is not visible in the catalog yet.
pub(crate) fn edited_addresses(
    catalog: &CatalogSnapshot,
    edit: &media_http::LibraryEdit,
) -> Vec<MediaAddress> {
    use media_http::LibraryEdit;
    let addressed = |folder: u16, file| {
        u8::try_from(folder)
            .ok()
            .filter(|folder| (1..=199).contains(folder))
            .map(|folder| MediaAddress::new(folder, file))
    };
    let source = |id| {
        catalog
            .location_of(id)
            .and_then(|location| addressed(location.folder, location.file))
    };
    match edit {
        LibraryEdit::RenameItem { id, .. } | LibraryEdit::SetItemBpm { id, .. } => {
            source(*id).into_iter().collect()
        }
        LibraryEdit::MoveItem {
            id, destination, ..
        } => source(*id)
            .into_iter()
            .chain(addressed(destination.folder, destination.file))
            .collect(),
        LibraryEdit::SwapFolders { first, second } => [*first, *second]
            .into_iter()
            .flat_map(|folder| (1..=254).filter_map(move |file| addressed(folder, file)))
            .collect(),
        LibraryEdit::RenameFolder { .. } | LibraryEdit::SetFolderIcon { .. } => Vec::new(),
    }
}

pub(crate) fn publish_import(
    catalog: &SharedCatalog,
    edits: &CatalogEdits,
    imported: MediaAddress,
    discover: impl FnOnce() -> Result<CatalogSnapshot, String>,
) -> Result<(), String> {
    // Acquire before scanning: a rename/move must not publish over a scan taken mid-edit.
    let _guard = edits
        .lock()
        .map_err(|_| "the catalog edit lock is unavailable".to_owned())?;
    let previous = catalog.load_full();
    let mut next = discover()?;
    for folder in &mut next.folders {
        for item in &mut folder.items {
            if folder.folder == u16::from(imported.folder) && item.file == imported.file {
                // The new ID invalidates a resident copy of a replaced clip even when all its
                // visible metadata happens to match the old one.
                continue;
            }
            if let Some(existing) = previous
                .folder(folder.folder)
                .and_then(|folder| folder.item(item.file))
            {
                // Discovery mints provisional IDs. Preserve identity only for the unchanged
                // item at this address; independently modified disk content stays a new asset.
                let discovered_id = item.id;
                item.id = existing.id;
                if item != existing {
                    item.id = discovered_id;
                }
            }
        }
    }
    next.revision = previous.revision.next();
    catalog.store(Arc::new(next));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::{
        AssetId,
        catalog::{CatalogItem, ItemKind},
    };

    fn item(file: u8, name: &str) -> CatalogItem {
        CatalogItem {
            id: AssetId::new(),
            file,
            name: name.to_owned(),
            kind: ItemKind::Image,
            width: 16,
            height: 16,
            frames: None,
            intrinsic_bpm: None,
        }
    }

    fn catalog(items: Vec<CatalogItem>) -> CatalogSnapshot {
        let mut catalog = CatalogSnapshot::default();
        for item in items {
            catalog.insert(1, item).unwrap();
        }
        catalog
    }

    #[test]
    fn unrelated_import_preserves_item_identity_and_advances_the_existing_revision() {
        let existing = item(1, "Opening");
        let mut before = catalog(vec![existing.clone()]);
        for n in 0..5 {
            before
                .rename_folder(1, Some(&format!("Looks {n}")))
                .unwrap();
        }
        let revision = before.revision;
        let published = Arc::new(arc_swap::ArcSwap::from_pointee(before));
        let edits = Arc::new(Mutex::new(()));
        publish_import(&published, &edits, MediaAddress::new(1, 2), || {
            Ok(catalog(vec![item(1, "Opening"), item(2, "New clip")]))
        })
        .unwrap();
        let after = published.load();
        assert_eq!(
            after.resolve(MediaAddress::new(1, 1)).unwrap().id,
            existing.id
        );
        assert_eq!(after.item_count(), 2);
        assert!(after.revision > revision);
    }

    #[test]
    fn replacement_changes_only_the_imported_assets_identity_even_with_identical_metadata() {
        let opening = item(1, "Opening");
        let closing = item(2, "Closing");
        let published = Arc::new(arc_swap::ArcSwap::from_pointee(catalog(vec![
            opening.clone(),
            closing.clone(),
        ])));
        publish_import(
            &published,
            &Arc::new(Mutex::new(())),
            MediaAddress::new(1, 1),
            || Ok(catalog(vec![item(1, "Opening"), item(2, "Closing")])),
        )
        .unwrap();
        let after = published.load();
        assert_ne!(
            after.resolve(MediaAddress::new(1, 1)).unwrap().id,
            opening.id
        );
        assert_eq!(
            after.resolve(MediaAddress::new(1, 2)).unwrap().id,
            closing.id
        );
    }

    #[test]
    fn publication_locks_before_scanning_and_keeps_a_following_operator_edit() {
        let existing = item(1, "Opening");
        let published = Arc::new(arc_swap::ArcSwap::from_pointee(catalog(vec![
            existing.clone(),
        ])));
        let edits = Arc::new(Mutex::new(()));
        let worker_catalog = published.clone();
        let worker_edits = edits.clone();
        let (started, scan_started) = std::sync::mpsc::channel();
        let (resume, scan_resume) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            publish_import(
                &worker_catalog,
                &worker_edits,
                MediaAddress::new(1, 2),
                || {
                    started.send(()).unwrap();
                    scan_resume
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                    Ok(catalog(vec![item(1, "Opening"), item(2, "New clip")]))
                },
            )
            .unwrap();
        });
        scan_started
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert!(
            edits.try_lock().is_err(),
            "the scan holds the same lock as operator edits"
        );
        resume.send(()).unwrap();
        let _guard = edits.lock().unwrap();
        let mut next = (*published.load_full()).clone();
        next.rename_item(existing.id, "Renamed").unwrap();
        published.store(Arc::new(next));
        worker.join().unwrap();
        assert_eq!(published.load().item_count(), 2);
        assert_eq!(
            published
                .load()
                .resolve(MediaAddress::new(1, 1))
                .unwrap()
                .name,
            "Renamed"
        );
    }

    #[test]
    fn import_guards_cover_empty_move_destinations_and_empty_folder_slots() {
        let existing = item(1, "Opening");
        let catalog = catalog(vec![existing.clone()]);
        let addresses = edited_addresses(
            &catalog,
            &media_http::LibraryEdit::MoveItem {
                id: existing.id,
                destination: media_domain::CatalogLocation::new(2, 8),
                swap: false,
            },
        );
        assert_eq!(
            addresses,
            vec![MediaAddress::new(1, 1), MediaAddress::new(2, 8)]
        );
        let folders = edited_addresses(
            &catalog,
            &media_http::LibraryEdit::SwapFolders {
                first: 1,
                second: 2,
            },
        );
        assert_eq!(folders.len(), 508);
        assert!(folders.contains(&MediaAddress::new(2, 254)));
    }
}
