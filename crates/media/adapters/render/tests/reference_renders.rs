//! Deterministic still-image reference renders.
//!
//! Each test composites a known source at a known layer state and asserts the pixels that come
//! back. The program target is linear RGBA8 and the sources are flat colours, so the expected
//! values are exact rather than approximate, and they are the same on every backend.
//!
//! These run off-screen: no window, no display, and a software adapter is acceptable. They fail
//! loudly when no adapter answers at all, because a render that silently did not happen looks
//! exactly like one that rendered black.

use media_domain::geometry::Size;
use media_domain::{
    AnalogTvParameters, BlurParameters, DigitalTvParameters, EffectSlot, FeedbackMotion,
    FeedbackParameters, FlipMirror, KaleidoscopeParameters, LayerState, MaskSource, MaskState,
    MasterState, MediaAddress, OutputId, PresentationMode, ScalingMode, SourceStatus, Timestamp,
    Tint,
};
use media_render::{Gpu, LayerDraw, OutputRenderer, SourceTexture};
use std::path::Path;

const OUTPUT: Size = Size::new(64, 64);

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

/// A layer that has a source and is allowed to draw.
fn ready(state: LayerState) -> LayerState {
    LayerState {
        address: MediaAddress::new(1, 1),
        source_status: SourceStatus::Ready,
        scaling_mode: ScalingMode::Stretch,
        ..state
    }
}

struct Bench {
    pub gpu: Gpu,
    renderer: OutputRenderer,
}

impl Bench {
    fn new() -> Self {
        let gpu = Gpu::off_screen().expect(
            "the reference renders need a GPU or software adapter; install a software Vulkan \
             driver (mesa-vulkan-drivers) on a machine with no GPU",
        );
        let renderer = OutputRenderer::off_screen(
            &gpu,
            OutputId::new(),
            OUTPUT,
            PresentationMode::DisplaySynchronized,
        )
        .expect("an off-screen 64x64 output is within every adapter's limits");
        Self { gpu, renderer }
    }

    fn solid(&self, size: Size, colour: [u8; 4]) -> SourceTexture {
        SourceTexture::solid(&self.gpu, size, colour).expect("a solid source uploads")
    }

