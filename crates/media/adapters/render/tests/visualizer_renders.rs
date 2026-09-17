//! Every generated visualizer, compiled and drawn on a real device.
//!
//! These prove three things a unit test cannot: that each shader compiles on the backend this
//! machine actually has, that each one puts pixels somewhere, and that each one *responds* to
//! audio rather than ignoring the analysis it was handed.
//!
//! They run off-screen, so a software adapter is acceptable and no display is needed.

use media_domain::audio::{Analysis, BANDS, WAVEFORM_POINTS};
use media_domain::geometry::Size;
use media_domain::visualizer::{
    ALL_KINDS, VisualizerConfiguration, VisualizerKind, VisualizerParameters,
};
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
        instruments: media_domain::Instruments::default(),
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
        .as_chunks::<4>()
        .0
        .iter()
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
    // a Kaleidoscope or a Starfield on one frame. Triangular Net is deliberately not among them:
    // it is carried by the music and by nothing else, and standing still is what it is for.
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
        VisualizerKind::FlyingProps,
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
fn the_triangular_net_stands_still_in_a_silent_room() {
    // The one visualizer that does not read the clock. Nothing should move it but the music, which
    // also makes it the honest way to see that an input is dead: a still net means no sound.
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, OUTPUT);
    let quiet = silence();
    let parameters = VisualizerConfiguration::new(VisualizerKind::TriangularNet).parameters;

    let kind = VisualizerKind::TriangularNet;
    let first = draw(
        &gpu,
        &mut renderer,
        kind,
        &parameters,
        &frame(&quiet, 0.5, 0.0),
    );
    let mut last = first.clone();
    // Play silence at it for a while, a frame at a time, the way an output would.
    for step in 1..120 {
        let seconds = 0.5 + step as f32 / 60.0;
        last = draw(
            &gpu,
            &mut renderer,
            kind,
            &parameters,
            &frame(&quiet, seconds, 0.0),
        );
    }
    assert_eq!(
        first, last,
        "Triangular Net moved with nothing to move it: it must be the music that carries it"
    );
}

#[test]
fn the_triangular_net_is_carried_forward_by_the_beat() {
    // And the other half of it: given instruments striking, it has to actually go somewhere, or the
    // visualizer is a still picture with extra steps.
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, OUTPUT);
    let playing = loud();
    let parameters = VisualizerConfiguration::new(VisualizerKind::TriangularNet).parameters;
    let strike = media_domain::Instrument {
        level: 1.0,
        hit: 1.0,
    };
    let beating = media_domain::Instruments {
        kick: strike,
        snare: strike,
        hihat: strike,
    };
    let at = |seconds: f32| VisualizerFrame {
        instruments: beating,
        ..frame(&playing, seconds, 1.0)
    };

    let kind = VisualizerKind::TriangularNet;
    let first = draw(&gpu, &mut renderer, kind, &parameters, &at(0.5));
    let mut last = first.clone();
    for step in 1..120 {
        last = draw(
            &gpu,
            &mut renderer,
            kind,
            &parameters,
            &at(0.5 + step as f32 / 60.0),
        );
    }
    assert_ne!(
        first, last,
        "two seconds of kicks, snares and hi-hats left Triangular Net exactly where it started"
    );
}

#[test]
fn the_triangular_net_ignores_a_loud_room_with_no_beat_in_it() {
    // The thing that was wrong before: sustained energy moved it, so a held bass note kept it
    // rolling and the beat never showed. Loud analysis with nothing striking must leave it still.
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, OUTPUT);
    let droning = loud();
    let parameters = VisualizerConfiguration::new(VisualizerKind::TriangularNet).parameters;

    let kind = VisualizerKind::TriangularNet;
    let first = draw(
        &gpu,
        &mut renderer,
        kind,
        &parameters,
        &frame(&droning, 0.5, 0.0),
    );
    let mut last = first.clone();
    for step in 1..120 {
        last = draw(
            &gpu,
            &mut renderer,
            kind,
            &parameters,
            &frame(&droning, 0.5 + step as f32 / 60.0, 0.0),
        );
    }
    assert_eq!(
        first, last,
        "a loud drone with no instrument striking moved Triangular Net"
    );
}

#[test]
fn the_triangular_net_never_goes_blank_however_hard_it_is_kicked() {
    // The worst case the walk has to survive. At full reactivity a held kick raises the swells
    // well above the camera, so rays start inside the net, look up at peaks, and cross it several
    // times; a mistake in where the walk may stop turns that into an empty frame on a wall. Kick it
    // hard for a few seconds, a frame at a time, and every single frame has to show net.
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, OUTPUT);
    let playing = loud();
    let mut parameters = VisualizerConfiguration::new(VisualizerKind::TriangularNet).parameters;
    parameters.reactivity = 2.0;
    parameters.amount = 1.0;
    parameters.speed = 8.0;
    let strike = media_domain::Instrument {
        level: 1.0,
        hit: 1.0,
    };
    let pounding = media_domain::Instruments {
        kick: strike,
        snare: strike,
        hihat: strike,
    };

    let kind = VisualizerKind::TriangularNet;
    for step in 0..240 {
        let seconds = 0.5 + step as f32 / 60.0;
        let driven = VisualizerFrame {
            instruments: pounding,
            ..frame(&playing, seconds, 1.0)
        };
        let pixels = draw(&gpu, &mut renderer, kind, &parameters, &driven);
        assert!(
            brightness(&pixels) > 0.0,
            "Triangular Net rendered an empty frame {step} frames into being kicked"
        );
    }
}

