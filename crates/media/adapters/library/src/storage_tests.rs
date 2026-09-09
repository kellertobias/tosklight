use media_domain::MediaAddress;

use super::test_support::Library;
use super::*;

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
    let rediscovered = crate::discover(library.storage.root()).unwrap();
    assert_eq!(
        rediscovered.folder(2).unwrap().name.as_deref(),
        Some("Stingers"),
        "the portable folder name must survive a server restart"
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
        .update_generated_folder_presentation(250, Some(Some("Equalizers")), Some(Some("waveform")))
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
    let image = image::DynamicImage::new_rgb8(8, 4);
    let mut uploaded = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut uploaded, image::ImageFormat::Png)
        .unwrap();
    library
        .storage
        .set_custom_thumbnail(&library.catalog, id, uploaded.get_ref())
        .unwrap();
    let stored = std::fs::read(&thumbnail).unwrap();
    assert_eq!(image::load_from_memory(&stored).unwrap().width(), 128);

    library
        .storage
        .move_item(&mut library.catalog, id, MediaAddress::new(1, 9))
        .unwrap();

    assert!(!thumbnail.exists());
    let moved = library.storage.thumbnail_path(MediaAddress::new(1, 9));
    assert_eq!(std::fs::read(moved).unwrap(), stored);
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
fn disabling_preserves_metadata_and_delete_removes_every_item_artifact() {
    let mut library = Library::new("disable-delete");
    let id = library.add(1, 3, "Clip");
    library
        .storage
        .set_intrinsic_bpm(&mut library.catalog, id, Some(128.0))
        .unwrap();
    library
        .storage
        .set_notes(
            &mut library.catalog,
            &[LibraryNoteTarget::Item(id)],
            Some("Licensed"),
        )
        .unwrap();
    let source = library.storage.root().join("001/003-Clip.mov");
    std::fs::write(&source, b"source").unwrap();
    let thumbnail = library.storage.thumbnail_path(MediaAddress::new(1, 3));
    std::fs::create_dir_all(thumbnail.parent().unwrap()).unwrap();
    std::fs::write(&thumbnail, b"thumb").unwrap();

    library
        .storage
        .set_item_enabled(&mut library.catalog, id, false)
        .unwrap();
    let metadata = read_item_metadata(&library.storage.metadata_path(MediaAddress::new(1, 3)));
    assert_eq!(
        metadata.get("intrinsicBpm"),
        Some(&serde_json::json!(128.0))
    );
    assert_eq!(metadata.get("note"), Some(&serde_json::json!("Licensed")));
    assert_eq!(metadata.get("enabled"), Some(&serde_json::json!(false)));
    assert!(
        library
            .storage
            .item_path(MediaAddress::new(1, 3), "Clip")
            .exists()
    );

    library
        .storage
        .remove_item(&mut library.catalog, id)
        .unwrap();
    assert!(library.catalog.item(id).is_none());
    assert!(!source.exists());
    assert!(!thumbnail.exists());
    assert!(
        !library
            .storage
            .metadata_path(MediaAddress::new(1, 3))
            .exists()
    );
}

#[test]
fn bulk_enable_and_delete_apply_to_exact_stable_item_sets() {
    let mut library = Library::new("bulk-enable-delete");
    let first = library.add(1, 1, "First");
    let second = library.add(1, 4, "Second");
    let untouched = library.add(1, 7, "Untouched");

    library
        .storage
        .set_items_enabled(&mut library.catalog, &[first, second], false)
        .unwrap();
    assert!(!library.catalog.item(first).unwrap().1.enabled);
    assert!(!library.catalog.item(second).unwrap().1.enabled);
    assert!(library.catalog.item(untouched).unwrap().1.enabled);

    library
        .storage
        .remove_items(&mut library.catalog, &[first, second])
        .unwrap();
    assert!(library.catalog.item(first).is_none());
    assert!(library.catalog.item(second).is_none());
    assert!(library.catalog.item(untouched).is_some());
    assert_eq!(
        library.contents(1, 7, "Untouched").as_deref(),
        Some("Untouched")
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
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(&original).unwrap()).unwrap(),
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

#[test]
fn one_note_update_covers_media_and_folders_and_preserves_other_metadata() {
    let mut library = Library::new("notes");
    let id = library.add(1, 3, "Clip");
    library
        .storage
        .set_intrinsic_bpm(&mut library.catalog, id, Some(127.5))
        .unwrap();
    library
        .storage
        .set_notes(
            &mut library.catalog,
            &[LibraryNoteTarget::Item(id), LibraryNoteTarget::Folder(1)],
            Some("Licence: CC BY 4.0\nCreator: Example"),
        )
        .unwrap();

    let discovered = crate::discover(library.storage.root()).unwrap();
    assert_eq!(
        discovered.folder(1).unwrap().note.as_deref(),
        Some("Licence: CC BY 4.0\nCreator: Example")
    );
    let metadata_path = library.storage.metadata_path(MediaAddress::new(1, 3));
    let item: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&metadata_path).unwrap()).unwrap();
    assert_eq!(
        item["note"],
        serde_json::json!("Licence: CC BY 4.0\nCreator: Example")
    );
    assert_eq!(item["intrinsicBpm"], serde_json::json!(127.5));

    library
        .storage
        .move_item(&mut library.catalog, id, CatalogLocation::new(900, 3))
        .unwrap();
    assert_eq!(
        library.catalog.item(id).unwrap().1.note.as_deref(),
        Some("Licence: CC BY 4.0\nCreator: Example")
    );
    assert!(!metadata_path.exists());
    let metadata_path = library.storage.metadata_path(CatalogLocation::new(900, 3));
    let moved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&metadata_path).unwrap()).unwrap();
    assert_eq!(
        moved["note"],
        serde_json::json!("Licence: CC BY 4.0\nCreator: Example")
    );
    library
        .storage
        .set_notes(
            &mut library.catalog,
            &[LibraryNoteTarget::Item(id), LibraryNoteTarget::Folder(1)],
            None,
        )
        .unwrap();
    assert_eq!(library.catalog.folder(1).unwrap().note, None);
    let item: serde_json::Value =
        serde_json::from_slice(&std::fs::read(metadata_path).unwrap()).unwrap();
    assert!(item.get("note").is_none());
    assert_eq!(item["intrinsicBpm"], serde_json::json!(127.5));
}

#[test]
fn folder_notes_move_with_a_folder_swap() {
    let mut library = Library::new("folder-note-swap");
    library
        .storage
        .set_notes(
            &mut library.catalog,
            &[LibraryNoteTarget::Folder(1), LibraryNoteTarget::Folder(2)],
            Some("Shared licence"),
        )
        .unwrap();
    library
        .storage
        .set_notes(
            &mut library.catalog,
            &[LibraryNoteTarget::Folder(2)],
            Some("Second licence"),
        )
        .unwrap();

    library
        .storage
        .swap_folders(&mut library.catalog, 1, 2)
        .unwrap();

    assert_eq!(
        library.catalog.folder(1).unwrap().note.as_deref(),
        Some("Second licence")
    );
    assert_eq!(
        library.catalog.folder(2).unwrap().note.as_deref(),
        Some("Shared licence")
    );
}