    fn render(&mut self, layers: &[LayerDraw<'_>], master: &MasterState) -> Image {
        self.render_at(layers, master, Timestamp::ZERO)
    }

    fn render_at(
        &mut self,
        layers: &[LayerDraw<'_>],
        master: &MasterState,
        now: Timestamp,
    ) -> Image {
        self.render_masked_at(layers, master, None, now)
    }

    fn render_masked(
        &mut self,
        layers: &[LayerDraw<'_>],
        master: &MasterState,
        master_mask: Option<&SourceTexture>,
    ) -> Image {
        self.render_masked_at(layers, master, master_mask, Timestamp::ZERO)
    }

    fn render_masked_at(
        &mut self,
        layers: &[LayerDraw<'_>],
        master: &MasterState,
        master_mask: Option<&SourceTexture>,
        now: Timestamp,
    ) -> Image {
        self.renderer.present(layers, master, master_mask, now);
        Image {
            pixels: self.renderer.read_image(),
        }
    }
}

fn patterned_source(gpu: &Gpu) -> SourceTexture {
    let mut pixels = Vec::with_capacity((OUTPUT.width * OUTPUT.height * 4) as usize);
    for y in 0..OUTPUT.height {
        for x in 0..OUTPUT.width {
            let checker = if (x / 8 + y / 8) % 2 == 0 { 48 } else { 208 };
            pixels.extend_from_slice(&[
                ((x * 255) / (OUTPUT.width - 1)) as u8,
                checker,
                ((y * 255) / (OUTPUT.height - 1)) as u8,
                255,
            ]);
        }
    }
    SourceTexture::from_rgba8(gpu, OUTPUT, &pixels).expect("the reference pattern uploads")
}

fn analog_state(parameters: AnalogTvParameters) -> LayerState {
    let mut effect = EffectSlot::analog_tv();
    effect.seed = 0x116;
    effect.parameters = parameters.as_array().to_vec();
    let mut effects: [EffectSlot; 4] = Default::default();
    effects[0] = effect;
    ready(LayerState {
        effects,
        ..Default::default()
    })
}

fn digital_state(parameters: DigitalTvParameters) -> LayerState {
    let mut effect = EffectSlot::digital_tv();
    effect.seed = 0x115;
    effect.parameters = parameters.as_array().to_vec();
    let mut effects: [EffectSlot; 4] = Default::default();
    effects[0] = effect;
    ready(LayerState {
        effects,
        ..Default::default()
    })
}

fn blur_state(amount: f32, enabled: bool) -> LayerState {
    let mut effect = EffectSlot::blur();
    effect.enabled = enabled;
    effect.parameters = BlurParameters { amount }.as_array().to_vec();
    let mut effects: [EffectSlot; 4] = Default::default();
    effects[0] = effect;
    ready(LayerState {
        effects,
        ..Default::default()
    })
}

fn kaleidoscope_state(repetitions: u8, angle_degrees: f32, enabled: bool) -> LayerState {
    let mut effect = EffectSlot::kaleidoscope();
    effect.enabled = enabled;
    effect.parameters = KaleidoscopeParameters {
        repetitions,
        angle_degrees,
    }
    .as_array()
    .to_vec();
    let mut effects: [EffectSlot; 4] = Default::default();
    effects[0] = effect;
    ready(LayerState {
        effects,
        ..Default::default()
    })
}

fn feedback_state(direction: FeedbackMotion, enabled: bool) -> LayerState {
    let mut effect = EffectSlot::feedback();
    effect.enabled = enabled;
    effect.seed = 0x310;
    effect.parameters = FeedbackParameters {
        amount: 0.9,
        motion: 1.0,
        direction,
    }
    .as_array()
    .to_vec();
    let mut effects: [EffectSlot; 4] = Default::default();
    effects[0] = effect;
    ready(LayerState {
        effects,
        ..Default::default()
    })
}

fn changed_pixels(left: &Image, right: &Image) -> usize {
    left.pixels
        .chunks_exact(4)
        .zip(right.pixels.chunks_exact(4))
        .filter(|(left, right)| left != right)
        .count()
}

fn write_evidence(name: &str, image: &Image) {
    let Ok(directory) = std::env::var("LIGHT_TMP_DIR") else {
        return;
    };
    let path = Path::new(&directory).join(name);
    std::fs::create_dir_all(&directory).expect("the configured artifact directory is writable");
    let file = std::fs::File::create(path).expect("the evidence image can be created");
    let mut encoder = png::Encoder::new(file, OUTPUT.width, OUTPUT.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("the PNG header is valid")
        .write_image_data(&image.pixels)
        .expect("the rendered pixels can be encoded");
}

#[test]
fn analog_tv_defaults_are_deterministic_and_zero_is_a_true_bypass() {
    let mut bench = Bench::new();
    let source = patterned_source(&bench.gpu);
    let plain = ready(LayerState::default());
    let zero = analog_state(AnalogTvParameters {
        curvature: 0.0,
        distortion: 0.0,
        image_grain: 0.0,
        glitching: 0.0,
    });
    let defaults = analog_state(AnalogTvParameters::default());
    let at = Timestamp::from_micros(12_345_678);
    let draw = |state| LayerDraw {
        state,
        source: &source,
        mask: None,
    };

    let untouched = bench.render_at(&[draw(&plain)], &MasterState::default(), at);
    let bypassed = bench.render_at(&[draw(&zero)], &MasterState::default(), at);
    assert_eq!(
        bypassed.pixels, untouched.pixels,
        "four zeroes are a bypass"
    );

    let first = bench.render_at(&[draw(&defaults)], &MasterState::default(), at);
    let repeated = bench.render_at(&[draw(&defaults)], &MasterState::default(), at);
    assert_eq!(
        first.pixels, repeated.pixels,
        "same seed and time, same frame"
    );
    assert!(
        changed_pixels(&first, &untouched) > 1_000,
        "the restrained defaults still read as a television"
    );
    assert_eq!(
        first.at(0, 0),
        BLACK,
        "curved invalid samples fade to black"
    );
    assert_eq!(
        first.at(63, 63),
        BLACK,
        "the sampler does not repeat an edge"
    );
    write_evidence("tl-116-source.png", &untouched);
    write_evidence("tl-116-analog-tv-defaults.png", &first);
}

#[test]
fn blur_changes_live_pixels_and_disabled_is_an_exact_bypass() {
    let mut bench = Bench::new();
    let source = patterned_source(&bench.gpu);
    let plain = ready(LayerState::default());
    let mild = blur_state(0.2, true);
    let strong = blur_state(1.0, true);
    let disabled = blur_state(1.0, false);
    let draw = |state| LayerDraw {
        state,
        source: &source,
        mask: None,
    };

    let clear = bench.render(&[draw(&plain)], &MasterState::default());
    let mild = bench.render(&[draw(&mild)], &MasterState::default());
    let strong = bench.render(&[draw(&strong)], &MasterState::default());
    let disabled = bench.render(&[draw(&disabled)], &MasterState::default());
    assert!(changed_pixels(&mild, &clear) > 500);
    assert!(changed_pixels(&strong, &mild) > 500);
    assert_eq!(disabled.pixels, clear.pixels);
}

#[test]
fn kaleidoscope_mirrors_repetitions_and_angle_live_with_exact_bypass() {
    let mut bench = Bench::new();
    let source = patterned_source(&bench.gpu);
    let plain = ready(LayerState::default());
    let one = kaleidoscope_state(1, 0.0, true);
    let two = kaleidoscope_state(2, 0.0, true);
    let six = kaleidoscope_state(6, 0.0, true);
    let angled = kaleidoscope_state(6, 37.0, true);
    let disabled = kaleidoscope_state(6, 37.0, false);
    let draw = |state| LayerDraw {
        state,
        source: &source,
        mask: None,
    };

    let clear = bench.render(&[draw(&plain)], &MasterState::default());
    let one = bench.render(&[draw(&one)], &MasterState::default());
    let two = bench.render(&[draw(&two)], &MasterState::default());
    let six = bench.render(&[draw(&six)], &MasterState::default());
    let angled = bench.render(&[draw(&angled)], &MasterState::default());
    let disabled = bench.render(&[draw(&disabled)], &MasterState::default());

    assert_eq!(one.pixels, clear.pixels, "one repetition is the source");
    assert!(changed_pixels(&two, &clear) > 500);
    assert!(changed_pixels(&six, &two) > 500);
    assert!(changed_pixels(&angled, &six) > 500);
    assert_eq!(disabled.pixels, clear.pixels, "bypass restores the source");
}

#[test]
fn feedback_retains_prior_frames_moves_them_and_clears_on_bypass() {
    let mut bench = Bench::new();
    let first = patterned_source(&bench.gpu);
    let next = bench.solid(OUTPUT, BLACK);
    let top = feedback_state(FeedbackMotion::Top, true);
    let rotate = feedback_state(FeedbackMotion::RotateRight, true);
    let bypassed = feedback_state(FeedbackMotion::Top, false);
    let plain = ready(LayerState::default());
    let draw = |state, source| LayerDraw {
        state,
        source,
        mask: None,
    };

    bench.render_at(
        &[draw(&top, &first)],
        &MasterState::default(),
        Timestamp::ZERO,
    );
    let top_trail = bench.render_at(
        &[draw(&top, &next)],
        &MasterState::default(),
        Timestamp::from_millis(100),
    );
    let clear = bench.render_at(
        &[draw(&bypassed, &next)],
        &MasterState::default(),
        Timestamp::from_millis(200),
    );
    let plain = bench.render_at(
        &[draw(&plain, &next)],
        &MasterState::default(),
        Timestamp::from_millis(300),
    );
    assert!(changed_pixels(&top_trail, &plain) > 1_000);
    assert_eq!(
        clear.pixels, plain.pixels,
        "bypass is the exact live source"
    );

    bench.render_at(
        &[draw(&rotate, &first)],
        &MasterState::default(),
        Timestamp::from_millis(400),
    );
    let rotated_trail = bench.render_at(
        &[draw(&rotate, &next)],
        &MasterState::default(),
        Timestamp::from_millis(500),
    );
    assert!(changed_pixels(&rotated_trail, &top_trail) > 500);
}

#[test]
fn analog_tv_parameters_have_independent_visual_endpoints() {
    let mut bench = Bench::new();
    let source = patterned_source(&bench.gpu);
    let plain = ready(LayerState::default());
    let at = Timestamp::from_micros(8_250_000);
    let baseline = bench.render_at(
        &[LayerDraw {
            state: &plain,
            source: &source,
            mask: None,
        }],
        &MasterState::default(),
        at,
    );
    let endpoint = |parameters: AnalogTvParameters, bench: &mut Bench, at| {
        let state = analog_state(parameters);
        bench.render_at(
            &[LayerDraw {
                state: &state,
                source: &source,
                mask: None,
            }],
            &MasterState::default(),
            at,
        )
    };
    let curvature = endpoint(
        AnalogTvParameters {
            curvature: 1.0,
            distortion: 0.0,
            image_grain: 0.0,
            glitching: 0.0,
        },
        &mut bench,
        at,
    );
    let distortion = endpoint(
        AnalogTvParameters {
            curvature: 0.0,
            distortion: 1.0,
            image_grain: 0.0,
            glitching: 0.0,
        },
        &mut bench,
        at,
    );
    let grain = endpoint(
        AnalogTvParameters {
            curvature: 0.0,
            distortion: 0.0,
            image_grain: 1.0,
            glitching: 0.0,
        },
        &mut bench,
        at,
    );
    assert!(changed_pixels(&curvature, &baseline) > 500);
    assert!(changed_pixels(&distortion, &baseline) > 500);
    assert!(changed_pixels(&grain, &baseline) > 500);
    assert_ne!(curvature.pixels, distortion.pixels);
    assert_ne!(distortion.pixels, grain.pixels);

    // Glitching is intermittent by definition. Search a bounded authoritative-time window and
    // prove that it produces a held analog event rather than pretending every frame must glitch.
    let glitch_parameters = AnalogTvParameters {
        curvature: 0.0,
        distortion: 0.0,
        image_grain: 0.0,
        glitching: 1.0,
    };
    let glitch = (0..120)
        .map(|frame| Timestamp::from_micros(frame * 166_667))
        .map(|time| endpoint(glitch_parameters, &mut bench, time))
        .find(|image| changed_pixels(image, &baseline) > 200)
        .expect("a full endpoint triggers within the bounded event schedule");
    assert_ne!(
        glitch.pixels, curvature.pixels,
        "analog tearing is not curvature"
    );
}

#[test]
fn digital_tv_defaults_are_deterministic_distinct_and_zero_is_a_true_bypass() {
    let mut bench = Bench::new();
    let source = patterned_source(&bench.gpu);
    let plain = ready(LayerState::default());
    let zero = digital_state(DigitalTvParameters {
        compression_damage: 0.0,
        block_size: 0.0,
        tile_displacement: 0.0,
        chroma_damage: 0.0,
        glitching: 0.0,
    });
    let digital = digital_state(DigitalTvParameters::default());
    let analog = analog_state(AnalogTvParameters::default());
    let at = Timestamp::from_micros(12_345_678);
    let draw = |state| LayerDraw {
        state,
        source: &source,
        mask: None,
    };

    let untouched = bench.render_at(&[draw(&plain)], &MasterState::default(), at);
    let bypassed = bench.render_at(&[draw(&zero)], &MasterState::default(), at);
    assert_eq!(
        bypassed.pixels, untouched.pixels,
        "five zeroes are a bypass"
    );
    let first = bench.render_at(&[draw(&digital)], &MasterState::default(), at);
    let repeated = bench.render_at(&[draw(&digital)], &MasterState::default(), at);
    let analog = bench.render_at(&[draw(&analog)], &MasterState::default(), at);
    assert_eq!(
        first.pixels, repeated.pixels,
        "same seed and time, same frame"
    );
    assert!(changed_pixels(&first, &untouched) > 500);
    assert_ne!(
        first.pixels, analog.pixels,
        "DVB-T blocks are not analog snow"
    );
    assert_ne!(first.at(0, 0), BLACK, "Digital TV does not curve the image");
    write_evidence("tl-115-digital-tv-defaults.png", &first);
    write_evidence("tl-115-analog-tv-defaults.png", &analog);
}

#[test]
fn digital_tv_parameters_have_independent_visual_endpoints() {
    let mut bench = Bench::new();
    let source = patterned_source(&bench.gpu);
    let baseline_state = ready(LayerState::default());
    let at = Timestamp::from_micros(8_250_000);
    let render = |parameters: DigitalTvParameters, bench: &mut Bench, at| {
        let state = digital_state(parameters);
        bench.render_at(
            &[LayerDraw {
                state: &state,
                source: &source,
                mask: None,
            }],
            &MasterState::default(),
            at,
        )
    };
    let baseline = bench.render_at(
        &[LayerDraw {
            state: &baseline_state,
            source: &source,
            mask: None,
        }],
        &MasterState::default(),
        at,
    );
    let endpoint = |index: usize, bench: &mut Bench| {
        let mut values = [0.0; 5];
        values[index] = 1.0;
        render(
            DigitalTvParameters {
                compression_damage: values[0],
                block_size: values[1],
                tile_displacement: values[2],
                chroma_damage: values[3],
                glitching: values[4],
            },
            bench,
            at,
        )
    };
    let compression = endpoint(0, &mut bench);
    let block_size = render(
        DigitalTvParameters {
            compression_damage: 1.0,
            block_size: 1.0,
            tile_displacement: 0.0,
            chroma_damage: 0.0,
            glitching: 0.0,
        },
        &mut bench,
        at,
    );
    let displaced = endpoint(2, &mut bench);
    let chroma = endpoint(3, &mut bench);
    assert!(changed_pixels(&compression, &baseline) > 500);
    assert_ne!(compression.pixels, block_size.pixels);
    assert!(changed_pixels(&displaced, &baseline) > 100);
    assert!(changed_pixels(&chroma, &baseline) > 500);

    // Glitching is held in rectangular event buckets. A bounded search proves one event, and two
    // frames in the same bucket reproduce the same damaged tile selection.
    let glitch_parameters = DigitalTvParameters {
        compression_damage: 0.0,
        block_size: 0.4,
        tile_displacement: 0.0,
        chroma_damage: 0.0,
        glitching: 1.0,
    };
    let (bucket_time, glitch) = (0..80)
        .map(|bucket| Timestamp::from_micros(bucket * 250_000 + 20_000))
        .map(|time| (time, render(glitch_parameters, &mut bench, time)))
        .find(|(_, image)| changed_pixels(image, &baseline) > 100)
        .expect("a full glitch endpoint triggers in the bounded schedule");
    let held = render(
        glitch_parameters,
        &mut bench,
        Timestamp::from_micros(bucket_time.as_micros() + 80_000),
    );
    assert_eq!(
        glitch.pixels, held.pixels,
        "corrupt tiles persist within a bucket"
    );
}

#[test]
fn digital_tv_respects_the_ordered_effect_stack() {
    let mut bench = Bench::new();
    let source = patterned_source(&bench.gpu);
    let at = Timestamp::from_micros(12_345_678);

    let mut analog = EffectSlot::analog_tv();
    analog.seed = 0x116;
    let mut digital = EffectSlot::digital_tv();
    digital.seed = 0x115;

    let render = |first: EffectSlot, second: EffectSlot, bench: &mut Bench| {
        let mut effects: [EffectSlot; 4] = Default::default();
        effects[0] = first;
        effects[1] = second;
        let state = ready(LayerState {
            effects,
            ..Default::default()
        });
        bench.render_at(
            &[LayerDraw {
                state: &state,
                source: &source,
                mask: None,
            }],
            &MasterState::default(),
            at,
        )
    };

    let analog_then_digital = render(analog.clone(), digital.clone(), &mut bench);
    let digital_then_analog = render(digital, analog, &mut bench);
    assert_ne!(
        analog_then_digital.pixels, digital_then_analog.pixels,
        "Digital TV processes the preceding slot instead of replacing the stack"
    );
}

struct Image {
    pixels: Vec<u8>,
}

impl Image {
    fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let index = (y as usize * OUTPUT.width as usize + x as usize) * 4;
        [
            self.pixels[index],
            self.pixels[index + 1],
            self.pixels[index + 2],
            self.pixels[index + 3],
        ]
    }

    fn center(&self) -> [u8; 4] {
        self.at(OUTPUT.width / 2, OUTPUT.height / 2)
    }
}

#[test]
fn an_output_with_no_layers_presents_black() {
    let mut bench = Bench::new();
    let image = bench.render(&[], &MasterState::default());
    assert_eq!(image.center(), BLACK);
    assert_eq!(image.at(0, 0), BLACK);
}

#[test]
fn a_stretched_layer_fills_the_whole_output() {
    let mut bench = Bench::new();
    let source = bench.solid(Size::new(4, 4), RED);
    let state = ready(LayerState::default());

    let image = bench.render(
        &[LayerDraw {
            state: &state,
            source: &source,
            mask: None,
        }],
        &MasterState::default(),
    );
    for (x, y) in [(0, 0), (63, 0), (0, 63), (63, 63), (32, 32)] {
        assert_eq!(image.at(x, y), RED, "pixel {x},{y}");
    }
}

#[test]
fn a_layer_that_does_not_draw_contributes_nothing_rather_than_black() {
    let mut bench = Bench::new();
    let source = bench.solid(Size::new(4, 4), RED);

    // Dimmer at zero.
    let dark = ready(LayerState {
        dimmer: 0.0,
        ..Default::default()
    });
    let image = bench.render(
        &[LayerDraw {
            state: &dark,
            source: &source,
            mask: None,
        }],
        &MasterState::default(),
    );
    assert_eq!(image.center(), BLACK);

    // Nothing selected.
    let blank = LayerState {
        address: MediaAddress::BLANK,
        source_status: SourceStatus::Ready,
        ..Default::default()
    };
    let image = bench.render(
        &[LayerDraw {
            state: &blank,
            source: &source,
            mask: None,
        }],
        &MasterState::default(),
    );
    assert_eq!(image.center(), BLACK);

    // A source that failed to load.
    let failed = LayerState {
        source_status: SourceStatus::Failed {
            failure: media_domain::SourceFailure::MissingFile,
        },
        ..ready(LayerState::default())
    };
    let image = bench.render(
        &[LayerDraw {
            state: &failed,
            source: &source,
            mask: None,
        }],
        &MasterState::default(),
    );
    assert_eq!(
        image.center(),
        BLACK,
        "a failed source draws transparent, never an error card"
    );
}

#[test]
fn later_layers_draw_above_earlier_ones() {
    let mut bench = Bench::new();
    let red = bench.solid(Size::new(4, 4), RED);
    let green = bench.solid(Size::new(4, 4), GREEN);
    let bottom = ready(LayerState::default());
    let top = ready(LayerState::default());

    let image = bench.render(
        &[
            LayerDraw {
                state: &bottom,
                source: &red,
                mask: None,
            },
            LayerDraw {
                state: &top,
                source: &green,
                mask: None,
            },
        ],
        &MasterState::default(),
    );
    assert_eq!(
        image.center(),
        GREEN,
        "layer 2 covers layer 1 where it is opaque"
    );
}

#[test]
fn a_half_dimmed_layer_blends_with_what_is_beneath_it() {
    let mut bench = Bench::new();
    let red = bench.solid(Size::new(4, 4), RED);
    let blue = bench.solid(Size::new(4, 4), BLUE);
    let bottom = ready(LayerState::default());
    let top = ready(LayerState {
        dimmer: 0.5,
        ..Default::default()
    });

    let image = bench.render(
        &[
            LayerDraw {
                state: &bottom,
                source: &red,
                mask: None,
            },
            LayerDraw {
                state: &top,
                source: &blue,
                mask: None,
            },
        ],
        &MasterState::default(),
    );
    let [red_channel, green_channel, blue_channel, _] = image.center();
    assert!(
        (127..=129).contains(&red_channel),
        "half the red survives: {red_channel}"
    );
    assert_eq!(green_channel, 0);
    assert!(
        (127..=129).contains(&blue_channel),
        "half the blue arrives: {blue_channel}"
    );
}

#[test]
fn a_layer_tint_multiplies_the_source() {
    let mut bench = Bench::new();
    let white = bench.solid(Size::new(4, 4), [255, 255, 255, 255]);
    let state = ready(LayerState {
        tint: Tint::new(1.0, 0.0, 0.0),
        ..Default::default()
    });

    let image = bench.render(
        &[LayerDraw {
            state: &state,
            source: &white,
            mask: None,
        }],
        &MasterState::default(),
    );
    assert_eq!(image.center(), RED);
}

#[test]
fn grayscale_uses_the_documented_luminance_weights() {
    let mut bench = Bench::new();
    let red = bench.solid(Size::new(4, 4), RED);
    let state = ready(LayerState {
        grayscale: 1.0,
        ..Default::default()
    });

    let image = bench.render(
        &[LayerDraw {
            state: &state,
            source: &red,
            mask: None,
        }],
        &MasterState::default(),
    );
    let [red_channel, green_channel, blue_channel, _] = image.center();
    // 0.299 of full scale is 76.
    for channel in [red_channel, green_channel, blue_channel] {
        assert!((75..=77).contains(&channel), "expected ~76, got {channel}");
    }
}

#[test]
fn the_master_dimmer_darkens_the_finished_composite() {
    let mut bench = Bench::new();
    let red = bench.solid(Size::new(4, 4), RED);
    let state = ready(LayerState::default());

    let image = bench.render(
        &[LayerDraw {
            state: &state,
            source: &red,
            mask: None,
        }],
        &MasterState {
            dimmer: 0.5,
            ..Default::default()
        },
    );
    let [red_channel, _, _, _] = image.center();
    assert!((127..=128).contains(&red_channel), "{red_channel}");
}

#[test]
fn the_master_tint_multiplies_the_finished_composite() {
    let mut bench = Bench::new();
    let white = bench.solid(Size::new(4, 4), [255, 255, 255, 255]);
    let state = ready(LayerState::default());

    let image = bench.render(
        &[LayerDraw {
            state: &state,
            source: &white,
            mask: None,
        }],
        &MasterState {
            tint: Tint::new(0.0, 1.0, 0.0),
            ..Default::default()
        },
    );
    assert_eq!(image.center(), GREEN);
}

#[test]
fn scaling_and_position_place_the_layer_where_the_geometry_says() {
    let mut bench = Bench::new();
    let red = bench.solid(Size::new(4, 4), RED);
    // A quarter-width, full-height bar pushed to the right-hand edge's inner half.
    let state = ready(LayerState {
        scale_x: 0.25,
        position_x: 0.75,
        ..Default::default()
    });

    let image = bench.render(
        &[LayerDraw {
            state: &state,
            source: &red,
            mask: None,
        }],
        &MasterState::default(),
    );
    // Centre 56, half-width 8: the bar covers x 48..64.
    assert_eq!(image.at(56, 32), RED, "inside the bar");
    assert_eq!(image.at(40, 32), BLACK, "left of the bar");
    assert_eq!(image.at(4, 32), BLACK, "far side of the output");
}

#[test]
fn the_master_flip_mirrors_the_output() {
    let mut bench = Bench::new();
    let red = bench.solid(Size::new(4, 4), RED);
    let state = ready(LayerState {
        scale_x: 0.25,
        position_x: -0.75,
        ..Default::default()
    });

    let unflipped = bench.render(
        &[LayerDraw {
            state: &state,
            source: &red,
            mask: None,
        }],
        &MasterState::default(),
    );
    assert_eq!(unflipped.at(8, 32), RED, "the bar starts on the left");
    assert_eq!(unflipped.at(56, 32), BLACK);

    let flipped = bench.render(
        &[LayerDraw {
            state: &state,
            source: &red,
            mask: None,
        }],
        &MasterState {
            flip_mirror: FlipMirror::Horizontal,
            ..Default::default()
        },
    );
    assert_eq!(
        flipped.at(56, 32),
        RED,
        "a horizontal flip moves it to the right"
    );
    assert_eq!(flipped.at(8, 32), BLACK);

    let vertical = ready(LayerState {
        scale_y: 0.25,
        position_y: -0.75,
        ..Default::default()
    });
    let flipped = bench.render(
        &[LayerDraw {
            state: &vertical,
            source: &red,
            mask: None,
        }],
        &MasterState {
            flip_mirror: FlipMirror::Vertical,
            ..Default::default()
        },
    );
    assert_eq!(
        flipped.at(32, 56),
        RED,
        "a vertical flip moves it to the bottom"
    );
    assert_eq!(flipped.at(32, 8), BLACK);
}

#[test]
fn rotation_turns_the_layer_around_its_own_centre() {
    let mut bench = Bench::new();
    let red = bench.solid(Size::new(4, 4), RED);
    // A horizontal bar across the middle: full width, a quarter height.
    let flat = ready(LayerState {
        scale_y: 0.25,
        ..Default::default()
    });
    let image = bench.render(
        &[LayerDraw {
            state: &flat,
            source: &red,
            mask: None,
        }],
        &MasterState::default(),
    );
    assert_eq!(image.at(4, 32), RED, "the bar reaches the left edge");
    assert_eq!(image.at(32, 4), BLACK, "and not the top");

    // A quarter turn makes it vertical without moving its centre.
    let turned = ready(LayerState {
        scale_y: 0.25,
        rotation: 90.0,
        ..Default::default()
    });
    let image = bench.render(
        &[LayerDraw {
            state: &turned,
            source: &red,
            mask: None,
        }],
        &MasterState::default(),
    );
    assert_eq!(image.at(32, 4), RED, "now it reaches the top");
    assert_eq!(image.at(4, 32), BLACK, "and not the left edge");
}

#[test]
fn recreating_an_output_changes_its_resolution_and_forgets_its_cadence() {
    let gpu = Gpu::off_screen().expect("an adapter is available");
    let mut renderer = OutputRenderer::off_screen(
        &gpu,
        OutputId::new(),
        OUTPUT,
        PresentationMode::DisplaySynchronized,
    )
    .unwrap();

    renderer.present(
        &[],
        &MasterState::default(),
        None,
        Timestamp::from_micros(0),
    );
    renderer.present(
        &[],
        &MasterState::default(),
        None,
        Timestamp::from_micros(16_667),
    );
    assert_eq!(renderer.cadence().frames, 2);

    renderer.recreate(Size::new(32, 16));
    assert_eq!(renderer.size(), Size::new(32, 16));
    assert_eq!(
        renderer.cadence().frames,
        0,
        "the cadence from before the change says nothing"
    );

    renderer.present(
        &[],
        &MasterState::default(),
        None,
        Timestamp::from_micros(0),
    );
    assert_eq!(renderer.read_image().len(), 32 * 16 * 4);
}

#[test]
fn two_outputs_on_one_device_render_independently() {
    let gpu = Gpu::off_screen().expect("an adapter is available");
    let mut first =
        OutputRenderer::off_screen(&gpu, OutputId::new(), OUTPUT, PresentationMode::Unlocked)
            .unwrap();
    let mut second = OutputRenderer::off_screen(
        &gpu,
        OutputId::new(),
        Size::new(32, 32),
        PresentationMode::DisplaySynchronized,
    )
    .unwrap();

    let red = SourceTexture::solid(&gpu, Size::new(4, 4), RED).unwrap();
    let state = ready(LayerState::default());

    first.present(
        &[LayerDraw {
            state: &state,
            source: &red,
            mask: None,
        }],
        &MasterState::default(),
        None,
        Timestamp::from_micros(0),
    );
    second.present(
        &[],
        &MasterState::default(),
        None,
        Timestamp::from_micros(0),
    );

    assert_ne!(first.id(), second.id());
    assert_eq!(first.read_image().len(), 64 * 64 * 4);
    assert_eq!(second.read_image().len(), 32 * 32 * 4);
    assert_eq!(
        &second.read_image()[0..4],
        &BLACK,
        "the second output drew none of the first's layers"
    );
}

#[test]
fn a_real_hap_frame_reaches_the_screen_through_the_compressed_path() {
    let mut bench = Bench::new();
    if !bench.gpu.samples_block_compression() {
        // Honest skip rather than a silent pass: this machine expands blocks instead, which the
        // RGBA path already covers.
        eprintln!("skipped: this adapter does not sample BC textures");
        return;
    }

    // A frame that goes through the whole codec: RGBA in, BC3 blocks out, straight to the GPU.
    let (width, height) = (16u32, 16u32);
    let payload = media_codec::encode(width, height, &RED.repeat((width * height) as usize))
        .expect("encodes");
    let blocks = media_codec::decode_blocks(width, height, &payload).expect("decodes");

    let source = SourceTexture::from_bc3_blocks(&bench.gpu, Size::new(width, height), &blocks)
        .expect("a BC3 upload on an adapter that samples BC");
    let state = ready(LayerState::default());

    let image = bench.render(
        &[LayerDraw {
            state: &state,
            source: &source,
            mask: None,
        }],
        &MasterState::default(),
    );
    let [red, green, blue, _] = image.center();
    assert!(red > 240, "red survived the codec and the upload: {red}");
    assert!(
        green < 16 && blue < 16,
        "and nothing else did: {green},{blue}"
    );
}

#[test]
fn transparency_survives_all_the_way_to_the_composite() {
    let mut bench = Bench::new();
    if !bench.gpu.samples_block_compression() {
        eprintln!("skipped: this adapter does not sample BC textures");
        return;
    }

    // The requirement that chose this codec, checked end to end: a half-transparent layer over an
    // opaque one has to let the lower layer through.
    let (width, height) = (16u32, 16u32);
    let under = bench.solid(Size::new(4, 4), GREEN);
    let payload = media_codec::encode(
        width,
        height,
        &[0, 0, 255, 128].repeat((width * height) as usize),
    )
    .expect("encodes");
    let blocks = media_codec::decode_blocks(width, height, &payload).expect("decodes");
    let over = SourceTexture::from_bc3_blocks(&bench.gpu, Size::new(width, height), &blocks)
        .expect("uploads");

    let bottom = ready(LayerState::default());
    let top = ready(LayerState::default());
    let image = bench.render(
        &[
            LayerDraw {
                state: &bottom,
                source: &under,
                mask: None,
            },
            LayerDraw {
                state: &top,
                source: &over,
                mask: None,
            },
        ],
        &MasterState::default(),
    );

    let [red, green, blue, _] = image.center();
    assert!(red < 16, "no red anywhere: {red}");
    assert!(
        green > 96 && green < 160,
        "the green underneath shows through: {green}"
    );
    assert!(
        blue > 96 && blue < 160,
        "and the half-transparent blue sits over it: {blue}"
    );
}

// Masks. A mask is a source like any other: what makes it a mask is how the layer pass reads it.

/// A source whose left half is white and right half is black — a mask with a hard edge.
fn half_and_half(gpu: &Gpu) -> SourceTexture {
    let mut pixels = Vec::new();
    for _ in 0..4 {
        for x in 0..4 {
            let value = if x < 2 { 255 } else { 0 };
            pixels.extend_from_slice(&[value, value, value, 255]);
        }
    }
    SourceTexture::from_rgba8(gpu, Size::new(4, 4), &pixels).expect("a mask uploads")
}

#[test]
fn a_luminance_mask_keeps_what_is_bright_and_hides_what_is_dark() {
    let mut bench = Bench::new();
    let source = bench.solid(Size::new(4, 4), RED);
    let mask = half_and_half(&bench.gpu);
    let state = ready(LayerState {
        mask: MaskState {
            address: MediaAddress::new(1, 2),
            opacity: 1.0,
            source: MaskSource::Luminance,
            ..Default::default()
        },
        ..Default::default()
    });

    let image = bench.render(
        &[LayerDraw {
            state: &state,
            source: &source,
            mask: Some(&mask),
        }],
        &MasterState::default(),
    );

    assert_eq!(image.at(8, 32), RED, "the bright half of the mask passes");
    assert_eq!(image.at(56, 32), BLACK, "the dark half is held back");
}

#[test]
fn inverting_a_mask_exchanges_what_it_keeps_for_what_it_hides() {
    let mut bench = Bench::new();
    let source = bench.solid(Size::new(4, 4), RED);
    let mask = half_and_half(&bench.gpu);
    let state = ready(LayerState {
        mask: MaskState {
            address: MediaAddress::new(1, 2),
            opacity: 1.0,
            invert: true,
            ..Default::default()
        },
        ..Default::default()
    });

    let image = bench.render(
        &[LayerDraw {
            state: &state,
            source: &source,
            mask: Some(&mask),
        }],
        &MasterState::default(),
    );

    assert_eq!(image.at(8, 32), BLACK);
    assert_eq!(image.at(56, 32), RED);
}

#[test]
fn an_alpha_mask_reads_transparency_rather_than_brightness() {
    let mut bench = Bench::new();
    let source = bench.solid(Size::new(4, 4), RED);
    // Uniformly white, so a luminance mask would pass everything; the alpha is what varies.
    let mut pixels = Vec::new();
    for _ in 0..4 {
        for x in 0..4 {
            pixels.extend_from_slice(&[255, 255, 255, if x < 2 { 255 } else { 0 }]);
        }
    }
    let mask =
        SourceTexture::from_rgba8(&bench.gpu, Size::new(4, 4), &pixels).expect("a mask uploads");

    let state = ready(LayerState {
        mask: MaskState {
            address: MediaAddress::new(1, 2),
            opacity: 1.0,
            source: MaskSource::Alpha,
            ..Default::default()
        },
        ..Default::default()
    });

    let image = bench.render(
        &[LayerDraw {
            state: &state,
            source: &source,
            mask: Some(&mask),
        }],
        &MasterState::default(),
    );

    assert_eq!(image.at(8, 32), RED, "opaque mask pixels pass the layer");
    assert_eq!(image.at(56, 32), BLACK, "transparent ones hide it");
}

#[test]
fn a_mask_at_no_opacity_changes_nothing_and_a_missing_mask_is_no_mask() {
    let mut bench = Bench::new();
    let source = bench.solid(Size::new(4, 4), RED);
    let mask = half_and_half(&bench.gpu);

    // Selected, but faded out entirely.
    let faded = ready(LayerState {
        mask: MaskState {
            address: MediaAddress::new(1, 2),
            opacity: 0.0,
            ..Default::default()
        },
        ..Default::default()
    });
    let image = bench.render(
        &[LayerDraw {
            state: &faded,
            source: &source,
            mask: Some(&mask),
        }],
        &MasterState::default(),
    );
    assert_eq!(image.at(56, 32), RED, "an unapplied mask hides nothing");

    // Selected and fully applied, but the mask has not loaded.
    let unloaded = ready(LayerState {
        mask: MaskState {
            address: MediaAddress::new(1, 2),
            opacity: 1.0,
            ..Default::default()
        },
        ..Default::default()
    });
    let image = bench.render(
        &[LayerDraw {
            state: &unloaded,
            source: &source,
            mask: None,
        }],
        &MasterState::default(),
    );
    assert_eq!(
        image.at(56, 32),
        RED,
        "a mask that is on its way must not black the layer out while it loads"
    );
}

#[test]
fn a_mask_carries_its_own_scale_rather_than_the_layer_s() {
    let mut bench = Bench::new();
    let source = bench.solid(Size::new(4, 4), RED);
    let mask = half_and_half(&bench.gpu);

    // The mask is scaled to a quarter of the layer, centred. Outside its own extent there is
    // nothing to read, and an unread mask hides.
    let state = ready(LayerState {
        mask: MaskState {
            address: MediaAddress::new(1, 2),
            opacity: 1.0,
            scale_x: 0.25,
            scale_y: 0.25,
            ..Default::default()
        },
        ..Default::default()
    });

    let image = bench.render(
        &[LayerDraw {
            state: &state,
            source: &source,
            mask: Some(&mask),
        }],
        &MasterState::default(),
    );

    assert_eq!(image.at(2, 2), BLACK, "outside the mask's own extent");
    assert_eq!(image.at(28, 32), RED, "inside it, the bright half passes");
    assert_eq!(image.at(36, 32), BLACK, "and the dark half does not");
}

#[test]
fn the_master_mask_shapes_the_whole_composite_after_every_layer_has_landed() {
    let mut bench = Bench::new();
    let lower = bench.solid(Size::new(4, 4), RED);
    let upper = bench.solid(Size::new(4, 4), BLUE);
    let mask = half_and_half(&bench.gpu);
    let first = ready(LayerState::default());
    let second = ready(LayerState::default());

    let master = MasterState {
        mask: MediaAddress::new(1, 3),
        ..Default::default()
    };
    let image = bench.render_masked(
        &[
            LayerDraw {
                state: &first,
                source: &lower,
                mask: None,
            },
            LayerDraw {
                state: &second,
                source: &upper,
                mask: None,
            },
        ],
        &master,
        Some(&mask),
    );

    assert_eq!(
        image.at(8, 32),
        BLUE,
        "the top layer survives where the master mask passes"
    );
    assert_eq!(
        image.at(56, 32),
        BLACK,
        "and the whole composite is held back where it does not"
    );
}
