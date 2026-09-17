//! The uniform blocks the layer and master shaders read, built from domain state.

use bytemuck::{Pod, Zeroable};
use media_domain::display_region::{DisplayRegion, RegionRotation};
use media_domain::geometry::{Size, layer_transform};
use media_domain::{LayerState, MaskSource, MasterState, OutputId, Timestamp, geometry};

use crate::texture::SourceTexture;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct LayerUniform {
    pub(super) center: [f32; 2],
    pub(super) size: [f32; 2],
    pub(super) rotation: [f32; 2],
    pub(super) output: [f32; 2],
    pub(super) tint: [f32; 4],
    pub(super) controls: [f32; 4],
    pub(super) blur: [f32; 4],
    pub(super) mask: [f32; 4],
    pub(super) mask_source: [f32; 4],
    pub(super) effect_types: [u32; 4],
    pub(super) effect_mixes: [f32; 4],
    pub(super) effect_parameters: [[f32; 4]; 4],
    /// Fifth typed parameter for effects that need it, one value per slot.
    pub(super) effect_parameter_tail: [f32; 4],
    pub(super) effect_seeds: [f32; 4],
    /// Authoritative playback seconds, output width/height, spare.
    pub(super) effect_clock: [f32; 4],
    /// Transient beat-scan event base positions and strength-derived line counts. Each vec4 maps
    /// one event across effect slots 1..4; these never enter persisted layer state.
    pub(super) beat_scan_positions: [[f32; 4]; 16],
    pub(super) beat_scan_counts: [[f32; 4]; 16],
    pub(super) beat_event_x: [[f32; 4]; 16],
    pub(super) beat_event_y: [[f32; 4]; 16],
}

impl LayerUniform {
    pub(super) fn new(
        layer: &LayerState,
        source: Size,
        output: Size,
        mask: Option<&SourceTexture>,
        output_id: OutputId,
        now: Timestamp,
    ) -> Self {
        let transform = layer_transform(layer, source, output);
        let (sin, cos) = transform.rotation_degrees.to_radians().sin_cos();
        let mut effect_types = [0; 4];
        let mut effect_mixes = [0.0; 4];
        let mut effect_parameters = [[0.0; 4]; 4];
        let mut effect_parameter_tail = [0.0; 4];
        let mut effect_seeds = [0.0; 4];
        let mut beat_scan_positions = [[-2.0; 4]; 16];
        let mut beat_scan_counts = [[0.0; 4]; 16];
        let mut beat_event_x = [[0.5; 4]; 16];
        let mut beat_event_y = [[0.5; 4]; 16];
        for (index, effect) in layer.effects.iter().enumerate() {
            if let Some(parameters) = effect.analog_tv_parameters() {
                effect_types[index] = 1;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index] = parameters.as_array();
                effect_seeds[index] = effect_seed(output_id, effect.seed, index);
            } else if let Some(parameters) = effect.digital_tv_parameters() {
                let values = parameters.as_array();
                effect_types[index] = 2;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index].copy_from_slice(&values[..4]);
                effect_parameter_tail[index] = values[4];
                effect_seeds[index] = effect_seed(output_id, effect.seed, index);
            } else if let Some(parameters) = effect.blur_parameters() {
                effect_types[index] = 3;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index][0] = parameters.amount;
                effect_parameters[index][1] = parameters.blur_type.parameter();
            } else if let Some(parameters) = effect.kaleidoscope_parameters() {
                effect_types[index] = 4;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index][0] = f32::from(parameters.repetitions);
                effect_parameters[index][1] = parameters.angle_degrees;
            } else if let Some(parameters) = effect.rasterize_parameters() {
                effect_types[index] = 5;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index][0] = parameters.mode.parameter();
                effect_parameters[index][1] = parameters.dot_size;
            } else if let Some(parameters) = effect.beat_scan_parameters() {
                effect_types[index] = 6;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index] = parameters.as_array();
                let (events, _) = effect.parameters[4..].as_chunks::<2>();
                for (event, values) in events.iter().take(16).enumerate() {
                    beat_scan_positions[event][index] = values[0];
                    beat_scan_counts[event][index] = values[1];
                }
            } else if let Some(parameters) = effect.beat_grid_wave_parameters() {
                let values = parameters.as_array();
                effect_types[index] = 7;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index].copy_from_slice(&values[..4]);
                effect_parameter_tail[index] = values[4];
                effect_seeds[index] = values[5];
                let (events, _) = effect.parameters[6..].as_chunks::<2>();
                for (event, values) in events.iter().take(16).enumerate() {
                    beat_scan_positions[event][index] = values[0];
                    beat_scan_counts[event][index] = values[1];
                }
            } else if let Some(parameters) = effect.beat_form_flash_parameters() {
                effect_types[index] = 8;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index] = parameters.as_array();
                let (events, _) = effect.parameters[4..].as_chunks::<4>();
                for (event, values) in events.iter().take(16).enumerate() {
                    beat_scan_positions[event][index] = values[0];
                    beat_event_x[event][index] = values[1];
                    beat_event_y[event][index] = values[2];
                    beat_scan_counts[event][index] = values[3];
                }
            } else if let Some(parameters) = effect.drawn_image_parameters() {
                effect_types[index] = 9;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                let values = parameters.as_array();
                effect_parameters[index][0] = values[0];
                effect_parameters[index][1] = values[1];
            } else if let Some(parameters) = effect.outline_parameters() {
                // Line colour and the (beat-modulated) intensity, then thickness and sensitivity
                // in the spare per-slot rows this effect has no other use for.
                let [red, green, blue] = parameters.colour();
                effect_types[index] = 10;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index] = [red, green, blue, parameters.intensity];
                effect_parameter_tail[index] = parameters.thickness;
                effect_seeds[index] = parameters.sensitivity;
            }
        }
        Self {
            center: [transform.center.x, transform.center.y],
            size: [transform.size.0, transform.size.1],
            rotation: [cos, sin],
            output: [output.width as f32, output.height as f32],
            // Layer dimmer becomes the alpha of the layer tint.
            tint: [
                layer.tint.red,
                layer.tint.green,
                layer.tint.blue,
                layer.dimmer,
            ],
            controls: [layer.grayscale, 0.0, 0.0, 0.0],
            blur: [layer.blur.clamp(0.0, 1.0), 0.0, 0.0, 0.0],
            // A mask that is selected but not loaded reports no opacity, so the layer draws
            // unmasked rather than vanishing while its mask is on its way.
            mask: [
                layer.mask.scale_x,
                layer.mask.scale_y,
                f32::from(u8::from(layer.mask.invert)),
                if mask.is_some() && layer.mask.is_active() {
                    layer.mask.opacity
                } else {
                    0.0
                },
            ],
            mask_source: [
                f32::from(u8::from(layer.mask.source == MaskSource::Alpha)),
                layer.mask.position_x,
                layer.mask.position_y,
                0.0,
            ],
            effect_types,
            effect_mixes,
            effect_parameters,
            effect_parameter_tail,
            effect_seeds,
            effect_clock: [
                (now.as_micros() as f64 / 1_000_000.0) as f32,
                output.width as f32,
                output.height as f32,
                0.0,
            ],
            beat_scan_positions,
            beat_scan_counts,
            beat_event_x,
            beat_event_y,
        }
    }
}

