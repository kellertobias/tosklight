//! Rendered reference cases for Media White Blend and tint (TL-569).
//!
//! A small generated patch chart goes through the real layer and master pipelines. Every rendered
//! patch is compared with the CPU reference in `media_domain::MediaColor::apply_encoded`, and each
//! case is also pinned to a committed golden image in `tests/golden/white-blend-*.png`. The chart
//! is generated in code, so there is no third-party source asset.
//!
//! Tolerance: a rendered channel may differ from the CPU reference by at most
//! [`CPU_TOLERANCE`] code values. The GPU's `pow` is an approximation and the program target
//! quantizes to eight bits between the layer and master passes; nothing else may differ.
//!
//! The semantic cases (TL-593) start from a desk `ColorIntent` instead of wire bytes: the shared
//! `light_fixture::media_color` translation on the shipped ToskLight Media Server profile writes the
//! personality controls, the profile's encoding plan places them in a universe, and the Media
//! personality decoder reads them back. They must reproduce the same approved goldens.
//!
//! Regenerate the goldens only for an intended, reviewed change:
//! `MEDIA_UPDATE_GOLDEN=1 cargo test -p media-render --test white_blend_renders`.
//! With `LIGHT_TMP_DIR` set, each render is also copied to `$LIGHT_TMP_DIR/media-white-blend/`.

use media_domain::geometry::Size;
use media_domain::personality::LAYER_SLOTS;
use media_domain::personality::channels::layer;
use media_domain::personality::decode::layer_state;
use media_domain::{
    LayerState, MasterState, MediaAddress, MediaColor, OutputId, PresentationMode, ScalingMode,
    SourceStatus, Timestamp, Tint,
};
use media_render::{Gpu, LayerDraw, OutputRenderer, SourceTexture};
use std::path::{Path, PathBuf};

/// Eight patches across, two rows down, each patch four by eight pixels.
const OUTPUT: Size = Size::new(32, 16);
const CPU_TOLERANCE: u8 = 2;

/// The chart: saturated primaries and secondaries, neutrals, mixed colours and black.
const PATCHES: [[u8; 3]; 16] = [
    [255, 0, 0],
    [0, 255, 0],
    [0, 0, 255],
    [255, 255, 255],
    [128, 128, 128],
    [255, 128, 0],
    [224, 172, 140],
    [0, 0, 0],
    [0, 255, 255],
    [255, 0, 255],
    [255, 255, 0],
    [32, 32, 32],
    [64, 160, 96],
    [200, 40, 120],
    [16, 96, 224],
    [0, 0, 0],
];

/// The black patches, which must stay black in every case.
const BLACK_PATCHES: [usize; 2] = [7, 15];

struct Bench {
    gpu: Gpu,
    renderer: OutputRenderer,
}

impl Bench {
    fn new() -> Self {
        let gpu = Gpu::off_screen().expect(
            "the White Blend renders need a GPU or software adapter; install a software Vulkan \
             driver (mesa-vulkan-drivers) on a machine with no GPU",
        );
        let renderer = OutputRenderer::off_screen(
            &gpu,
            OutputId::new(),
            OUTPUT,
            PresentationMode::DisplaySynchronized,
        )
        .expect("an off-screen 32x16 output is within every adapter's limits");
        Self { gpu, renderer }
    }

    fn chart(&self, alpha: u8) -> SourceTexture {
        let mut pixels = Vec::with_capacity((OUTPUT.width * OUTPUT.height * 4) as usize);
        for y in 0..OUTPUT.height {
            for x in 0..OUTPUT.width {
                let [red, green, blue] = PATCHES[patch_index(x, y)];
                pixels.extend_from_slice(&[red, green, blue, alpha]);
            }
        }
        SourceTexture::from_rgba8(&self.gpu, OUTPUT, &pixels).expect("the chart uploads")
    }

