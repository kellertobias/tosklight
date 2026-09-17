//! The Outline effect on a real device (TL-426).
//!
//! A warm square on a dark ground has exactly one kind of edge, so every property of the
//! single Intensity control reads from three places: the square's inside, the ground far away,
//! and the square's border.

use media_domain::geometry::Size;
use media_domain::{
    EffectLibrary, EffectSlot, LayerState, MasterState, MediaAddress, OUTLINE_PRESET_SLOT,
    OutlineParameters, OutputId, PresentationMode, ScalingMode, SourceStatus, Timestamp,
};
use media_render::{Gpu, LayerDraw, OutputRenderer, SourceTexture};

const OUTPUT: Size = Size::new(64, 64);
const SQUARE: std::ops::Range<u32> = 20..44;
const INSIDE: [u8; 3] = [160, 120, 80];
const GROUND: [u8; 3] = [20, 20, 30];

struct Bench {
    _gpu: Gpu,
    renderer: OutputRenderer,
    source: SourceTexture,
}

impl Bench {
    fn new() -> Self {
        let gpu = Gpu::off_screen().expect("the effect renders need a GPU or software adapter");
        let renderer = OutputRenderer::off_screen(
            &gpu,
            OutputId::new(),
            OUTPUT,
            PresentationMode::DisplaySynchronized,
        )
        .expect("an off-screen 64x64 output is within every adapter's limits");
        let mut pixels = Vec::with_capacity((OUTPUT.width * OUTPUT.height * 4) as usize);
        for y in 0..OUTPUT.height {
            for x in 0..OUTPUT.width {
                let inside = SQUARE.contains(&x) && SQUARE.contains(&y);
                pixels.extend_from_slice(if inside { &INSIDE } else { &GROUND });
                pixels.push(255);
            }
        }
        let source = SourceTexture::from_rgba8(&gpu, OUTPUT, &pixels).expect("the square uploads");
        Self {
            _gpu: gpu,
            renderer,
            source,
        }
    }

    fn render(&mut self, effect: Option<EffectSlot>) -> Vec<u8> {
        let mut state = LayerState {
            address: MediaAddress::new(1, 1),
            source_status: SourceStatus::Ready,
            scaling_mode: ScalingMode::Stretch,
            ..Default::default()
        };
        if let Some(effect) = effect {
            state.effects[0] = effect;
        }
        let draw = LayerDraw {
            state: &state,
            source: &self.source,
            mask: None,
        };
        self.renderer.present(
            &[draw],
            &MasterState::default(),
            None,
            Timestamp::from_millis(0),
            None,
        );
        self.renderer.read_image()
    }
}

fn outline(parameters: OutlineParameters) -> EffectSlot {
    let mut effect = EffectSlot::outline();
    effect.parameters = parameters.as_array().to_vec();
    effect
}

fn at(intensity: f32) -> EffectSlot {
    outline(OutlineParameters {
        intensity,
        ..Default::default()
    })
}

fn rgb(image: &[u8], x: u32, y: u32) -> [u8; 3] {
    let index = ((y * OUTPUT.width + x) * 4) as usize;
    [image[index], image[index + 1], image[index + 2]]
}

fn close(actual: [u8; 3], expected: [u8; 3]) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(actual, expected)| actual.abs_diff(expected) <= 3)
}

/// Pixels on the square's left border row that read as bright line colour.
fn line_width(image: &[u8]) -> usize {
    (10..30)
        .filter(|x| rgb(image, *x, 32).iter().all(|channel| *channel > 220))
        .count()
}

#[test]
fn zero_intensity_is_the_untouched_picture() {
    let mut bench = Bench::new();
    let original = bench.render(None);
    assert_eq!(bench.render(Some(at(0.0))), original);
    let mut bypassed = at(1.0);
    bypassed.mix = 0.0;
    assert_eq!(bench.render(Some(bypassed)), original);
    assert!(close(rgb(&original, 32, 32), INSIDE));
}