fn effect_seed(output: OutputId, seed: u32, slot: usize) -> f32 {
    let mut hash = 2_166_136_261_u32;
    for byte in output
        .as_uuid()
        .as_bytes()
        .iter()
        .copied()
        .chain(seed.to_le_bytes())
        .chain((slot as u32).to_le_bytes())
    {
        hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
    }
    (hash & 0x00ff_ffff) as f32 / 0x00ff_ffff as f32
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct MasterUniform {
    pub(super) tint: [f32; 4],
    pub(super) flip_mask: [f32; 4],
    pub(super) transform: [f32; 4],
    pub(super) rotation: [f32; 4],
    pub(super) mask_transform: [f32; 4],
    pub(super) shaper_edges: [f32; 4],
    pub(super) shaper_edge_tangents: [f32; 4],
    pub(super) region: [f32; 4],
    pub(super) region_rotation: [f32; 4],
}

impl MasterUniform {
    pub(super) fn new(
        master: &MasterState,
        mask: Option<&SourceTexture>,
        preserve_alpha: bool,
        region: Option<&DisplayRegion>,
    ) -> Self {
        let (flip_x, flip_y) = geometry::flip_signs(master.flip_mirror);
        let (scale_x, scale_y) = master.effective_scale();
        // A negative scale mirrors exactly as Flip/mirror does, so the composite quad keeps a
        // positive size and the master mask and shapers stay where the operator placed them.
        let (horizontal, vertical) = (flip_x * scale_x.signum(), flip_y * scale_y.signum());
        let (scale_x, scale_y) = (scale_x.abs(), scale_y.abs());
        Self {
            tint: [
                master.tint.red,
                master.tint.green,
                master.tint.blue,
                master.dimmer,
            ],
            flip_mask: [
                horizontal,
                vertical,
                if mask.is_some() && master.has_mask() {
                    1.0
                } else {
                    0.0
                },
                if preserve_alpha { -1.0 } else { 0.0 },
            ],
            transform: [master.position_x, master.position_y, scale_x, scale_y],
            rotation: [
                master.rotation.to_radians().cos(),
                master.rotation.to_radians().sin(),
                master.shaper.rotation.to_radians().cos(),
                master.shaper.rotation.to_radians().sin(),
            ],
            mask_transform: [master.mask_position_x, master.mask_position_y, 0.0, 0.0],
            shaper_edges: [
                master.shaper.left,
                master.shaper.right,
                master.shaper.top,
                master.shaper.bottom,
            ],
            shaper_edge_tangents: [
                master.shaper.left_rotation.to_radians().tan(),
                master.shaper.right_rotation.to_radians().tan(),
                master.shaper.top_rotation.to_radians().tan(),
                master.shaper.bottom_rotation.to_radians().tan(),
            ],
            region: region.map_or([0.0, 0.0, 1.0, 1.0], |region| {
                let start_x = region.source.start.x.min(region.source.end.x);
                let start_y = region.source.start.y.min(region.source.end.y);
                [
                    start_x,
                    start_y,
                    region.source.width(),
                    region.source.height(),
                ]
            }),
            // A quarter-turn is exact, so its cosine and sine are written rather than computed
            // from an angle that would land a hair off zero.
            region_rotation: match region.map(|region| region.rotation) {
                Some(RegionRotation::Clockwise90) => [0.0, 1.0, 0.0, 0.0],
                Some(RegionRotation::Half) => [-1.0, 0.0, 0.0, 0.0],
                Some(RegionRotation::CounterClockwise90) => [0.0, -1.0, 0.0, 0.0],
                Some(RegionRotation::None) | None => [1.0, 0.0, 0.0, 0.0],
            },
        }
    }
}