    fn render(
        &mut self,
        state: &LayerState,
        source: &SourceTexture,
        master: &MasterState,
    ) -> Vec<u8> {
        let draw = LayerDraw {
            state,
            source,
            mask: None,
        };
        self.renderer
            .present(&[draw], master, None, Timestamp::ZERO, None);
        self.renderer.read_image()
    }
}

const fn patch_index(x: u32, y: u32) -> usize {
    ((y / 8) * 8 + x / 4) as usize
}

/// The centre pixel of a patch, well away from any neighbour.
fn patch_centre(pixels: &[u8], index: usize) -> [u8; 4] {
    let (x, y) = ((index % 8) as u32 * 4 + 2, (index / 8) as u32 * 8 + 4);
    let at = ((y * OUTPUT.width + x) * 4) as usize;
    [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
}

fn ready(state: LayerState) -> LayerState {
    LayerState {
        address: MediaAddress::new(1, 1),
        source_status: SourceStatus::Ready,
        scaling_mode: ScalingMode::Stretch,
        ..state
    }
}

/// A layer decoded from real personality slots, so the wire decode is part of every case.
fn decoded(cyan: u8, magenta: u8, yellow: u8, grayscale: u8, dimmer: u8) -> LayerState {
    let mut slots = [0u8; LAYER_SLOTS as usize];
    slots[layer::FOLDER] = 1;
    slots[layer::FILE] = 1;
    slots[layer::SCALE_X] = 0x80;
    slots[layer::SCALE_Y] = 0x80;
    slots[layer::POSITION_X] = 0x80;
    slots[layer::POSITION_Y] = 0x80;
    slots[layer::ROTATION] = 0x80;
    slots[layer::DIMMER] = dimmer;
    slots[layer::CYAN] = cyan;
    slots[layer::MAGENTA] = magenta;
    slots[layer::YELLOW] = yellow;
    slots[layer::GRAYSCALE] = grayscale;
    ready(layer_state(&slots))
}

fn unit(value: u8) -> f32 {
    f32::from(value) / 255.0
}

fn quantize(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// What one patch must look like on the output: the layer color stage, composited with its alpha
/// over the black program, quantized, then the master color stage and master dimmer.
fn expected_patch(state: &LayerState, master: &MasterState, source: [u8; 3], alpha: u8) -> [u8; 3] {
    let layer_rgb = MediaColor::of_layer(state).apply_encoded(source.map(unit));
    let coverage = unit(alpha) * state.dimmer;
    let program = layer_rgb.map(|channel| unit(quantize(channel * coverage)));
    MediaColor::of_master(master)
        .apply_encoded(program)
        .map(|channel| quantize(channel * master.dimmer))
}

fn assert_matches_cpu(
    name: &str,
    pixels: &[u8],
    state: &LayerState,
    master: &MasterState,
    alpha: u8,
) {
    for (index, source) in PATCHES.iter().enumerate() {
        let expected = expected_patch(state, master, *source, alpha);
        let actual = patch_centre(pixels, index);
        for channel in 0..3 {
            assert!(
                expected[channel].abs_diff(actual[channel]) <= CPU_TOLERANCE,
                "{name}: patch {index} {source:?} rendered {actual:?}, CPU reference {expected:?}"
            );
        }
        assert_eq!(actual[3], 255, "{name}: the output is opaque");
    }
    for index in BLACK_PATCHES {
        assert_eq!(
            &patch_centre(pixels, index)[..3],
            &[0, 0, 0],
            "{name}: black stays black"
        );
    }
}

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("white-blend-{name}.png"))
}

fn write_png(path: &Path, pixels: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("a parent directory")).expect("directory");
    let file = std::fs::File::create(path).expect("the image can be created");
    let mut encoder = png::Encoder::new(file, OUTPUT.width, OUTPUT.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("PNG header")
        .write_image_data(pixels)
        .expect("PNG body");
}