/// How much of a frame something covers.
fn coverage(pixels: &[u8]) -> f64 {
    let chunks = pixels.as_chunks::<4>().0;
    chunks.iter().filter(|pixel| pixel[3] > 0).count() as f64 / chunks.len() as f64
}

fn flying_props(mode: u8) -> VisualizerParameters {
    let mut parameters = VisualizerConfiguration::new(VisualizerKind::FlyingProps).parameters;
    parameters.mode = mode;
    parameters
}

#[test]
fn flying_props_flies_each_pattern_its_own_way() {
    // Fly-through, Drift, Orbit and Rise each place the same props differently, and every one of
    // them has something on screen at an ordinary moment.
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, Size::new(160, 90));
    let quiet = silence();
    let kind = VisualizerKind::FlyingProps;
    let frames: Vec<Vec<u8>> = (0..4)
        .map(|mode| {
            draw(
                &gpu,
                &mut renderer,
                kind,
                &flying_props(mode),
                &frame(&quiet, 3.7, 0.0),
            )
        })
        .collect();
    for (mode, pixels) in frames.iter().enumerate() {
        assert!(coverage(pixels) > 0.0, "pattern {mode} shows no props");
    }
    for first in 0..4 {
        for second in first + 1..4 {
            assert_ne!(
                frames[first], frames[second],
                "patterns {first} and {second} fly identically"
            );
        }
    }
    // The pattern repeats every four, so a desk byte past the last pattern still flies.
    assert_eq!(
        frames[1],
        draw(
            &gpu,
            &mut renderer,
            kind,
            &flying_props(5),
            &frame(&quiet, 3.7, 0.0)
        )
    );
}

#[test]
fn flying_props_density_and_size_fill_more_of_the_picture() {
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, Size::new(160, 90));
    let quiet = silence();
    let kind = VisualizerKind::FlyingProps;
    let now = frame(&quiet, 3.7, 0.0);
    let mut sparse = flying_props(2);
    sparse.count = 3;
    let mut dense = flying_props(2);
    dense.count = 48;
    let sparse = coverage(&draw(&gpu, &mut renderer, kind, &sparse, &now));
    let dense = coverage(&draw(&gpu, &mut renderer, kind, &dense, &now));
    assert!(
        dense > sparse,
        "Density 48 covers {dense}, Density 3 {sparse}"
    );

    let mut small = flying_props(2);
    small.size = 0.05;
    let mut large = flying_props(2);
    large.size = 0.3;
    let small = coverage(&draw(&gpu, &mut renderer, kind, &small, &now));
    let large = coverage(&draw(&gpu, &mut renderer, kind, &large, &now));
    assert!(large > small, "Size 0.3 covers {large}, Size 0.05 {small}");
}

#[test]
fn flying_props_keep_their_shape_on_a_raw_live_input_and_a_pounding_beat() {
    // A live input reaches the shader unbounded -- a band of 931, energy near 5, a peak of 10 --
    // and a hard beat holds every instrument at full. The props may swell and flash, but every
    // frame must still show props and none may flood the picture.
    let gpu = gpu();
    let mut renderer = VisualizerRenderer::new(&gpu, OUTPUT);
    let raw = Analysis {
        waveform: vec![10.0; WAVEFORM_POINTS],
        spectrum: vec![931.0; BANDS],
        bass: 400.0,
        mid: 200.0,
        treble: 90.0,
        energy: 4.9,
        peak: 10.3,
    };
    let strike = media_domain::Instrument {
        level: 1.0,
        hit: 1.0,
    };
    let pounding = media_domain::Instruments {
        kick: strike,
        snare: strike,
        hihat: strike,
    };
    let mut parameters = flying_props(2);
    parameters.reactivity = 8.0;
    parameters.speed = 8.0;
    let kind = VisualizerKind::FlyingProps;
    let calm = coverage(&draw(
        &gpu,
        &mut renderer,
        kind,
        &parameters,
        &frame(&silence(), 0.5, 0.0),
    ));
    for step in 0..120 {
        let driven = VisualizerFrame {
            instruments: pounding,
            ..frame(&raw, 0.5 + step as f32 / 60.0, 1.0)
        };
        let covered = coverage(&draw(&gpu, &mut renderer, kind, &parameters, &driven));
        assert!(covered > 0.0, "an empty frame {step} frames into the beat");
        assert!(
            covered < (calm * 3.0).max(0.6),
            "the beat flooded the picture: {covered} against {calm} at rest"
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
