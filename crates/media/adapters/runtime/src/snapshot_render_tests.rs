use std::sync::Arc;

use arc_swap::ArcSwap;
use media_application::MediaConfiguration;
use media_application::configuration::{OutputConfiguration, OutputTarget};
use media_domain::geometry::Size;
use media_domain::personality::channels::{
    LAYER_CHANNELS, MASTER_CHANNELS, Resolution, layer, master,
};
use media_domain::{
    LayerPersonality, MediaAddress, MediaState, OutputState, SourceStatus, Timestamp,
};
use media_http::{SnapshotFailure, SnapshotRequest};

use super::{fitted, nothing_draws, render, snapshot_state};
use crate::presentation::Shared;

const LAYER: usize = 59;

fn home(specs: &[media_domain::personality::ChannelSpec]) -> Vec<u8> {
    let mut slots = vec![0; specs.len()];
    for spec in specs {
        let at = usize::from(spec.offset);
        match spec.resolution {
            Resolution::Byte => slots[at] = spec.default_value as u8,
            Resolution::Coarse => {
                slots[at] = (spec.default_value >> 8) as u8;
                slots[at + 1] = spec.default_value as u8;
            }
            Resolution::Fine => {}
        }
    }
    slots
}

/// Two layers at their home values followed by an open master.
fn slots() -> Vec<u8> {
    let mut slots = [home(LAYER_CHANNELS), home(LAYER_CHANNELS)].concat();
    let mut master_slots = home(MASTER_CHANNELS);
    master_slots[master::DIMMER] = 255;
    slots.extend(master_slots);
    slots
}

fn select(slots: &mut [u8], index: usize, folder: u8, file: u8, dimmer: u8) {
    let base = index * LAYER;
    slots[base + layer::FOLDER] = folder;
    slots[base + layer::FILE] = file;
    slots[base + layer::DIMMER] = dimmer;
}

fn request(
    output: media_domain::OutputId,
    layer: Option<usize>,
    slots: Vec<u8>,
) -> SnapshotRequest {
    SnapshotRequest {
        output,
        layer,
        slots,
        width: 64,
        height: 64,
    }
}

fn live() -> OutputState {
    let mut output = OutputState::new(media_domain::OutputId::new(), LayerPersonality::TwoLayers);
    output.layers[0].address = MediaAddress::new(3, 3);
    output.layers[0].source_status = SourceStatus::Ready;
    output
}

#[test]
fn the_snapshot_state_is_the_supplied_frame_not_the_live_one() {
    let live = live();
    let mut supplied = slots();
    select(&mut supplied, 1, 250, 2, 255);
    let state = snapshot_state(&live, &request(live.id, None, supplied), Timestamp::ZERO).unwrap();
    assert_eq!(state.layers[0].address, MediaAddress::new(0, 0));
    assert_eq!(state.layers[0].source_status, SourceStatus::default());
    assert_eq!(state.layers[1].address, MediaAddress::new(250, 2));
    assert!((state.layers[1].dimmer - 1.0).abs() < f32::EPSILON);
    assert!((state.master.dimmer - 1.0).abs() < f32::EPSILON);
    assert!(!nothing_draws(&state, None));
    assert!(nothing_draws(&state, Some(0)), "layer 1 selects nothing");
    assert!(!nothing_draws(&state, Some(1)));
}

#[test]
fn a_layer_snapshot_blanks_every_other_layer() {
    let live = live();
    let mut supplied = slots();
    select(&mut supplied, 0, 250, 1, 255);
    select(&mut supplied, 1, 250, 2, 255);
    let state =
        snapshot_state(&live, &request(live.id, Some(1), supplied), Timestamp::ZERO).unwrap();
    assert_eq!(state.layers[0].address, MediaAddress::new(0, 0));
    assert_eq!(state.layers[1].address, MediaAddress::new(250, 2));
}

#[test]
fn a_faded_master_or_short_frame_is_reported() {
    let live = live();
    let mut supplied = slots();
    select(&mut supplied, 0, 250, 1, 255);
    let master_dimmer = 2 * LAYER + master::DIMMER;
    supplied[master_dimmer] = 0;
    let state = snapshot_state(&live, &request(live.id, None, supplied), Timestamp::ZERO).unwrap();
    assert!(nothing_draws(&state, None), "a closed master draws nothing");
    assert!(
        !nothing_draws(&state, Some(0)),
        "a layer preview ignores the master"
    );

    let short = snapshot_state(&live, &request(live.id, None, vec![0; 10]), Timestamp::ZERO);
    assert!(matches!(short, Err(SnapshotFailure::Invalid(_))));
}