fn read_png(path: &Path) -> Vec<u8> {
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(path).unwrap_or_else(|_| panic!("missing golden {}", path.display())),
    ));
    let mut reader = decoder.read_info().expect("golden PNG header");
    let mut pixels = vec![0; reader.output_buffer_size().expect("golden size")];
    let info = reader.next_frame(&mut pixels).expect("golden PNG body");
    pixels.truncate(info.buffer_size());
    pixels
}

/// Flat patches leave no filtering to round differently, so the golden tolerance is the same
/// small per-channel allowance as the CPU comparison.
fn assert_matches_golden(name: &str, pixels: &[u8]) {
    assert_matches_golden_as(name, name, pixels);
}

/// Compare with the golden `name`, copying the render as `evidence` under `LIGHT_TMP_DIR`.
fn assert_matches_golden_as(evidence: &str, name: &str, pixels: &[u8]) {
    if let Ok(directory) = std::env::var("LIGHT_TMP_DIR") {
        let directory = Path::new(&directory).join("media-white-blend");
        write_png(&directory.join(format!("{evidence}.png")), pixels);
    }
    let path = golden_path(name);
    if std::env::var_os("MEDIA_UPDATE_GOLDEN").is_some() {
        write_png(&path, pixels);
        return;
    }
    let golden = read_png(&path);
    assert_eq!(golden.len(), pixels.len(), "{name}: golden size");
    let worst = golden
        .iter()
        .zip(pixels)
        .map(|(expected, actual)| expected.abs_diff(*actual))
        .max()
        .unwrap_or(0);
    assert!(
        worst <= CPU_TOLERANCE,
        "{name}: output differs from the approved golden by {worst} code values"
    );
}

fn render_case(name: &str, state: &LayerState, master: &MasterState, alpha: u8) -> Vec<u8> {
    let mut bench = Bench::new();
    let chart = bench.chart(alpha);
    let pixels = bench.render(state, &chart, master);
    assert_matches_cpu(name, &pixels, state, master, alpha);
    assert_matches_golden(name, &pixels);
    pixels
}

#[test]
fn original_is_an_exact_bypass_of_the_source() {
    let state = decoded(0, 0, 0, 0, 255);
    let pixels = render_case("original", &state, &MasterState::default(), 255);
    for (index, source) in PATCHES.iter().enumerate() {
        assert_eq!(
            &patch_centre(&pixels, index)[..3],
            source,
            "patch {index} is untouched"
        );
    }
}

#[test]
fn half_white_blend_moves_every_patch_part_way_to_grayscale() {
    let original = decoded(0, 0, 0, 0, 255);
    let half = decoded(0, 0, 0, 128, 255);
    let full = decoded(0, 0, 0, 255, 255);
    let master = MasterState::default();
    let pixels = render_case("half", &half, &master, 255);
    let spread = |rgb: [u8; 4]| rgb[..3].iter().max().unwrap() - rgb[..3].iter().min().unwrap();
    let mut bench = Bench::new();
    let chart = bench.chart(255);
    let before = bench.render(&original, &chart, &master);
    let after = bench.render(&full, &chart, &master);
    for index in 0..PATCHES.len() {
        let (source, blended, gray) = (
            spread(patch_centre(&before, index)),
            spread(patch_centre(&pixels, index)),
            spread(patch_centre(&after, index)),
        );
        assert!(gray <= 1, "patch {index} is neutral at 100%: spread {gray}");
        assert!(
            source <= 1 || (blended < source && blended > gray),
            "patch {index} is part way: {source} > {blended} > {gray}"
        );
    }
}

#[test]
fn full_white_blend_with_a_white_tint_is_neutral_linear_grayscale() {
    let state = decoded(0, 0, 0, 255, 255);
    let pixels = render_case("full", &state, &MasterState::default(), 255);
    // Pure red: linear luminance 0.2126 encodes to 127, not the legacy gamma-space 76.
    let red = patch_centre(&pixels, 0);
    assert!(
        red[..3].iter().all(|channel| (126..=128).contains(channel)),
        "{red:?}"
    );
    // A neutral source is unchanged by White Blend.
    assert_eq!(&patch_centre(&pixels, 4)[..3], &[128, 128, 128]);
}

