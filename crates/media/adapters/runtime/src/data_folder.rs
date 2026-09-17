//! The operator-chosen configuration and media folder.
//!
//! The configuration file this process starts from is its *base* location: `MEDIA_CONFIG`, or the
//! platform default. Choosing another folder in Settings leaves a small location record beside the
//! base configuration, and every later start reads the chosen folder's `media-server.json`
//! instead. The chosen folder is also the media-library root of a configuration created there.
//!
//! A folder is validated before anything is written: it must exist, be readable and writable, and
//! any configuration already in it must load. A refused folder leaves every file untouched, so the
//! running configuration stays exactly as it was. The server then restarts itself, because the
//! library, importer, model store, and listeners are all bound to the folder at startup.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use media_application::{MediaConfiguration, configuration};
use media_http::{DataFolders, FolderChange, FolderEntry, FolderListing, FolderRefusal};
use serde::{Deserialize, Serialize};

use crate::shutdown::{Shutdown, ShutdownReason};
use crate::startup;

/// The configuration file inside a data folder.
pub const CONFIGURATION_FILE: &str = "media-server.json";

/// The record, beside the base configuration, that names the chosen folder.
pub const LOCATION_FILE: &str = "media-server-location.json";

const LOCATION_VERSION: u32 = 1;

static RESTART_REQUESTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocationRecord {
    version: u32,
    directory: PathBuf,
}

fn location_path(base_configuration: &Path) -> PathBuf {
    base_configuration
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(LOCATION_FILE)
}

/// The configuration file a chosen folder redirects the base location to, if any.
///
/// A record that cannot be read, or that names a folder which is gone, is ignored with a warning
/// and the base location is used: a missing show disk must not stop the server from starting.
pub fn redirected_configuration(base_configuration: &Path) -> Option<PathBuf> {
    let record_path = location_path(base_configuration);
    let serialized = match std::fs::read_to_string(&record_path) {
        Ok(serialized) => serialized,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!(path = %record_path.display(), %error, "the chosen data folder record could not be read; using the default folder");
            return None;
        }
    };
    let record = match serde_json::from_str::<LocationRecord>(&serialized) {
        Ok(record) if record.version == LOCATION_VERSION && record.directory.is_absolute() => {
            record
        }
        Ok(_) | Err(_) => {
            tracing::warn!(path = %record_path.display(), "the chosen data folder record is not usable; using the default folder");
            return None;
        }
    };
    if !record.directory.is_dir() {
        tracing::warn!(directory = %record.directory.display(), "the chosen data folder is not available; using the default folder");
        return None;
    }
    Some(record.directory.join(CONFIGURATION_FILE))
}

/// Records the chosen folder, or removes the record when the base folder is chosen again.
fn record_location(base_configuration: &Path, directory: &Path) -> std::io::Result<()> {
    let record_path = location_path(base_configuration);
    let base_directory = startup::resolved_path(base_configuration)
        .parent()
        .map(Path::to_path_buf);
    let is_base = base_configuration.file_name() == Some(CONFIGURATION_FILE.as_ref())
        && base_directory.as_deref() == Some(directory);
    if is_base {
        return match std::fs::remove_file(&record_path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    if let Some(parent) = record_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let record = LocationRecord {
        version: LOCATION_VERSION,
        directory: directory.to_path_buf(),
    };
    let serialized = serde_json::to_string_pretty(&record).map_err(std::io::Error::other)?;
    let temporary = record_path.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&temporary, serialized)?;
    std::fs::rename(&temporary, &record_path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temporary);
    })
}

fn has_configuration(directory: &Path) -> bool {
    directory.join(CONFIGURATION_FILE).is_file()
}

