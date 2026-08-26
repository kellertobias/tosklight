use std::{collections::BTreeMap, fs, path::PathBuf};

use axum::http::StatusCode;
use uuid::Uuid;

use super::browse::directory_entry_info;
use super::input_context::{FileInputAction, pending_file_action};
use super::operations::safe_name;
use super::paths::{ConfiguredRoot, configured_roots_from, confined};
use super::streaming::parse_range;
use super::text::{SaveText, read_text_document, save_text_document, text_revision};

fn temporary_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("light-file-manager-{}", Uuid::new_v4()));
    fs::create_dir_all(root.join("folder")).unwrap();
    fs::write(root.join("folder/note.txt"), b"hello").unwrap();
    root
}

#[test]
fn confinement_rejects_parent_and_symlink_escapes() {
    let root = temporary_root();
    assert_eq!(
        confined(&root, "", false).unwrap(),
        fs::canonicalize(&root).unwrap()
    );
    assert!(confined(&root, "folder/note.txt", false).is_ok());
    assert!(confined(&root, "../outside", false).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(std::env::temp_dir(), root.join("escape")).unwrap();
        assert!(confined(&root, "escape", false).is_err());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn directory_listing_skips_an_entry_that_disappears_after_enumeration() {
    let root = temporary_root();
    let transient = root.join("temporary.show-wal");
    fs::write(&transient, b"transient").unwrap();
    let item = fs::read_dir(&root)
        .unwrap()
        .map(Result::unwrap)
        .find(|item| item.file_name() == "temporary.show-wal")
        .unwrap();
    fs::remove_file(&transient).unwrap();

    assert!(
        directory_entry_info(
            &fs::canonicalize(&root).unwrap(),
            &fs::canonicalize(&root).unwrap(),
            item,
            true,
            false
        )
        .unwrap()
        .is_none()
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn a_listed_symlink_that_leaves_the_root_is_still_rejected() {
    let root = temporary_root();
    let outside = std::env::temp_dir().join(format!("tosklight-outside-{}", std::process::id()));
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
    let item = fs::read_dir(&root)
        .unwrap()
        .map(Result::unwrap)
        .find(|item| item.file_name() == "escape")
        .unwrap();

    assert!(
        directory_entry_info(
            &fs::canonicalize(&root).unwrap(),
            &fs::canonicalize(&root).unwrap(),
            item,
            true,
            false
        )
        .is_err(),
        "a link out of the root must not be listed even though ordinary entries skip resolution"
    );
    fs::remove_dir_all(&outside).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ranges_are_inclusive_at_the_http_boundary() {
    assert_eq!(parse_range(None, 10).unwrap(), (0, 10, StatusCode::OK));
    assert_eq!(
        parse_range(Some("bytes=2-4"), 10).unwrap(),
        (2, 5, StatusCode::PARTIAL_CONTENT)
    );
    assert_eq!(
        parse_range(Some("bytes=7-"), 10).unwrap(),
        (7, 10, StatusCode::PARTIAL_CONTENT)
    );
    assert_eq!(
        parse_range(Some("bytes=-3"), 10).unwrap(),
        (7, 10, StatusCode::PARTIAL_CONTENT)
    );
    assert!(parse_range(Some("bytes=10-12"), 10).is_err());
    assert_eq!(
        parse_range(Some("bytes=10-12"), 10).unwrap_err().status,
        StatusCode::RANGE_NOT_SATISFIABLE
    );
    assert!(parse_range(Some("bytes=0-1,4-5"), 10).is_err());
}

#[test]
fn portable_names_and_pending_file_keys_are_strict() {
    assert_eq!(safe_name(Some("Cue Notes.txt")).unwrap(), "Cue Notes.txt");
    for name in ["", ".", "..", "../escape", "CON", "name.", "bad\0name"] {
        assert!(
            safe_name(Some(name)).is_err(),
            "{name:?} should be rejected"
        );
    }
    assert_eq!(pending_file_action(" COPY "), Some(FileInputAction::Copy));
    assert_eq!(pending_file_action("MOVE"), Some(FileInputAction::Move));
    assert_eq!(pending_file_action("DELETE 2"), None);
}

#[test]
fn removable_roots_are_runtime_only_and_disappear_from_the_next_discovery_snapshot() {
    let default = PathBuf::from("/desk/shows");
    let removable = PathBuf::from("/media/operator/TOUR_USB");
    let attached = configured_roots_from(
        Vec::new(),
        default.clone(),
        BTreeMap::new(),
        vec![removable.clone()],
    );
    assert!(
        attached
            .iter()
            .any(|(root, runtime)| *runtime && root.path == removable)
    );
    assert!(
        attached
            .iter()
            .any(|(root, runtime)| !runtime && root.id == "shows")
    );

    let detached = configured_roots_from(Vec::new(), default, BTreeMap::new(), Vec::new());
    assert_eq!(detached.len(), 1);
    assert_eq!(detached[0].0.id, "shows");
    assert!(!detached[0].1);
}

#[test]
fn the_selected_audio_library_is_browsable_beside_shows() {
    let library = PathBuf::from("/Volumes/Show/Audio");
    let roots = configured_roots_from(
        Vec::new(),
        PathBuf::from("/desk/shows"),
        BTreeMap::from([
            (
                "default".to_string(),
                library.to_string_lossy().into_owned(),
            ),
            ("stage".to_string(), "/Volumes/Stage/Audio".to_string()),
            ("empty".to_string(), "  ".to_string()),
        ]),
        Vec::new(),
    );
    assert_eq!(
        roots
            .iter()
            .map(|(root, _)| (root.id.as_str(), root.label.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("shows", "Shows"),
            ("audio-library-default", "Audio Library"),
            ("audio-library-stage", "Audio Library · stage"),
        ]
    );
    // A configured library is desk configuration, not a removable volume.
    assert!(roots.iter().all(|(_, runtime)| !runtime));
    assert_eq!(roots[1].0.path, library);
}

#[test]
fn an_audio_library_that_repeats_a_configured_root_is_offered_only_once() {
    let shared = temporary_root();
    let configured = vec![ConfiguredRoot {
        id: "media".into(),
        label: "Media".into(),
        path: shared.clone(),
        icon: None,
    }];
    let roots = configured_roots_from(
        configured,
        PathBuf::from("/desk/shows"),
        BTreeMap::from([("default".to_string(), shared.to_string_lossy().into_owned())]),
        Vec::new(),
    );
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].0.id, "media");
    fs::remove_dir_all(shared).unwrap();
}

#[test]
fn text_revisions_identify_content_even_when_size_and_timestamp_could_match() {
    let first = text_revision(b"ABCD");
    let second = text_revision(b"WXYZ");
    assert_ne!(first, second);
    assert_eq!(first, text_revision(b"ABCD"));
    assert!(first.starts_with("sha256:"));
}

#[tokio::test]
async fn concurrent_saves_with_one_revision_have_exactly_one_winner() {
    let root = temporary_root();
    let original = read_text_document("test".into(), &root, "folder/note.txt".into()).unwrap();
    let first = save_text_document(
        "test".into(),
        &root,
        SaveText {
            path: "folder/note.txt".into(),
            text: "first writer".into(),
            revision: Some(original.revision.clone()),
        },
    );
    let second = save_text_document(
        "test".into(),
        &root,
        SaveText {
            path: "folder/note.txt".into(),
            text: "other writer".into(),
            revision: Some(original.revision),
        },
    );

    let (first, second) = tokio::join!(first, second);
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let conflict = first.err().or_else(|| second.err()).unwrap();
    assert_eq!(conflict.status, StatusCode::CONFLICT);
    let stored = fs::read_to_string(root.join("folder/note.txt")).unwrap();
    assert!(stored == "first writer" || stored == "other writer");
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn save_as_is_atomic_and_refuses_to_replace_an_existing_file() {
    let root = temporary_root();
    let created = save_text_document(
        "test".into(),
        &root,
        SaveText {
            path: "folder/copy.txt".into(),
            text: "copy".into(),
            revision: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(created.revision, text_revision(b"copy"));
    let conflict = save_text_document(
        "test".into(),
        &root,
        SaveText {
            path: "folder/copy.txt".into(),
            text: "replace".into(),
            revision: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(conflict.status, StatusCode::CONFLICT);
    assert_eq!(
        fs::read_to_string(root.join("folder/copy.txt")).unwrap(),
        "copy"
    );
    assert!(fs::read_dir(root.join("folder")).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".light-tmp")
    }));
    fs::remove_dir_all(root).unwrap();
}
