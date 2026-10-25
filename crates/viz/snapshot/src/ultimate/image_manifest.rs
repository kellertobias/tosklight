//! Capture-only, immutable Cycles input. Built before the chooser; no later scene/media queries.
#[cfg(test)]
mod fidelity_tests;
mod fog;
mod geometry;
mod media;
#[cfg(test)]
mod tests;
pub(super) mod validation;

use super::{UltimateError, optics_manifest::OpticsManifest};
use glam::{Mat4, Vec3, Vec4};
pub use media::{ImageMedia, ImagePanel, ImageProjector};
use serde::Serialize;
use std::{
    hash::{DefaultHasher, Hasher},
    path::Path,
    sync::atomic::AtomicBool,
};
use viz_render::{FrameStyle, ResolvedCamera, media::FrozenMedia};
use viz_scene::{Scene, SceneValues, ViewConfiguration};

pub const IMAGE_MANIFEST_FILE: &str = "image-manifest.json";

#[derive(Serialize)]
pub struct ImageManifest {
    pub format: u32,
    pub scene_revision: u64,
    pub values_frame: u64,
    pub time_seconds: f32,
    /// Invocation-time local preference; capture budgets never increase this requested amount.
    pub crowd_amount: f32,
    /// Raw invocation-time preference, independent of the live view's quality.
    pub detailed_people: bool,
    pub geometry_file: String,
    pub geometry_hash: u64,
    pub show: String,
    pub camera: ImageCamera,
    pub look: ImageLook,
    pub bounds: crate::SnapshotBounds,
    pub media: ImageMedia,
    pub optics: OpticsManifest,
    pub fog: fog::FogManifest,
    pub unsupported: Vec<String>,
    #[serde(skip)]
    geometry: Vec<u8>,
}

#[derive(Serialize)]
pub struct ImageCamera {
    /// Column-major Blender Z-up camera-to-world transform, including invocation-time roll.
    pub world_from_camera: [[f32; 4]; 4],
    pub orthographic: bool,
    pub vertical_fov_degrees: f32,
    pub vertical_half_height: f32,
    pub near: f32,
    pub far: f32,
    /// Blender cannot use a negative orthographic near plane. Translating only its render
    /// camera backwards preserves identical XY projection and the complete native depth range.
    pub orthographic_clip_shift: f32,
}

#[derive(Serialize)]
pub struct ImageLook {
    pub background: [f32; 3],
    pub ambient: f32,
    pub exposure: f32,
    pub fog: f32,
    /// The renderer's relative ambient/exposure become explicit Cycles environment/stops.
    pub mapping: &'static str,
}

