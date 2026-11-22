//! Generated textures exercise runner command batches through the reducer and real compositor.
//! Readiness is acknowledged for the current reset generation after each batch; decoding and
//! disk I/O are outside these bounded transition cases.

use media_domain::geometry::Size;
use media_domain::playlist::{AuxiliaryLayer, AuxiliaryMode, LayerPreset, Playlist, PlaylistEntry};
use media_domain::{
    AssetId, CatalogItem, CatalogSnapshot, Command, CommandKind, CommandSource, ItemKind,
    LayerPersonality, MediaState, OutputId, OutputState, PresentationMode, SourceStatus, Timestamp,
    apply,
};
use media_render::{Gpu, LayerDraw, OutputRenderer, SourceTexture};

use super::runner::{PlaylistRunner, RunnerPhase, RunnerStatus};

const SIZE: Size = Size::new(16, 8);
const TOLERANCE: u8 = 2;

struct RenderBench {
    _gpu: Gpu,
    renderer: OutputRenderer,
    sources: Vec<SourceTexture>,
    state: MediaState,
    catalog: CatalogSnapshot,
    runner: PlaylistRunner,
    alpha: u8,
    overlay_reset: u32,
}

fn at(millis: u64) -> Timestamp {
    Timestamp::from_millis(millis)
}

fn pixel(file: u8, x: u32, alpha: u8) -> [u8; 4] {
    match file {
        1 | 2 if x < 8 => [180, 180, 180, alpha],
        1 => [255, 0, 0, alpha],
        2 => [0, 0, 255, alpha],
        3 if x >= 12 => [0, 255, 0, 255],
        3 => [0, 0, 0, 0],
        _ => panic!("unassigned generated texture"),
    }
}

fn submit(state: &mut MediaState, commands: Vec<CommandKind>, now: u64) {
    for kind in commands {
        let result = apply(
            state,
            &Command::new(kind, CommandSource::Scheduler, at(now)),
        );
        assert!(
            result.is_accepted(),
            "runner command was refused: {result:?}"
        );
    }
    // These textures already exist on the GPU. Acknowledge exactly the selections/reset
    // generations just submitted, rather than letting stale Ready state satisfy a preload.
    let output = &state.outputs[0];
    let ready: Vec<_> = output
        .layers
        .iter()
        .enumerate()
        .filter_map(|(layer, value)| {
            (!value.address.is_blank() && value.source_status == SourceStatus::Loading).then_some(
                CommandKind::ReportSourceStatus {
                    output: output.id,
                    layer,
                    status: SourceStatus::Ready,
                },
            )
        })
        .collect();
    for kind in ready {
        assert!(apply(state, &Command::new(kind, CommandSource::Internal, at(now))).is_accepted());
    }
}

impl RenderBench {
    fn new(alpha: u8, fade_millis: u64) -> Self {
        let gpu = Gpu::off_screen().expect("playlist rendering requires a GPU or software adapter");
        let output = OutputId::new();
        let renderer =
            OutputRenderer::off_screen(&gpu, output, SIZE, PresentationMode::DisplaySynchronized)
                .expect("a 16x8 render target is supported");
        let sources = (1..=3)
            .map(|file| {
                let pixels: Vec<_> = (0..SIZE.height)
                    .flat_map(|_| (0..SIZE.width).flat_map(|x| pixel(file, x, alpha)))
                    .collect();
                SourceTexture::from_rgba8(&gpu, SIZE, &pixels).expect("generated texture uploads")
            })
            .collect();
        let mut catalog = CatalogSnapshot::default();
        let mut ids = Vec::new();
        for file in 1..=3 {
            let id = AssetId::new();
            ids.push(id);
            catalog
                .insert(
                    1,
                    CatalogItem {
                        id,
                        file,
                        name: format!("Generated {file}"),
                        kind: ItemKind::Image,
                        width: SIZE.width,
                        height: SIZE.height,
                        frames: None,
                        duration_millis: None,
                        intrinsic_bpm: None,
                        note: None,
                        enabled: true,
                    },
                )
                .unwrap();
        }
        let playlist = Playlist {
            entries: ids[..2]
                .iter()
                .map(|id| PlaylistEntry {
                    id: Playlist::default().id,
                    asset_id: *id,
                    plays: 1,
                    hold_millis: Some(100),
                    fade_millis,
                })
                .collect(),
            auxiliary_layers: vec![AuxiliaryLayer {
                layer: 8,
                mode: AuxiliaryMode::Fixed,
                asset_id: Some(ids[2]),
                controls: LayerPreset::default(),
            }],
            ..Playlist::default()
        };
        let mut state = MediaState::with_outputs(vec![OutputState::new(
            output,
            LayerPersonality::EightLayers,
        )]);
        assert!(
            apply(
                &mut state,
                &Command::new(
                    CommandKind::SetSchedulerOwnership {
                        output,
                        active: true,
                        dmx_silence_millis: 10_000,
                    },
                    CommandSource::Scheduler,
                    at(0)
                )
            )
            .is_accepted()
        );
        let (runner, update) =
            PlaylistRunner::activate(&playlist, &state.outputs[0], &catalog, at(0), true).unwrap();
        submit(&mut state, update.commands, 0);
        let overlay_reset = state.outputs[0].layers[7].reset_trigger_id;
        let mut bench = Self {
            _gpu: gpu,
            renderer,
            sources,
            state,
            catalog,
            runner,
            alpha,
            overlay_reset,
        };
        assert_eq!(bench.tick(1).phase, RunnerPhase::Playing);
        // Prepare the second clip before its due time.
        bench.tick(2);
        bench
    }

