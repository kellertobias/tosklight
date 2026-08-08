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
    FlipMirror, LayerState, MasterState, MediaAddress, OutputId, PresentationMode, ScalingMode,
    SourceStatus, Timestamp, Tint,
};
use media_render::{Gpu, LayerDraw, OutputRenderer, SourceTexture};

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
        self.renderer
            .present(layers, master, Timestamp::from_micros(0));
        Image {
            pixels: self.renderer.read_image(),
        }
    }
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
            },
            LayerDraw {
                state: &top,
                source: &green,
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
            },
            LayerDraw {
                state: &top,
                source: &blue,
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
        }],
        &MasterState::default(),
    );
    assert_eq!(unflipped.at(8, 32), RED, "the bar starts on the left");
    assert_eq!(unflipped.at(56, 32), BLACK);

    let flipped = bench.render(
        &[LayerDraw {
            state: &state,
            source: &red,
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

    renderer.present(&[], &MasterState::default(), Timestamp::from_micros(0));
    renderer.present(&[], &MasterState::default(), Timestamp::from_micros(16_667));
    assert_eq!(renderer.cadence().frames, 2);

    renderer.recreate(Size::new(32, 16));
    assert_eq!(renderer.size(), Size::new(32, 16));
    assert_eq!(
        renderer.cadence().frames,
        0,
        "the cadence from before the change says nothing"
    );

    renderer.present(&[], &MasterState::default(), Timestamp::from_micros(0));
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
        }],
        &MasterState::default(),
        Timestamp::from_micros(0),
    );
    second.present(&[], &MasterState::default(), Timestamp::from_micros(0));

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
            },
            LayerDraw {
                state: &top,
                source: &over,
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
