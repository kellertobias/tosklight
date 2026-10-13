//! MED-05: effect banks must not replace a running library session or reset its frame clock.
use super::*;
use media_codec::container::{ClipHeader, ClipWriter};
use media_domain::OutputId;
use media_domain::catalog::{CatalogItem, ItemKind};
use media_playback::{MediaLoader, loader::ClipLoader};

struct ObservedLoader {
    inner: ClipLoader,
    selected: Vec<media_domain::AssetId>,
    released: Vec<media_domain::AssetId>,
    frames: Vec<(media_domain::AssetId, usize)>,
}
impl MediaLoader for ObservedLoader {
    fn begin_selection(&mut self, asset: media_domain::AssetId) {
        self.selected.push(asset);
        self.inner.begin_selection(asset);
    }
    fn request_load(
        &mut self,
        asset: media_domain::AssetId,
        path: &std::path::Path,
    ) -> Result<Option<media_playback::loader::LoadedClip>, media_domain::SourceFailure> {
        self.inner.request_load(asset, path)
    }
    fn finish_selection(&mut self, asset: media_domain::AssetId) {
        self.inner.finish_selection(asset);
    }
    fn release_selection(&mut self, asset: media_domain::AssetId) {
        self.released.push(asset);
        self.inner.release_selection(asset);
    }
    fn request_frame(
        &mut self,
        asset: media_domain::AssetId,
        frame: usize,
        consumer: u64,
    ) -> Option<(usize, std::sync::Arc<[u8]>)> {
        self.frames.push((asset, frame));
        self.inner.request_frame(asset, frame, consumer)
    }
}

fn write_clip(storage: &media_library::LibraryStorage) -> CatalogSnapshot {
    let mut catalog = CatalogSnapshot::default();
    catalog
        .insert(
            1,
            CatalogItem {
                id: media_domain::AssetId::new(),
                file: 1,
                name: "Frame clock".into(),
                kind: ItemKind::Video,
                width: 16,
                height: 16,
                frames: Some(100),
                duration_millis: None,
                intrinsic_bpm: None,
                note: None,
                enabled: true,
            },
        )
        .unwrap();
    let path = storage.item_path(MediaAddress::new(1, 1), "Frame clock");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut writer = ClipWriter::new(
        std::fs::File::create(path).unwrap(),
        ClipHeader {
            width: 16,
            height: 16,
            frame_count: 0,
            frame_rate: (25, 1),
            intrinsic_bpm: None,
        },
    )
    .unwrap();
    for frame in 0..100u64 {
        let rgba = [frame as u8, 255 - frame as u8, 0, 255].repeat(256);
        writer
            .write_frame(&media_codec::encode(16, 16, &rgba).unwrap(), frame * 40_000)
            .unwrap();
    }
    writer.finish().unwrap();
    catalog
}

fn prepare(
    pipeline: &mut LayerPipeline,
    loader: &mut ObservedLoader,
    output: &OutputState,
    catalog: &CatalogSnapshot,
    configuration: &MediaConfiguration,
    millis: u64,
) {
    let resolved = crate::effect_banks::resolve_output(output, configuration);
    let analysis = media_domain::audio::Analysis::default();
    let frame = FrameContext {
        catalog,
        configuration,
        analysis: &analysis,
        now_unix_millis: 0,
        beat: 0.0,
        bpm: 120.0,
        beat_phase: 0.0,
        instruments: Default::default(),
        seconds: millis as f32 / 1000.0,
        now: Timestamp::from_millis(millis),
        tempo: Default::default(),
    };
    let prepared = pipeline.prepare(&resolved, frame, loader);
    assert!(
        prepared
            .layers
            .iter()
            .any(|layer| layer.index == 0 && matches!(layer.source, Slot::Media(0)))
    );
    assert!(
        pipeline.texture(Slot::Media(0)).is_some(),
        "actual decoded texture remains available"
    );
}

