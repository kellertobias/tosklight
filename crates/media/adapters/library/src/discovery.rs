//! Reading the library off disk into a catalog snapshot.
//!
//! Discovery is deliberately forgiving about what it finds and strict about what it accepts. A
//! stray file, an unreadable clip, or a directory that is not a library folder is skipped with a
//! reason in the log rather than failing the scan — an operator with one bad file still gets a
//! working library.
//!
//! Identities are minted per scan. They become durable when the catalog is persisted, which is
//! what lets a rename keep its identity while a freshly discovered library gets fresh ones.

use std::path::{Path, PathBuf};

use media_codec::container::ClipReader;
use media_domain::catalog::{CatalogItem, CatalogSnapshot, ItemKind};
use media_domain::{AssetId, authored_tempo};

use crate::naming;

/// Why a library could not be scanned at all.
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("cannot read the library at {}: {source}", root.display())]
    Unreadable {
        root: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Scans a library root into a catalog snapshot.
///
/// A root that does not exist yet is an empty library, not a failure: a first run has no media.
pub fn discover(root: &Path) -> Result<CatalogSnapshot, DiscoveryError> {
    let mut catalog = CatalogSnapshot::default();
    if !root.exists() {
        return Ok(catalog);
    }

    let entries = std::fs::read_dir(root).map_err(|source| DiscoveryError::Unreadable {
        root: root.to_path_buf(),
        source,
    })?;

    let mut folders: Vec<(u8, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_owned();
            let folder = naming::parse_folder_directory(&name)?;
            entry.path().is_dir().then(|| (folder, entry.path()))
        })
        .collect();
    folders.sort_by_key(|(folder, _)| *folder);

    for (folder, path) in folders {
        // Folder zero is valid on disk — the contract is explicit that only DMX reads zero as
        // blank — but nothing in it is addressable, so it is not carried into the catalog.
        if folder == 0 {
            tracing::debug!(%folder, "folder 000 holds no addressable media");
            continue;
        }
        for item in scan_folder(&path) {
            if let Err(error) = catalog.insert(folder, item) {
                tracing::warn!(%folder, %error, "skipping an item the catalog will not accept");
            }
        }
        // A library from the legacy application is full of `.mp4` and `.png` files, which are
        // import formats rather than playback ones. Finding none of them playable and saying
        // nothing would leave an operator looking at an empty library with no idea why.
        let waiting = count_awaiting_import(&path);
        if waiting > 0 {
            tracing::warn!(
                %folder,
                files = waiting,
                path = %path.display(),
                "this folder holds files that are not normalised media, so they cannot be played \
                 yet; import them into the library to convert them"
            );
        }
        if let Some(name) = read_folder_name(&path) {
            let _ = catalog.rename_folder(folder, Some(&name));
        }
    }

    Ok(catalog)
}

fn scan_folder(path: &Path) -> Vec<CatalogItem> {
    let Ok(entries) = std::fs::read_dir(path) else {
        tracing::warn!(path = %path.display(), "cannot read folder");
        return Vec::new();
    };

    let mut items: Vec<CatalogItem> = entries
        .flatten()
        .filter_map(|entry| {
            let filename = entry.file_name().to_str()?.to_owned();
            let (file, name) = naming::parse_item_filename(&filename)?;
            read_item(&entry.path(), file, &name)
        })
        .collect();
    items.sort_by_key(|item| item.file);
    items
}

fn read_item(path: &Path, file: u8, name: &str) -> Option<CatalogItem> {
    let handle = std::fs::File::open(path)
        .inspect_err(|error| tracing::warn!(path = %path.display(), %error, "cannot open clip"))
        .ok()?;
    let reader = ClipReader::open(handle)
        .inspect_err(|error| tracing::warn!(path = %path.display(), %error, "not a usable clip"))
        .ok()?;
    let header = reader.header();

    Some(CatalogItem {
        id: AssetId::new(),
        file,
        name: name.to_owned(),
        // A single frame is a still; anything longer has a timeline.
        kind: if header.frame_count <= 1 {
            ItemKind::Image
        } else {
            ItemKind::Video
        },
        width: header.width,
        height: header.height,
        frames: (header.frame_count > 1).then_some(header.frame_count),
        // The clip carries the tempo import parsed. The filename is consulted only as a fallback
        // for a clip written before the field existed; runtime never re-infers it.
        intrinsic_bpm: header
            .intrinsic_bpm
            .or_else(|| authored_tempo::from_filename(name)),
    })
}

/// How many files in a folder look like media this server could play once imported.
///
/// Counted rather than listed, because the point is to tell an operator that their library needs
/// importing — not to enumerate a venue's whole archive into a log.
fn count_awaiting_import(path: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .filter(|filename| {
            // A dotted file is the library's own bookkeeping, never media.
            !filename.starts_with('.')
                && naming::parse_item_filename(filename).is_none()
                && looks_like_media(filename)
        })
        .count()
}

