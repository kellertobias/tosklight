use super::*;
use media_codec::container::{ClipHeader, ClipWriter};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct BpmBench {
    root: PathBuf,
    storage: LibraryStorage,
    catalog: CatalogSnapshot,
    sessions: LayerSessions,
    loader: ClipLoader,
    asset: AssetId,
    media: Vec<u8>,
}

fn test_workspace() -> PathBuf {
    let resolver =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../tools/artifact_paths.py");
    let output = std::process::Command::new("python3")
        .args([
            "-B",
            "-c",
            "import runpy,sys; print(runpy.run_path(sys.argv[1])['artifact_path']('LIGHT_TMP_DIR', 'TMP_ROOT'))",
        ])
        .arg(resolver)
        .output()
        .expect("run the repository artifact path resolver");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let base = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
    let parent = base.join("tl820-bpm-tests");
    std::fs::create_dir_all(&parent).unwrap();
    let root = parent.join(format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).expect("create an exclusive test workspace");
    eprintln!("TL820 test workspace: {}", root.display());
    root
}

impl BpmBench {
    fn new(header_bpm: Option<f64>) -> Self {
        let root = test_workspace();
        let storage = LibraryStorage::new(root.clone());
        let path = storage.item_path(media_domain::catalog::CatalogLocation::new(1, 1), "Movie");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut writer = ClipWriter::new(
            std::fs::File::create(&path).unwrap(),
            ClipHeader {
                width: 320,
                height: 180,
                frame_count: 0,
                frame_rate: (10, 1),
                intrinsic_bpm: header_bpm,
            },
        )
        .unwrap();
        for frame in 0..100 {
            writer.write_frame(&[1; 24], frame * 100_000).unwrap();
        }
        writer.finish().unwrap();
        let media = std::fs::read(path).unwrap();
        let catalog = media_library::discover(&root).unwrap();
        let asset = catalog.resolve(MediaAddress::new(1, 1)).unwrap().id;
        let sessions = LayerSessions::new(OutputId::new(), storage.clone());
        Self {
            root,
            storage,
            catalog,
            sessions,
            loader: ClipLoader::new(10_000_000),
            asset,
            media,
        }
    }

    fn correct(&mut self, bpm: Option<f64>) {
        self.storage
            .set_intrinsic_bpm(&mut self.catalog, self.asset, bpm)
            .unwrap();
    }

    fn show(&mut self, layer: &LayerState, millis: u64) -> usize {
        self.sessions
            .reconcile(
                0,
                layer,
                &self.catalog,
                &mut self.loader,
                Timestamp::from_millis(millis),
            )
            .unwrap()
            .frame
            .unwrap()
    }

    fn assert_file_unchanged_and_rediscovery(&self, bpm: Option<f64>) {
        let item = self.catalog.resolve(MediaAddress::new(1, 1)).unwrap();
        assert_eq!(item.id, self.asset);
        assert_eq!(
            std::fs::read(self.storage.item_path(MediaAddress::new(1, 1), &item.name)).unwrap(),
            self.media
        );
        let restarted = media_library::discover(&self.root).unwrap();
        assert_eq!(
            restarted
                .resolve(MediaAddress::new(1, 1))
                .unwrap()
                .intrinsic_bpm,
            bpm
        );
    }
}

impl Drop for BpmBench {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn synced() -> LayerState {
    LayerState {
        address: MediaAddress::new(1, 1),
        play_mode: PlayMode::LoopSynced,
        playback_bpm: Some(120),
        ..Default::default()
    }
}

#[test]
fn catalog_correction_drives_real_loader_when_header_has_no_tempo() {
    let mut bench = BpmBench::new(None);
    bench.correct(Some(60.0));
    bench.assert_file_unchanged_and_rediscovery(Some(60.0));
    assert_eq!(bench.show(&synced(), 0), 0);
    assert_eq!(
        bench.show(&synced(), 500),
        10,
        "catalog60 must retime target120 to twice speed"
    );
}

#[test]
fn catalog_correction_overrides_real_clip_header_tempo() {
    let mut bench = BpmBench::new(Some(120.0));
    bench.correct(Some(60.0));
    bench.assert_file_unchanged_and_rediscovery(Some(60.0));
    assert_eq!(bench.show(&synced(), 0), 0);
    assert_eq!(bench.show(&synced(), 500), 10);
}

#[test]
fn explicit_catalog_null_clears_embedded_tempo_without_modifying_clip() {
    let mut bench = BpmBench::new(Some(60.0));
    bench.correct(None);
    bench.assert_file_unchanged_and_rediscovery(None);
    assert_eq!(bench.show(&synced(), 0), 0);
    assert_eq!(
        bench.show(&synced(), 500),
        5,
        "cleared authored tempo uses documented ratio1"
    );
}

#[test]
fn active_catalog_correction_and_clear_preserve_playhead_and_peer_transport() {
    let mut bench = BpmBench::new(None);
    let layer = synced();
    let mut peer = synced();
    peer.play_mode = PlayMode::Loop;
    assert_eq!(bench.show(&layer, 0), 0);
    let peer_frame = |bench: &mut BpmBench, millis| {
        bench
            .sessions
            .reconcile(
                1,
                &peer,
                &bench.catalog,
                &mut bench.loader,
                Timestamp::from_millis(millis),
            )
            .unwrap()
            .frame
            .unwrap()
    };
    assert_eq!(peer_frame(&mut bench, 0), 0);
    assert_eq!(bench.show(&layer, 500), 5);
    bench.correct(Some(60.0));
    assert_eq!(
        bench.show(&layer, 500),
        5,
        "correction must not jump the existing playhead"
    );
    assert_eq!(
        bench.show(&layer, 1000),
        15,
        "new rate applies only after the edit"
    );
    assert_eq!(
        peer_frame(&mut bench, 1000),
        10,
        "unsynced peer keeps its own ordinary transport"
    );
    bench.correct(None);
    assert_eq!(
        bench.show(&layer, 1000),
        15,
        "clearing also preserves the playhead"
    );
    assert_eq!(bench.show(&layer, 1500), 20);
    assert_eq!(peer_frame(&mut bench, 1500), 15);
    bench.assert_file_unchanged_and_rediscovery(None);
}

#[test]
fn corrected_tempo_survives_move_and_cold_rediscovery_without_rewriting_media() {
    let mut bench = BpmBench::new(Some(120.0));
    bench.correct(Some(60.0));
    bench
        .storage
        .move_item(&mut bench.catalog, bench.asset, MediaAddress::new(2, 3))
        .unwrap();
    let item = bench.catalog.resolve(MediaAddress::new(2, 3)).unwrap();
    assert_eq!(item.id, bench.asset);
    assert_eq!(
        std::fs::read(bench.storage.item_path(MediaAddress::new(2, 3), &item.name)).unwrap(),
        bench.media
    );
    bench.catalog = media_library::discover(&bench.root).unwrap();
    assert_eq!(
        bench
            .catalog
            .resolve(MediaAddress::new(2, 3))
            .unwrap()
            .intrinsic_bpm,
        Some(60.0)
    );
    let mut layer = synced();
    layer.address = MediaAddress::new(2, 3);
    assert_eq!(bench.show(&layer, 0), 0);
    assert_eq!(bench.show(&layer, 500), 10);
}

#[test]
fn unchanged_old_header_tempo_still_drives_synced_playback_without_metadata() {
    let mut bench = BpmBench::new(Some(60.0));
    assert_eq!(bench.show(&synced(), 0), 0);
    assert_eq!(bench.show(&synced(), 500), 10);
    bench.assert_file_unchanged_and_rediscovery(Some(60.0));
}
