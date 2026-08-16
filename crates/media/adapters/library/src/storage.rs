//! Applying catalog edits to disk.
//!
//! Each operation validates against the catalog first, then touches the filesystem, then publishes
//! the new snapshot. If the filesystem step fails the catalog is not published, so what an
//! operator sees never claims something the disk does not hold.
//!
//! Renames go through a staging name where two files would otherwise collide mid-operation, so a
//! swap cannot lose a file even if the process dies between the two renames — the leftover
//! staging file is recoverable, whereas an overwritten clip is not.

use std::path::{Path, PathBuf};

use media_domain::catalog::{CatalogError, CatalogSnapshot};
use media_domain::{AssetId, CatalogLocation};

use crate::naming;

/// Presentation attached to any playable address-space folder, including generated text and
/// visualizer banks that contain no filesystem media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderPresentation {
    pub folder: u16,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub picture_content_type: Option<String>,
}

/// Why a library edit could not be applied.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("cannot {operation} {}: {source}", path.display())]
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("folder {folder} is outside the media address space 1-255")]
    InvalidPresentationFolder { folder: u16 },
    #[error("folder pictures must use an image MIME type")]
    InvalidPictureContentType,
}

/// The library's root on disk, and the operations that change it.
#[derive(Debug, Clone)]
pub struct LibraryStorage {
    root: PathBuf,
}

