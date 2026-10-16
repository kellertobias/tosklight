//! Exercise terminal transport through the actual shared GPU compositor.
use super::*;
use media_codec::container::{ClipHeader, ClipWriter};
use media_domain::catalog::{CatalogItem, ItemKind};
use media_domain::personality::LayerPersonality;
use media_domain::{AssetId, OnceEndState, OutputId, PresentationMode, SourceStatus};
use media_render::OutputRenderer;

#[test]
fn completed_once_hold_black_and_transparent_have_distinct_compositor_results() {
    let size = Size::new(16, 8);
    let root = std::env::temp_dir()
        .join("media-once-compositor")
        .join(AssetId::new().to_string());
    let storage = media_library::LibraryStorage::new(root.clone());
    let mut catalog = CatalogSnapshot::default();
    for (file, name, colour) in [
        (1, "Lower green", [0, 255, 0, 255]),
        (2, "Upper red", [255, 0, 0, 255]),
    ] {
        let id = AssetId::new();
        catalog
            .insert(
                1,
                CatalogItem {
                    id,
                    file,
                    name: name.into(),
                    kind: ItemKind::Video,
                    width: size.width,
                    height: size.height,
                    frames: Some(2),
                    duration_millis: None,
                    intrinsic_bpm: None,
                    note: None,
                    enabled: true,
                },
            )
            .unwrap();
        let path = storage.item_path(MediaAddress::new(1, file), name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut writer = ClipWriter::new(
            std::fs::File::create(path).unwrap(),
            ClipHeader {
                width: size.width,
                height: size.height,
                frame_count: 0,
                frame_rate: (25, 1),
                intrinsic_bpm: None,
            },
        )
        .unwrap();
        let payload = media_codec::encode(size.width, size.height, &colour.repeat(128)).unwrap();
        for index in 0..2 {
            writer.write_frame(&payload, index * 40_000).unwrap();
        }
        writer.finish().unwrap();
    }
    let gpu = Gpu::off_screen().expect("an adapter is available");
    let id = OutputId::new();
    let mut pipeline = LayerPipeline::new(&gpu, id, storage, size);
    let mut renderer =
        OutputRenderer::off_screen(&gpu, id, size, PresentationMode::DisplaySynchronized).unwrap();
    let mut loader = ClipLoader::new(1024 * 1024);
    let configuration = MediaConfiguration::default();
    let analysis = Analysis::default();
    let mut output = OutputState::new(id, LayerPersonality::TwoLayers);
    output.layers[0].address = MediaAddress::new(1, 1);
    output.layers[1].address = MediaAddress::new(1, 2);
    output.layers[0].source_status = SourceStatus::Ready;
    output.layers[1].source_status = SourceStatus::Ready;
    output.layers[0].dimmer = 1.0;
    output.layers[1].dimmer = 1.0;
    let mut draw = |output: &OutputState, millis| {
        let now = Timestamp::from_millis(millis);
        let prepared = pipeline.prepare(
            output,
            FrameContext {
                catalog: &catalog,
                configuration: &configuration,
                analysis: &analysis,
                now_unix_millis: 0,
                beat: 0.0,
                bpm: 120.0,
                beat_phase: 0.0,
                instruments: media_domain::Instruments::default(),
                seconds: millis as f32 / 1000.0,
                now,
                tempo: media_domain::OutputTempo::default(),
            },
            &mut loader,
        );
        let draws = pipeline.draws(output, &prepared);
        renderer.present(&draws, &output.master, None, now, None);
        (renderer.read_image(), prepared.statuses)
    };
    // Establish the exact lower-only reference through the same compositor.
    output.layers[1].dimmer = 0.0;
    let (lower, _) = draw(&output, 0);
    output.layers[1].dimmer = 1.0;
    let mut results = Vec::new();
    for (index, end_state) in [
        OnceEndState::Hold,
        OnceEndState::Black,
        OnceEndState::Transparent,
    ]
    .into_iter()
    .enumerate()
    {
        let start = 1000 + index as u64 * 1000;
        output.layers[1].play_mode = PlayMode::Once { end_state };
        draw(&output, start);
        let (pixels, statuses) = draw(&output, start + 200);
        assert!(
            statuses.contains(&(1, SourceStatus::Completed)),
            "{end_state:?}"
        );
        assert_eq!(
            output.layers[1].dimmer, 1.0,
            "completion does not mutate authored opacity"
        );
        results.push(pixels);
    }
    assert_ne!(results[0], lower, "Hold retains the upper movie");
    assert!(
        results[1]
            .chunks_exact(4)
            .all(|pixel| pixel[..3] == [0, 0, 0]),
        "Black must cover the lower layer with opaque black"
    );
    assert_eq!(
        results[2], lower,
        "Transparent must reveal the unchanged lower layer"
    );
    let _ = std::fs::remove_dir_all(root);
}
