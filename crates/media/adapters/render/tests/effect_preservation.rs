//! Keeps the approved Feedback effect frozen while Blur improves around it (TL-457).
//!
//! Feedback is pinned three ways: its shader and processor sources hash to recorded values, its
//! defaults and parameter contract are literal, and a multi-frame render at the shipped preset
//! matches a committed golden image. Blur is measured against properties the earlier nine-tap
//! kernel could not meet: it must flatten fine detail rather than leave a ghost of it.
//!
//! Regenerate the goldens only for an intended, reviewed change:
//! `MEDIA_UPDATE_GOLDEN=1 cargo test -p media-render --test effect_preservation`.

use media_domain::geometry::Size;
use media_domain::{
    AnalogTvParameters, BlurParameters, BlurType, EffectLibrary, EffectSlot, FeedbackMotion,
    FeedbackParameters, LayerState, MasterState, MediaAddress, OutputId, PresentationMode,
    ScalingMode, SourceStatus, Timestamp,
};
use media_render::{Gpu, LayerDraw, OutputRenderer, SourceTexture};
use std::path::{Path, PathBuf};

const OUTPUT: Size = Size::new(64, 64);

/// FNV-1a over the approved Feedback sources. A change here is a change to Feedback.
const FEEDBACK_SHADER_FNV: u64 = 0x6a9d_f2cb_0ae3_c3bd;
const FEEDBACK_PROCESSOR_FNV: u64 = 0x6dbb_bdc5_8012_a362;

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Line endings are normalised so a Windows checkout hashes the same text.
fn source_hash(text: &str) -> u64 {
    fnv1a(text.replace("\r\n", "\n").as_bytes())
}

struct Bench {
    gpu: Gpu,
    renderer: OutputRenderer,
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
        Self { gpu, renderer }
    }

    fn render(&mut self, state: &LayerState, source: &SourceTexture, now: Timestamp) -> Vec<u8> {
        let draw = LayerDraw {
            state,
            source,
            mask: None,
        };
        self.renderer
            .present(&[draw], &MasterState::default(), None, now, None);
        self.renderer.read_image()
    }
}

fn ready(effects: [EffectSlot; 4]) -> LayerState {
    LayerState {
        address: MediaAddress::new(1, 1),
        source_status: SourceStatus::Ready,
        scaling_mode: ScalingMode::Stretch,
        effects,
        ..Default::default()
    }
}

fn shipped(slot: u8) -> EffectSlot {
    EffectLibrary::default()
        .resolve(slot)
        .expect("the default library ships this slot")
        .effect
        .clone()
}

/// A checkerboard of one-texel cells over a colour ramp: the finest detail a blur can remove.
fn fine_checker(gpu: &Gpu) -> SourceTexture {
    let mut pixels = Vec::with_capacity((OUTPUT.width * OUTPUT.height * 4) as usize);
    for y in 0..OUTPUT.height {
        for x in 0..OUTPUT.width {
            let value = if (x + y) % 2 == 0 { 0 } else { 255 };
            pixels.extend_from_slice(&[value, value, value, 255]);
        }
    }
    SourceTexture::from_rgba8(gpu, OUTPUT, &pixels).expect("the checker uploads")
}

/// A bright square on black at a given column, so Feedback leaves a visible trail as it moves.
fn square_at(gpu: &Gpu, left: u32) -> SourceTexture {
    let mut pixels = Vec::with_capacity((OUTPUT.width * OUTPUT.height * 4) as usize);
    for y in 0..OUTPUT.height {
        for x in 0..OUTPUT.width {
            let inside = (left..left + 12).contains(&x) && (26..38).contains(&y);
            let red = ((x * 255) / (OUTPUT.width - 1)) as u8;
            pixels.extend_from_slice(&if inside {
                [red, 255, 255 - red, 255]
            } else {
                [0, 0, 0, 255]
            });
        }
    }
    SourceTexture::from_rgba8(gpu, OUTPUT, &pixels).expect("the square uploads")
}

/// Six frames of the shipped Feedback preset over a moving square, returning the last frame.
fn feedback_sequence(bench: &mut Bench, state: &LayerState) -> Vec<u8> {
    let sources: Vec<_> = (0..6)
        .map(|step| square_at(&bench.gpu, 6 + step * 8))
        .collect();
    let mut last = Vec::new();
    for (step, source) in sources.iter().enumerate() {
        last = bench.render(state, source, Timestamp::from_millis(step as u64 * 40));
    }
    last
}

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name)
}