/// Whether a filename is one of the formats an import accepts.
///
/// Deliberately a short list of what the legacy application actually held and what an operator
/// exports from an editor. Anything else in a folder is somebody's notes.
fn looks_like_media(filename: &str) -> bool {
    const IMPORTABLE: [&str; 8] = ["mp4", "mov", "m4v", "mkv", "png", "jpg", "jpeg", "tif"];
    filename
        .rsplit_once('.')
        .is_some_and(|(_, extension)| IMPORTABLE.contains(&extension.to_ascii_lowercase().as_str()))
}

/// A folder's operator-given name.
///
/// This build writes the name as the file's whole contents. The legacy application wrote a JSON
/// object — `{"name": "Video Files"}` — so both are read: an existing library must not come up with
/// a folder called `{ "name": ... }`.
fn read_folder_name(path: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(path.join(naming::FOLDER_NAME_FILE)).ok()?;
    let name = match serde_json::from_str::<serde_json::Value>(&contents) {
        Ok(serde_json::Value::Object(document)) => document
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned(),
        // Anything that is not a JSON object is this build's own format: the name itself.
        _ => contents.trim().to_owned(),
    };
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use media_codec::container::{ClipHeader, ClipWriter};
    use media_domain::MediaAddress;

    use super::*;

    fn clip(frames: usize, bpm: Option<f64>) -> Vec<u8> {
        let mut writer = ClipWriter::new(
            Cursor::new(Vec::new()),
            ClipHeader {
                width: 1920,
                height: 1080,
                frame_count: 0,
                frame_rate: (25, 1),
                intrinsic_bpm: bpm,
            },
        )
        .unwrap();
        for index in 0..frames {
            writer
                .write_frame(&[7u8; 16], index as u64 * 40_000)
                .unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    struct Library(PathBuf);

    impl Library {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir()
                .join("media-library-discovery")
                .join(name);
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }

        fn put(&self, relative: &str, bytes: &[u8]) {
            let path = self.0.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
    }

    impl Drop for Library {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_library_is_discovered_into_addressable_items() {
        let library = Library::new("basic");
        library.put("001/001-First.toskclip", &clip(10, None));
        library.put("001/007-Seventh.toskclip", &clip(10, None));
        library.put("003/002-Elsewhere.toskclip", &clip(1, None));

        let catalog = discover(&library.0).unwrap();
        assert_eq!(catalog.item_count(), 3);
        assert_eq!(
            catalog.resolve(MediaAddress::new(1, 1)).unwrap().name,
            "First"
        );
        assert_eq!(
            catalog.resolve(MediaAddress::new(1, 7)).unwrap().name,
            "Seventh"
        );
        assert_eq!(
            catalog.resolve(MediaAddress::new(3, 2)).unwrap().name,
            "Elsewhere"
        );
    }

    #[test]
    fn a_single_frame_clip_is_a_still_and_a_longer_one_is_a_video() {
        let library = Library::new("kinds");
        library.put("001/001-Still.toskclip", &clip(1, None));
        library.put("001/002-Moving.toskclip", &clip(30, None));

        let catalog = discover(&library.0).unwrap();
        let still = catalog.resolve(MediaAddress::new(1, 1)).unwrap();
        assert_eq!(still.kind, ItemKind::Image);
        assert_eq!(still.frames, None);

        let video = catalog.resolve(MediaAddress::new(1, 2)).unwrap();
        assert_eq!(video.kind, ItemKind::Video);
        assert_eq!(video.frames, Some(30));
        assert_eq!(video.width, 1920);
    }

    #[test]
    fn the_tempo_in_the_clip_beats_the_one_in_the_filename() {
        let library = Library::new("tempo");
        library.put("001/001-Loop_BPM90.toskclip", &clip(10, Some(128.0)));
        library.put("001/002-Loop_BPM90.toskclip", &clip(10, None));

        let catalog = discover(&library.0).unwrap();
        assert_eq!(
            catalog
                .resolve(MediaAddress::new(1, 1))
                .unwrap()
                .intrinsic_bpm,
            Some(128.0),
            "the stored value is authoritative, not the name"
        );
        assert_eq!(
            catalog
                .resolve(MediaAddress::new(1, 2))
                .unwrap()
                .intrinsic_bpm,
            Some(90.0),
            "the filename is only a fallback for a clip written before the field existed"
        );
    }

    #[test]
    fn one_bad_file_does_not_lose_the_rest_of_the_library() {
        let library = Library::new("resilient");
        library.put("001/001-Good.toskclip", &clip(10, None));
        library.put("001/002-Corrupt.toskclip", b"this is not a clip");
        library.put("001/003-Also-Good.toskclip", &clip(10, None));

        let catalog = discover(&library.0).unwrap();
        assert_eq!(
            catalog.item_count(),
            2,
            "the corrupt file was skipped, not fatal"
        );
        assert!(catalog.resolve(MediaAddress::new(1, 1)).is_some());
        assert!(catalog.resolve(MediaAddress::new(1, 2)).is_none());
        assert!(catalog.resolve(MediaAddress::new(1, 3)).is_some());
    }

    #[test]
    fn things_that_are_not_library_content_are_ignored() {
        let library = Library::new("noise");
        library.put("001/001-Real.toskclip", &clip(10, None));
        library.put("001/notes.txt", b"not media");
        library.put("001/.DS_Store", b"junk");
        library.put("001/.thumbs/001-thumb.jpg", b"a thumbnail");
        library.put(".system/something.toskclip", &clip(10, None));
        library.put("readme.md", b"not a folder");

        let catalog = discover(&library.0).unwrap();
        assert_eq!(catalog.item_count(), 1);
        assert_eq!(catalog.folders.len(), 1);
    }

    #[test]
    fn folder_zero_is_valid_on_disk_but_holds_nothing_addressable() {
        let library = Library::new("folder-zero");
        library.put("000/001-Hidden.toskclip", &clip(10, None));
        library.put("001/001-Visible.toskclip", &clip(10, None));

        let catalog = discover(&library.0).unwrap();
        assert_eq!(catalog.item_count(), 1);
        assert!(catalog.folder(0).is_none());
        assert!(catalog.resolve(MediaAddress::new(1, 1)).is_some());
    }

    #[test]
    fn a_folder_name_is_read_from_its_info_file() {
        let library = Library::new("named");
        library.put("001/001-Clip.toskclip", &clip(10, None));
        library.put("001/.info", b"  Intros  \n");
        library.put("003/001-Clip.toskclip", &clip(10, None));

        let catalog = discover(&library.0).unwrap();
        assert_eq!(catalog.folder(1).unwrap().name.as_deref(), Some("Intros"));
        assert_eq!(catalog.folder(3).unwrap().name, None);
    }

    #[test]
    fn a_legacy_folder_name_is_read_from_the_json_the_c_application_wrote() {
        let library = Library::new("legacy-named");
        library.put("001/001-Clip.toskclip", &clip(10, None));
        library.put("001/.info", br#"{"name": "Video Files"}"#);
        // An object with no name in it is not a name, and must not become one.
        library.put("002/001-Clip.toskclip", &clip(10, None));
        library.put("002/.info", br#"{"colour": "blue"}"#);

        let catalog = discover(&library.0).unwrap();
        assert_eq!(
            catalog.folder(1).unwrap().name.as_deref(),
            Some("Video Files"),
            "an existing library must not come up with a folder called `{{ \"name\": ... }}`"
        );
        assert_eq!(catalog.folder(2).unwrap().name, None);
    }

    #[test]
    fn a_library_of_files_that_need_importing_is_counted_rather_than_passed_over_in_silence() {
        let library = Library::new("needs-import");
        // What a legacy installation actually holds.
        library.put("001/001.mp4", b"not a normalised clip");
        library.put("001/004-LoopTest.mp4", b"not a normalised clip");
        library.put("002/001-Unknown.png", b"not a normalised clip");
        library.put("002/notes.txt", b"somebody's notes");
        library.put("002/.thumbs/001-thumb.jpg", b"bookkeeping, not media");

        let catalog = discover(&library.0).unwrap();
        assert_eq!(catalog.item_count(), 0, "none of it is playable yet");
        assert_eq!(
            count_awaiting_import(&library.0.join("001")),
            2,
            "both clips are waiting"
        );
        assert_eq!(
            count_awaiting_import(&library.0.join("002")),
            1,
            "the notes and the thumbnail are not media"
        );
    }

    #[test]
    fn a_library_that_does_not_exist_yet_is_empty_rather_than_a_failure() {
        let catalog = discover(Path::new("/does/not/exist/media")).unwrap();
        assert_eq!(catalog.item_count(), 0);
        assert!(catalog.folders.is_empty());
    }

    #[test]
    fn discovery_is_stable_across_runs() {
        let library = Library::new("stable");
        for file in [5u8, 1, 3] {
            library.put(&format!("002/{file:03}-Clip.toskclip"), &clip(10, None));
        }

        let first = discover(&library.0).unwrap();
        let second = discover(&library.0).unwrap();
        let addresses = |catalog: &CatalogSnapshot| -> Vec<u8> {
            catalog
                .folder(2)
                .unwrap()
                .items
                .iter()
                .map(|item| item.file)
                .collect()
        };
        assert_eq!(
            addresses(&first),
            [1, 3, 5],
            "ordered by index, not by directory order"
        );
        assert_eq!(addresses(&first), addresses(&second));
    }
}
