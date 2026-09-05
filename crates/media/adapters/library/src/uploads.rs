//! Streaming a browser upload into the operator's library.
//!
//! The HTTP adapter owns multipart framing, not files. This adapter owns the destination, safe
//! naming, collision checks, the byte limit, and cleanup. Bytes first land in a hidden staging
//! file and become a pending import only after the request finishes, so discovery never offers a
//! half-uploaded clip.

use std::collections::HashSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use media_domain::MediaAddress;

use crate::naming;

/// A generous hard ceiling that still makes an accidentally unbounded upload impossible.
///
/// Uploads are streamed, not held in memory. Eight GiB covers long mezzanine sources while an
/// operator who genuinely needs more gets an actionable error and can copy the file locally.
pub const MAX_UPLOAD_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// Why an upload could not be staged.
#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("folder {folder} is outside the addressable media library 1..=199")]
    FolderOutOfRange { folder: u8 },
    #[error("file {file} is a blank sentinel; choose a file from 1 through 254")]
    FileOutOfRange { file: u8 },
    #[error("{filename} is not an importable media file")]
    Unsupported { filename: String },
    #[error("media address {address} already has a clip, source, or active upload/import")]
    AddressTaken { address: MediaAddress },
    #[error("the upload exceeds the {limit_bytes} byte limit")]
    TooLarge { limit_bytes: u64 },
    #[error("cannot {operation} {}: {source}", path.display())]
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// One in-progress upload. Dropping it removes its staging file.
pub struct Upload {
    file: Option<std::fs::File>,
    staging: PathBuf,
    destination: PathBuf,
    written: u64,
    committed: bool,
    _reservation: Reservation,
}

// Reserve the numeric address, not a display name: two browser clients may use different
// filenames for the same slot. In-memory leases are released on cancellation and process exit.
pub(crate) struct Reservation(PathBuf);
static UPLOADING: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

/// Excludes new upload/import reservations while a library edit checks and changes its addresses.
/// Keep this guard alive for the complete filesystem and catalog mutation.
pub struct IdleAddressesGuard {
    _active: std::sync::MutexGuard<'static, HashSet<PathBuf>>,
}

pub fn guard_idle_addresses(
    root: &Path,
    addresses: &[MediaAddress],
) -> Result<IdleAddressesGuard, UploadError> {
    let active = UPLOADING
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for address in addresses {
        // A folder which does not exist cannot have an active reservation.
        if let Ok(folder) = root
            .join(naming::folder_directory(address.folder))
            .canonicalize()
            && active.contains(&folder.join(format!("{:03}", address.file)))
        {
            return Err(UploadError::AddressTaken { address: *address });
        }
    }
    Ok(IdleAddressesGuard { _active: active })
}

impl Reservation {
    pub(crate) fn acquire(folder: &Path, address: MediaAddress) -> Result<Self, UploadError> {
        let folder = folder
            .canonicalize()
            .map_err(|source| UploadError::Filesystem {
                operation: "resolve upload folder",
                path: folder.to_owned(),
                source,
            })?;
        let key = folder.join(format!("{:03}", address.file));
        let mut active = UPLOADING
            .get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !active.insert(key.clone()) {
            return Err(UploadError::AddressTaken { address });
        }
        Ok(Self(key))
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        UPLOADING
            .get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.0);
    }
}

impl Upload {
    /// Begins a new upload at one explicit address.
    pub fn begin(
        root: &Path,
        address: MediaAddress,
        name: &str,
        original_filename: &str,
    ) -> Result<Self, UploadError> {
        Self::begin_with_policy(root, address, name, original_filename, false)
    }

    /// Stages an explicit replacement while leaving the currently playable clip in place until
    /// the new source has been uploaded completely and the importer can publish atomically.
    pub fn begin_replacement(
        root: &Path,
        address: MediaAddress,
        name: &str,
        original_filename: &str,
    ) -> Result<Self, UploadError> {
        Self::begin_with_policy(root, address, name, original_filename, true)
    }