fn write_png(path: &Path, pixels: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("golden directory")).expect("golden dir");
    let file = std::fs::File::create(path).expect("the golden image can be created");
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

/// Exact on the recording adapter; a different adapter may round a filtered sample differently,
/// so allow a small per-channel tolerance and nothing more.
fn assert_matches_golden(name: &str, pixels: &[u8]) {
    let path = golden_path(name);
    if std::env::var_os("MEDIA_UPDATE_GOLDEN").is_some() {
        write_png(&path, pixels);
        return;
    }
    let golden = read_png(&path);
    assert_eq!(golden.len(), pixels.len(), "{name}: golden size");
    let mut worst = 0u8;
    let mut differing = 0usize;
    for (expected, actual) in golden.iter().zip(pixels) {
        let difference = expected.abs_diff(*actual);
        worst = worst.max(difference);
        if difference > 2 {
            differing += 1;
        }
    }
    assert!(
        worst <= 12 && differing * 200 <= pixels.len(),
        "{name}: output differs from the approved golden (worst channel {worst}, \
         {differing} channels beyond rounding)"
    );
}

/// Mean absolute difference between horizontally adjacent pixels in the interior: the detail a
/// blur has left behind, from 0 (flat) to 255 (a one-texel checker).
fn interior_detail(pixels: &[u8]) -> f32 {
    let width = OUTPUT.width as usize;
    let mut total = 0.0;
    let mut count = 0.0;
    for y in 8..OUTPUT.height as usize - 8 {
        for x in 8..width - 9 {
            let left = pixels[(y * width + x) * 4];
            let right = pixels[(y * width + x + 1) * 4];
            total += f32::from(left.abs_diff(right));
            count += 1.0;
        }
    }
    total / count
}

fn blur_state(amount: f32, blur_type: BlurType) -> LayerState {
    let mut effect = EffectSlot::blur();
    effect.parameters = BlurParameters { amount, blur_type }.as_array().to_vec();
    let mut effects: [EffectSlot; 4] = Default::default();
    effects[0] = effect;
    ready(effects)
}

#[test]
fn feedback_sources_are_the_approved_ones() {
    let shader = source_hash(include_str!("../src/shaders/feedback.wgsl"));
    let processor = source_hash(include_str!("../src/feedback.rs"));
    assert_eq!(
        (shader, processor),
        (FEEDBACK_SHADER_FNV, FEEDBACK_PROCESSOR_FNV),
        "Feedback is approved as-is (TL-457): shader {shader:#x}, processor {processor:#x}"
    );
}

#[test]
fn feedback_defaults_and_parameter_contract_are_unchanged() {
    assert_eq!(
        FeedbackParameters::default(),
        FeedbackParameters {
            amount: 0.94,
            motion: 0.12,
            direction: FeedbackMotion::Tunnel,
        }
    );
    let preset = EffectLibrary::default();
    let feedback = preset.resolve(4).expect("slot 4 ships Feedback");
    assert_eq!(feedback.name, "Feedback");
    assert_eq!(feedback.effect, {
        let mut effect = EffectSlot::feedback();
        effect.enabled = true;
        effect.mix = 1.0;
        effect
    });
    assert_eq!(
        feedback.effect.parameters,
        FeedbackParameters::default().as_array().to_vec()
    );
    assert_eq!(
        media_domain::effect_parameter_ids("feedback"),
        ["feedback-amount", "feedback-motion", "feedback-direction"]
    );
}

#[test]
fn shipped_feedback_renders_the_approved_trail() {
    let mut bench = Bench::new();
    let mut effects: [EffectSlot; 4] = Default::default();
    effects[0] = shipped(4);
    let state = ready(effects);
    let first = feedback_sequence(&mut bench, &state);
    assert_matches_golden("feedback-shipped.png", &first);

    // A fresh renderer replays the same history, so the golden is a property of Feedback alone.
    let mut bench = Bench::new();
    let repeated = feedback_sequence(&mut bench, &state);
    assert_eq!(first, repeated, "feedback is deterministic frame to frame");
}