    fn tick(&mut self, millis: u64) -> RunnerStatus {
        let update = self
            .runner
            .tick(&self.state.outputs[0], &self.catalog, at(millis));
        submit(&mut self.state, update.commands, millis);
        update.status
    }

    fn render(&mut self, millis: u64) -> Vec<u8> {
        let output = &self.state.outputs[0];
        let draws: Vec<_> = output
            .layers
            .iter()
            .filter(|layer| !layer.address.is_blank())
            .map(|state| LayerDraw {
                state,
                source: &self.sources[usize::from(state.address.file) - 1],
                mask: None,
            })
            .collect();
        self.renderer
            .present(&draws, &output.master, None, at(millis), None);
        self.renderer.read_image()
    }

    fn assert_overlay_unchanged(&self) {
        let overlay = &self.state.outputs[0].layers[7];
        assert_eq!(overlay.reset_trigger_id, self.overlay_reset);
        assert_eq!(overlay.address, media_domain::MediaAddress::new(1, 3));
        assert_eq!(overlay.dimmer, 1.0);
    }

    fn expected(&self, x: u32) -> [u8; 3] {
        let mut accumulated = [0u8; 3];
        for layer in &self.state.outputs[0].layers {
            if layer.address.is_blank() {
                continue;
            }
            let source = pixel(layer.address.file, x, self.alpha);
            let alpha = f32::from(source[3]) / 255.0 * layer.dimmer;
            for channel in 0..3 {
                accumulated[channel] = (f32::from(source[channel]) * alpha
                    + f32::from(accumulated[channel]) * (1.0 - alpha))
                    .round() as u8;
            }
        }
        accumulated
    }

    fn assert_pixels(&mut self, millis: u64, opaque: bool) {
        self.assert_overlay_unchanged();
        let image = self.render(millis);
        for x in [4, 10, 14] {
            let offset = ((4 * SIZE.width + x) * 4) as usize;
            let actual = &image[offset..offset + 4];
            let expected = self.expected(x);
            for channel in 0..3 {
                assert!(
                    actual[channel].abs_diff(expected[channel]) <= TOLERANCE,
                    "t={millis} x={x}: GPU {actual:?}, source-over reference {expected:?}"
                );
            }
            assert_eq!(actual[3], 255);
            if opaque && x == 4 {
                assert!(
                    actual[..3]
                        .iter()
                        .all(|value| value.abs_diff(180) <= TOLERANCE),
                    "opaque identical greys must not dip during a fade: {actual:?}"
                );
            }
            if x == 14 {
                assert_eq!(
                    actual,
                    &[0, 255, 0, 255],
                    "fixed upper overlay stays visible"
                );
            }
        }
    }
}

#[test]
fn rendered_opaque_fades_keep_brightness_in_both_layer_directions_and_preserve_overlay() {
    let mut bench = RenderBench::new(255, 20);
    bench.assert_pixels(2, true);
    for (time, phase, entry) in [
        (81, RunnerPhase::Fading, 1),
        (91, RunnerPhase::Fading, 1),
        (101, RunnerPhase::Playing, 1),
        (102, RunnerPhase::Playing, 1),
        (161, RunnerPhase::Fading, 0),
        (171, RunnerPhase::Fading, 0),
        (181, RunnerPhase::Playing, 0),
    ] {
        let status = bench.tick(time);
        assert_eq!(status.phase, phase, "time {time}");
        assert_eq!(status.entry_index, Some(entry));
        bench.assert_pixels(time, true);
    }
}

#[test]
fn rendered_transparent_fades_follow_ordinary_source_over_in_both_directions() {
    let mut bench = RenderBench::new(128, 20);
    for time in [2, 81, 91, 101, 102, 161, 171, 181] {
        bench.tick(time);
        bench.assert_pixels(time, false);
    }
}

#[test]
fn rendered_zero_duration_transitions_cut_to_each_next_clip_without_overlay_reset() {
    let mut bench = RenderBench::new(255, 0);
    for (time, entry, color) in [
        (100, 0, [255, 0, 0]),
        (101, 1, [0, 0, 255]),
        (102, 1, [0, 0, 255]),
        (200, 1, [0, 0, 255]),
        (201, 0, [255, 0, 0]),
    ] {
        let status = bench.tick(time);
        assert_eq!(status.phase, RunnerPhase::Playing);
        assert_eq!(status.entry_index, Some(entry));
        bench.assert_pixels(time, true);
        let image = bench.render(time);
        let offset = ((4 * SIZE.width + 10) * 4) as usize;
        assert_eq!(&image[offset..offset + 3], &color);
    }
}