#[test]
fn half_intensity_draws_lines_over_the_visible_picture() {
    let mut bench = Bench::new();
    let image = bench.render(Some(at(0.5)));
    assert!(
        close(rgb(&image, 32, 32), INSIDE),
        "the inside stays visible"
    );
    assert!(close(rgb(&image, 4, 4), GROUND), "the ground stays visible");
    assert!(line_width(&image) > 0, "the border is outlined in white");
}

#[test]
fn full_intensity_leaves_only_the_lines_on_black() {
    let mut bench = Bench::new();
    let image = bench.render(Some(at(1.0)));
    assert_eq!(rgb(&image, 32, 32), [0, 0, 0], "the inside is black");
    assert_eq!(rgb(&image, 4, 4), [0, 0, 0], "the ground is black");
    assert!(line_width(&image) > 0, "the border is still drawn");
}

#[test]
fn intensity_between_the_anchors_fades_monotonically() {
    let mut bench = Bench::new();
    let quarter = bench.render(Some(at(0.25)));
    let half = bench.render(Some(at(0.5)));
    let three_quarters = bench.render(Some(at(0.75)));
    // The lines are half in at a quarter, the picture half dark at three quarters.
    let border = |image: &[u8]| rgb(image, 20, 32)[1].max(rgb(image, 19, 32)[1]);
    assert!(
        border(&quarter) < border(&half),
        "lines fade in up to one half"
    );
    let inside = rgb(&three_quarters, 32, 32);
    assert!(
        inside[0] < INSIDE[0] - 40 && inside[0] > 40,
        "the picture darkens above one half: {inside:?}"
    );
}

#[test]
fn thickness_widens_the_line_and_hue_colours_it() {
    let mut bench = Bench::new();
    let thin = bench.render(Some(outline(OutlineParameters {
        intensity: 1.0,
        thickness: 1.0,
        ..Default::default()
    })));
    let thick = bench.render(Some(outline(OutlineParameters {
        intensity: 1.0,
        thickness: 8.0,
        ..Default::default()
    })));
    assert!(
        line_width(&thick) > line_width(&thin),
        "{} vs {}",
        line_width(&thick),
        line_width(&thin)
    );

    let green = bench.render(Some(outline(OutlineParameters {
        intensity: 1.0,
        thickness: 4.0,
        hue_degrees: 120.0,
        saturation: 1.0,
        ..Default::default()
    })));
    let brightest = (10..30)
        .map(|x| rgb(&green, x, 32))
        .max_by_key(|pixel| pixel[1])
        .expect("a row");
    assert!(brightest[1] > 220 && brightest[0] < 30 && brightest[2] < 30);
}

#[test]
fn low_sensitivity_ignores_soft_contrast() {
    let mut bench = Bench::new();
    let dull = bench.render(Some(outline(OutlineParameters {
        intensity: 1.0,
        sensitivity: 0.0,
        ..Default::default()
    })));
    let keen = bench.render(Some(outline(OutlineParameters {
        intensity: 1.0,
        sensitivity: 1.0,
        ..Default::default()
    })));
    assert!(line_width(&keen) >= line_width(&dull));
    let lit = |image: &[u8]| image.chunks(4).filter(|pixel| pixel[0] > 8).count();
    assert!(lit(&keen) > lit(&dull), "more sensitivity finds more edge");
}

#[test]
fn the_shipped_preset_outlines_over_the_picture_without_a_beat() {
    let preset = EffectLibrary::default()
        .resolve(OUTLINE_PRESET_SLOT)
        .expect("Outline ships")
        .effect
        .clone();
    let parameters = preset.outline_parameters().expect("an enabled Outline");
    assert_eq!(parameters.intensity, 0.5);
    assert_eq!(
        parameters.beat_depth, 0.0,
        "beat modulation is off by default"
    );
    let mut bench = Bench::new();
    let image = bench.render(Some(preset));
    assert!(close(rgb(&image, 32, 32), INSIDE));
    assert!(line_width(&image) > 0);
}
