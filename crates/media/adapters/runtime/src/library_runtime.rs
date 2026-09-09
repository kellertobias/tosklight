pub(super) struct RuntimeUpload {
    pub(super) upload: Option<media_library::Upload>,
    pub(super) importer: media_library::Importer,
    pub(super) address: media_domain::MediaAddress,
    pub(super) name: String,
}

pub(super) fn edit_library(
    storage: &media_library::LibraryStorage,
    catalog: &crate::presentation::SharedCatalog,
    edit_lock: &crate::catalog_publication::CatalogEdits,
    operation: media_http::LibraryEdit,
) -> Result<(), String> {
    let _guard = edit_lock
        .lock()
        .map_err(|_| "the library edit lock is unavailable".to_owned())?;
    let mut next = (*catalog.load_full()).clone();
    let affected = crate::catalog_publication::edited_addresses(&next, &operation);
    let _idle = media_library::uploads::guard_idle_addresses(storage.root(), &affected)
        .map_err(|error| error.to_string())?;
    match operation {
        media_http::LibraryEdit::RenameItem { id, name } => {
            storage.rename_item(&mut next, id, &name)
        }
        media_http::LibraryEdit::MoveItem {
            id,
            destination,
            swap,
        } => {
            let occupant = next
                .folder(destination.folder)
                .and_then(|folder| folder.item(destination.file))
                .map(|item| item.id);
            match (occupant, swap) {
                (Some(other), true) => storage.swap_items(&mut next, id, other),
                _ => storage.move_item(&mut next, id, destination),
            }
        }
        media_http::LibraryEdit::SetItemBpm { id, bpm } => {
            storage.set_intrinsic_bpm(&mut next, id, bpm)
        }
        media_http::LibraryEdit::SetItemEnabled { id, enabled } => {
            storage.set_item_enabled(&mut next, id, enabled)
        }
        media_http::LibraryEdit::SetItemsEnabled { ids, enabled } => {
            storage.set_items_enabled(&mut next, &ids, enabled)
        }
        media_http::LibraryEdit::DeleteItem { id } => storage.remove_item(&mut next, id),
        media_http::LibraryEdit::DeleteItems { ids } => storage.remove_items(&mut next, &ids),
        media_http::LibraryEdit::RenameFolder { folder, name } => {
            storage.rename_folder(&mut next, folder, name.as_deref())
        }
        media_http::LibraryEdit::SetFolderIcon { folder, icon } => {
            storage.set_folder_icon(&mut next, folder, icon.as_deref())
        }
        media_http::LibraryEdit::SetNotes { targets, note } => {
            let targets = targets
                .into_iter()
                .map(|target| match target {
                    media_http::LibraryNoteTarget::Item(id) => {
                        media_library::LibraryNoteTarget::Item(id)
                    }
                    media_http::LibraryNoteTarget::Folder(folder) => {
                        media_library::LibraryNoteTarget::Folder(folder)
                    }
                })
                .collect::<Vec<_>>();
            storage.set_notes(&mut next, &targets, note.as_deref())
        }
        media_http::LibraryEdit::SwapFolders { first, second } => {
            storage.swap_folders(&mut next, first, second)
        }
        media_http::LibraryEdit::CompactFolder { folder } => {
            storage.compact_folder(&mut next, folder)
        }
    }
    .map_err(|error| error.to_string())?;
    catalog.store(std::sync::Arc::new(next));
    Ok(())
}

