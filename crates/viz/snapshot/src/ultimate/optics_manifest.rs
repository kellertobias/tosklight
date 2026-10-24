//! Frozen native angular emission, not a second interpretation of fixture channel profiles.
//! Projection uses Cycles' rotation-aware light Normal coordinates:
//! https://docs.blender.org/manual/en/latest/render/shader_nodes/input/texture_coordinate.html
mod gate;
#[cfg(test)]
mod tests;
use super::UltimateError;
use gate::Gate;
use glam::Vec3;
use serde::Serialize;
use std::{
    collections::HashMap,
    fs::OpenOptions,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use viz_scene::{GoboArtwork, RenderQuality, Scene, SceneValues, ViewConfiguration};
const EDGE: u32 = 512;
const MAX_MASKS: usize = 256;
const MAX_ARTWORK_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Serialize)]
pub struct OpticsManifest {
    pub lights: Vec<OpticalLight>,
    pub time_seconds: f32,
    pub angular_mask_edge: u32,
    pub angular_mask_peak: f32,
    #[serde(skip)]
    unsupported: Vec<String>,
    #[serde(skip)]
    masks: Vec<Gate>,
    #[serde(skip)]
    artwork: Vec<GoboArtwork>,
}
#[derive(Clone, Serialize)]
pub struct OpticalLight {
    pub fixture_id: String,
    pub fixture_instance_id: String,
    pub emitter_index: u32,
    pub cell_index: u32,
    pub name: String,
    pub origin: [f32; 3],
    pub direction: [f32; 3],
    pub tangent: [f32; 3],
    pub colour: [f32; 3],
    pub power_watts: f32,
    pub cone_radians: f32,
    pub source_radius: f32,
    pub reach: f32,
    pub mask_file: String,
}
impl OpticsManifest {
    pub fn build(
        scene: &Scene,
        values: &SceneValues,
        _view: &ViewConfiguration,
        time: f32,
    ) -> Self {
        let mut made = Self {
            lights: Vec::new(),
            time_seconds: time,
            angular_mask_edge: EDGE,
            angular_mask_peak: gate::PEAK,
            unsupported: Vec::new(),
            masks: Vec::new(),
            artwork: Vec::new(),
        };
        if !time.is_finite() || time < 0.0 {
            made.unsupported
                .push("Optics require a finite frozen scene time".into());
            return made;
        }
        let bytes = scene
            .gobo_artwork
            .iter()
            .try_fold(0usize, |sum, image| sum.checked_add(image.mask.len()));
        if bytes.is_none_or(|bytes| bytes > MAX_ARTWORK_BYTES) {
            made.unsupported
                .push("Frozen gobo artwork exceeds the 64 MiB image-job budget".into());
            return made;
        }
        let semantic = viz_render::semantic_lights(scene, values);
        if semantic.len() > 4096 {
            made.unsupported
                .push("Ultimate supports at most 4096 emitting cells per image".into());
            return made;
        }
        // Media lights are owned by image_scene.py. The native builder supplies all profile,
        // physical-pose, cell and wheel-motion interpretation, with no GPU allocation.
        let frame = viz_render::build_instances(
            scene,
            values,
            &viz_render::FrameStyle {
                quality: RenderQuality::Ultra,
                media_content: false,
                draw_beams: false,
                floor_grid: false,
                scenery: |_| false,
                crowd_amount: 0.0,
                crowd_person_budget: 0,
                effect_particle_budget: 0,
                ..Default::default()
            },
        );
        if semantic.len() != frame.lights.len() {
            made.unsupported.push("Native semantic and optical light identities do not agree; refusing mismatched cells".into());
            return made;
        }
        let mut masks = HashMap::new();
        for (semantic, packed) in semantic.iter().zip(&frame.lights) {
            let origin = Vec3::from_slice(&packed.position_range[..3]);
            if !origin.is_finite()
                || !semantic.direction.is_finite()
                || !semantic.colour.is_finite()
                || !semantic.source_radius.is_finite()
                || !semantic.reach.is_finite()
                || semantic.reach <= 0.0
                || origin.distance_squared(semantic.origin) > 1e-8
            {
                made.unsupported
                    .push("Native semantic and optical cell origins disagree".into());
                break;
            }
            let gate = Gate {
                cos_outer: packed.direction_cos_outer[3],
                params: packed.params,
                frost: packed.tangent_frost[3],
                shapers: packed.shapers,
                angles: packed.shaper_angles,
                gobos: packed.gobo_wheels,
                prisms: packed.prism_wheels,
                gobo_count: packed.gate[1] as usize,
                prism_count: packed.gate[2] as usize,
            };
            if let Some(reason) = gate.invalid_reason(&scene.gobo_artwork) {
                made.unsupported.push(reason);
                break;
            }
            let key = gate.key();
            let slot = if let Some(&slot) = masks.get(&key) {
                slot
            } else {
                if made.masks.len() == MAX_MASKS {
                    made.unsupported.push(
                        "Ultimate exceeds 256 distinct frozen optical masks; choose Ultra".into(),
                    );
                    break;
                }
                let slot = made.masks.len();
                made.masks.push(gate);
                masks.insert(key, slot);
                slot
            };
            let emitter = &scene.emitters[semantic.emitter_index as usize];
            let fixture = &scene.fixtures[emitter.fixture_index as usize];
            made.lights.push(OpticalLight {
                fixture_id: fixture.fixture_id.to_string(),
                fixture_instance_id: fixture.instance_id.to_string(),
                emitter_index: semantic.emitter_index,
                cell_index: semantic.cell_index,
                name: format!(
                    "{} {} cell {}",
                    fixture.name,
                    emitter.label,
                    semantic.cell_index + 1
                ),
                origin: crate::to_z_up(origin.to_array()),
                direction: crate::to_z_up(semantic.direction.to_array()),
                tangent: crate::to_z_up([
                    packed.tangent_frost[0],
                    packed.tangent_frost[1],
                    packed.tangent_frost[2],
                ]),
                colour: semantic.colour.to_array(),
                power_watts: crate::watts(
                    semantic.output,
                    semantic.intensity,
                    semantic.outer_half_angle,
                ),
                cone_radians: semantic.outer_half_angle * 2.0,
                source_radius: semantic.source_radius,
                reach: semantic.reach,
                mask_file: format!("optics-mask-{slot:04}.png"),
            });
        }
        made.artwork = scene.gobo_artwork.clone();
        made
    }
    pub fn reasons(&self) -> &[String] {
        &self.unsupported
    }
    pub fn asset_files(&self) -> Vec<String> {
        (0..self.masks.len())
            .map(|slot| format!("optics-mask-{slot:04}.png"))
            .collect()
    }
    /// Worker-only CPU rasterization. Files are unique job-owned assets, never operator images.
    /// 512-square 16-bit masks match the native gobo atlas scale; PNG stores linear transmission.
    pub fn write_assets(&self, directory: &Path) -> Result<(), UltimateError> {
        self.write_assets_with_cancel(directory, &AtomicBool::new(false))
    }
    pub fn write_assets_with_cancel(
        &self,
        directory: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), UltimateError> {
        if !self.unsupported.is_empty() {
            return Err(UltimateError::Unsupported(self.unsupported.clone()));
        }
        for (slot, gate) in self.masks.iter().enumerate() {
            if cancel.load(Ordering::Acquire) {
                return Err(UltimateError::Cancelled);
            }
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join(format!("optics-mask-{slot:04}.png")))?;
            let mut encoder = png::Encoder::new(file, EDGE, EDGE);
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::Sixteen);
            let mut bytes = Vec::with_capacity((EDGE * EDGE * 2) as usize);
            for y in 0..EDGE {
                if cancel.load(Ordering::Acquire) {
                    return Err(UltimateError::Cancelled);
                }
                for x in 0..EDGE {
                    let position = glam::Vec2::new(
                        (x as f32 + 0.5) / EDGE as f32 * 2.0 - 1.0,
                        (y as f32 + 0.5) / EDGE as f32 * 2.0 - 1.0,
                    );
                    let value = (gate.transmission(position, &self.artwork) / gate::PEAK * 65535.0)
                        .clamp(0.0, 65535.0)
                        .round() as u16;
                    bytes.extend(value.to_be_bytes());
                }
            }
            let mut writer = encoder
                .write_header()
                .map_err(|e| UltimateError::Failed(format!("Optical mask header: {e}")))?;
            writer
                .write_image_data(&bytes)
                .map_err(|e| UltimateError::Failed(format!("Optical mask PNG: {e}")))?;
            writer
                .finish()
                .map_err(|e| UltimateError::Failed(format!("Optical mask completion: {e}")))?;
        }
        Ok(())
    }
}
