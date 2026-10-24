use super::*;
use std::collections::BTreeMap;
use viz_render::{FrameInstances, media::FrozenMediaSource};
use viz_scene::uuid::Uuid;

#[derive(Serialize, Default)]
pub struct ImageMedia {
    pub assets: Vec<MediaAsset>,
    pub panels: Vec<ImagePanel>,
    pub projectors: Vec<ImageProjector>,
    #[serde(skip)]
    frozen: Vec<FrozenMediaSource>,
}
#[derive(Serialize)]
pub struct MediaAsset {
    pub source_id: Uuid,
    pub sequence: u64,
    pub available: bool,
    pub flicker: f32,
    pub file: String,
    pub width: u32,
    pub height: u32,
    pub rgba_hash: u64,
}
#[derive(Serialize)]
pub struct ImagePanel {
    pub world_from_panel: [[f32; 4]; 4],
    pub crop: [f32; 4],
    /// Native kind (screen=0, TV=1, LED=2), gain, roughness and edge feather/pixel pitch.
    pub material: [f32; 4],
    pub source_id: Option<Uuid>,
    pub fallback_source_id: Option<Uuid>,
    pub selected_file: Option<String>,
}
#[derive(Serialize)]
pub struct ImageProjector {
    pub id: Uuid,
    pub lens_node: String,
    #[serde(skip)]
    pub(super) native_lens_model: [[f32; 4]; 4],
    pub name: String,
    pub position: [f32; 3],
    pub direction: [f32; 3],
    pub colour: [f32; 3],
    pub spill: f32,
    pub flicker_gain: f32,
    pub reach: f32,
}

impl ImageMedia {
    pub(super) fn build(
        scene: &Scene,
        frame: &FrameInstances,
        frozen: &FrozenMedia,
    ) -> Result<Self, UltimateError> {
        if frozen.sources().len() >= viz_render::media::MAX_SOURCES as usize {
            return Err(UltimateError::Failed(
                "Frozen media exceeds the native source budget".into(),
            ));
        }
        let mut result = Self {
            frozen: frozen.sources().to_vec(),
            ..Default::default()
        };
        for source in frozen.sources() {
            let pixels = &source.frame;
            if pixels.width != viz_render::media::EDGE
                || pixels.height != viz_render::media::EDGE
                || pixels.rgba.len() != (pixels.width * pixels.height * 4) as usize
                || !source.flicker.is_finite()
            {
                return Err(UltimateError::Failed(
                    "Invalid frozen media pixels or flicker".into(),
                ));
            }
            result.assets.push(MediaAsset {
                source_id: pixels.source_id,
                sequence: pixels.sequence,
                available: source.available,
                flicker: source.flicker,
                file: format!("media-{}.png", pixels.source_id),
                width: pixels.width,
                height: pixels.height,
                rgba_hash: hash(&pixels.rgba),
            });
        }
        for panel in &frame.media_panels {
            if !panel.model.is_finite()
                || panel
                    .crop
                    .iter()
                    .chain(&panel.material)
                    .any(|value| !value.is_finite())
            {
                return Err(UltimateError::Failed(
                    "Frozen media panel transform or crop is not finite".into(),
                ));
            }
            let selected =
                selected_source(frozen.sources(), panel.source_id, panel.fallback_source_id);
            result.panels.push(ImagePanel {
                world_from_panel: (z_up_matrix() * panel.model).to_cols_array_2d(),
                crop: panel.crop,
                material: panel.material,
                source_id: panel.source_id,
                fallback_source_id: panel.fallback_source_id,
                selected_file: selected
                    .map(|source| format!("media-{}.png", source.frame.source_id)),
            });
        }
        let appearance = appearances(scene, frozen);
        for projector in &scene.media_projectors {
            if !projector.position.is_finite()
                || !projector.rotation_degrees.is_finite()
                || !projector.cone_length_metres.is_finite()
                || projector.cone_length_metres <= 0.0
                || !projector.spill.is_finite()
            {
                return Err(UltimateError::Failed(
                    "Frozen projector pose, reach and spill must be finite with positive reach"
                        .into(),
                ));
            }
            let rotation = viz_scene::euler_degrees(projector.rotation_degrees);
            let direction = (rotation * Vec3::NEG_Z).normalize_or(Vec3::NEG_Z);
            let (colour, flicker) = appearance
                .get(&projector.surface_id)
                .copied()
                .unwrap_or_default();
            result.projectors.push(ImageProjector {
                id: projector.id,
                lens_node: format!("tosklight-projector-lens-{}", projector.id),
                native_lens_model: Mat4::from_scale_rotation_translation(
                    Vec3::new(0.1, 0.1, 0.025),
                    glam::Quat::from_rotation_arc(Vec3::Z, direction),
                    projector.position + direction * 0.3,
                )
                .to_cols_array_2d(),
                name: projector.name.clone(),
                position: crate::to_z_up((projector.position + direction * 0.3).to_array()),
                direction: crate::to_z_up(direction.to_array()),
                colour,
                spill: projector.spill.clamp(0.0, 1.0),
                flicker_gain: (1.0 + flicker * 2.0).clamp(1.0, 1.8),
                reach: projector.cone_length_metres,
            });
        }
        Ok(result)
    }

    pub(super) fn write_assets(
        &self,
        directory: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), UltimateError> {
        for (asset, source) in self.assets.iter().zip(&self.frozen) {
            crate::ultimate::check_cancel(cancel)?;
            let mut bytes = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut bytes, asset.width, asset.height);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder
                    .write_header()
                    .map_err(|error| UltimateError::Failed(error.to_string()))?
                    .write_image_data(&source.frame.rgba)
                    .map_err(|error| UltimateError::Failed(error.to_string()))?;
            }
            write_new(directory, &asset.file, &bytes)?;
        }
        Ok(())
    }
}

pub(super) fn selected_source(
    frozen: &[FrozenMediaSource],
    source: Option<Uuid>,
    fallback: Option<Uuid>,
) -> Option<&FrozenMediaSource> {
    source
        .and_then(|id| {
            frozen
                .iter()
                .find(|source| source.frame.source_id == id && source.available)
        })
        .or_else(|| {
            fallback.and_then(|id| frozen.iter().find(|source| source.frame.source_id == id))
        })
}

fn appearances(scene: &Scene, frozen: &FrozenMedia) -> BTreeMap<Uuid, ([f32; 3], f32)> {
    scene
        .media_surfaces_with_sources()
        .into_iter()
        .map(|(surface, source, fallback)| {
            let (average, flicker) = selected_source(frozen.sources(), source, fallback).map_or(
                ([0.0; 3], 0.0),
                |source| {
                    // Identical sampling/byte average to native MediaAtlas::appearance, frozen once.
                    let mut sum = [0_u64; 3];
                    let mut count = 0_u64;
                    for pixel in source.frame.rgba.chunks_exact(4).step_by(64) {
                        for channel in 0..3 {
                            sum[channel] += u64::from(pixel[channel]);
                        }
                        count += 1;
                    }
                    (
                        sum.map(|sum| sum as f32 / count.max(1) as f32 / 255.0),
                        source.flicker,
                    )
                },
            );
            (surface, (average, flicker))
        })
        .collect()
}

pub(super) fn set_appearances(style: &mut FrameStyle, scene: &Scene, frozen: &FrozenMedia) {
    for (surface, (average, flicker)) in appearances(scene, frozen) {
        let appearance = style.media_appearance.entry(surface).or_default();
        appearance.average = Vec3::from(average);
        appearance.flicker = flicker;
    }
}
