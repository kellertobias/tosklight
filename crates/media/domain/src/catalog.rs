//! The media catalog.
//!
//! Filesystem names are a storage adapter, not the domain model. An item's identity is its
//! [`AssetId`], which follows it through renames, moves, and reindexing; its `(folder, file)`
//! address is where a desk currently points at it, and that moves. Keeping the two distinct is
//! what makes reindexing a deliberate operation rather than an accident.
//!
//! Every mutation here returns a new snapshot or an error. Nothing is applied halfway: a rename
//! that would collide leaves the catalog exactly as it was, so a failed edit can never publish a
//! library that contradicts itself.

use serde::{Deserialize, Serialize};

use crate::address::{AddressClass, AssetId, MediaAddress};

/// The lowest and highest file index an item may occupy. `0` and `255` are blank sentinels.
pub const FIRST_FILE: u8 = 1;
pub const LAST_FILE: u8 = 254;

/// Increments whenever the catalog changes, so a consumer can tell whether the snapshot it holds
/// is still current without comparing contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CatalogRevision(u64);

impl CatalogRevision {
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// What kind of thing an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ItemKind {
    Image,
    Video,
}

/// One addressable item in the library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItem {
    /// Stable across renames, moves, and reindexing.
    pub id: AssetId,
    /// Where a desk currently addresses it within its folder.
    pub file: u8,
    /// The operator-visible name, without the index prefix or the extension.
    pub name: String,
    pub kind: ItemKind,
    pub width: u32,
    pub height: u32,
    /// Frames, for a video. Absent for a still.
    pub frames: Option<u32>,
    /// The tempo the asset was authored at, if it has one. An operator may correct or clear it.
    pub intrinsic_bpm: Option<f64>,
}

/// One library folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFolder {
    /// The folder's DMX index.
    pub folder: u8,
    /// The operator's name for it, when they have given one.
    pub name: Option<String>,
    /// Items in file order. Order is maintained so a reindex is a deliberate rewrite rather than
    /// something that depends on how a directory happened to be read.
    pub items: Vec<CatalogItem>,
}

impl CatalogFolder {
    pub fn item(&self, file: u8) -> Option<&CatalogItem> {
        self.items.iter().find(|item| item.file == file)
    }
}

/// An immutable view of the whole library.
///
/// Adapters read this. None of them rescans the filesystem or reinterprets a filename on its own,
/// so the API, the React UI, the renderer, and CITP always describe the same library.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSnapshot {
    pub revision: CatalogRevision,
    /// Folders in index order.
    pub folders: Vec<CatalogFolder>,
}

/// Why a catalog edit cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogError {
    #[error("folder {folder} is not a library folder; only 1-199 hold filesystem media")]
    NotALibraryFolder { folder: u8 },
    #[error("file {file} is a blank sentinel, not a usable index")]
    NotAUsableFile { file: u8 },
    #[error("no folder {folder}")]
    NoSuchFolder { folder: u8 },
    #[error("no item with that identity in the catalog")]
    NoSuchItem,
    #[error("folder {folder} already has an item at file {file}")]
    AddressTaken { folder: u8, file: u8 },
    #[error("an item name cannot be empty")]
    EmptyName,
    #[error("intrinsic BPM must be a finite positive number")]
    InvalidIntrinsicBpm,
}

impl CatalogSnapshot {
    /// Resolves the address a desk is pointing at.
    ///
    /// A blank address resolves to nothing, which is not a failure: it is a layer with nothing
    /// selected.
    pub fn resolve(&self, address: MediaAddress) -> Option<&CatalogItem> {
        if address.classify() != AddressClass::Library {
            return None;
        }
        self.folder(address.folder)?.item(address.file)
    }

    pub fn folder(&self, folder: u8) -> Option<&CatalogFolder> {
        self.folders
            .iter()
            .find(|candidate| candidate.folder == folder)
    }