#[test]
fn a_snapshot_keeps_the_output_aspect_ratio_inside_the_requested_box() {
    assert_eq!(
        fitted(Size::new(1920, 1080), Size::new(240, 240)),
        Size::new(240, 135)
    );
    assert_eq!(
        fitted(Size::new(1080, 1920), Size::new(240, 135)),
        Size::new(75, 135)
    );
    assert_eq!(
        fitted(Size::new(64, 64), Size::new(64, 36)),
        Size::new(36, 36)
    );
}

fn shared(output: &OutputConfiguration, configuration: MediaConfiguration) -> Shared {
    let state = MediaState::with_outputs(vec![OutputState::new(output.id, output.personality)]);
    let root = std::env::temp_dir().join(format!("tosklight-snapshot-{}", output.id));
    Shared {
        state: Arc::new(ArcSwap::from_pointee(state)),
        catalog: Arc::new(ArcSwap::from_pointee(Default::default())),
        previews: crate::preview::SharedPreviews::configured(&configuration),
        configuration: Arc::new(ArcSwap::from_pointee(configuration)),
        analysis: Arc::new(ArcSwap::from_pointee(Default::default())),
        universe_inputs: crate::dmx::universe_inputs(),
        models: crate::model_store::Models::new(&root),
        speed_groups: crate::speed_groups::shared(),
    }
}

fn pixels(png_bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    let mut reader = decoder.read_info().unwrap();
    let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buffer).unwrap();
    buffer.truncate(info.buffer_size());
    (info.width, info.height, buffer)
}

/// Renders a real visualizer through the real pipeline on the test machine's adapter, the same
/// way the reference renders do.
#[test]
fn a_program_and_a_layer_snapshot_render_the_supplied_state_with_the_layer_transparent() {
    let gpu = media_render::Gpu::off_screen().expect(
        "snapshot renders need a GPU or software adapter; install a software Vulkan driver",
    );
    let mut output = OutputConfiguration::new("Snapshot");
    output.target = OutputTarget::OffScreen;
    output.personality = LayerPersonality::TwoLayers;
    output.resolution.width = 64;
    output.resolution.height = 64;
    let configuration = MediaConfiguration {
        outputs: vec![output.clone()],
        ..MediaConfiguration::default()
    };
    // Colour Cycling fills its whole layer without needing any audio.
    let visualizer = configuration
        .visualizers
        .entries
        .iter()
        .find(|entry| entry.configuration.kind == media_domain::VisualizerKind::ColorCycling)
        .expect("Colour Cycling ships with every server")
        .address;
    let shared = shared(&output, configuration);

    let mut supplied = slots();
    select(&mut supplied, 1, visualizer.folder, visualizer.file, 255);
    // Half size in both axes, so a layer picture has transparent margins.
    for axis in [layer::SCALE_X, layer::SCALE_Y] {
        supplied[LAYER + axis] = 0x40;
        supplied[LAYER + axis + 1] = 0;
    }
    let started = std::time::Instant::now();

    let program = render(
        &gpu,
        &shared,
        &request(output.id, None, supplied.clone()),
        started,
    )
    .unwrap();
    assert!(!program.empty);
    let (width, height, rgba) = pixels(&program.png);
    assert_eq!((width, height), (64, 64));
    assert!(
        rgba.chunks(4).all(|pixel| pixel[3] == 255),
        "the Program is opaque"
    );
    assert!(
        rgba.chunks(4).any(|pixel| pixel[..3] != [0, 0, 0]),
        "the visualizer draws into the Program"
    );

    let layer_picture = render(
        &gpu,
        &shared,
        &request(output.id, Some(1), supplied.clone()),
        started,
    )
    .unwrap();
    assert!(!layer_picture.empty);
    let (_, _, rgba) = pixels(&layer_picture.png);
    let corner = &rgba[0..4];
    assert_eq!(
        corner[3], 0,
        "outside the scaled layer the picture is transparent"
    );
    let centre = (32 * 64 + 32) * 4;
    assert!(rgba[centre + 3] > 0, "the layer itself is drawn");

    let empty = render(
        &gpu,
        &shared,
        &request(output.id, Some(0), supplied),
        started,
    )
    .unwrap();
    assert!(empty.empty, "layer 1 selects nothing");
    let (_, _, rgba) = pixels(&empty.png);
    assert!(rgba.chunks(4).all(|pixel| pixel[3] == 0));

    // The live output was never touched.
    let live = shared.state.load();
    assert_eq!(live.outputs[0].layers[1].address, MediaAddress::new(0, 0));
}
