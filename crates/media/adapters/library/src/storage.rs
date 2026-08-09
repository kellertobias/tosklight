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
use media_domain::{AssetId, MediaAddress};

use crate::naming;

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
    pub fn item_path(&self, address: MediaAddress, name: &str) -> PathBuf {
        self.root
            .join(naming::folder_directory(address.folder))
            .join(naming::item_filename(address.file, name))
    }

    /// Where an item's thumbnail lives.
    pub fn thumbnail_path(&self, address: MediaAddress) -> PathBuf {
        self.root
            .join(naming::folder_directory(address.folder))
            .join(naming::THUMBNAIL_DIRECTORY)
            .join(naming::thumbnail_filename(address.file))
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
        to: MediaAddress,
    ) -> Result<(), StorageError> {
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

        *catalog = proposed;
        Ok(())
    }

    /// Names a folder, writing or clearing its `.info`.
    pub fn rename_folder(
        &self,
        catalog: &mut CatalogSnapshot,
        folder: u8,
        name: Option<&str>,
    ) -> Result<(), StorageError> {
        let mut proposed = catalog.clone();
        proposed.rename_folder(folder, name)?;

        let path = self
            .root
            .join(naming::folder_directory(folder))
            .join(naming::FOLDER_NAME_FILE);
        match proposed.folder(folder).and_then(|entry| entry.name.clone()) {
            Some(name) => {
                self.ensure_folder(folder)?;
                std::fs::write(&path, name).map_err(|source| StorageError::Filesystem {
                    operation: "write",
                    path,
                    source,
                })?;
            }
            None => {
                if path.exists() {
                    std::fs::remove_file(&path).map_err(|source| StorageError::Filesystem {
                        operation: "remove",
                        path,
                        source,
                    })?;
                }
            }
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
        catalog.remove_item(id);
        Ok(())
    }

    /// Creates a folder's directory if it is not there yet.
    pub fn ensure_folder(&self, folder: u8) -> Result<(), StorageError> {
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
    ) -> Result<(MediaAddress, String), StorageError> {
        let (folder, item) = catalog.item(id).ok_or(CatalogError::NoSuchItem)?;
        Ok((
            MediaAddress::new(folder.folder, item.file),
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
        from: MediaAddress,
        to: MediaAddress,
    ) -> Result<(), StorageError> {
        let source = self.thumbnail_path(from);
        if !source.exists() || from == to {
            return Ok(());
        }
        self.rename(&source, &self.thumbnail_path(to))
    }

    fn rename_source_if_present(
        &self,
        from: MediaAddress,
        from_name: &str,
        to: MediaAddress,
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
        first: MediaAddress,
        first_name: &str,
        second: MediaAddress,
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

    fn source_path(&self, address: MediaAddress, name: &str) -> Option<PathBuf> {
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

    fn source_destination(&self, address: MediaAddress, name: &str, extension: &str) -> PathBuf {
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
        first: MediaAddress,
        second: MediaAddress,
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
}

fn staging(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".swapping");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
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
                    folder,
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
}