    /// Finds an item by identity, wherever it currently lives.
    pub fn item(&self, id: AssetId) -> Option<(&CatalogFolder, &CatalogItem)> {
        self.folders.iter().find_map(|folder| {
            folder
                .items
                .iter()
                .find(|item| item.id == id)
                .map(|item| (folder, item))
        })
    }

    /// The address an item currently occupies.
    pub fn address_of(&self, id: AssetId) -> Option<MediaAddress> {
        let (folder, item) = self.item(id)?;
        Some(MediaAddress::new(folder.folder, item.file))
    }

    pub fn item_count(&self) -> usize {
        self.folders.iter().map(|folder| folder.items.len()).sum()
    }

    /// Adds an item at an address, if that address is usable and free.
    pub fn insert(&mut self, folder: u8, item: CatalogItem) -> Result<(), CatalogError> {
        validate_address(MediaAddress::new(folder, item.file))?;
        if item.name.trim().is_empty() {
            return Err(CatalogError::EmptyName);
        }
        if self
            .folder(folder)
            .and_then(|entry| entry.item(item.file))
            .is_some()
        {
            return Err(CatalogError::AddressTaken {
                folder,
                file: item.file,
            });
        }

        let entry = match self
            .folders
            .iter_mut()
            .position(|entry| entry.folder == folder)
        {
            Some(position) => &mut self.folders[position],
            None => {
                self.folders.push(CatalogFolder {
                    folder,
                    name: None,
                    items: Vec::new(),
                });
                self.folders.sort_by_key(|entry| entry.folder);
                self.folders
                    .iter_mut()
                    .find(|entry| entry.folder == folder)
                    .expect("just inserted")
            }
        };
        entry.items.push(item);
        entry.items.sort_by_key(|item| item.file);
        self.bump();
        Ok(())
    }

    /// Renames an item without moving it. Its identity and address are untouched.
    pub fn rename_item(&mut self, id: AssetId, name: &str) -> Result<(), CatalogError> {
        if name.trim().is_empty() {
            return Err(CatalogError::EmptyName);
        }
        let item = self
            .folders
            .iter_mut()
            .find_map(|folder| folder.items.iter_mut().find(|item| item.id == id))
            .ok_or(CatalogError::NoSuchItem)?;
        item.name = name.trim().to_owned();
        self.bump();
        Ok(())
    }

    /// Corrects or clears the authored tempo without changing the item's identity or address.
    pub fn set_intrinsic_bpm(&mut self, id: AssetId, bpm: Option<f64>) -> Result<(), CatalogError> {
        if bpm.is_some_and(|value| !value.is_finite() || value <= 0.0) {
            return Err(CatalogError::InvalidIntrinsicBpm);
        }
        let item = self
            .folders
            .iter_mut()
            .find_map(|folder| folder.items.iter_mut().find(|item| item.id == id))
            .ok_or(CatalogError::NoSuchItem)?;
        item.intrinsic_bpm = bpm;
        self.bump();
        Ok(())
    }

    /// Moves an item to another address, keeping its identity.
    ///
    /// The destination must be free. Swapping two items is two moves through a free slot, or
    /// [`Self::swap_items`], rather than a move that silently overwrites.
    pub fn move_item(&mut self, id: AssetId, to: MediaAddress) -> Result<(), CatalogError> {
        validate_address(to)?;
        let from = self.address_of(id).ok_or(CatalogError::NoSuchItem)?;
        if from == to {
            return Ok(());
        }
        if self.resolve(to).is_some() {
            return Err(CatalogError::AddressTaken {
                folder: to.folder,
                file: to.file,
            });
        }

        let mut item = self.take(id).expect("looked up above");
        item.file = to.file;
        // Insert bumps the revision, and cannot fail: the address was just checked free.
        self.insert(to.folder, item)
    }