impl LibraryStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where an item at this address with this name lives.
    pub fn item_path(&self, address: impl Into<CatalogLocation>, name: &str) -> PathBuf {
        let address = address.into();
        self.root
            .join(naming::folder_directory(address.folder))
            .join(naming::item_filename(address.file, name))
    }

    /// Where an item's thumbnail lives.
    pub fn thumbnail_path(&self, address: impl Into<CatalogLocation>) -> PathBuf {
        let address = address.into();
        self.root
            .join(naming::folder_directory(address.folder))
            .join(naming::THUMBNAIL_DIRECTORY)
            .join(naming::thumbnail_filename(address.file))
    }

    pub fn folder_picture_path(&self, folder: u16) -> Result<PathBuf, StorageError> {
        validate_presentation_folder(folder)?;
        Ok(self
            .root
            .join(naming::folder_directory(folder))
            .join(naming::FOLDER_PICTURE_FILE))
    }

    /// Reads stored presentation without interpreting generated folders as physical media.
    pub fn folder_presentation(&self, folder: u16) -> Result<FolderPresentation, StorageError> {
        validate_presentation_folder(folder)?;
        let directory = self.root.join(naming::folder_directory(folder));
        let mut metadata = read_folder_metadata(&directory);
        if !directory.join(naming::FOLDER_PICTURE_FILE).is_file() {
            metadata.picture_content_type = None;
        }
        Ok(FolderPresentation {
            folder,
            name: metadata.name,
            icon: metadata.icon,
            picture_content_type: metadata.picture_content_type,
        })
    }

    /// Updates name and icon for generated folders. Library folders continue through the catalog
    /// mutation methods so their live snapshot changes at the same boundary.
    pub fn update_generated_folder_presentation(
        &self,
        folder: u16,
        name: Option<Option<&str>>,
        icon: Option<Option<&str>>,
    ) -> Result<FolderPresentation, StorageError> {
        validate_presentation_folder(folder)?;
        let mut metadata = read_folder_metadata(&self.root.join(naming::folder_directory(folder)));
        if let Some(name) = name {
            metadata.name = normalized(name);
        }
        if let Some(icon) = icon {
            metadata.icon = normalized(icon);
        }
        self.write_raw_folder_metadata(folder, &metadata)?;
        self.folder_presentation(folder)
    }

    pub fn write_folder_picture(
        &self,
        catalog: &mut CatalogSnapshot,
        folder: u16,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<(), StorageError> {
        validate_presentation_folder(folder)?;
        if !content_type.starts_with("image/") {
            return Err(StorageError::InvalidPictureContentType);
        }
        self.ensure_folder(folder)?;
        let path = self.folder_picture_path(folder)?;
        std::fs::write(&path, bytes).map_err(|source| StorageError::Filesystem {
            operation: "write",
            path,
            source,
        })?;
        if media_domain::catalog::is_storage_folder(folder) {
            catalog.set_folder_picture(folder, Some(content_type))?;
            self.write_folder_metadata(catalog, folder)?;
        } else {
            let mut metadata =
                read_folder_metadata(&self.root.join(naming::folder_directory(folder)));
            metadata.picture_content_type = Some(content_type.to_owned());
            self.write_raw_folder_metadata(folder, &metadata)?;
        }
        Ok(())
    }

    pub fn remove_folder_picture(
        &self,
        catalog: &mut CatalogSnapshot,
        folder: u16,
    ) -> Result<(), StorageError> {
        let path = self.folder_picture_path(folder)?;
        if path.exists() {
            std::fs::remove_file(&path).map_err(|source| StorageError::Filesystem {
                operation: "remove",
                path,
                source,
            })?;
        }
        if media_domain::catalog::is_storage_folder(folder) {
            catalog.set_folder_picture(folder, None)?;
            self.write_folder_metadata(catalog, folder)?;
        } else {
            let mut metadata =
                read_folder_metadata(&self.root.join(naming::folder_directory(folder)));
            metadata.picture_content_type = None;
            self.write_raw_folder_metadata(folder, &metadata)?;
        }
        Ok(())
    }

    pub fn read_folder_picture(&self, folder: u16) -> Result<(String, Vec<u8>), StorageError> {
        let presentation = self.folder_presentation(folder)?;
        let content_type = presentation
            .picture_content_type
            .ok_or(StorageError::InvalidPictureContentType)?;
        let path = self.folder_picture_path(folder)?;
        let bytes = std::fs::read(&path).map_err(|source| StorageError::Filesystem {
            operation: "read",
            path,
            source,
        })?;
        Ok((content_type, bytes))
    }

    fn metadata_path(&self, address: impl Into<CatalogLocation>) -> PathBuf {
        let address = address.into();
        self.root
            .join(naming::folder_directory(address.folder))
            .join(naming::METADATA_DIRECTORY)
            .join(naming::metadata_filename(address.file))
    }

    /// Persists an operator tempo correction separately from the immutable imported clip.
    pub fn set_intrinsic_bpm(
        &self,
        catalog: &mut CatalogSnapshot,
        id: AssetId,
        bpm: Option<f64>,
    ) -> Result<(), StorageError> {
        let (address, _) = self.locate(catalog, id)?;
        let mut proposed = catalog.clone();
        proposed.set_intrinsic_bpm(id, bpm)?;
        let path = self.metadata_path(address);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| StorageError::Filesystem {
                operation: "create",
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let bytes = serde_json::to_vec(&serde_json::json!({ "intrinsicBpm": bpm }))
            .expect("metadata is serializable");
        std::fs::write(&path, bytes).map_err(|source| StorageError::Filesystem {
            operation: "write",
            path,
            source,
        })?;
        *catalog = proposed;
        Ok(())
    }

    /// Renames an item, keeping its address and identity.
    pub fn rename_item(
        &self,
        catalog: &mut CatalogSnapshot,
        id: AssetId,
        name: &str,
    ) -> Result<(), StorageError> {
        let (address, current) = self.locate(catalog, id)?;
        let from = self.item_path(address, &current);

        // Validate through the catalog before touching anything, so an invalid name never moves a
        // file. This runs on a copy: the real catalog is only updated once the disk agrees.
        let mut proposed = catalog.clone();
        proposed.rename_item(id, name)?;
        let renamed = proposed.item(id).expect("just renamed").1.name.clone();
        let to = self.item_path(address, &renamed);

        if from != to {
            self.rename(&from, &to)?;
            self.rename_source_if_present(address, &current, address, &renamed)?;
            self.rename_thumbnail_if_present(address, address)?;
        }
        *catalog = proposed;
        Ok(())
    }

    /// Moves an item to a free address.
    pub fn move_item(
        &self,
        catalog: &mut CatalogSnapshot,
        id: AssetId,
        to: impl Into<CatalogLocation>,
    ) -> Result<(), StorageError> {
        let to = to.into();
        let (from_address, name) = self.locate(catalog, id)?;
        let mut proposed = catalog.clone();
        proposed.move_item(id, to)?;

        if from_address != to {
            self.ensure_folder(to.folder)?;
            self.rename(
                &self.item_path(from_address, &name),
                &self.item_path(to, &name),
            )?;
            self.rename_source_if_present(from_address, &name, to, &name)?;
            self.rename_thumbnail_if_present(from_address, to)?;
            self.rename_metadata_if_present(from_address, to)?;
        }
        *catalog = proposed;
        Ok(())
    }

    /// Exchanges two items' addresses.
    ///
    /// The first file moves aside before the second takes its place, so neither is ever
    /// overwritten. A crash between the renames leaves a `.swapping` file, which is recoverable;
    /// an overwrite would not be.
    pub fn swap_items(
        &self,
        catalog: &mut CatalogSnapshot,
        first: AssetId,
        second: AssetId,
    ) -> Result<(), StorageError> {
        let (first_address, first_name) = self.locate(catalog, first)?;
        let (second_address, second_name) = self.locate(catalog, second)?;
        let mut proposed = catalog.clone();
        proposed.swap_items(first, second)?;
        if first == second {
            return Ok(());
        }

        let first_path = self.item_path(first_address, &first_name);
        let second_path = self.item_path(second_address, &second_name);
        let staging = staging(&first_path);

        self.rename(&first_path, &staging)?;
        self.rename(&second_path, &self.item_path(first_address, &second_name))?;
        self.rename(&staging, &self.item_path(second_address, &first_name))?;
        self.swap_sources(first_address, &first_name, second_address, &second_name)?;
        self.swap_thumbnails(first_address, second_address)?;
        self.swap_metadata(first_address, second_address)?;

        *catalog = proposed;
        Ok(())
    }

    /// Names a folder, writing or clearing its `.info`.
    pub fn rename_folder(
        &self,
        catalog: &mut CatalogSnapshot,
        folder: u16,
        name: Option<&str>,
    ) -> Result<(), StorageError> {
        let mut proposed = catalog.clone();
        proposed.rename_folder(folder, name)?;

        self.write_folder_metadata(&proposed, folder)?;
        *catalog = proposed;
        Ok(())
    }

    /// Sets or clears a folder icon while retaining the folder name.
    pub fn set_folder_icon(
        &self,
        catalog: &mut CatalogSnapshot,
        folder: u16,
        icon: Option<&str>,
    ) -> Result<(), StorageError> {
        let mut proposed = catalog.clone();
        proposed.set_folder_icon(folder, icon)?;
        self.write_folder_metadata(&proposed, folder)?;
        *catalog = proposed;
        Ok(())
    }

    fn write_folder_metadata(
        &self,
        catalog: &CatalogSnapshot,
        folder: u16,
    ) -> Result<(), StorageError> {
        let entry = catalog.folder(folder);
        let metadata = StoredFolderMetadata {
            name: entry.and_then(|entry| entry.name.clone()),
            icon: entry.and_then(|entry| entry.icon.clone()),
            picture_content_type: entry.and_then(|entry| entry.picture_content_type.clone()),
        };
        self.write_raw_folder_metadata(folder, &metadata)
    }

    fn write_raw_folder_metadata(
        &self,
        folder: u16,
        metadata: &StoredFolderMetadata,
    ) -> Result<(), StorageError> {
        let path = self
            .root
            .join(naming::folder_directory(folder))
            .join(naming::FOLDER_NAME_FILE);
        if metadata.name.is_none()
            && metadata.icon.is_none()
            && metadata.picture_content_type.is_none()
        {
            if path.exists() {
                std::fs::remove_file(&path).map_err(|source| StorageError::Filesystem {
                    operation: "remove",
                    path,
                    source,
                })?;
            }
            return Ok(());
        }
        self.ensure_folder(folder)?;
        // Preserve the legacy plain-text representation when a name is the only metadata.
        let bytes = if metadata.icon.is_none() && metadata.picture_content_type.is_none() {
            metadata.name.clone().unwrap_or_default().into_bytes()
        } else {
            serde_json::to_vec_pretty(&serde_json::json!({
                "name": metadata.name,
                "icon": metadata.icon,
                "pictureContentType": metadata.picture_content_type,
            }))
            .expect("folder metadata serializes")
        };
        std::fs::write(&path, bytes).map_err(|source| StorageError::Filesystem {
            operation: "write",
            path,
            source,
        })
    }

    /// Exchanges two complete physical folders, including names, originals, metadata and
    /// thumbnails. This is also how a playable folder is parked or restored as one operation.
    pub fn swap_folders(
        &self,
        catalog: &mut CatalogSnapshot,
        first: u16,
        second: u16,
    ) -> Result<(), StorageError> {
        let mut proposed = catalog.clone();
        proposed.swap_folders(first, second)?;
        if first == second {
            return Ok(());
        }

        let first_path = self.root.join(naming::folder_directory(first));
        let second_path = self.root.join(naming::folder_directory(second));
        match (first_path.exists(), second_path.exists()) {
            (true, true) => {
                let staging = first_path.with_extension("swapping");
                if staging.exists() {
                    return Err(StorageError::Filesystem {
                        operation: "stage folder swap because a recovery folder already exists at",
                        path: staging,
                        source: std::io::Error::new(
                            std::io::ErrorKind::AlreadyExists,
                            "recover the earlier folder swap before retrying",
                        ),
                    });
                }
                self.rename(&first_path, &staging)?;
                self.rename(&second_path, &first_path)?;
                self.rename(&staging, &second_path)?;
            }
            (true, false) => self.rename(&first_path, &second_path)?,
            (false, true) => self.rename(&second_path, &first_path)?,
            (false, false) => {}
        }
        *catalog = proposed;
        Ok(())
    }

    /// Deletes an item and its thumbnail.
    pub fn remove_item(
        &self,
        catalog: &mut CatalogSnapshot,
        id: AssetId,
    ) -> Result<(), StorageError> {
        let (address, name) = self.locate(catalog, id)?;
        let path = self.item_path(address, &name);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|source| StorageError::Filesystem {
                operation: "remove",
                path,
                source,
            })?;
        }
        let thumbnail = self.thumbnail_path(address);
        let _ = std::fs::remove_file(thumbnail);
        let _ = std::fs::remove_file(self.metadata_path(address));
        catalog.remove_item(id);
        Ok(())
    }

    /// Creates a folder's directory if it is not there yet.
    pub fn ensure_folder(&self, folder: impl Into<u16>) -> Result<(), StorageError> {
        let folder = folder.into();
        let path = self.root.join(naming::folder_directory(folder));
        std::fs::create_dir_all(&path).map_err(|source| StorageError::Filesystem {
            operation: "create",
            path,
            source,
        })
    }

    fn locate(
        &self,
        catalog: &CatalogSnapshot,
        id: AssetId,
    ) -> Result<(CatalogLocation, String), StorageError> {
        let (folder, item) = catalog.item(id).ok_or(CatalogError::NoSuchItem)?;
        Ok((
            CatalogLocation::new(folder.folder, item.file),
            item.name.clone(),
        ))
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), StorageError> {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|source| StorageError::Filesystem {
                operation: "create",
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::rename(from, to).map_err(|source| StorageError::Filesystem {
            operation: "rename",
            path: from.to_path_buf(),
            source,
        })
    }

    /// Thumbnails follow their item. A missing one is not an error — it is regenerated on demand.
    fn rename_thumbnail_if_present(
        &self,
        from: CatalogLocation,
        to: CatalogLocation,
    ) -> Result<(), StorageError> {
        let source = self.thumbnail_path(from);
        if !source.exists() || from == to {
            return Ok(());
        }
        self.rename(&source, &self.thumbnail_path(to))
    }

    fn rename_metadata_if_present(
        &self,
        from: CatalogLocation,
        to: CatalogLocation,
    ) -> Result<(), StorageError> {
        let source = self.metadata_path(from);
        if !source.exists() || from == to {
            return Ok(());
        }
        self.rename(&source, &self.metadata_path(to))
    }

    fn rename_source_if_present(
        &self,
        from: CatalogLocation,
        from_name: &str,
        to: CatalogLocation,
        to_name: &str,
    ) -> Result<(), StorageError> {
        let Some(source) = self.source_path(from, from_name) else {
            return Ok(());
        };
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        let destination = self.source_destination(to, to_name, extension);
        if source != destination {
            self.rename(&source, &destination)?;
        }
        Ok(())
    }

    fn swap_sources(
        &self,
        first: CatalogLocation,
        first_name: &str,
        second: CatalogLocation,
        second_name: &str,
    ) -> Result<(), StorageError> {
        let first_source = self.source_path(first, first_name);
        let second_source = self.source_path(second, second_name);
        match (first_source, second_source) {
            (Some(a), Some(b)) => {
                let a_extension = a
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default();
                let b_extension = b
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default();
                let staging = staging(&a);
                self.rename(&a, &staging)?;
                self.rename(
                    &b,
                    &self.source_destination(first, second_name, b_extension),
                )?;
                self.rename(
                    &staging,
                    &self.source_destination(second, first_name, a_extension),
                )
            }
            (Some(a), None) => {
                let extension = a
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default();
                self.rename(&a, &self.source_destination(second, first_name, extension))
            }
            (None, Some(b)) => {
                let extension = b
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default();
                self.rename(&b, &self.source_destination(first, second_name, extension))
            }
            (None, None) => Ok(()),
        }
    }

    fn source_path(&self, address: CatalogLocation, name: &str) -> Option<PathBuf> {
        let folder = self.root.join(naming::folder_directory(address.folder));
        std::fs::read_dir(folder)
            .ok()?
            .flatten()
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|value| value.to_str())
                    .and_then(naming::parse_source_filename)
                    .is_some_and(|(file, source_name)| file == address.file && source_name == name)
            })
    }

    fn source_destination(&self, address: CatalogLocation, name: &str, extension: &str) -> PathBuf {
        let safe = naming::safe_name(name);
        let stem = if safe.is_empty() {
            format!("{:03}", address.file)
        } else {
            format!("{:03}-{safe}", address.file)
        };
        self.root
            .join(naming::folder_directory(address.folder))
            .join(format!("{stem}.{extension}"))
    }

    fn swap_thumbnails(
        &self,
        first: CatalogLocation,
        second: CatalogLocation,
    ) -> Result<(), StorageError> {
        let (a, b) = (self.thumbnail_path(first), self.thumbnail_path(second));
        match (a.exists(), b.exists()) {
            (true, true) => {
                let staging = staging(&a);
                self.rename(&a, &staging)?;
                self.rename(&b, &a)?;
                self.rename(&staging, &b)
            }
            (true, false) => self.rename(&a, &b),
            (false, true) => self.rename(&b, &a),
            (false, false) => Ok(()),
        }
    }

    fn swap_metadata(
        &self,
        first: CatalogLocation,
        second: CatalogLocation,
    ) -> Result<(), StorageError> {
        let (a, b) = (self.metadata_path(first), self.metadata_path(second));
        match (a.exists(), b.exists()) {
            (true, true) => {
                let staging = staging(&a);
                self.rename(&a, &staging)?;
                self.rename(&b, &a)?;
                self.rename(&staging, &b)
            }
            (true, false) => self.rename(&a, &b),
            (false, true) => self.rename(&b, &a),
            (false, false) => Ok(()),
        }
    }
}

