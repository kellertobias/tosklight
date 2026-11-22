use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::time::{Duration, SystemTime};

use media_codec::container::{ClipHeader, ClipWriter};
use media_domain::{CatalogLocation, MediaAddress};
use serde_json::{Value, json};

use crate::{LibraryStorage, discover};

struct Library(PathBuf);

impl Library {
    fn new() -> Self {
        let path = std::env::temp_dir()
            .join("media-library-identities")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(path.join("001")).unwrap();
        Self(path)
    }
    fn put(&self, file: u8, name: &str) -> PathBuf {
        let path = self.0.join(format!("001/{file:03}-{name}.toskclip"));
        let mut writer = ClipWriter::new(
            Cursor::new(Vec::new()),
            ClipHeader {
                width: 4,
                height: 4,
                frame_count: 0,
                frame_rate: (25, 1),
                intrinsic_bpm: None,
            },
        )
        .unwrap();
        writer.write_frame(&[7; 16], 0).unwrap();
        std::fs::write(&path, writer.finish().unwrap().into_inner()).unwrap();
        path
    }
    fn metadata(&self, file: u8) -> PathBuf {
        self.0.join(format!("001/.metadata/{file:03}.json"))
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn unannotated_media_receives_one_durable_identity_before_publication() {
    let library = Library::new();
    library.put(1, "Opening");
    library.put(2, "Closing");
    let first = discover(&library.0).unwrap();
    let bytes = std::fs::read(library.metadata(1)).unwrap();
    let metadata: Value = serde_json::from_slice(&bytes).unwrap();
    let second = discover(&library.0).unwrap();
    for file in [1, 2] {
        assert_eq!(
            first.resolve(MediaAddress::new(1, file)).unwrap().id,
            second.resolve(MediaAddress::new(1, file)).unwrap().id
        );
    }
    assert_eq!(
        metadata["assetId"],
        first
            .resolve(MediaAddress::new(1, 1))
            .unwrap()
            .id
            .to_string()
    );
    assert_eq!(
        bytes,
        std::fs::read(library.metadata(1)).unwrap(),
        "a later scan does not rewrite metadata"
    );
}

#[test]
fn anchoring_preserves_notes_disabled_bpm_and_unknown_metadata() {
    let library = Library::new();
    library.put(1, "Attributed");
    std::fs::create_dir_all(library.metadata(1).parent().unwrap()).unwrap();
    let original = json!({"note":"CC BY: creator","enabled":false,"intrinsicBpm":123.5,"future":{"nested":[1,"keep"]}});
    std::fs::write(library.metadata(1), original.to_string()).unwrap();
    let catalog = discover(&library.0).unwrap();
    let actual: Value =
        serde_json::from_slice(&std::fs::read(library.metadata(1)).unwrap()).unwrap();
    for (key, value) in original.as_object().unwrap() {
        assert_eq!(&actual[key], value);
    }
    let item = catalog.resolve(MediaAddress::new(1, 1)).unwrap();
    assert!(!item.enabled);
    assert_eq!(item.intrinsic_bpm, Some(123.5));
    assert_eq!(item.note.as_deref(), Some("CC BY: creator"));
}

#[test]
fn copying_media_and_sidecar_with_changed_modification_time_preserves_identity() {
    let source = Library::new();
    let original_path = source.put(1, "Portable");
    let original = discover(&source.0)
        .unwrap()
        .resolve(MediaAddress::new(1, 1))
        .unwrap()
        .id;
    let target = Library::new();
    let copied = target.0.join("001/001-Portable.toskclip");
    std::fs::copy(original_path, &copied).unwrap();
    std::fs::create_dir_all(target.metadata(1).parent().unwrap()).unwrap();
    std::fs::copy(source.metadata(1), target.metadata(1)).unwrap();
    let before = std::fs::read(target.metadata(1)).unwrap();
    std::fs::File::options()
        .write(true)
        .open(copied)
        .unwrap()
        .set_times(
            std::fs::FileTimes::new()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(123456789)),
        )
        .unwrap();
    assert_eq!(
        discover(&target.0)
            .unwrap()
            .resolve(MediaAddress::new(1, 1))
            .unwrap()
            .id,
        original
    );
    assert_eq!(std::fs::read(target.metadata(1)).unwrap(), before);
}

#[test]
fn identities_follow_move_swap_and_parking_across_rediscovery() {
    let library = Library::new();
    library.put(1, "First");
    library.put(2, "Second");
    let storage = LibraryStorage::new(&library.0);
    let mut catalog = discover(&library.0).unwrap();
    let first = catalog.resolve(MediaAddress::new(1, 1)).unwrap().id;
    let second = catalog.resolve(MediaAddress::new(1, 2)).unwrap().id;
    storage
        .move_item(&mut catalog, first, CatalogLocation::new(2, 7))
        .unwrap();
    let mut catalog = discover(&library.0).unwrap();
    assert_eq!(catalog.location_of(first), Some(CatalogLocation::new(2, 7)));
    storage.swap_items(&mut catalog, first, second).unwrap();
    let mut catalog = discover(&library.0).unwrap();
    assert_eq!(catalog.location_of(first), Some(CatalogLocation::new(1, 2)));
    assert_eq!(
        catalog.location_of(second),
        Some(CatalogLocation::new(2, 7))
    );
    storage
        .move_item(&mut catalog, first, CatalogLocation::new(900, 8))
        .unwrap();
    let catalog = discover(&library.0).unwrap();
    assert_eq!(
        catalog.location_of(first),
        Some(CatalogLocation::new(900, 8))
    );
    assert_eq!(catalog.address_of(first), None);
    assert_eq!(
        catalog.location_of(second),
        Some(CatalogLocation::new(2, 7))
    );
}

#[test]
fn malformed_metadata_is_retained_and_cannot_publish_a_provisional_id() {
    let library = Library::new();
    library.put(1, "Original");
    std::fs::create_dir_all(library.metadata(1).parent().unwrap()).unwrap();
    let bytes = b"{broken original metadata";
    std::fs::write(library.metadata(1), bytes).unwrap();
    let error = discover(&library.0).unwrap_err().to_string();
    assert!(error.contains("cannot preserve the media identity"));
    assert!(error.contains("repair"));
    assert_eq!(std::fs::read(library.metadata(1)).unwrap(), bytes);
}

#[test]
fn competing_initial_scans_publish_the_same_anchored_identity() {
    let library = Library::new();
    library.put(1, "Shared");
    let barrier = Arc::new(Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let path = library.0.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                discover(&path)
                    .unwrap()
                    .resolve(MediaAddress::new(1, 1))
                    .unwrap()
                    .id
            })
        })
        .collect();
    let ids: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert!(ids.iter().all(|id| *id == ids[0]));
}