#[test]
fn tv_then_feedback_two_bank_composition_keeps_working() {
    let mut effects: [EffectSlot; 4] = Default::default();
    effects[0] = shipped(1);
    effects[0].seed = 0x0100;
    effects[1] = shipped(4);
    effects[1].seed = 0x0101;
    assert_eq!(
        effects[0].analog_tv_parameters(),
        Some(AnalogTvParameters::default())
    );
    let mut bench = Bench::new();
    let composed = feedback_sequence(&mut bench, &ready(effects.clone()));

    let mut feedback_only: [EffectSlot; 4] = Default::default();
    feedback_only[1] = effects[1].clone();
    let mut bench = Bench::new();
    let without_tv = feedback_sequence(&mut bench, &ready(feedback_only));

    let mut tv_only: [EffectSlot; 4] = Default::default();
    tv_only[0] = effects[0].clone();
    let mut bench = Bench::new();
    let without_feedback = feedback_sequence(&mut bench, &ready(tv_only));

    let changed = |left: &[u8], right: &[u8]| {
        left.as_chunks::<4>()
            .0
            .iter()
            .zip(right.as_chunks::<4>().0.iter())
            .filter(|(left, right)| left != right)
            .count()
    };
    assert!(
        changed(&composed, &without_tv) > 200,
        "the TV bank still shapes the composition"
    );
    assert!(
        changed(&composed, &without_feedback) > 200,
        "the Feedback bank still leaves its trail in the composition"
    );
    let lit = composed
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[1] > 32)
        .count();
    assert!(lit > 150, "the composition is visible, not black: {lit}");
}

#[test]
fn blur_flattens_fine_detail_instead_of_ghosting_it() {
    let mut bench = Bench::new();
    let checker = fine_checker(&bench.gpu);
    let plain = bench.render(&ready(Default::default()), &checker, Timestamp::ZERO);
    let untouched = interior_detail(&plain);
    assert!(untouched > 200.0, "the probe really is a fine checker");

    let off = bench.render(
        &blur_state(0.0, BlurType::Gaussian),
        &checker,
        Timestamp::ZERO,
    );
    assert_eq!(off, plain, "zero amount is an exact bypass");

    let gaussian = bench.render(
        &blur_state(0.35, BlurType::Gaussian),
        &checker,
        Timestamp::ZERO,
    );
    let gaussian_detail = interior_detail(&gaussian);
    // The previous nine-tap kernel kept 24% of the centre texel and left roughly a quarter of
    // the checker contrast visible even at full amount; the shipped default must now smooth it.
    assert!(
        gaussian_detail < untouched * 0.06,
        "the shipped Gaussian blur leaves {gaussian_detail} of {untouched} detail"
    );
    let shape = bench.render(
        &blur_state(0.35, BlurType::Shape),
        &checker,
        Timestamp::ZERO,
    );
    assert!(
        interior_detail(&shape) < untouched * 0.1,
        "the shape blur smooths as well: {}",
        interior_detail(&shape)
    );
    let linear = bench.render(
        &blur_state(0.35, BlurType::Linear),
        &checker,
        Timestamp::ZERO,
    );
    assert!(
        interior_detail(&linear) < untouched * 0.1,
        "the linear blur smooths along its axis: {}",
        interior_detail(&linear)
    );
}

#[test]
fn blur_amount_widens_the_kernel_monotonically() {
    let mut bench = Bench::new();
    let source = square_at(&bench.gpu, 26);
    let spread = |pixels: &[u8]| {
        // How far light reaches beyond the square's 12-texel row.
        (0..OUTPUT.width as usize)
            .filter(|x| pixels[(32 * OUTPUT.width as usize + x) * 4 + 1] > 8)
            .count()
    };
    let small = bench.render(
        &blur_state(0.1, BlurType::Gaussian),
        &source,
        Timestamp::ZERO,
    );
    let medium = bench.render(
        &blur_state(0.4, BlurType::Gaussian),
        &source,
        Timestamp::ZERO,
    );
    let large = bench.render(
        &blur_state(1.0, BlurType::Gaussian),
        &source,
        Timestamp::ZERO,
    );
    assert!(spread(&small) > 12, "even a small blur softens the edge");
    assert!(spread(&medium) > spread(&small));
    assert!(spread(&large) > spread(&medium));
    // A blur conserves light: it spreads the square rather than dimming the whole frame.
    let energy = |pixels: &[u8]| {
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| u64::from(pixel[1]))
            .sum::<u64>()
    };
    let sharp = bench.render(&ready(Default::default()), &source, Timestamp::ZERO);
    let ratio = energy(&medium) as f32 / energy(&sharp) as f32;
    assert!(
        (0.9..1.1).contains(&ratio),
        "a blur conserves brightness: {ratio}"
    );
}
