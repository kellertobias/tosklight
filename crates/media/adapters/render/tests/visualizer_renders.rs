//! Every generated visualizer, compiled and drawn on a real device.
//!
//! These prove three things a unit test cannot: that each shader compiles on the backend this
//! machine actually has, that each one puts pixels somewhere, and that each one *responds* to
//! audio rather than ignoring the analysis it was handed.
//!
//! They run off-screen, so a software adapter is acceptable and no display is needed.

use media_domain::audio::{Analysis, BANDS, WAVEFORM_POINTS};
use media_domain::geometry::Size;
use media_domain::visualizer::{ALL_KINDS, VisualizerKind, VisualizerParameters};
use media_render::{Gpu, SourceTexture, VisualizerFrame, VisualizerRenderer};

const OUTPUT: Size = Size::new(64, 64);

fn gpu() -> Gpu {
    Gpu::off_screen().expect(
        "the visualizer renders need a GPU or software adapter; install a software Vulkan \
         driver (mesa-vulkan-drivers) on a machine with no GPU",
    )
}

/// Silence: every band and every sample at rest.
fn silence() -> Analysis {
    Analysis {
        waveform: vec![0.0; WAVEFORM_POINTS],
        spectrum: vec![0.0; BANDS],
        bass: 0.0,
        mid: 0.0,
        treble: 0.0,
        energy: 0.0,
        peak: 0.0,
    }
}

/// A loud, broadband moment with a beat landing on it.
fn loud() -> Analysis {
    Analysis {
        waveform: (0..WAVEFORM_POINTS)
            .map(|index| (index as f32 * 0.05).sin())
            .collect(),
        spectrum: (0..BANDS)
            .map(|index| 0.9 - index as f32 / BANDS as f32 * 0.5)
            .collect(),
        bass: 0.9,
        mid: 0.8,
        treble: 0.7,
        energy: 0.85,
        peak: 1.0,
    }
}

fn frame<'a>(analysis: &'a Analysis, seconds: f32, beat: f32) -> VisualizerFrame<'a> {
    VisualizerFrame {
        seconds,
        analysis,
        beat,
        bpm: 128.0,
        beat_phase: 0.0,
    }
}

/// Renders one visualizer and reads its texture back.
fn draw(
    gpu: &Gpu,
    renderer: &mut VisualizerRenderer,
    kind: VisualizerKind,
    parameters: &VisualizerParameters,
    frame: &VisualizerFrame<'_>,
) -> Vec<u8> {
    let texture = renderer
        .render(0, kind, parameters, frame)
        .unwrap_or_else(|error| panic!("{} did not render: {error}", kind.label()));
    read_back(gpu, texture)
}

fn read_back(gpu: &Gpu, texture: &SourceTexture) -> Vec<u8> {
    texture
        .read_rgba8(gpu)
        .expect("a generated source is readable")
}

/// How much light a frame carries, so two frames can be compared without asserting pixels that
/// depend on a hash function's exact output.
fn brightness(pixels: &[u8]) -> f64 {
    pixels
        .chunks_exact(4)
        .map(|pixel| {
            let alpha = f64::from(pixel[3]) / 255.0;
            (f64::from(pixel[0]) + f64::from(pixel[1]) + f64::from(pixel[2])) / 765.0 * alpha
        })
        .sum::<f64>()
        / (pixels.len() / 4) as f64
}

#[test]
fn every_visualizer_compiles_on_this_backend() {
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, OUTPUT);

    let failures: Vec<String> = renderer
        .validate()
        .into_iter()
        .filter_map(|(kind, outcome)| {
            outcome
                .err()
                .map(|error| format!("{}: {error}", kind.label()))
        })
        .collect();

    assert!(
        failures.is_empty(),
        "visualizers this backend cannot build:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_visualizer_draws_something_when_the_music_is_loud() {
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, OUTPUT);
    let analysis = loud();
    let frame = frame(&analysis, 1.25, 1.0);

    for kind in ALL_KINDS {
        let parameters = VisualizerParameters::default();
        let pixels = draw(&gpu, &mut renderer, kind, &parameters, &frame);
        assert!(
            brightness(&pixels) > 0.0,
            "{} rendered a completely empty frame on a loud input",
            kind.label()
        );
    }
}