#[test]
fn duplicate_address_candidates_do_not_rotate_the_sidecar_between_scans() {
    let library = Library::new();
    library.put(1, "A");
    library.put(1, "B");
    let first = discover(&library.0).unwrap();
    let second = discover(&library.0).unwrap();
    assert_eq!(first.item_count(), 1);
    assert_eq!(first.resolve(MediaAddress::new(1, 1)).unwrap().name, "A");
    assert_eq!(
        first.resolve(MediaAddress::new(1, 1)).unwrap().id,
        second.resolve(MediaAddress::new(1, 1)).unwrap().id
    );
}

#[test]
fn an_unreadable_duplicate_does_not_hide_the_first_usable_clip() {
    let library = Library::new();
    let bad = library.put(1, "A");
    library.put(1, "B");
    std::fs::write(bad, b"not a usable clip").unwrap();
    assert_eq!(
        discover(&library.0)
            .unwrap()
            .resolve(MediaAddress::new(1, 1))
            .unwrap()
            .name,
        "B"
    );
}

#[test]
fn failed_replacement_publication_restores_exact_metadata_and_old_playable_bytes() {
    let library = Library::new();
    let destination = library.put(1, "Original");
    let old = discover(&library.0)
        .unwrap()
        .resolve(MediaAddress::new(1, 1))
        .unwrap()
        .id;
    let original = std::fs::read(&destination).unwrap();
    let metadata = std::fs::read(library.metadata(1)).unwrap();
    let missing = library.0.join("001/.missing-import-stage");
    assert!(super::publish(&missing, &destination, 1).is_err());
    assert_eq!(std::fs::read(&destination).unwrap(), original);
    assert_eq!(std::fs::read(library.metadata(1)).unwrap(), metadata);
    assert_eq!(
        discover(&library.0)
            .unwrap()
            .resolve(MediaAddress::new(1, 1))
            .unwrap()
            .id,
        old
    );
    let empty_destination = library.0.join("001/002-New.toskclip");
    assert!(super::publish(&missing, &empty_destination, 2).is_err());
    assert!(!empty_destination.exists());
    assert!(!library.metadata(2).exists());
}