#[test]
fn the_tint_stays_active_at_full_white_blend() {
    // Magenta at half and yellow full: an orange tint of (1, 0.498, 0) on the wire.
    let state = decoded(0, 128, 255, 255, 255);
    assert_eq!(state.tint, Tint::new(1.0, 1.0 - unit(128), 0.0));
    let pixels = render_case("full-tint", &state, &MasterState::default(), 255);
    let white = patch_centre(&pixels, 3);
    assert_eq!(white[0], 255, "white source, full red tint component");
    assert!(
        (185..=187).contains(&white[1]),
        "half linear green encodes to ~186: {white:?}"
    );
    assert_eq!(
        white[2], 0,
        "the blue tint component is zero, so no blue appears"
    );
    let green = patch_centre(&pixels, 1);
    assert!(
        green[0] > green[1] && green[1] > 0 && green[2] == 0,
        "tinted gray: {green:?}"
    );
}

#[test]
fn a_black_tint_renders_black_whatever_the_white_blend() {
    let state = decoded(255, 255, 255, 128, 255);
    let pixels = render_case("black", &state, &MasterState::default(), 255);
    assert!(
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel == &[0, 0, 0, 255])
    );
}

#[test]
fn alpha_layer_dimmer_and_master_dimmer_stay_independent_of_white_blend() {
    // Source alpha 160, layer dimmer 153 (0.6), White Blend 50%, a warm tint and a 0.8 master.
    let state = decoded(0, 77, 153, 128, 153);
    let master = MasterState {
        dimmer: 0.8,
        ..Default::default()
    };
    render_case("alpha-intensity", &state, &master, 160);

    // Dimmer and alpha scale the result exactly as they did without White Blend: the color stage
    // runs first and never sees them.
    let mut bench = Bench::new();
    let chart = bench.chart(160);
    let mut preview_state = state.clone();
    let preview = bench.renderer.capture_layer_preview(
        OUTPUT,
        LayerDraw {
            state: &preview_state,
            source: &chart,
            mask: None,
        },
        Timestamp::ZERO,
    );
    let coverage = quantize(unit(160) * 0.6);
    for index in 0..PATCHES.len() {
        let alpha = patch_centre(&preview, index)[3];
        assert!(
            alpha.abs_diff(coverage) <= 1,
            "patch {index}: alpha {alpha}, expected {coverage}"
        );
    }
    preview_state.grayscale = 0.0;
    preview_state.tint = Tint::WHITE;
    let neutral = bench.renderer.capture_layer_preview(
        OUTPUT,
        LayerDraw {
            state: &preview_state,
            source: &chart,
            mask: None,
        },
        Timestamp::ZERO,
    );
    for index in 0..PATCHES.len() {
        assert_eq!(
            patch_centre(&preview, index)[3],
            patch_centre(&neutral, index)[3],
            "patch {index}: White Blend and tint leave alpha alone"
        );
    }
}

#[test]
fn master_tint_uses_the_same_linear_stage_and_blackout_still_wins() {
    let state = decoded(0, 0, 0, 255, 255);
    let tinted = MasterState {
        tint: Tint::new(1.0, 0.5, 0.25),
        ..Default::default()
    };
    let mut bench = Bench::new();
    let chart = bench.chart(255);
    let pixels = bench.render(&state, &chart, &tinted);
    assert_matches_cpu("master-tint", &pixels, &state, &tinted, 255);

    let blackout = MasterState {
        dimmer: 0.0,
        ..tinted
    };
    let dark = bench.render(&state, &chart, &blackout);
    assert!(
        dark.as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[..3] == [0, 0, 0])
    );
}

mod semantic {
    //! TL-593: desk semantic Color intent → Media personality bytes → decode → render.
    use super::*;
    use light_core::programming::{ColorIntent, VirtualColorAuthoringV1, VirtualColorRecipe};
    use light_fixture::FixtureProfile;
    use light_fixture::media_color::{MediaColorControls, MediaColorHead, MediaColorSurface};
    use media_domain::linear_to_srgb;
    use media_domain::personality::MASTER_SLOTS;
    use media_domain::personality::decode::master_state;

