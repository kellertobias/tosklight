//! What happens over a show's length, rather than over a frame.
//!
//! A media server runs for hours while an operator switches layers, and the failures that matter
//! at that scale are the ones a short test cannot see: a cache that grows past its budget, a pin
//! that is never released, a reader left open for a clip nothing is showing. These drive the real
//! session and loader hard enough for those to appear.
//!
//! They are deliberately bounded in wall-clock time so they can run on every commit. The number of
//! switches is what makes them meaningful, not the seconds.

use std::io::Cursor;
use std::path::PathBuf;

use media_codec::container::{ClipHeader, ClipWriter};
use media_domain::catalog::{CatalogItem, CatalogSnapshot, ItemKind};
use media_domain::{AssetId, LayerState, MediaAddress, OutputId, PlayMode, Timestamp};
use media_library::LibraryStorage;
use media_playback::{ClipLoader, LayerSessions};

/// Enough clips that switching between them evicts, and enough frames that each is worth caching.
const CLIPS: u8 = 8;
const FRAMES: usize = 24;
/// A budget that holds a few clips but not all of them, so eviction actually happens.
const BUDGET: u64 = 400_000;

struct Library {
    root: PathBuf,
    catalog: CatalogSnapshot,
}

impl Library {
    fn build(name: &str) -> Self {
        let root = std::env::temp_dir().join("media-endurance").join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a temporary library");

        let storage = LibraryStorage::new(root.clone());
        let mut catalog = CatalogSnapshot::default();
        for file in 1..=CLIPS {
            let id = AssetId::new();
            catalog
                .insert(
                    1,
                    CatalogItem {
                        id,
                        file,
                        name: format!("Clip {file}"),
                        kind: ItemKind::Video,
                        width: 320,
                        height: 180,
                        frames: Some(FRAMES as u32),
                        intrinsic_bpm: None,
                    },
                )
                .expect("a fresh address");

            let path = storage.item_path(MediaAddress::new(1, file), &format!("Clip {file}"));
            std::fs::create_dir_all(path.parent().expect("a folder")).expect("a folder");
            std::fs::write(path, clip()).expect("a clip on disk");
        }
        Self { root, catalog }
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A clip big enough that a few of them exceed the budget.
fn clip() -> Vec<u8> {
    let mut writer = ClipWriter::new(
        Cursor::new(Vec::new()),
        ClipHeader {
            width: 320,
            height: 180,
            frame_count: 0,
            frame_rate: (25, 1),
            intrinsic_bpm: None,
        },
    )
    .expect("a writer");
    for index in 0..FRAMES {
        writer
            .write_frame(&vec![index as u8; 4_096], index as u64 * 40_000)
            .expect("a frame");
    }
    writer.finish().expect("a finished clip").into_inner()
}

fn pointing_at(file: u8) -> LayerState {
    LayerState {
        address: MediaAddress::new(1, file),
        play_mode: PlayMode::Loop,
        ..Default::default()
    }
}

#[test]
fn thousands_of_layer_switches_leave_the_cache_inside_its_budget() {
    let library = Library::build("switching");
    let mut sessions =
        LayerSessions::new(OutputId::new(), LibraryStorage::new(library.root.clone()));
    let mut loader = ClipLoader::new(BUDGET);

    // Two layers switching independently, the way an operator building a look does.
    for step in 0..2_000u64 {
        for layer in 0..2usize {
            let file = ((step + layer as u64) % u64::from(CLIPS)) as u8 + 1;
            let state = pointing_at(file);
            let resolved = sessions.reconcile(
                layer,
                &state,
                &library.catalog,
                &mut loader,
                Timestamp::from_millis(step * 20),
            );
            assert!(resolved.is_some(), "every clip in the library resolves");
        }

        assert!(
            loader.cache().used() <= BUDGET,
            "the cache grew past its budget after {step} switches: {} bytes",
            loader.cache().used()
        );
    }
}

#[test]
fn a_layer_that_lets_go_of_a_clip_releases_its_pin() {
    let library = Library::build("pins");
    let mut sessions =
        LayerSessions::new(OutputId::new(), LibraryStorage::new(library.root.clone()));
    let mut loader = ClipLoader::new(BUDGET);
    let mut now = 0u64;

    for round in 0..500u64 {
        let file = (round % u64::from(CLIPS)) as u8 + 1;
        now += 20;
        sessions.reconcile(
            0,
            &pointing_at(file),
            &library.catalog,
            &mut loader,
            Timestamp::from_millis(now),
        );
    }

    // Nothing is selected any more, so nothing may still be pinned — a pin that outlived its
    // selection would make the cache unable to evict for the rest of a show.
    now += 20;
    sessions.reconcile(
        0,
        &LayerState::default(),
        &library.catalog,
        &mut loader,
        Timestamp::from_millis(now),
    );
    for entry in library.catalog.folders[0].items.iter() {
        assert!(
            !loader.cache().is_pinned(entry.id),
            "{} is still pinned after nothing selected it",
            entry.name
        );
    }
}

#[test]
fn a_clip_played_end_to_end_many_times_keeps_advancing() {
    let library = Library::build("long-play");
    let mut sessions =
        LayerSessions::new(OutputId::new(), LibraryStorage::new(library.root.clone()));
    let mut loader = ClipLoader::new(BUDGET);
    let state = pointing_at(1);

    // Twenty-four frames at 25 fps is under a second, so this is several hundred loops of it.
    let mut seen = std::collections::HashSet::new();
    for step in 0..5_000u64 {
        let resolved = sessions
            .reconcile(
                0,
                &state,
                &library.catalog,
                &mut loader,
                Timestamp::from_millis(step * 20),
            )
            .expect("it stays resolved");
        assert!(!resolved.status.is_failed(), "at step {step}");
        if let Some(frame) = resolved.frame {
            seen.insert(frame);
        }
    }

    assert_eq!(
        seen.len(),
        FRAMES,
        "a looping clip must reach every frame rather than settling on a few"
    );
}