    /// Exchanges the addresses of two items.
    ///
    /// One operation rather than two moves, so an operator dragging one item onto another never
    /// leaves the catalog with one of them temporarily homeless.
    pub fn swap_items(&mut self, first: AssetId, second: AssetId) -> Result<(), CatalogError> {
        let first_address = self.address_of(first).ok_or(CatalogError::NoSuchItem)?;
        let second_address = self.address_of(second).ok_or(CatalogError::NoSuchItem)?;
        if first == second {
            return Ok(());
        }

        let mut a = self.take(first).expect("looked up above");
        let mut b = self.take(second).expect("looked up above");
        a.file = second_address.file;
        b.file = first_address.file;
        self.insert(second_address.folder, a)?;
        self.insert(first_address.folder, b)?;
        Ok(())
    }

    /// Names a folder, or clears its name.
    pub fn rename_folder(&mut self, folder: u8, name: Option<&str>) -> Result<(), CatalogError> {
        let entry = self
            .folders
            .iter_mut()
            .find(|entry| entry.folder == folder)
            .ok_or(CatalogError::NoSuchFolder { folder })?;
        entry.name = name
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
        self.bump();
        Ok(())
    }

    /// Exchanges two whole folders, contents and names together.
    pub fn swap_folders(&mut self, first: u8, second: u8) -> Result<(), CatalogError> {
        for folder in [first, second] {
            if !matches!(
                MediaAddress::new(folder, FIRST_FILE).classify(),
                AddressClass::Library
            ) {
                return Err(CatalogError::NotALibraryFolder { folder });
            }
        }
        if first == second {
            return Ok(());
        }
        for entry in &mut self.folders {
            if entry.folder == first {
                entry.folder = second;
            } else if entry.folder == second {
                entry.folder = first;
            }
        }
        self.folders.sort_by_key(|entry| entry.folder);
        self.bump();
        Ok(())
    }

    /// Removes an item. Returns whether there was one.
    pub fn remove_item(&mut self, id: AssetId) -> bool {
        let removed = self.take(id).is_some();
        if removed {
            self.bump();
        }
        removed
    }

    fn take(&mut self, id: AssetId) -> Option<CatalogItem> {
        for folder in &mut self.folders {
            if let Some(position) = folder.items.iter().position(|item| item.id == id) {
                return Some(folder.items.remove(position));
            }
        }
        None
    }

    fn bump(&mut self) {
        self.revision = self.revision.next();
    }
}