    fn shipped_media_server() -> FixtureProfile {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../assets/fixture-library/tosklight--media-server.toskfixture");
        light_fixture::read_fixture_package(&std::fs::read(path).expect("shipped profile"))
            .expect("the shipped profile reads")
    }

    /// A desk request whose recipe is authored in encoded sRGB, like the Color editor.
    fn request(encoded: [f32; 3], white_blend: f32) -> ColorIntent {
        let recipe = VirtualColorRecipe {
            version: 1,
            rgb: encoded,
            amber: 0.0,
            approximate: false,
        };
        ColorIntent {
            base_xyz: VirtualColorAuthoringV1::recipe_xyz(&recipe).unwrap(),
            recipe,
            white_blend,
            ..ColorIntent::default()
        }
    }

    /// The recipe the Color editor holds for a linear tint.
    fn tinted(linear: [f32; 3], white_blend: f32) -> ColorIntent {
        request(linear.map(linear_to_srgb), white_blend)
    }

    /// The universe the desk emits for the 2-layer personality when `surface`'s head carries
    /// the translated request and every other control is at its profile default.
    fn universe(
        intent: &ColorIntent,
        surface: MediaColorSurface,
    ) -> ([u8; 512], MediaColorControls) {
        let profile = shipped_media_server();
        let mode = &profile.modes[0];
        let head = mode
            .heads
            .iter()
            .filter_map(|head| MediaColorHead::from_mode(mode, head.id))
            .find(|head| head.surface == surface)
            .expect("the personality has this Media color head");
        let resolved = head.resolve(intent).expect("a valid request");
        let mut raws: Vec<u32> = mode.channels.iter().map(|c| c.default_raw).collect();
        for (control, raw) in head.controls().iter().zip(&resolved.raws) {
            raws[control.channel_index as usize] = *raw;
        }
        let plan = mode.compile_encoding_plan().unwrap();
        let mut bytes = [0u8; 512];
        let values: Vec<_> = (0u32..).zip(raws).collect();
        plan.encode_split_by_index(&mut bytes, 1, 1, &values)
            .unwrap();
        (bytes, resolved.achieved)
    }

    /// The first layer decoded from the desk universe; only source selection, transform and the
    /// Intensity dimmer are set here, never a color slot.
    fn semantic_layer(intent: &ColorIntent, dimmer: u8) -> LayerState {
        let (bytes, achieved) = universe(intent, MediaColorSurface::Layer);
        let mut slots = bytes[..LAYER_SLOTS as usize].to_vec();
        slots[layer::FOLDER] = 1;
        slots[layer::FILE] = 1;
        slots[layer::DIMMER] = dimmer;
        let state = ready(layer_state(&slots));
        // The desk publishes exactly what the Media decoder reads.
        let media = MediaColor::of_layer(&state);
        assert_eq!(
            [media.tint.red, media.tint.green, media.tint.blue],
            achieved.tint
        );
        assert_eq!(media.white_blend, achieved.white_blend);
        state
    }

    fn render_semantic(golden: &str, state: &LayerState, master: &MasterState, alpha: u8) {
        let mut bench = Bench::new();
        let chart = bench.chart(alpha);
        let pixels = bench.render(state, &chart, master);
        assert_matches_cpu(golden, &pixels, state, master, alpha);
        assert_matches_golden_as(&format!("semantic-{golden}"), golden, &pixels);
    }

    #[test]
    fn white_blend_0_50_100_from_desk_intent_reproduce_the_approved_goldens() {
        for (golden, blend, grayscale) in
            [("original", 0.0, 0), ("half", 0.5, 128), ("full", 1.0, 255)]
        {
            let state = semantic_layer(&request([1.0; 3], blend), 255);
            let wire = decoded(0, 0, 0, grayscale, 255);
            assert_eq!(
                (state.tint, state.grayscale),
                (wire.tint, wire.grayscale),
                "{golden}"
            );
            render_semantic(golden, &state, &MasterState::default(), 255);
        }
    }