#[derive(Default)]
struct StoredFolderMetadata {
    name: Option<String>,
    icon: Option<String>,
    picture_content_type: Option<String>,
}

fn validate_presentation_folder(folder: u16) -> Result<(), StorageError> {
    if (1..=255).contains(&folder) {
        Ok(())
    } else {
        Err(StorageError::InvalidPresentationFolder { folder })
    }
}

fn normalized(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn read_folder_metadata(path: &Path) -> StoredFolderMetadata {
    let Ok(contents) = std::fs::read_to_string(path.join(naming::FOLDER_NAME_FILE)) else {
        return StoredFolderMetadata::default();
    };
    match serde_json::from_str::<serde_json::Value>(&contents) {
        Ok(serde_json::Value::Object(document)) => StoredFolderMetadata {
            name: document
                .get("name")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| normalized(Some(value))),
            icon: document
                .get("icon")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| normalized(Some(value))),
            picture_content_type: document
                .get("pictureContentType")
                .and_then(serde_json::Value::as_str)
                .filter(|value| value.starts_with("image/"))
                .map(str::to_owned),
        },
        _ => StoredFolderMetadata {
            name: normalized(Some(&contents)),
            ..StoredFolderMetadata::default()
        },
    }
}

fn staging(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".swapping");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use media_domain::MediaAddress;
    use media_domain::catalog::{CatalogItem, ItemKind};

    use super::*;

    struct Library {
        storage: LibraryStorage,
        catalog: CatalogSnapshot,
    }

    impl Library {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir()
                .join("media-library-storage")
                .join(name);
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            Self {
                storage: LibraryStorage::new(root),
                catalog: CatalogSnapshot::default(),
            }
        }

        fn add(&mut self, folder: u8, file: u8, name: &str) -> AssetId {
            let id = AssetId::new();
            self.catalog
                .insert(
                    u16::from(folder),
                    CatalogItem {
                        id,
                        file,
                        name: name.to_owned(),
                        kind: ItemKind::Video,
                        width: 16,
                        height: 16,
                        frames: Some(2),
                        intrinsic_bpm: None,
                    },
                )
                .unwrap();
            let path = self
                .storage
                .item_path(MediaAddress::new(folder, file), name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, name.as_bytes()).unwrap();
            id
        }

        fn contents(&self, folder: u8, file: u8, name: &str) -> Option<String> {
            std::fs::read_to_string(
                self.storage
                    .item_path(MediaAddress::new(folder, file), name),
            )
            .ok()
        }

        fn files(&self, folder: u8) -> Vec<String> {
            let mut names: Vec<String> =
                std::fs::read_dir(self.storage.root().join(naming::folder_directory(folder)))
                    .map(|entries| {
                        entries
                            .flatten()
                            .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
            names.sort();
            names
        }
    }

    impl Drop for Library {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.storage.root());
        }
    }

    #[test]
    fn renaming_moves_the_file_and_keeps_the_address() {
        let mut library = Library::new("rename");
        let id = library.add(1, 7, "Before");

        library
            .storage
            .rename_item(&mut library.catalog, id, "After")
            .unwrap();

        assert_eq!(library.files(1), ["007-After.toskclip"]);
        assert_eq!(
            library.contents(1, 7, "After").as_deref(),
            Some("Before"),
            "same file"
        );
        assert_eq!(
            library.catalog.address_of(id),
            Some(MediaAddress::new(1, 7))
        );
    }

    #[test]
    fn moving_relocates_the_file_and_creates_the_folder() {
        let mut library = Library::new("move");
        let id = library.add(1, 7, "Clip");

        library
            .storage
            .move_item(&mut library.catalog, id, MediaAddress::new(4, 20))
            .unwrap();

        assert!(library.files(1).is_empty());
        assert_eq!(library.files(4), ["020-Clip.toskclip"]);
        assert_eq!(
            library.catalog.address_of(id),
            Some(MediaAddress::new(4, 20))
        );
    }

    #[test]
    fn a_swap_never_overwrites_either_file() {
        let mut library = Library::new("swap");
        let first = library.add(1, 1, "First");
        let second = library.add(1, 2, "Second");

        library
            .storage
            .swap_items(&mut library.catalog, first, second)
            .unwrap();

        assert_eq!(
            library.files(1),
            ["001-Second.toskclip", "002-First.toskclip"]
        );
        assert_eq!(library.contents(1, 1, "Second").as_deref(), Some("Second"));
        assert_eq!(library.contents(1, 2, "First").as_deref(), Some("First"));
        assert_eq!(
            library.catalog.address_of(first),
            Some(MediaAddress::new(1, 2))
        );
        assert_eq!(
            library.catalog.address_of(second),
            Some(MediaAddress::new(1, 1))
        );
    }

    #[test]
    fn a_refused_edit_leaves_both_the_catalog_and_the_disk_alone() {
        let mut library = Library::new("refused");
        let first = library.add(1, 1, "First");
        library.add(1, 2, "Second");
        let before_catalog = library.catalog.clone();
        let before_files = library.files(1);

        let error = library
            .storage
            .move_item(&mut library.catalog, first, MediaAddress::new(1, 2))
            .unwrap_err();

        assert!(matches!(
            error,
            StorageError::Catalog(CatalogError::AddressTaken { .. })
        ));
        assert_eq!(library.catalog, before_catalog);
        assert_eq!(library.files(1), before_files);
    }

    #[test]
    fn a_name_that_the_catalog_refuses_never_touches_the_disk() {
        let mut library = Library::new("bad-name");
        let id = library.add(1, 1, "Good");
        let before = library.files(1);

        let error = library
            .storage
            .rename_item(&mut library.catalog, id, "   ")
            .unwrap_err();
        assert!(matches!(
            error,
            StorageError::Catalog(CatalogError::EmptyName)
        ));
        assert_eq!(library.files(1), before);
    }

    #[test]
    fn a_folder_name_is_written_and_cleared() {
        let mut library = Library::new("folder-name");
        library.add(2, 1, "Clip");

        library
            .storage
            .rename_folder(&mut library.catalog, 2, Some("Stingers"))
            .unwrap();
        let info = library
            .storage
            .root()
            .join("002")
            .join(naming::FOLDER_NAME_FILE);
        assert_eq!(std::fs::read_to_string(&info).unwrap(), "Stingers");
        assert_eq!(
            library.catalog.folder(2).unwrap().name.as_deref(),
            Some("Stingers")
        );

        library
            .storage
            .rename_folder(&mut library.catalog, 2, None)
            .unwrap();
        assert!(!info.exists(), "clearing the name removes the file");
        assert_eq!(library.catalog.folder(2).unwrap().name, None);
    }

    #[test]
    fn a_folder_icon_is_stored_with_and_does_not_erase_the_name() {
        let mut library = Library::new("folder-icon");
        library.add(2, 1, "Clip");
        library
            .storage
            .rename_folder(&mut library.catalog, 2, Some("Stingers"))
            .unwrap();
        library
            .storage
            .set_folder_icon(&mut library.catalog, 2, Some("▶"))
            .unwrap();

        let info = library
            .storage
            .root()
            .join("002")
            .join(naming::FOLDER_NAME_FILE);
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(info).unwrap()).unwrap();
        assert_eq!(document["name"], "Stingers");
        assert_eq!(document["icon"], "▶");
        assert_eq!(
            library.catalog.folder(2).unwrap().icon.as_deref(),
            Some("▶")
        );
    }

    #[test]
    fn generated_folder_presentation_round_trips_without_becoming_catalog_media() {
        let library = Library::new("generated-folder-presentation");
        let updated = library
            .storage
            .update_generated_folder_presentation(
                250,
                Some(Some("Equalizers")),
                Some(Some("waveform")),
            )
            .unwrap();

        assert_eq!(updated.folder, 250);
        assert_eq!(updated.name.as_deref(), Some("Equalizers"));
        assert_eq!(updated.icon.as_deref(), Some("waveform"));
        assert!(library.catalog.folder(250).is_none());

        let cleared = library
            .storage
            .update_generated_folder_presentation(250, Some(Some("")), None)
            .unwrap();
        assert_eq!(cleared.name, None);
        assert_eq!(cleared.icon.as_deref(), Some("waveform"));
    }

    #[test]
    fn folder_picture_round_trips_for_media_and_generated_folders() {
        let mut library = Library::new("folder-picture-round-trip");
        let pixels = b"not decoded here; presentation preserves the uploaded image bytes";

        library
            .storage
            .write_folder_picture(&mut library.catalog, 2, "image/png", pixels)
            .unwrap();
        assert_eq!(
            library
                .catalog
                .folder(2)
                .unwrap()
                .picture_content_type
                .as_deref(),
            Some("image/png")
        );
        assert_eq!(
            library.storage.read_folder_picture(2).unwrap(),
            ("image/png".to_owned(), pixels.to_vec())
        );

        library
            .storage
            .write_folder_picture(&mut library.catalog, 200, "image/webp", pixels)
            .unwrap();
        assert_eq!(
            library.storage.read_folder_picture(200).unwrap().0,
            "image/webp"
        );
        library
            .storage
            .remove_folder_picture(&mut library.catalog, 200)
            .unwrap();
        assert!(library.storage.read_folder_picture(200).is_err());
    }

    #[test]
    fn legacy_plain_folder_name_survives_new_presentation_reads() {
        let library = Library::new("legacy-folder-presentation");
        library.storage.ensure_folder(200_u16).unwrap();
        std::fs::write(
            library
                .storage
                .root
                .join("200")
                .join(naming::FOLDER_NAME_FILE),
            "  Legacy Text  \n",
        )
        .unwrap();

        let presentation = library.storage.folder_presentation(200).unwrap();
        assert_eq!(presentation.name.as_deref(), Some("Legacy Text"));
        assert_eq!(presentation.icon, None);
        assert_eq!(presentation.picture_content_type, None);
    }

    #[test]
    fn stale_picture_metadata_never_advertises_a_missing_image() {
        let library = Library::new("missing-folder-picture");
        library.storage.ensure_folder(250_u16).unwrap();
        std::fs::write(
            library
                .storage
                .root
                .join("250")
                .join(naming::FOLDER_NAME_FILE),
            r#"{"pictureContentType":"image/png"}"#,
        )
        .unwrap();

        assert_eq!(
            library
                .storage
                .folder_presentation(250)
                .unwrap()
                .picture_content_type,
            None
        );
    }

    #[test]
    fn a_complete_folder_can_be_parked_and_restored_on_disk() {
        let mut library = Library::new("park-folder");
        let id = library.add(1, 7, "Opening");

        library
            .storage
            .swap_folders(&mut library.catalog, 1, 900)
            .unwrap();
        assert_eq!(
            library.catalog.location_of(id),
            Some(CatalogLocation::new(900, 7))
        );
        assert!(
            library
                .storage
                .item_path(CatalogLocation::new(900, 7), "Opening")
                .exists()
        );

        library
            .storage
            .swap_folders(&mut library.catalog, 900, 1)
            .unwrap();
        assert_eq!(
            library.catalog.address_of(id),
            Some(MediaAddress::new(1, 7))
        );
        assert_eq!(
            library.contents(1, 7, "Opening").as_deref(),
            Some("Opening")
        );
    }

    #[test]
    fn thumbnails_follow_their_item() {
        let mut library = Library::new("thumbnails");
        let id = library.add(1, 5, "Clip");
        let thumbnail = library.storage.thumbnail_path(MediaAddress::new(1, 5));
        std::fs::create_dir_all(thumbnail.parent().unwrap()).unwrap();
        std::fs::write(&thumbnail, b"thumb").unwrap();

        library
            .storage
            .move_item(&mut library.catalog, id, MediaAddress::new(1, 9))
            .unwrap();

        assert!(!thumbnail.exists());
        let moved = library.storage.thumbnail_path(MediaAddress::new(1, 9));
        assert_eq!(std::fs::read(moved).unwrap(), b"thumb");
    }

    #[test]
    fn preserved_import_sources_follow_renames_and_moves() {
        let mut library = Library::new("sources-follow");
        let id = library.add(1, 5, "Clip");
        let source = library.storage.root().join("001/005-Clip.png");
        std::fs::write(&source, b"original").unwrap();

        library
            .storage
            .rename_item(&mut library.catalog, id, "Opening")
            .unwrap();
        assert!(!source.exists());
        assert_eq!(
            std::fs::read(library.storage.root().join("001/005-Opening.png")).unwrap(),
            b"original"
        );

        library
            .storage
            .move_item(&mut library.catalog, id, MediaAddress::new(2, 8))
            .unwrap();
        assert!(!library.storage.root().join("001/005-Opening.png").exists());
        assert_eq!(
            std::fs::read(library.storage.root().join("002/008-Opening.png")).unwrap(),
            b"original"
        );
    }

    #[test]
    fn a_missing_thumbnail_is_not_an_error() {
        let mut library = Library::new("no-thumbnail");
        let id = library.add(1, 5, "Clip");
        library
            .storage
            .move_item(&mut library.catalog, id, MediaAddress::new(1, 6))
            .unwrap();
        assert_eq!(
            library.catalog.address_of(id),
            Some(MediaAddress::new(1, 6))
        );
    }

    #[test]
    fn removing_takes_the_item_and_its_thumbnail() {
        let mut library = Library::new("remove");
        let id = library.add(1, 3, "Clip");
        let thumbnail = library.storage.thumbnail_path(MediaAddress::new(1, 3));
        std::fs::create_dir_all(thumbnail.parent().unwrap()).unwrap();
        std::fs::write(&thumbnail, b"thumb").unwrap();

        library
            .storage
            .remove_item(&mut library.catalog, id)
            .unwrap();

        assert!(library.catalog.item(id).is_none());
        assert!(!thumbnail.exists());
        assert!(
            !library
                .files(1)
                .iter()
                .any(|name| name.ends_with(".toskclip"))
        );
    }

    #[test]
    fn editing_something_that_is_not_in_the_catalog_reports_it() {
        let mut library = Library::new("absent");
        let error = library
            .storage
            .rename_item(&mut library.catalog, AssetId::new(), "New")
            .unwrap_err();
        assert!(matches!(
            error,
            StorageError::Catalog(CatalogError::NoSuchItem)
        ));
    }

    #[test]
    fn an_operator_bpm_correction_is_persisted_and_follows_a_move() {
        let mut library = Library::new("bpm-correction");
        let id = library.add(1, 3, "Clip");
        library
            .storage
            .set_intrinsic_bpm(&mut library.catalog, id, Some(127.5))
            .unwrap();
        assert_eq!(
            library.catalog.item(id).unwrap().1.intrinsic_bpm,
            Some(127.5)
        );
        let original = library.storage.metadata_path(MediaAddress::new(1, 3));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(&original).unwrap())
                .unwrap(),
            serde_json::json!({ "intrinsicBpm": 127.5 })
        );

        library
            .storage
            .move_item(&mut library.catalog, id, MediaAddress::new(2, 8))
            .unwrap();
        assert!(!original.exists());
        assert!(
            library
                .storage
                .metadata_path(MediaAddress::new(2, 8))
                .exists()
        );
    }
}