#[test]
fn successful_replacement_rotates_identity_even_when_clip_bytes_are_identical() {
    let library = Library::new();
    let destination = library.put(1, "Original");
    let first = discover(&library.0)
        .unwrap()
        .resolve(MediaAddress::new(1, 1))
        .unwrap()
        .id;
    let staging = library.0.join("001/.converted-import");
    std::fs::copy(&destination, &staging).unwrap();
    let replacement = super::publish(&staging, &destination, 1).unwrap();
    assert_ne!(replacement, first);
    assert!(!staging.exists());
    let catalog = discover(&library.0).unwrap();
    assert_eq!(
        catalog.resolve(MediaAddress::new(1, 1)).unwrap().id,
        replacement
    );
    assert!(
        catalog.item(first).is_none(),
        "old playlist references never follow an address replacement"
    );
    assert_eq!(
        discover(&library.0)
            .unwrap()
            .resolve(MediaAddress::new(1, 1))
            .unwrap()
            .id,
        replacement
    );
}

#[test]
fn unavailable_identity_metadata_cannot_replace_an_existing_clip() {
    let library = Library::new();
    let destination = library.put(1, "Original");
    let id = discover(&library.0)
        .unwrap()
        .resolve(MediaAddress::new(1, 1))
        .unwrap()
        .id;
    let original = std::fs::read(&destination).unwrap();
    let metadata = std::fs::read(library.metadata(1)).unwrap();
    let staged = library.put(2, "Converted");
    let lock = library.metadata(1).with_extension("identity-lock");
    std::fs::create_dir(&lock).unwrap();
    assert!(super::publish(&staged, &destination, 1).is_err());
    std::fs::remove_dir(lock).unwrap();
    assert_eq!(std::fs::read(destination).unwrap(), original);
    assert_eq!(std::fs::read(library.metadata(1)).unwrap(), metadata);
    assert_eq!(
        discover(&library.0)
            .unwrap()
            .resolve(MediaAddress::new(1, 1))
            .unwrap()
            .id,
        id
    );
}

#[cfg(unix)]
#[test]
fn read_only_anchored_media_remains_playable_but_unanchored_media_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let library = Library::new();
    library.put(1, "Anchored");
    let id = discover(&library.0)
        .unwrap()
        .resolve(MediaAddress::new(1, 1))
        .unwrap()
        .id;
    let directory = library.metadata(1).parent().unwrap().to_path_buf();
    let bytes = std::fs::read(library.metadata(1)).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555)).unwrap();
    let anchored = discover(&library.0);
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        anchored
            .unwrap()
            .resolve(MediaAddress::new(1, 1))
            .unwrap()
            .id,
        id
    );
    assert_eq!(std::fs::read(library.metadata(1)).unwrap(), bytes);
    library.put(2, "Unanchored");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555)).unwrap();
    let probe = directory.join("permission-probe");
    let enforced = std::fs::write(&probe, b"probe").is_err();
    let result = discover(&library.0);
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    if enforced {
        assert!(result.unwrap_err().to_string().contains("writable"));
    } else {
        eprintln!(
            "read-only refusal requires filesystem permission enforcement; this process bypasses permission bits"
        );
    }
}