pub(super) fn update_folder_presentation(
    storage: &media_library::LibraryStorage,
    catalog: &crate::presentation::SharedCatalog,
    edit_lock: &crate::catalog_publication::CatalogEdits,
    folder: u16,
    name: Option<Option<String>>,
    icon: Option<Option<String>>,
) -> Result<media_http::FolderPresentation, String> {
    let _guard = edit_lock
        .lock()
        .map_err(|_| "the folder presentation edit lock is unavailable".to_owned())?;
    if media_domain::catalog::is_storage_folder(folder) {
        let mut next = (*catalog.load_full()).clone();
        match (name, icon) {
            (Some(name), None) => storage.rename_folder(&mut next, folder, name.as_deref()),
            (None, Some(icon)) => storage.set_folder_icon(&mut next, folder, icon.as_deref()),
            _ => unreachable!("the HTTP route validates one presentation intent"),
        }
        .map_err(|error| error.to_string())?;
        catalog.store(std::sync::Arc::new(next));
    } else {
        storage
            .update_generated_folder_presentation(
                folder,
                name.as_ref().map(|value| value.as_deref()),
                icon.as_ref().map(|value| value.as_deref()),
            )
            .map_err(|error| error.to_string())?;
    }
    storage
        .folder_presentation(folder)
        .map(super::folder_presentation_of)
        .map_err(|error| error.to_string())
}

impl media_http::UploadStream for RuntimeUpload {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.upload
            .as_mut()
            .ok_or_else(|| "the upload is already complete".to_owned())?
            .write(bytes)
            .map_err(|error| error.to_string())
    }

    fn finish(mut self: Box<Self>) -> Result<String, String> {
        self.upload
            .take()
            .ok_or_else(|| "the upload is already complete".to_owned())?
            .finish_and_import(&self.importer, self.address, &self.name)
            .map(|id| id.to_string())
            .map_err(|error| error.to_string())
    }
}

/// What the API can ask and tell the import pool.
pub(super) fn imports_of(
    importer: &media_library::Importer,
    library_root: &std::path::Path,
) -> media_http::Imports {
    let reading = importer.clone();
    let starting = importer.clone();
    let cancelling = importer.clone();
    let root = library_root.to_path_buf();
    let start_root = root.clone();

    media_http::Imports {
        state: std::sync::Arc::new(move || {
            let pending = media_library::pending_imports(&root)
                .into_iter()
                .map(|item| media_http::PendingImport {
                    destination: item.destination,
                    name: item.name,
                    filename: filename_of(&item.source),
                })
                .collect();
            let jobs = reading.jobs().iter().map(job_of).collect();
            (pending, jobs)
        }),
        start: std::sync::Arc::new(move |address| {
            media_library::pending_imports(&start_root)
                .into_iter()
                .filter(|item| address.is_none_or(|wanted| item.destination == wanted))
                .map(|item| {
                    starting.submit(item.source, item.destination, &item.name);
                })
                .count()
        }),
        cancel: std::sync::Arc::new(move |id| {
            cancelling
                .jobs()
                .iter()
                .find(|job| job.id.to_string() == id)
                .is_some_and(|job| cancelling.cancel(job.id))
        }),
        // Import shells out to FFmpeg. A machine without it should say so before an operator
        // queues a whole library that will fail one clip at a time.
        available: media_codec::import::ffmpeg_available(),
    }
}

fn filename_of(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned()
}

fn job_of(job: &media_library::Job) -> media_http::ImportJob {
    use media_library::JobState;
    let (outcome, frames_done, frames_total) = match &job.state {
        JobState::Queued => (media_http::ImportOutcome::Queued, None, None),
        JobState::Running {
            frames_done,
            frames_total,
        } => (
            media_http::ImportOutcome::Running,
            Some(*frames_done),
            *frames_total,
        ),
        JobState::Succeeded { frames } => {
            (media_http::ImportOutcome::Succeeded, Some(*frames), None)
        }
        JobState::Failed { reason } => (
            media_http::ImportOutcome::Failed {
                reason: reason.clone(),
            },
            None,
            None,
        ),
        JobState::Cancelled => (media_http::ImportOutcome::Cancelled, None, None),
    };
    media_http::ImportJob {
        id: job.id.to_string(),
        batch_id: job.batch.map(|batch| batch.to_string()),
        destination: job.destination,
        filename: filename_of(&job.source),
        outcome,
        attempts: job.attempts,
        fraction: job.state.fraction(),
        frames_done,
        frames_total,
    }
}