    #[test]
    fn a_tinted_desk_intent_keeps_its_tint_at_full_white_blend_with_one_transfer_conversion() {
        // The linear half green tint of the approved golden, authored as its encoded recipe.
        let state = semantic_layer(&tinted([1.0, 1.0 - unit(128), 0.0], 1.0), 255);
        assert_eq!(state.tint, decoded(0, 128, 255, 255, 255).tint);
        render_semantic("full-tint", &state, &MasterState::default(), 255);

        // Reading the encoded recipe as linear, or decoding the linear tint again, both move the
        // magenta byte far from the linear 128 the golden pins.
        let encoded_green = linear_to_srgb(1.0 - unit(128));
        assert!((1.0 - encoded_green) * 255.0 < 70.0);
        assert!((1.0 - media_domain::srgb_to_linear(1.0 - unit(128))) * 255.0 > 200.0);
    }

    #[test]
    fn a_black_desk_intent_renders_black_at_any_white_blend() {
        let state = semantic_layer(&request([0.0; 3], 0.5), 255);
        assert_eq!(state.tint, Tint::new(0.0, 0.0, 0.0));
        render_semantic("black", &state, &MasterState::default(), 255);
        let mut off = request([1.0; 3], 1.0);
        off.relative_output = 0.0;
        assert_eq!(semantic_layer(&off, 255).tint, Tint::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn transparent_pixels_layer_and_master_intensity_stay_independent_of_desk_color() {
        let intent = tinted([1.0, 1.0 - unit(77), 1.0 - unit(153)], 0.5);
        let state = semantic_layer(&intent, 153);
        let wire = decoded(0, 77, 153, 128, 153);
        assert_eq!(
            (state.tint, state.grayscale, state.dimmer),
            (wire.tint, wire.grayscale, wire.dimmer)
        );
        let master = MasterState {
            dimmer: 0.8,
            ..Default::default()
        };
        render_semantic("alpha-intensity", &state, &master, 160);
        // Fully transparent source pixels contribute nothing, whatever the color.
        let mut bench = Bench::new();
        let clear = bench.chart(0);
        let pixels = bench.render(&state, &clear, &master);
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[..3] == [0, 0, 0])
        );
        // relativeOutput dims the tint linearly; the layer dimmer stays the Intensity control.
        let mut dim = intent.clone();
        dim.relative_output = 0.5;
        let dim = semantic_layer(&dim, 153);
        assert_eq!(dim.dimmer, state.dimmer);
        assert!((dim.tint.green - 0.5 * state.tint.green).abs() <= 1.0 / 255.0);
    }

    #[test]
    fn the_master_takes_the_desk_tint_and_keeps_white_blend_absent() {
        let intent = tinted([1.0, 0.5, 0.25], 0.7);
        let (bytes, achieved) = universe(&intent, MediaColorSurface::Master);
        let start = 2 * LAYER_SLOTS as usize;
        let master = master_state(&bytes[start..start + MASTER_SLOTS as usize]);
        let media = MediaColor::of_master(&master);
        assert_eq!(
            media.white_blend, None,
            "the master passes White Blend through"
        );
        assert_eq!(achieved.white_blend, None);
        assert_eq!(
            [media.tint.red, media.tint.green, media.tint.blue],
            achieved.tint
        );
        let state = decoded(0, 0, 0, 0, 255);
        let mut bench = Bench::new();
        let chart = bench.chart(255);
        let pixels = bench.render(&state, &chart, &master);
        assert_matches_cpu("semantic-master", &pixels, &state, &master, 255);
        // The layers of that universe keep their untouched defaults.
        assert_eq!(
            layer_state(&bytes[..LAYER_SLOTS as usize]).tint,
            Tint::WHITE
        );
    }
}