/// Lists the visible subfolders of a folder, starting in `fallback` when none is named.
pub fn browse(directory: Option<&Path>, fallback: &Path) -> Result<FolderListing, FolderRefusal> {
    let directory = directory.unwrap_or(fallback).to_path_buf();
    if !directory.is_absolute() {
        return Err(FolderRefusal::Invalid(format!(
            "{} is not a full folder path.",
            directory.display()
        )));
    }
    if !directory.is_dir() {
        return Err(FolderRefusal::Invalid(format!(
            "{} is not a folder on the Media Server computer.",
            directory.display()
        )));
    }
    let entries = std::fs::read_dir(&directory).map_err(|error| {
        FolderRefusal::Unreadable(format!("{} cannot be read: {error}", directory.display()))
    })?;
    let mut folders: Vec<FolderEntry> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_owned();
            let path = entry.path();
            (!name.starts_with('.') && path.is_dir()).then(|| FolderEntry {
                has_configuration: has_configuration(&path),
                name,
                directory: path,
            })
        })
        .collect();
    folders.sort_by_key(|folder| folder.name.to_lowercase());
    Ok(FolderListing {
        parent: directory.parent().map(Path::to_path_buf),
        has_configuration: has_configuration(&directory),
        folders,
        directory,
    })
}

/// Validates a folder and makes it the configuration and media-library root of the next start.
///
/// A folder with a configuration keeps that configuration, which is what the restart loads. Any
/// other folder receives a copy of the current configuration whose library is the folder itself.
pub fn change(
    base_configuration: &Path,
    directory: &Path,
    current: &MediaConfiguration,
) -> Result<FolderChange, FolderRefusal> {
    let directory = accessible_directory(directory)?;
    let configuration_path = directory.join(CONFIGURATION_FILE);
    let loaded_existing = configuration_path.exists();
    if loaded_existing {
        check_existing_configuration(&configuration_path)?;
    } else {
        let mut adopted = current.clone();
        adopted.library.root = directory.clone();
        startup::write_configuration(&configuration_path, &adopted)
            .map_err(|error| FolderRefusal::Unwritable(error.to_string()))?;
    }
    record_location(base_configuration, &directory).map_err(|error| {
        // Leave a folder that is not used as it was found.
        if !loaded_existing {
            let _ = std::fs::remove_file(&configuration_path);
        }
        FolderRefusal::Unwritable(format!(
            "the chosen folder could not be remembered beside {}: {error}",
            base_configuration.display()
        ))
    })?;
    Ok(FolderChange {
        directory,
        loaded_existing,
    })
}

fn accessible_directory(directory: &Path) -> Result<PathBuf, FolderRefusal> {
    if !directory.is_absolute() {
        return Err(FolderRefusal::Invalid(format!(
            "{} is not a full folder path.",
            directory.display()
        )));
    }
    let metadata = std::fs::metadata(directory).map_err(|error| {
        FolderRefusal::Invalid(format!("{} is not available: {error}", directory.display()))
    })?;
    if !metadata.is_dir() {
        return Err(FolderRefusal::Invalid(format!(
            "{} is a file, not a folder.",
            directory.display()
        )));
    }
    std::fs::read_dir(directory).map_err(|error| {
        FolderRefusal::Unreadable(format!("{} cannot be read: {error}", directory.display()))
    })?;
    let probe = directory.join(format!(".tosklight-write-check-{}", std::process::id()));
    std::fs::write(&probe, b"").map_err(|error| {
        FolderRefusal::Unwritable(format!(
            "{} cannot be written: {error}",
            directory.display()
        ))
    })?;
    let _ = std::fs::remove_file(&probe);
    Ok(std::fs::canonicalize(directory).unwrap_or_else(|_| directory.to_path_buf()))
}

/// Refuses a configuration the next start could not run, before anything points at it.
fn check_existing_configuration(path: &Path) -> Result<(), FolderRefusal> {
    if !path.is_file() {
        return Err(FolderRefusal::InvalidConfiguration(format!(
            "{} is not a configuration file.",
            path.display()
        )));
    }
    let serialized = std::fs::read_to_string(path).map_err(|error| {
        FolderRefusal::Unreadable(format!("{} cannot be read: {error}", path.display()))
    })?;
    let mut existing = configuration::load(&serialized).map_err(|error| {
        FolderRefusal::InvalidConfiguration(format!(
            "{} is not a usable Media Server configuration: {error}",
            path.display()
        ))
    })?;
    startup::resolve_portable_library_root(&mut existing, path);
    startup::recover_moved_macos_library(&mut existing, path);
    let library = &existing.library.root;
    if library.exists() && !library.is_dir() {
        return Err(FolderRefusal::InvalidConfiguration(format!(
            "its media library {} is not a folder.",
            library.display()
        )));
    }
    if library.is_dir() {
        std::fs::read_dir(library).map_err(|error| {
            FolderRefusal::Unreadable(format!(
                "its media library {} cannot be read: {error}",
                library.display()
            ))
        })?;
    }
    Ok(())
}