    fn begin_with_policy(
        root: &Path,
        address: MediaAddress,
        name: &str,
        original_filename: &str,
        replace: bool,
    ) -> Result<Self, UploadError> {
        if !(1..=199).contains(&address.folder) {
            return Err(UploadError::FolderOutOfRange {
                folder: address.folder,
            });
        }
        if !(1..=254).contains(&address.file) {
            return Err(UploadError::FileOutOfRange { file: address.file });
        }
        let extension =
            importable_extension(original_filename).ok_or_else(|| UploadError::Unsupported {
                filename: original_filename.to_owned(),
            })?;
        let folder = root.join(naming::folder_directory(address.folder));
        std::fs::create_dir_all(&folder).map_err(|source| UploadError::Filesystem {
            operation: "create",
            path: folder.clone(),
            source,
        })?;

        let reservation = Reservation::acquire(&folder, address)?;
        if !replace && address_is_taken(&folder, address.file) {
            return Err(UploadError::AddressTaken { address });
        }

        let safe = naming::safe_name(name);
        let safe = safe.trim_matches(['-', ' ', '.']);
        let stem = if safe.is_empty() {
            format!("{:03}", address.file)
        } else {
            format!("{:03}-{safe}", address.file)
        };
        let destination = folder.join(format!("{stem}.{extension}"));
        let staging = folder.join(format!(".{stem}.{}.uploading", uuid::Uuid::new_v4()));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|source| UploadError::Filesystem {
                operation: "create",
                path: staging.clone(),
                source,
            })?;
        Ok(Self {
            file: Some(file),
            staging,
            destination,
            written: 0,
            committed: false,
            _reservation: reservation,
        })
    }

    /// Appends one multipart chunk without ever buffering the whole source.
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), UploadError> {
        let next = self.written.saturating_add(bytes.len() as u64);
        if next > MAX_UPLOAD_BYTES {
            return Err(UploadError::TooLarge {
                limit_bytes: MAX_UPLOAD_BYTES,
            });
        }
        self.file
            .as_mut()
            .expect("an unfinished upload has a file")
            .write_all(bytes)
            .map_err(|source| UploadError::Filesystem {
                operation: "write",
                path: self.staging.clone(),
                source,
            })?;
        self.written = next;
        Ok(())
    }

    /// Publishes the complete source and returns where the importer can read it.
    pub fn finish(mut self) -> Result<PathBuf, UploadError> {
        self.publish()?;
        Ok(self.destination.clone())
    }

    /// Publishes and queues conversion while keeping this address reserved until the job stops.
    /// There is no gap in which a replacement can overwrite a source a queued job will read.
    pub fn finish_and_import(
        mut self,
        importer: &crate::Importer,
        address: MediaAddress,
        name: &str,
    ) -> Result<crate::JobId, UploadError> {
        self.publish()?;
        let source = self.destination.clone();
        Ok(importer.submit_upload(source, address, name, self))
    }

    fn publish(&mut self) -> Result<(), UploadError> {
        let mut file = self.file.take().expect("an unfinished upload has a file");
        file.flush().map_err(|source| UploadError::Filesystem {
            operation: "flush",
            path: self.staging.clone(),
            source,
        })?;
        drop(file);
        // std::fs::rename replaces files on both Unix and Windows. Do not unlink first:
        // a failed publication must leave the previous source intact.
        std::fs::rename(&self.staging, &self.destination).map_err(|source| {
            UploadError::Filesystem {
                operation: "publish",
                path: self.staging.clone(),
                source,
            }
        })?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for Upload {
    fn drop(&mut self) {
        if !self.committed {
            // Close before unlinking so cancellation cleans up on Windows as well.
            drop(self.file.take());
            let _ = std::fs::remove_file(&self.staging);
        }
    }
}

fn importable_extension(filename: &str) -> Option<String> {
    const IMPORTABLE: [&str; 8] = ["mp4", "mov", "m4v", "mkv", "png", "jpg", "jpeg", "tif"];
    let extension = filename.rsplit_once('.')?.1.to_ascii_lowercase();
    IMPORTABLE
        .contains(&extension.as_str())
        .then_some(extension)
}

fn address_is_taken(folder: &Path, file: u8) -> bool {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .any(|filename| {
            naming::parse_item_filename(&filename).is_some_and(|(at, _)| at == file)
                || naming::parse_source_filename(&filename).is_some_and(|(at, _)| at == file)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join("media-library-upload").join(name);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn concurrent_uploads_reserve_the_address_even_with_different_names() {
        let root = root("concurrent");
        let address = MediaAddress::new(1, 1);
        let mut first = Upload::begin(&root, address, "First", "first.mov").unwrap();
        first.write(b"intact").unwrap();
        assert!(matches!(
            Upload::begin(&root, address, "Second", "second.mp4"),
            Err(UploadError::AddressTaken { .. })
        ));
        assert!(matches!(
            Upload::begin_replacement(&root, address, "First", "first.mov"),
            Err(UploadError::AddressTaken { .. })
        ));
        assert_eq!(std::fs::read(first.finish().unwrap()).unwrap(), b"intact");
    }

    #[test]
    fn cancelling_releases_the_address_for_a_retry() {
        let root = root("retry");
        let address = MediaAddress::new(1, 1);
        drop(Upload::begin(&root, address, "First", "first.mov").unwrap());
        assert!(Upload::begin(&root, address, "Retry", "retry.mov").is_ok());
    }

    #[test]
    fn a_complete_upload_becomes_a_pending_import_with_a_safe_name() {
        let root = root("complete");
        let mut upload = Upload::begin(
            &root,
            MediaAddress::new(3, 7),
            "../Opening: Look",
            "source.MP4",
        )
        .unwrap();
        upload.write(b"first").unwrap();
        upload.write(b"second").unwrap();
        let path = upload.finish().unwrap();

        assert_eq!(path.file_name().unwrap(), "007-Opening- Look.mp4");
        assert_eq!(std::fs::read(path).unwrap(), b"firstsecond");
        let pending = crate::pending_imports(&root);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].destination, MediaAddress::new(3, 7));
    }

    #[test]
    fn an_explicit_replacement_accepts_an_occupied_address_only_after_full_upload() {
        let root = root("replacement");
        let folder = root.join("001");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("001-Loop.toskclip"), b"old clip").unwrap();
        std::fs::write(folder.join("001-Loop.mov"), b"old source").unwrap();

        assert!(Upload::begin(&root, MediaAddress::new(1, 1), "Loop", "new.mov").is_err());
        let mut replacement =
            Upload::begin_replacement(&root, MediaAddress::new(1, 1), "Loop", "new.mov").unwrap();
        replacement.write(b"new source").unwrap();
        assert_eq!(
            std::fs::read(folder.join("001-Loop.mov")).unwrap(),
            b"old source"
        );
        let source = replacement.finish().unwrap();

        assert_eq!(std::fs::read(source).unwrap(), b"new source");
        assert_eq!(
            std::fs::read(folder.join("001-Loop.toskclip")).unwrap(),
            b"old clip",
            "the playable clip remains until import succeeds"
        );
    }

    #[test]
    fn dropping_an_upload_leaves_no_source_behind() {
        let root = root("dropped");
        {
            let mut upload =
                Upload::begin(&root, MediaAddress::new(1, 1), "Clip", "clip.mov").unwrap();
            upload.write(b"partial").unwrap();
        }
        assert!(crate::pending_imports(&root).is_empty());
        assert!(
            std::fs::read_dir(root.join("001"))
                .unwrap()
                .next()
                .is_none()
        );
    }

    #[test]
    fn an_existing_source_or_clip_is_never_overwritten() {
        let root = root("occupied");
        let folder = root.join("001");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("004-Original.mp4"), b"keep me").unwrap();
        let error = Upload::begin(&root, MediaAddress::new(1, 4), "New", "new.mp4")
            .err()
            .expect("occupied");
        assert!(matches!(error, UploadError::AddressTaken { .. }));
        assert_eq!(
            std::fs::read(folder.join("004-Original.mp4")).unwrap(),
            b"keep me"
        );
    }

    #[test]
    fn blank_sentinels_and_unknown_formats_are_refused() {
        assert!(matches!(
            Upload::begin(&root("blank"), MediaAddress::new(1, 0), "Blank", "clip.mp4")
                .err()
                .expect("blank"),
            UploadError::FileOutOfRange { .. }
        ));
        assert!(matches!(
            Upload::begin(
                &root("format"),
                MediaAddress::new(1, 1),
                "Notes",
                "notes.txt"
            )
            .err()
            .expect("format"),
            UploadError::Unsupported { .. }
        ));
    }
}