impl ImageManifest {
    pub fn build(
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
        time_seconds: f32,
        frozen: &FrozenMedia,
    ) -> Result<Self, UltimateError> {
        Self::build_with_crowd_amount(scene, values, view, time_seconds, frozen, 1.0)
    }
    pub fn build_with_crowd_amount(
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
        time_seconds: f32,
        frozen: &FrozenMedia,
        crowd_amount: f32,
    ) -> Result<Self, UltimateError> {
        if !crowd_amount.is_finite() || !(0.0..=1.0).contains(&crowd_amount) {
            return Err(UltimateError::Failed(
                "Frozen crowd amount must be between zero and one".into(),
            ));
        }
        if !time_seconds.is_finite() || time_seconds < 0.0 {
            return Err(UltimateError::Failed(
                "Frozen image time must be finite".into(),
            ));
        }
        if view
            .background_colour()
            .into_iter()
            .chain([view.ambient, view.exposure, values.atmosphere.density])
            .any(|value| !value.is_finite())
        {
            return Err(UltimateError::Failed(
                "Frozen image look is not finite".into(),
            ));
        }
        let camera =
            ResolvedCamera::resolve(&view.camera, view.mode, 3840.0 / 2160.0, scene.bounds);
        if !camera.view.is_finite() || !camera.projection.is_finite() {
            return Err(UltimateError::Failed(
                "Frozen image camera is not finite".into(),
            ));
        }
        let mut style = FrameStyle {
            quality: viz_scene::RenderQuality::Ultra,
            camera_position: Some(camera.position),
            crowd_amount,
            detailed_people: view.detailed_people,
            crowd_person_budget: usize::MAX,
            effect_particle_budget: usize::MAX,
            media_content: true,
            draw_beams: false,
            draw_aim_lines: false,
            floor_grid: view.floor_grid,
            ..Default::default()
        };
        media::set_appearances(&mut style, scene, frozen);
        let frame = viz_render::build_instances(scene, values, &style);
        let media = ImageMedia::build(scene, &frame, frozen)?;
        drop(frame);
        // Cycles GI comes from the frozen textured front, not the native average-colour
        // backing fallback. Preserve all real backing geometry with neutral media faces.
        style.media_content = false;
        let geometry = viz_render::scene_geometry_with_style(scene, values, &style);
        let optics = OpticsManifest::build(scene, values, view, time_seconds);
        let fog = fog::FogManifest::build(scene, values, view, time_seconds)?;
        let mut unsupported = optics.reasons().to_vec();
        if view.mode.is_plot() {
            unsupported.push("Ultimate does not yet preserve 2D plan symbols/outlines; choose Ultra to keep this orthographic view".into());
        }
        if scene.emitters.len() != values.emitters.len() {
            unsupported.push("resolved emitter values are incomplete for this scene".into());
        }
        if scene.physics_scenery.len() != values.physics_frames.len() {
            unsupported.push("frozen physics poses are incomplete for this scene".into());
        }
        if values.laser_scans.iter().any(|scan| {
            scan.points
                .iter()
                .any(|point| point.colour.iter().any(|c| *c > 0.0))
        }) {
            unsupported
                .push("frozen laser scan geometry/radiometry is not implemented in Cycles".into());
        }
        unsupported.truncate(32);
        let names = geometry::projector_lens_names(&geometry, &media)?;
        let geometry =
            crate::gltf::write_glb_with_names(&geometry, "ToskLight frozen Ultimate image", &names);
        let native_near = camera.projection.w_axis.z / camera.projection.z_axis.z;
        let clip_shift = if camera.orthographic {
            (-native_near + 0.05).max(0.0)
        } else {
            0.0
        };
        Ok(Self {
            format: 1,
            scene_revision: scene.revision,
            values_frame: values.frame,
            time_seconds,
            crowd_amount,
            detailed_people: view.detailed_people,
            geometry_file: crate::GEOMETRY_FILE.into(),
            geometry_hash: hash(&geometry),
            show: scene.show_name.clone(),
            camera: ImageCamera {
                world_from_camera: (z_up_matrix() * camera.view.inverse()).to_cols_array_2d(),
                orthographic: camera.orthographic,
                vertical_fov_degrees: (1.0 / camera.projection.y_axis.y.abs()).atan().to_degrees()
                    * 2.0,
                vertical_half_height: 1.0 / camera.projection.y_axis.y.abs(),
                near: camera.near,
                far: if camera.orthographic {
                    (camera.projection.w_axis.z - 1.0) / camera.projection.z_axis.z + clip_shift
                } else {
                    camera.far
                },
                orthographic_clip_shift: clip_shift,
            },
            look: ImageLook {
                background: view.background_colour(),
                ambient: view.ambient,
                exposure: view.exposure,
                fog: values.atmosphere.density,
                mapping: "Cycles white environment strength=ambient; camera background isolated; exposure=log2(native multiplier); AgX; frozen native lamp-density field mapped to Cycles volume; neutral media backing geometry with frozen textured front emission",
            },
            bounds: crate::SnapshotBounds {
                min: corner(scene.bounds, f32::min),
                max: corner(scene.bounds, f32::max),
            },
            media,
            optics,
            fog,
            unsupported,
            geometry,
        })
    }

    pub fn write_package(&self, directory: &Path) -> Result<(), UltimateError> {
        self.write_package_with_cancel(directory, &AtomicBool::new(false))
    }
    pub fn write_package_with_cancel(
        &self,
        directory: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), UltimateError> {
        super::check_cancel(cancel)?;
        if !directory.is_dir() {
            return Err(UltimateError::Failed(
                "Image package directory does not exist".into(),
            ));
        }
        write_new(directory, &self.geometry_file, &self.geometry)?;
        self.media.write_assets(directory, cancel)?;
        super::check_cancel(cancel)?;
        self.optics.write_assets_with_cancel(directory, cancel)?;
        super::check_cancel(cancel)?;
        write_new(directory, IMAGE_MANIFEST_FILE, &self.bytes()?)
    }

    pub(super) fn bytes(&self) -> Result<Vec<u8>, UltimateError> {
        serde_json::to_vec(self).map_err(|error| UltimateError::Failed(error.to_string()))
    }
    pub(super) fn fingerprint(&self) -> Result<u64, UltimateError> {
        Ok(hash(&self.bytes()?))
    }
}

pub(super) fn z_up_matrix() -> Mat4 {
    Mat4::from_cols(Vec4::X, Vec4::Z, -Vec4::Y, Vec4::W)
}
fn corner(bounds: viz_scene::Aabb, pick: fn(f32, f32) -> f32) -> [f32; 3] {
    if !bounds.min.is_finite() || !bounds.max.is_finite() {
        return [pick(-1.0, 1.0); 3];
    }
    let a = crate::to_z_up(bounds.min.to_array());
    let b = crate::to_z_up(bounds.max.to_array());
    [pick(a[0], b[0]), pick(a[1], b[1]), pick(a[2], b[2])]
}
pub(super) fn hash(bytes: &[u8]) -> u64 {
    let mut hash = DefaultHasher::new();
    hash.write(bytes);
    hash.finish()
}
pub(super) fn write_new(directory: &Path, name: &str, bytes: &[u8]) -> Result<(), UltimateError> {
    use std::io::Write;
    if Path::new(name).components().count() != 1
        || name.starts_with('.')
        || name.contains(['/', '\\'])
    {
        return Err(UltimateError::Failed(
            "Image assets require package-local filenames".into(),
        ));
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(name))?;
    file.write_all(bytes)?;
    Ok(())
}

#[cfg(test)]
mod detailed_people_tests;
