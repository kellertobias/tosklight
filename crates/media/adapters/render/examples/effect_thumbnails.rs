//! Renders one Media Library thumbnail per shipped effect preset.
//!
//! The thumbnails are real off-screen frames from the program compositor and its shaders, applied
//! to one colourful reference picture. They are committed under
//! `apps/media/src/features/effects/thumbnails/`, keyed by the effect type the library editor
//! shows, so a user preset of the same type shows the same picture without any runtime rendering.
//!
//! ```sh
//! cargo run -p media-render --example effect_thumbnails -- <directory>
//! ```
//!
//! Beat Move and Beat Scale & Turn are transform effects the runtime applies at a beat. The sheet
//! shows their resting layer as a faint ghost under the transform a landed beat produces, which is
//! the same arithmetic the runtime performs.

use std::path::PathBuf;

use media_domain::geometry::Size;
use media_domain::{
    BeatMoveDirection, EffectLibrary, EffectSlot, LayerState, MasterState, MediaAddress, OutputId,
    PresentationMode, ScalingMode, SourceStatus, Timestamp, Tint,
};
use media_render::{Gpu, LayerDraw, OutputRenderer, SourceTexture};

const SIZE: Size = Size::new(256, 144);

fn main() -> anyhow::Result<()> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| ".artifacts/tmp/effect-thumbnails".to_owned()),
    );
    std::fs::create_dir_all(&directory)?;
    let gpu = Gpu::off_screen().map_err(|error| anyhow::anyhow!("{error}"))?;
    let library = EffectLibrary::default();

    for preset in &library.entries {
        let key = thumbnail_key(&preset.effect);
        let mut renderer = OutputRenderer::off_screen(
            &gpu,
            OutputId::new(),
            SIZE,
            PresentationMode::DisplaySynchronized,
        )
        .map_err(|error| anyhow::anyhow!("{error}"))?;
        let pixels = render_preset(&gpu, &mut renderer, &preset.effect)?;
        let path = directory.join(format!("{key}.png"));
        let file = std::fs::File::create(&path)?;
        let mut encoder = png::Encoder::new(file, SIZE.width, SIZE.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let rgb: Vec<u8> = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();
        encoder.write_header()?.write_image_data(&rgb)?;
        println!("{} <- slot {} {}", path.display(), preset.slot, preset.name);
    }
    Ok(())
}

/// The library editor's effect type: Rasterize is split by its mode.
fn thumbnail_key(effect: &EffectSlot) -> String {
    let effect_type = effect.effect_type.clone().unwrap_or_default();
    if effect_type == "rasterize" {
        let cmyk = effect.parameters.first().copied().unwrap_or(0.0) >= 0.5;
        return if cmyk {
            "rasterize-cmyk"
        } else {
            "rasterize-bw"
        }
        .to_owned();
    }
    effect_type
}

fn layer(effect: Option<EffectSlot>) -> LayerState {
    let mut effects: [EffectSlot; 4] = Default::default();
    if let Some(effect) = effect {
        effects[0] = effect;
    }
    LayerState {
        address: MediaAddress::new(1, 1),
        source_status: SourceStatus::Ready,
        scaling_mode: ScalingMode::Stretch,
        effects,
        ..Default::default()
    }
}

