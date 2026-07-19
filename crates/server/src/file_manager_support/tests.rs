use super::*;

fn temporary_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("light-file-support-{name}-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn discovery_adapters_cover_macos_linux_and_windows_mount_layouts() {
    let root = temporary_root("mounts");
    fs::create_dir_all(root.join("direct")).unwrap();
    assert_eq!(
        discover_directories_under(&root, false),
        vec![root.join("direct")]
    );
    fs::create_dir_all(root.join("operator/usb")).unwrap();
    let nested = discover_directories_under(&root, true);
    assert!(nested.contains(&root.join("operator/usb")));

    let linux = linux_removable_mount_paths(
        "24 1 8:1 / /media/operator/TOUR\\040USB rw - vfat /dev/sdb1 rw\n25 1 8:2 / /run/media/operator/SECOND rw - exfat /dev/sdc1 rw\n26 1 8:3 / /mnt/internal rw - ext4 /dev/sda1 rw\n",
    );
    assert_eq!(
        linux,
        vec![
            PathBuf::from("/media/operator/TOUR USB"),
            PathBuf::from("/run/media/operator/SECOND"),
        ]
    );

    assert_eq!(
        windows_removable_drive_paths(b"E:\r\nnot-a-drive\r\nF:\r\n"),
        vec![PathBuf::from("E:\\"), PathBuf::from("F:\\")],
    );
    remove_permanent(&root).unwrap();
}

#[test]
fn keep_both_copy_and_replace_are_safe() {
    let root = temporary_root("conflicts");
    let source = root.join("source.txt");
    let target = root.join("target.txt");
    fs::write(&source, b"new").unwrap();
    fs::write(&target, b"old").unwrap();
    let copied = copy_or_move(&source, &target, false, false, ConflictChoice::KeepBoth).unwrap();
    let TransferOutcome::Completed(copied) = copied else {
        panic!("copy unexpectedly skipped")
    };
    assert_eq!(copied.file_name().unwrap(), "target copy.txt");
    assert_eq!(fs::read(&target).unwrap(), b"old");
    copy_or_move(&source, &target, false, false, ConflictChoice::Replace).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"new");
    remove_permanent(&root).unwrap();
}

#[test]
fn cross_root_move_verifies_nested_content_before_deleting_source() {
    let source_root = temporary_root("source");
    let target_root = temporary_root("target");
    let source = source_root.join("folder");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("show.txt"), b"verified payload").unwrap();
    let target = target_root.join("folder");
    copy_or_move(&source, &target, true, true, ConflictChoice::Error).unwrap();
    assert!(!source.exists());
    assert_eq!(
        fs::read(target.join("show.txt")).unwrap(),
        b"verified payload"
    );
    remove_permanent(&source_root).unwrap();
    remove_permanent(&target_root).unwrap();
}

#[test]
fn failed_cross_root_copy_never_deletes_the_source() {
    let source_root = temporary_root("failure-source");
    let target_root = temporary_root("failure-target");
    let source = source_root.join("show.txt");
    fs::write(&source, b"must survive").unwrap();
    let impossible = target_root.join("missing-parent/show.txt");
    assert!(copy_or_move(&source, &impossible, true, true, ConflictChoice::Error).is_err());
    assert_eq!(fs::read(&source).unwrap(), b"must survive");
    remove_permanent(&source_root).unwrap();
    remove_permanent(&target_root).unwrap();
}

#[test]
fn skip_conflicts_leave_both_items_unchanged() {
    let root = temporary_root("skip");
    let source = root.join("source.txt");
    let target = root.join("target.txt");
    fs::write(&source, b"source").unwrap();
    fs::write(&target, b"target").unwrap();
    assert_eq!(
        copy_or_move(&source, &target, true, false, ConflictChoice::Skip).unwrap(),
        TransferOutcome::Skipped(target.clone())
    );
    assert_eq!(fs::read(&source).unwrap(), b"source");
    assert_eq!(fs::read(&target).unwrap(), b"target");
    remove_permanent(&root).unwrap();
}

#[test]
fn raster_thumbnail_is_bounded_png() {
    let root = temporary_root("thumbnail");
    let source = root.join("image.png");
    DynamicImage::new_rgb8(640, 320).save(&source).unwrap();
    let bytes = thumbnail_png(&source, 64).unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    let decoded = image::load_from_memory(&bytes).unwrap();
    assert!(decoded.width() <= 64 && decoded.height() <= 64);
    remove_permanent(&root).unwrap();
}

#[test]
fn unix_hidden_adapter_uses_dotfile_convention() {
    let root = temporary_root("hidden");
    let visible = root.join("visible");
    let hidden = root.join(".hidden");
    fs::write(&visible, []).unwrap();
    fs::write(&hidden, []).unwrap();
    #[cfg(not(target_os = "windows"))]
    {
        assert!(!is_hidden(
            visible.file_name().unwrap(),
            &fs::metadata(&visible).unwrap()
        ));
        assert!(is_hidden(
            hidden.file_name().unwrap(),
            &fs::metadata(&hidden).unwrap()
        ));
    }
    remove_permanent(&root).unwrap();
}

#[test]
fn native_notes_never_create_sidecar_files() {
    let root = temporary_root("notes");
    let file = root.join("item.txt");
    fs::write(&file, b"item").unwrap();
    if native_notes_supported(&file) {
        write_native_note(&file, "operator note").unwrap();
        assert_eq!(
            read_native_note(&file).unwrap().as_deref(),
            Some("operator note")
        );
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        write_native_note(&file, "").unwrap();
        assert_eq!(read_native_note(&file).unwrap(), None);
    }
    remove_permanent(&root).unwrap();
}