/// The capabilities the API is handed for this process.
pub fn access(configuration: &MediaConfiguration, shutdown: Shutdown) -> DataFolders {
    let base = startup::base_configuration_path();
    let fallback = startup::current_portable_data_directory(configuration)
        .or_else(|| {
            startup::resolved_path(&startup::ConfigurationSource::from_environment().path())
                .parent()
                .map(Path::to_path_buf)
        })
        .unwrap_or_else(|| PathBuf::from("/"));
    DataFolders {
        browse: Arc::new(move |directory| browse(directory, &fallback)),
        change: Arc::new(move |directory, current| change(&base, directory, current)),
        restart: Arc::new(move || {
            RESTART_REQUESTED.store(true, Ordering::SeqCst);
            let shutdown = shutdown.clone();
            // Late enough for the answer to reach the browser before the listener closes.
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(400));
                shutdown.request(ShutdownReason::Requested);
            });
        }),
    }
}

/// Starts this executable again once everything this process held has been released.
pub fn relaunch_if_requested() {
    if !RESTART_REQUESTED.load(Ordering::SeqCst) {
        return;
    }
    let spawned = std::env::current_exe().and_then(|executable| {
        std::process::Command::new(executable)
            .args(std::env::args_os().skip(1))
            .stdin(std::process::Stdio::null())
            .spawn()
    });
    match spawned {
        Ok(child) => tracing::info!(pid = child.id(), "restarted to load the chosen data folder"),
        Err(error) => {
            tracing::error!(%error, "the Media Server could not restart itself; start it again")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("media-data-folder-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::canonicalize(root).unwrap()
    }

    fn current() -> MediaConfiguration {
        let mut configuration = MediaConfiguration::default();
        configuration.time.utc_offset_minutes = 120;
        configuration
    }

    #[test]
    fn an_empty_folder_becomes_the_configuration_and_library_root() {
        let root = scratch("empty");
        let base = root.join("home/media-server.json");
        let chosen = root.join("show");
        std::fs::create_dir_all(&chosen).unwrap();

        let changed = change(&base, &chosen, &current()).unwrap();

        assert!(!changed.loaded_existing);
        assert_eq!(changed.directory, chosen);
        assert_eq!(
            redirected_configuration(&base),
            Some(chosen.join(CONFIGURATION_FILE))
        );
        let source = startup::ConfigurationSource::File {
            path: chosen.join(CONFIGURATION_FILE),
            required: true,
        };
        let mut loaded = startup::load_configuration(&source).unwrap();
        assert_eq!(loaded.time.utc_offset_minutes, 120, "settings carried over");
        startup::resolve_portable_library_root(&mut loaded, &source.path());
        assert_eq!(loaded.library.root, chosen);
    }

    #[test]
    fn a_folder_with_a_configuration_keeps_and_loads_it() {
        let root = scratch("existing");
        let base = root.join("home/media-server.json");
        let chosen = root.join("show");
        std::fs::create_dir_all(chosen.join("Media")).unwrap();
        let mut theirs = MediaConfiguration::default();
        theirs.time.utc_offset_minutes = -300;
        theirs.library.root = chosen.join("Media");
        startup::write_configuration(&chosen.join(CONFIGURATION_FILE), &theirs).unwrap();
        let before = std::fs::read_to_string(chosen.join(CONFIGURATION_FILE)).unwrap();

        let changed = change(&base, &chosen, &current()).unwrap();

        assert!(changed.loaded_existing);
        assert_eq!(
            std::fs::read_to_string(chosen.join(CONFIGURATION_FILE)).unwrap(),
            before,
            "an existing configuration is loaded, never overwritten"
        );
        let path = redirected_configuration(&base).unwrap();
        let loaded = startup::load_configuration(&startup::ConfigurationSource::File {
            path,
            required: true,
        })
        .unwrap();
        assert_eq!(loaded.time.utc_offset_minutes, -300);
    }

    #[test]
    fn unusable_folders_are_refused_and_nothing_is_written() {
        let root = scratch("refused");
        let base = root.join("home/media-server.json");
        let broken = root.join("broken");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join(CONFIGURATION_FILE), "{ not json").unwrap();
        let file = root.join("a-file");
        std::fs::write(&file, "").unwrap();
        let odd = root.join("odd");
        std::fs::create_dir_all(odd.join(CONFIGURATION_FILE)).unwrap();

        let refusals = [
            change(&base, &broken, &current()).unwrap_err(),
            change(&base, &root.join("missing"), &current()).unwrap_err(),
            change(&base, &file, &current()).unwrap_err(),
            change(&base, Path::new("relative/show"), &current()).unwrap_err(),
            change(&base, &odd, &current()).unwrap_err(),
        ];

        assert!(matches!(
            refusals[0],
            FolderRefusal::InvalidConfiguration(_)
        ));
        assert!(matches!(refusals[1], FolderRefusal::Invalid(_)));
        assert!(matches!(refusals[2], FolderRefusal::Invalid(_)));
        assert!(matches!(refusals[3], FolderRefusal::Invalid(_)));
        assert!(matches!(
            refusals[4],
            FolderRefusal::InvalidConfiguration(_)
        ));
        assert!(!location_path(&base).exists(), "no location was recorded");
        assert_eq!(
            std::fs::read_to_string(broken.join(CONFIGURATION_FILE)).unwrap(),
            "{ not json"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_folder_is_refused() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = scratch("read-only");
        let base = root.join("home/media-server.json");
        let locked = root.join("locked");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
        let refused = change(&base, &locked, &current());
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Root may write anywhere; the refusal is only meaningful for an ordinary user.
        if let Err(refusal) = refused {
            assert!(matches!(refusal, FolderRefusal::Unwritable(_)));
            assert!(!location_path(&base).exists());
        }
    }

    #[test]
    fn a_stale_or_damaged_record_falls_back_to_the_base_folder() {
        let root = scratch("stale");
        let base = root.join("media-server.json");
        std::fs::write(location_path(&base), "{ damaged").unwrap();
        assert_eq!(redirected_configuration(&base), None);
        std::fs::write(
            location_path(&base),
            r#"{"version":1,"directory":"/no/such/tosklight/folder"}"#,
        )
        .unwrap();
        assert_eq!(redirected_configuration(&base), None);
    }

    #[test]
    fn choosing_the_base_folder_again_removes_the_record() {
        let root = scratch("return");
        let base = root.join("media-server.json");
        let chosen = root.join("show");
        std::fs::create_dir_all(&chosen).unwrap();
        change(&base, &chosen, &current()).unwrap();
        assert!(location_path(&base).exists());

        change(&base, &root, &current()).unwrap();

        assert!(!location_path(&base).exists());
        assert_eq!(redirected_configuration(&base), None);
    }

    #[test]
    fn the_picker_lists_visible_subfolders_and_marks_configured_ones() {
        let root = scratch("browse");
        std::fs::create_dir_all(root.join("b-show")).unwrap();
        std::fs::write(root.join("b-show").join(CONFIGURATION_FILE), "{}").unwrap();
        std::fs::create_dir_all(root.join("A-media")).unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();
        std::fs::write(root.join("file.txt"), "").unwrap();

        let listing = browse(None, &root).unwrap();

        assert_eq!(listing.directory, root);
        let names: Vec<_> = listing.folders.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["A-media", "b-show"]);
        assert!(listing.folders[1].has_configuration);
        assert!(matches!(
            browse(Some(&root.join("file.txt")), &root),
            Err(FolderRefusal::Invalid(_))
        ));
    }
}