fn render_preset(
    gpu: &Gpu,
    renderer: &mut OutputRenderer,
    effect: &EffectSlot,
) -> anyhow::Result<Vec<u8>> {
    let scene = reference_picture(gpu, 0.0)?;
    let at = Timestamp::from_millis(1_300);
    let master = MasterState::default();
    let draw = |state| LayerDraw {
        state,
        source: &scene,
        mask: None,
    };
    let key = thumbnail_key(effect);
    match key.as_str() {
        "feedback" => {
            // A travelling picture leaves the trail Feedback is recognised by.
            let state = layer(Some(effect.clone()));
            let frames = (0..16)
                .map(|step| reference_picture(gpu, step as f32 / 15.0))
                .collect::<anyhow::Result<Vec<_>>>()?;
            for (step, frame) in frames.iter().enumerate() {
                let draw = LayerDraw {
                    state: &state,
                    source: frame,
                    mask: None,
                };
                renderer.present(
                    &[draw],
                    &master,
                    None,
                    Timestamp::from_millis(step as u64 * 50),
                    None,
                );
            }
        }
        "beat-move" => {
            let parameters = effect
                .beat_move_parameters()
                .ok_or_else(|| anyhow::anyhow!("beat move preset"))?;
            let (x, y) = match parameters.direction {
                BeatMoveDirection::Up => (0.0, -1.0),
                BeatMoveDirection::Down => (0.0, 1.0),
                BeatMoveDirection::Left => (-1.0, 0.0),
                BeatMoveDirection::Right => (1.0, 0.0),
            };
            // Rest, a fading step, and the landed beat, spread far enough apart to read small.
            let placed = |offset: f32, state: LayerState| LayerState {
                scale_x: 0.3,
                scale_y: 0.3,
                position_x: x * offset,
                position_y: y * offset,
                ..state
            };
            let resting = placed(-0.62, ghost(layer(None), 0.35));
            let passing = placed(0.0, ghost(layer(None), 0.65));
            let landed = placed(0.62, layer(None));
            renderer.present(
                &[draw(&resting), draw(&passing), draw(&landed)],
                &master,
                None,
                at,
                None,
            );
        }
        "beat-scale-turn" => {
            let parameters = effect
                .beat_scale_turn_parameters()
                .ok_or_else(|| anyhow::anyhow!("beat scale preset"))?;
            let resting = LayerState {
                scale_x: 0.5,
                scale_y: 0.5,
                ..ghost(layer(None), 0.3)
            };
            let scale = 0.5 * (1.0 + parameters.scale_amount.max(0.3));
            let landed = LayerState {
                scale_x: scale,
                scale_y: scale,
                rotation: if parameters.turn_enabled {
                    parameters.rotation_degrees
                } else {
                    15.0
                },
                ..layer(None)
            };
            renderer.present(&[draw(&resting), draw(&landed)], &master, None, at, None);
        }
        _ => {
            let mut effect = effect.clone();
            // Beat-driven overlays draw only while an event is live: show one mid-flight.
            match key.as_str() {
                "beat-scan" => effect.parameters.extend([0.35, 1.0, 0.7, 2.0]),
                "beat-form-flash" => effect
                    .parameters
                    .extend([0.15, 0.3, 0.45, 1.0, 0.35, 0.7, 0.55, 1.2]),
                _ => {}
            }
            let state = layer(Some(effect));
            if key.starts_with("rasterize-") {
                // Print paper is transparent, so show it on white as it would appear printed.
                let paper = SourceTexture::solid(gpu, SIZE, [255, 255, 255, 255])
                    .map_err(|error| anyhow::anyhow!("{error}"))?;
                let base = layer(None);
                let paper = LayerDraw {
                    state: &base,
                    source: &paper,
                    mask: None,
                };
                renderer.present(&[paper, draw(&state)], &master, None, at, None);
            } else {
                renderer.present(&[draw(&state)], &master, None, at, None);
            }
        }
    }
    Ok(renderer.read_image())
}

fn ghost(state: LayerState, dimmer: f32) -> LayerState {
    LayerState {
        tint: Tint {
            red: 1.0,
            green: 1.0,
            blue: 1.0,
        },
        dimmer,
        ..state
    }
}

/// A saturated stage-like picture: a gradient sky, a sun, a checkered floor and bold bars, so
/// blur, rasterizing, colour and geometry changes all read at a glance. `phase` slides the sun.
fn reference_picture(gpu: &Gpu, phase: f32) -> anyhow::Result<SourceTexture> {
    let (width, height) = (SIZE.width as f32, SIZE.height as f32);
    let sun = (width * (0.28 + phase * 0.44), height * 0.36);
    let mut pixels = Vec::with_capacity((SIZE.width * SIZE.height * 4) as usize);
    for y in 0..SIZE.height {
        for x in 0..SIZE.width {
            let (fx, fy) = (x as f32, y as f32);
            let v = fy / height;
            let mut colour = [0.10 + 0.55 * (1.0 - v), 0.05 + 0.15 * v, 0.35 + 0.55 * v];
            let horizon = height * 0.62;
            if fy > horizon {
                let checker = ((x / 16) + (y / 8)) % 2 == 0;
                colour = if checker {
                    [0.95, 0.85, 0.2]
                } else {
                    [0.08, 0.08, 0.12]
                };
            }
            for (index, bar) in [0.12_f32, 0.84].iter().enumerate() {
                if (fx - width * bar).abs() < 7.0 && fy < horizon {
                    colour = if index == 0 {
                        [0.1, 0.95, 0.55]
                    } else {
                        [0.95, 0.2, 0.6]
                    };
                }
            }
            let distance = ((fx - sun.0).powi(2) + (fy - sun.1).powi(2)).sqrt();
            if distance < height * 0.2 {
                colour = [1.0, 0.95, 0.75];
            } else if distance < height * 0.23 {
                colour = [1.0, 0.45, 0.1];
            }
            pixels.extend(colour.map(|channel| (channel.clamp(0.0, 1.0) * 255.0) as u8));
            pixels.push(255);
        }
    }
    SourceTexture::from_rgba8(gpu, SIZE, &pixels).map_err(|error| anyhow::anyhow!("{error}"))
}