/// Whether an address can hold a filesystem item at all.
pub const fn validate_address(address: MediaAddress) -> Result<(), CatalogError> {
    match address.classify() {
        AddressClass::Library => Ok(()),
        AddressClass::Blank => Err(CatalogError::NotAUsableFile { file: address.file }),
        // Text banks and generated visualizers are addressed, but nothing on disk lives there.
        AddressClass::TextBank | AddressClass::GeneratedVisualizer => {
            Err(CatalogError::NotALibraryFolder {
                folder: address.folder,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(file: u8, name: &str) -> CatalogItem {
        CatalogItem {
            id: AssetId::new(),
            file,
            name: name.to_owned(),
            kind: ItemKind::Video,
            width: 1920,
            height: 1080,
            frames: Some(100),
            intrinsic_bpm: None,
        }
    }

    fn catalog() -> CatalogSnapshot {
        let mut catalog = CatalogSnapshot::default();
        catalog.insert(1, item(1, "First")).unwrap();
        catalog.insert(1, item(2, "Second")).unwrap();
        catalog.insert(3, item(7, "Elsewhere")).unwrap();
        catalog
    }

    #[test]
    fn a_desk_address_resolves_to_the_item_there() {
        let catalog = catalog();
        assert_eq!(
            catalog.resolve(MediaAddress::new(1, 1)).unwrap().name,
            "First"
        );
        assert_eq!(
            catalog.resolve(MediaAddress::new(3, 7)).unwrap().name,
            "Elsewhere"
        );
        assert!(catalog.resolve(MediaAddress::new(1, 9)).is_none());
    }

    #[test]
    fn a_blank_or_generated_address_resolves_to_nothing_rather_than_failing() {
        let catalog = catalog();
        assert!(catalog.resolve(MediaAddress::BLANK).is_none());
        assert!(catalog.resolve(MediaAddress::new(1, 0)).is_none());
        assert!(catalog.resolve(MediaAddress::new(1, 255)).is_none());
        assert!(
            catalog.resolve(MediaAddress::new(200, 1)).is_none(),
            "a text bank"
        );
        assert!(
            catalog.resolve(MediaAddress::new(230, 1)).is_none(),
            "a visualizer"
        );
    }

    #[test]
    fn identity_survives_a_rename_and_a_move() {
        let mut catalog = catalog();
        let id = catalog.resolve(MediaAddress::new(1, 1)).unwrap().id;

        catalog.rename_item(id, "Renamed").unwrap();
        assert_eq!(catalog.address_of(id), Some(MediaAddress::new(1, 1)));

        catalog.move_item(id, MediaAddress::new(5, 20)).unwrap();
        assert_eq!(catalog.address_of(id), Some(MediaAddress::new(5, 20)));
        assert_eq!(
            catalog.item(id).unwrap().1.name,
            "Renamed",
            "the rename came with it"
        );
        assert!(
            catalog.resolve(MediaAddress::new(1, 1)).is_none(),
            "and it left its old address"
        );
    }

    #[test]
    fn a_move_onto_an_occupied_address_is_refused_and_changes_nothing() {
        let mut catalog = catalog();
        let before = catalog.clone();
        let first = catalog.folder(1).unwrap().items[0].id;

        let error = catalog
            .move_item(first, MediaAddress::new(1, 2))
            .unwrap_err();
        assert_eq!(error, CatalogError::AddressTaken { folder: 1, file: 2 });
        assert_eq!(
            catalog, before,
            "a refused edit leaves the catalog untouched"
        );
    }

    #[test]
    fn two_items_swap_without_either_being_homeless() {
        let mut catalog = catalog();
        let first = catalog.resolve(MediaAddress::new(1, 1)).unwrap().id;
        let second = catalog.resolve(MediaAddress::new(3, 7)).unwrap().id;

        catalog.swap_items(first, second).unwrap();
        assert_eq!(catalog.address_of(first), Some(MediaAddress::new(3, 7)));
        assert_eq!(catalog.address_of(second), Some(MediaAddress::new(1, 1)));
        assert_eq!(catalog.item_count(), 3, "nothing was lost in the exchange");
    }

    #[test]
    fn folders_swap_with_their_contents_and_names() {
        let mut catalog = catalog();
        catalog.rename_folder(1, Some("Intros")).unwrap();
        catalog.rename_folder(3, Some("Stingers")).unwrap();

        catalog.swap_folders(1, 3).unwrap();
        assert_eq!(catalog.folder(3).unwrap().name.as_deref(), Some("Intros"));
        assert_eq!(catalog.folder(1).unwrap().name.as_deref(), Some("Stingers"));
        assert_eq!(
            catalog.resolve(MediaAddress::new(1, 7)).unwrap().name,
            "Elsewhere"
        );
        assert_eq!(
            catalog.resolve(MediaAddress::new(3, 1)).unwrap().name,
            "First"
        );
    }

    #[test]
    fn folders_stay_in_index_order_however_they_were_built() {
        let mut catalog = CatalogSnapshot::default();
        catalog.insert(9, item(1, "Nine")).unwrap();
        catalog.insert(2, item(1, "Two")).unwrap();
        catalog.insert(5, item(1, "Five")).unwrap();
        let order: Vec<u8> = catalog.folders.iter().map(|folder| folder.folder).collect();
        assert_eq!(order, [2, 5, 9]);
    }

    #[test]
    fn items_stay_in_file_order_within_a_folder() {
        let mut catalog = CatalogSnapshot::default();
        for file in [7u8, 2, 30, 1] {
            catalog.insert(1, item(file, "Clip")).unwrap();
        }
        let order: Vec<u8> = catalog
            .folder(1)
            .unwrap()
            .items
            .iter()
            .map(|i| i.file)
            .collect();
        assert_eq!(order, [1, 2, 7, 30]);
    }

    #[test]
    fn nothing_may_be_stored_at_an_address_that_cannot_hold_it() {
        let mut catalog = CatalogSnapshot::default();
        assert_eq!(
            catalog.insert(1, item(0, "Blank")).unwrap_err(),
            CatalogError::NotAUsableFile { file: 0 }
        );
        assert_eq!(
            catalog.insert(1, item(255, "Blank")).unwrap_err(),
            CatalogError::NotAUsableFile { file: 255 }
        );
        assert_eq!(
            catalog.insert(200, item(1, "Text")).unwrap_err(),
            CatalogError::NotALibraryFolder { folder: 200 }
        );
        assert_eq!(
            catalog.insert(0, item(1, "Nowhere")).unwrap_err(),
            CatalogError::NotAUsableFile { file: 1 },
            "folder zero is blank on the wire"
        );
    }

    #[test]
    fn an_empty_name_is_refused() {
        let mut catalog = catalog();
        assert_eq!(
            catalog.insert(1, item(9, "  ")).unwrap_err(),
            CatalogError::EmptyName
        );
        let id = catalog.folder(1).unwrap().items[0].id;
        assert_eq!(
            catalog.rename_item(id, "").unwrap_err(),
            CatalogError::EmptyName
        );
    }

    #[test]
    fn every_edit_advances_the_revision_and_a_refused_one_does_not() {
        let mut catalog = catalog();
        let start = catalog.revision;

        let id = catalog.folder(1).unwrap().items[0].id;
        catalog.rename_item(id, "Changed").unwrap();
        assert!(catalog.revision > start);

        let after_rename = catalog.revision;
        let _ = catalog.rename_item(id, "");
        assert_eq!(
            catalog.revision, after_rename,
            "a refused edit publishes nothing"
        );
    }

    #[test]
    fn editing_something_that_is_not_there_reports_rather_than_inventing_it() {
        let mut catalog = catalog();
        let absent = AssetId::new();
        assert_eq!(
            catalog.rename_item(absent, "New").unwrap_err(),
            CatalogError::NoSuchItem
        );
        assert_eq!(
            catalog
                .move_item(absent, MediaAddress::new(1, 9))
                .unwrap_err(),
            CatalogError::NoSuchItem
        );
        assert_eq!(
            catalog.rename_folder(42, Some("Nope")).unwrap_err(),
            CatalogError::NoSuchFolder { folder: 42 }
        );
        assert!(!catalog.remove_item(absent));
    }

    #[test]
    fn moving_an_item_onto_its_own_address_is_a_no_op() {
        let mut catalog = catalog();
        let id = catalog.resolve(MediaAddress::new(1, 1)).unwrap().id;
        catalog.move_item(id, MediaAddress::new(1, 1)).unwrap();
        assert_eq!(catalog.address_of(id), Some(MediaAddress::new(1, 1)));
        assert_eq!(catalog.item_count(), 3);
    }

    #[test]
    fn the_whole_usable_range_is_addressable() {
        let mut catalog = CatalogSnapshot::default();
        for file in FIRST_FILE..=LAST_FILE {
            catalog.insert(199, item(file, "Clip")).unwrap();
        }
        assert_eq!(catalog.folder(199).unwrap().items.len(), 254);
        assert!(
            catalog
                .resolve(MediaAddress::new(199, FIRST_FILE))
                .is_some()
        );
        assert!(catalog.resolve(MediaAddress::new(199, LAST_FILE)).is_some());
    }
}