#[test]
fn effect_banks_keep_running_clip_frame_clock_and_paused_hold_with_real_library_pipeline() {
    let root = std::env::var_os("LIGHT_TMP_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../.artifacts/tmp")
        })
        .join(format!(
            "effect-continuity-{}",
            media_domain::AssetId::new()
        ));
    let storage = media_library::LibraryStorage::new(root.clone());
    let catalog = write_clip(&storage);
    let asset = catalog.resolve(MediaAddress::new(1, 1)).unwrap().id;
    let gpu = Gpu::off_screen().expect("real pipeline needs an adapter");
    let output_id = OutputId::new();
    let mut configuration = MediaConfiguration::default();
    configuration.outputs = vec![media_application::OutputConfiguration::new("Continuity")];
    configuration.outputs[0].id = output_id;
    let mut output = OutputState::new(output_id, media_domain::LayerPersonality::TwoLayers);
    output.layers[0].address = MediaAddress::new(1, 1);
    output.layers[0].play_mode = PlayMode::Loop;
    let original = output.layers[0].clone();
    let mut pipeline = LayerPipeline::new(&gpu, output_id, storage, Size::new(16, 16));
    let mut loader = ObservedLoader {
        inner: ClipLoader::new(16 * 1024 * 1024),
        selected: vec![],
        released: vec![],
        frames: vec![],
    };
    prepare(
        &mut pipeline,
        &mut loader,
        &output,
        &catalog,
        &configuration,
        0,
    );
    assert_eq!(loader.frames.last(), Some(&(asset, 0)));
    output.layers[0].effect_banks[0].select = 6;
    output.layers[0].effect_banks[0].strength = 1.0;
    let effected = crate::effect_banks::resolve_output(&output, &configuration);
    assert!(effected.layers[0].effects[0].enabled);
    assert!(effected.layers[0].effects[0].effect_type.is_some());
    prepare(
        &mut pipeline,
        &mut loader,
        &output,
        &catalog,
        &configuration,
        1200,
    );
    assert_eq!(
        loader.frames.last(),
        Some(&(asset, 30)),
        "effect On continues fixed25fps source time"
    );
    output.layers[0].effect_banks = Default::default();
    assert_eq!(
        output.layers[0], original,
        "effect Off restores every non-effect field"
    );
    prepare(
        &mut pipeline,
        &mut loader,
        &output,
        &catalog,
        &configuration,
        1960,
    );
    assert_eq!(
        loader.frames.last(),
        Some(&(asset, 49)),
        "effect Off does not restart the pass"
    );
    prepare(
        &mut pipeline,
        &mut loader,
        &output,
        &catalog,
        &configuration,
        4200,
    );
    assert_eq!(
        loader.frames.last(),
        Some(&(asset, 5)),
        "same pass wraps at its original4s boundary"
    );
    output.layers[0].play_mode = PlayMode::Pause;
    output.layers[0].effect_banks[0].select = 6;
    output.layers[0].effect_banks[0].strength = 1.0;
    prepare(
        &mut pipeline,
        &mut loader,
        &output,
        &catalog,
        &configuration,
        4300,
    );
    assert_eq!(loader.frames.last(), Some(&(asset, 5)));
    output.layers[0].effect_banks = Default::default();
    prepare(
        &mut pipeline,
        &mut loader,
        &output,
        &catalog,
        &configuration,
        9000,
    );
    assert_eq!(
        loader.frames.last(),
        Some(&(asset, 5)),
        "paused held source remains the same across effects"
    );
    assert_eq!(loader.selected, vec![asset]);
    assert!(
        loader.released.is_empty(),
        "effect changes do not release/reselect source"
    );
    output.layers[0].play_mode = PlayMode::Loop;
    prepare(
        &mut pipeline,
        &mut loader,
        &output,
        &catalog,
        &configuration,
        10_000,
    );
    prepare(
        &mut pipeline,
        &mut loader,
        &output,
        &catalog,
        &configuration,
        11_200,
    );
    assert_eq!(loader.frames.last(), Some(&(asset, 30)));
    output.layers[0].reset_trigger_id += 1;
    prepare(
        &mut pipeline,
        &mut loader,
        &output,
        &catalog,
        &configuration,
        11_240,
    );
    assert_eq!(
        loader.frames.last(),
        Some(&(asset, 0)),
        "real reset is an observable negative control"
    );
    std::fs::remove_dir_all(root).unwrap();
}