#[test]
fn every_visualizer_answers_to_audio() {
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, OUTPUT);
    let quiet = silence();
    let busy = loud();
    let parameters = VisualizerParameters::default();

    for kind in ALL_KINDS {
        // The same instant, twice, differing only in what the analysis says. A visualizer that
        // produces identical pixels is not reacting, whatever it looks like on its own.
        let at_rest = draw(
            &gpu,
            &mut renderer,
            kind,
            &parameters,
            &frame(&quiet, 3.0, 0.0),
        );
        let driven = draw(
            &gpu,
            &mut renderer,
            kind,
            &parameters,
            &frame(&busy, 3.0, 1.0),
        );
        assert_ne!(
            at_rest,
            driven,
            "{} ignores the audio analysis entirely",
            kind.label()
        );
    }
}

#[test]
fn waveform_size_changes_the_live_trace_expansion() {
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, OUTPUT);
    let analysis = loud();
    let frame = frame(&analysis, 1.0, 0.0);
    let small = VisualizerParameters {
        size: 0.005,
        ..Default::default()
    };
    let large = VisualizerParameters {
        size: 0.1,
        ..Default::default()
    };
    let small = draw(
        &gpu,
        &mut renderer,
        VisualizerKind::WaveformOscilloscope,
        &small,
        &frame,
    );
    let large = draw(
        &gpu,
        &mut renderer,
        VisualizerKind::WaveformOscilloscope,
        &large,
        &frame,
    );
    assert_ne!(small, large, "Size must change rendered waveform pixels");
}

#[test]
fn a_visualizer_animates_without_audio() {
    // Several visualizers are driven by time as much as by sound; a silent room must not freeze
    // a Kaleidoscope or a Starfield on one frame.
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, OUTPUT);
    let quiet = silence();
    let parameters = VisualizerParameters::default();

    for kind in [
        VisualizerKind::Kaleidoscope,
        VisualizerKind::Starfield,
        VisualizerKind::ColorCycling,
        VisualizerKind::CrossingLines,
        VisualizerKind::RotatingShape,
        VisualizerKind::CityTunnel,
        VisualizerKind::GridLandscape,
        VisualizerKind::MatrixDigitalRain,
    ] {
        let early = draw(
            &gpu,
            &mut renderer,
            kind,
            &parameters,
            &frame(&quiet, 0.5, 0.0),
        );
        let later = draw(
            &gpu,
            &mut renderer,
            kind,
            &parameters,
            &frame(&quiet, 4.5, 0.0),
        );
        assert_ne!(
            early,
            later,
            "{} stands still in a silent room",
            kind.label()
        );
    }
}

#[test]
fn a_visualizer_renders_at_the_output_size_and_follows_a_resize() {
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, OUTPUT);
    let analysis = loud();
    let frame = frame(&analysis, 1.0, 0.0);
    let parameters = VisualizerParameters::default();

    let first = renderer
        .render(0, VisualizerKind::ColorCycling, &parameters, &frame)
        .expect("renders");
    assert_eq!(first.size(), OUTPUT);

    let larger = Size::new(128, 96);
    renderer.resize(larger);
    let second = renderer
        .render(0, VisualizerKind::ColorCycling, &parameters, &frame)
        .expect("renders after a resize");
    assert_eq!(
        second.size(),
        larger,
        "a generated source is output-sized, so a monitor change resizes it"
    );
}

#[test]
fn two_layers_running_the_same_visualizer_do_not_share_a_texture() {
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, OUTPUT);
    let analysis = loud();
    let parameters = VisualizerParameters::default();

    // Different instants on different layers: if they shared one target, the second render would
    // overwrite the first and both layers would show the same thing.
    let first = draw(
        &gpu,
        &mut renderer,
        VisualizerKind::ColorCycling,
        &parameters,
        &frame(&analysis, 0.0, 0.0),
    );
    renderer
        .render(
            1,
            VisualizerKind::ColorCycling,
            &parameters,
            &frame(&analysis, 5.0, 0.0),
        )
        .expect("renders");
    let first_again = read_back(&gpu, renderer.target(0).expect("layer zero has a target"));

    assert_eq!(
        first, first_again,
        "rendering layer two disturbed layer one's texture"
    );
}
