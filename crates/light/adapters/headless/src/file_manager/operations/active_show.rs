//! The active show's file is the show the desk is running.
//!
//! The shows folder is an ordinary file root, so without this guard the file manager could
//! replace, move, rename or delete the running show's file underneath the desk: the desk would keep
//! running content no file holds any more, and the next commit would write into a different file
//! or fail. Those changes belong to the show library — open another show first, or use Save As,
//! Rename or Delete there — so the file manager refuses them with that advice.

use std::path::{Path, PathBuf};

use super::super::super::{ApiError, AppState};
use super::super::paths::confined;
use super::{FileOperation, FileOperationKind, OperationContext};

/// Refuses an operation that would change the active show's file, or a folder holding it.
pub(super) fn reject_active_show_changes(
    state: &AppState,
    context: &OperationContext,
    input: &FileOperation,
) -> Result<(), ApiError> {
    let Some(entry) = state.active_show.current() else {
        return Ok(());
    };
    let Ok(active) = std::fs::canonicalize(&entry.path) else {
        return Ok(());
    };
    let protected = protected_paths(&active);
    let touches = |path: &Path| protected.iter().any(|file| file.starts_with(path));
    let sources = || {
        input
            .sources
            .iter()
            .filter_map(|source| confined(&context.source_root.path, source, false).ok())
    };
    let refused = match input.operation {
        FileOperationKind::Delete | FileOperationKind::Trash => {
            sources().any(|path| touches(&path))
        }
        FileOperationKind::Rename => sources().any(|path| {
            touches(&path)
                || input
                    .name
                    .as_deref()
                    .zip(path.parent())
                    .is_some_and(|(name, parent)| protected.contains(&parent.join(name)))
        }),
        FileOperationKind::Move | FileOperationKind::Copy => {
            let moving = matches!(input.operation, FileOperationKind::Move);
            let destination = input
                .destination
                .as_deref()
                .map(|value| confined(&context.destination_root.path, value, false))
                .transpose()
                .ok()
                .flatten()
                .unwrap_or_else(|| context.canonical_destination_root.clone());
            sources().any(|path| {
                (moving && touches(&path))
                    || path
                        .file_name()
                        .is_some_and(|name| protected.contains(&destination.join(name)))
            })
        }
        FileOperationKind::CreateFile | FileOperationKind::CreateFolder => false,
    };
    if refused {
        let name = active
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Err(ApiError::conflict(format!(
            "{name} is the show this desk is running, so it cannot be replaced, moved, renamed or \
             deleted here. Open another show first, or use the show library to rename, save a copy \
             of or delete it."
        )));
    }
    Ok(())
}

/// The show file and the SQLite companions written beside it.
fn protected_paths(active: &Path) -> Vec<PathBuf> {
    let mut paths = vec![active.to_path_buf()];
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut name = active.as_os_str().to_owned();
        name.push(suffix);
        paths.push(PathBuf::from(name));
    }
    paths
}
