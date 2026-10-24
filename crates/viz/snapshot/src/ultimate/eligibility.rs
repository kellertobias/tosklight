//! Conservative refusal for features the legacy scene converter cannot faithfully carry.
use super::UltimateError;
use std::path::Path;
use viz_scene::{EmitterKind, Scene, SceneValues, ViewConfiguration};

#[derive(Clone, Debug)]
pub struct UltimateEligibility {
    scene_revision: u64,
    reasons: Vec<String>,
    image_hash: Option<u64>,
}
impl UltimateEligibility {
    /// Inspect the already frozen state. Keep this report with that state through the chooser.
    /// A future fidelity backend must implement a feature before removing its refusal here.
    pub fn inspect(scene: &Scene, values: &SceneValues, view: &ViewConfiguration) -> Self {
        let mut reasons = Vec::new();
        if scene.emitters.len() != values.emitters.len() {
            add(
                &mut reasons,
                "resolved emitter values are incomplete for this scene".into(),
            );
        }
        if scene.physics_scenery.len() != values.physics_frames.len() {
            add(
                &mut reasons,
                "frozen physics poses are incomplete for this scene".into(),
            );
        }
        for (index, (emitter, value)) in scene.emitters.iter().zip(&values.emitters).enumerate() {
            if value.retained_visible_intensity() <= f32::EPSILON {
                continue;
            }
            let label = format!("head {}", index + 1);
            if value.gobo > 0.004
                || value.gobo_wheels.iter().any(|wheel| wheel.position > 0.004)
                || value
                    .gobo_wheel_motion
                    .visible_slot()
                    .is_some_and(|slot| slot != 0)
            {
                add(&mut reasons, format!("active gobo optics ({label})"));
            }
            if value.prism > 0.02
                || value
                    .prism_wheels
                    .iter()
                    .any(|wheel| wheel.position > 0.004)
            {
                add(&mut reasons, format!("active prism optics ({label})"));
            }
            if value.shaper_blades.iter().any(|blade| *blade > 0.0001) {
                add(&mut reasons, format!("active framing shutters ({label})"));
            }
            if emitter.kind == EmitterKind::Laser {
                add(&mut reasons, "laser scanner geometry and radiometry".into());
            }
            if !value.cells.is_empty() {
                add(
                    &mut reasons,
                    format!("independently driven multi-cell source ({label})"),
                );
            }
        }
        if values.laser_scans.iter().any(|scan| {
            scan.points
                .iter()
                .any(|point| point.colour.iter().any(|c| *c > 0.0))
        }) {
            add(&mut reasons, "frozen laser scan paths".into());
        }
        if values
            .effect_frames
            .iter()
            .any(|frame| !frame.emitters.is_empty())
        {
            add(&mut reasons, "frozen particle/effect output".into());
        }
        if !scene.media_sections.is_empty()
            || !scene.media_projectors.is_empty()
            || !scene.media_sources.is_empty()
        {
            add(
                &mut reasons,
                "frozen media textures/projectors (no image transfer to Blender yet)".into(),
            );
        }
        if values.atmosphere.density > 0.0005
            && (view.fog_variation.lamp_cloudiness > 0.0
                || view.fog_variation.laser_cloudiness > 0.0)
        {
            add(&mut reasons, "spatial/turbulent native fog".into());
        }
        if (view.camera.up - glam::Vec3::Y).length_squared() > 0.00001 {
            add(
                &mut reasons,
                "camera roll/up vector absent from legacy snapshot camera".into(),
            );
        }
        if view.ambient > f32::EPSILON {
            add(
                &mut reasons,
                "native ambient illumination absent from legacy Blender converter".into(),
            );
        }
        if view
            .background_colour()
            .iter()
            .any(|channel| *channel > f32::EPSILON)
        {
            add(
                &mut reasons,
                "non-black background absent from legacy snapshot look".into(),
            );
        }
        Self {
            scene_revision: scene.revision,
            reasons,
            image_hash: None,
        }
    }
    /// The image backend carries camera/world/media and frozen full-budget geometry explicitly.
    /// Its immutable manifest decides fidelity before the operator chooses an engine.
    pub fn inspect_image(image: &super::ImageManifest) -> Result<Self, UltimateError> {
        Ok(Self {
            scene_revision: image.scene_revision,
            reasons: image.unsupported.clone(),
            image_hash: Some(image.fingerprint()?),
        })
    }
    pub(super) fn image_backend(&self) -> bool {
        self.image_hash.is_some()
    }
    /// Additional frozen input categories can explicitly refuse unsupported transfer without
    /// weakening the existing checks (e.g. a future decoded texture or effect type).
    pub fn require_unsupported(&mut self, reason: impl Into<String>) {
        add(&mut self.reasons, reason.into());
    }
    pub fn reasons(&self) -> &[String] {
        &self.reasons
    }
    pub fn is_supported(&self) -> bool {
        self.reasons.is_empty()
    }
    pub(super) fn validate(&self, directory: &Path) -> Result<(), UltimateError> {
        if !self.reasons.is_empty() {
            return Err(UltimateError::Unsupported(self.reasons.clone()));
        }
        if let Some(hash) = self.image_hash {
            return super::image_manifest::validation::validate(
                directory,
                hash,
                self.scene_revision,
            );
        }
        let document = directory.join(crate::DOCUMENT_FILE);
        if document.metadata()?.len() > 16 * 1024 * 1024 {
            return Err(UltimateError::Failed(
                "Frozen snapshot metadata exceeds 16 MiB".into(),
            ));
        }
        let document: crate::SnapshotDocument =
            serde_json::from_reader(std::fs::File::open(document)?).map_err(|error| {
                UltimateError::Failed(format!("Invalid frozen snapshot: {error}"))
            })?;
        if document.format != crate::FORMAT_VERSION
            || document.scene_revision != self.scene_revision
        {
            return Err(UltimateError::Failed(
                "Frozen snapshot and its Ultimate fidelity report do not match".into(),
            ));
        }
        let geometry = Path::new(&document.geometry_file);
        if geometry.components().count() != 1
            || !directory.join(geometry).is_file()
            || !directory
                .join(geometry)
                .canonicalize()?
                .starts_with(directory.canonicalize()?)
        {
            return Err(UltimateError::Failed(
                "Frozen snapshot geometry must be a file inside its package".into(),
            ));
        }
        Ok(())
    }
}
fn add(reasons: &mut Vec<String>, reason: String) {
    // Reason strings and count remain bounded even for a very large show.
    let reason: String = reason.chars().take(256).collect();
    if reasons.len() < 32 && !reasons.contains(&reason) {
        reasons.push(reason);
    }
}
