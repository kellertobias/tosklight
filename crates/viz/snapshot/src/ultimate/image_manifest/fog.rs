//! Frozen native lamp-density field; the Cycles transport coefficient is an explicit mapping.
use super::*;

const EDGE: usize = 32;
#[derive(Serialize)]
pub struct FogManifest {
    pub density: f32,
    pub lamp_cloudiness: f32,
    pub lamp_turbulence: f32,
    pub time_seconds: f32,
    pub noise_edge: usize,
    pub noise_voxels: Vec<u8>,
    /// Blender Z-up air bounds include every native light's complete finite reach.
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub mapping: &'static str,
}
impl FogManifest {
    pub fn build(
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
        time_seconds: f32,
    ) -> Result<Self, UltimateError> {
        let authored = view.fog_variation;
        if !time_seconds.is_finite()
            || time_seconds < 0.0
            || !values.atmosphere.density.is_finite()
            || [
                authored.lamp_cloudiness,
                authored.lamp_turbulence,
                authored.laser_cloudiness,
                authored.laser_turbulence,
            ]
            .iter()
            .any(|x| !x.is_finite())
        {
            return Err(UltimateError::Failed(
                "Frozen fog parameters must be finite".into(),
            ));
        }
        // Ultimate is HQ: retain the authored Ultra field even if the live lane was Draft.
        let fog = authored.for_quality(viz_scene::RenderQuality::Ultra);
        let density = values.atmosphere.density.clamp(0.0, 1.0);
        let mut low = view.camera.position - Vec3::splat(6.0);
        let mut high = view.camera.position + Vec3::splat(6.0);
        if !scene.bounds.is_empty() {
            low = low.min(scene.bounds.min - Vec3::splat(6.0));
            high = high.max(scene.bounds.max + Vec3::splat(6.0));
        }
        for light in viz_render::semantic_lights(scene, values) {
            if !light.origin.is_finite() || !light.reach.is_finite() {
                return Err(UltimateError::Failed(
                    "Frozen fog light bounds must be finite".into(),
                ));
            }
            let extent = Vec3::splat(light.reach.max(0.0) + light.source_radius.max(0.0) + 1.0);
            low = low.min(light.origin - extent);
            high = high.max(light.origin + extent);
        }
        for projector in &scene.media_projectors {
            if !projector.position.is_finite() || !projector.cone_length_metres.is_finite() {
                return Err(UltimateError::Failed(
                    "Frozen fog projector bounds must be finite".into(),
                ));
            }
            let extent = Vec3::splat(projector.cone_length_metres.max(0.0) + 1.0);
            low = low.min(projector.position - extent);
            high = high.max(projector.position + extent);
        }
        let a = Vec3::from_array(crate::to_z_up(low.to_array()));
        let b = Vec3::from_array(crate::to_z_up(high.to_array()));
        if !a.is_finite() || !b.is_finite() || !(high - low).is_finite() {
            return Err(UltimateError::Failed(
                "Frozen fog air bounds must be finite".into(),
            ));
        }
        Ok(Self {
            density,
            lamp_cloudiness: fog.lamp_cloudiness,
            lamp_turbulence: fog.lamp_turbulence,
            time_seconds,
            noise_edge: EDGE,
            noise_voxels: if density > 0.0005 && fog.lamp_cloudiness > 0.001 {
                noise_voxels()
            } else {
                vec![]
            },
            min: a.min(b).to_array(),
            max: a.max(b).to_array(),
            mapping: "Native periodic R8 trilinear lamp field, three octaves and frozen-time drift/shaping; Cycles volume density=0.06*native_density*field, anisotropy=0.35. Cycles transport differs from native per-beam integration/opacity. Independent laser haze remains unsupported.",
        })
    }
}

/// Exact u64 xorshift and byte order used by viz-render renderer::build_haze_volume.
fn noise_voxels() -> Vec<u8> {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    (0..EDGE * EDGE * EDGE)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 33) as u8
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_default_cloudiness_transfers_exact_native_voxels_and_time() {
        let scene = Scene::default();
        let mut values = SceneValues::default();
        values.atmosphere.density = 0.2;
        let view = ViewConfiguration::default();
        let frozen = FogManifest::build(&scene, &values, &view, 17.5).unwrap();
        assert_eq!(frozen.noise_voxels.len(), 32 * 32 * 32);
        assert_eq!(
            &frozen.noise_voxels[..8],
            &[5, 195, 32, 253, 150, 25, 66, 146]
        );
        assert_eq!(frozen.lamp_cloudiness, 0.7);
        assert_eq!(frozen.lamp_turbulence, 1.0);
        assert_eq!(frozen.time_seconds, 17.5);
        assert!(frozen.min.iter().zip(frozen.max).all(|(a, b)| *a < b));
    }
    #[test]
    fn uniform_or_clear_air_needs_no_noise_and_nonfinite_time_is_rejected() {
        let scene = Scene::default();
        let mut values = SceneValues::default();
        let mut view = ViewConfiguration::default();
        values.atmosphere.density = 0.2;
        view.fog_variation.lamp_cloudiness = 0.0;
        assert!(
            FogManifest::build(&scene, &values, &view, 0.0)
                .unwrap()
                .noise_voxels
                .is_empty()
        );
        view.fog_variation.lamp_cloudiness = 0.7;
        values.atmosphere.density = 0.0;
        assert!(
            FogManifest::build(&scene, &values, &view, 0.0)
                .unwrap()
                .noise_voxels
                .is_empty()
        );
        assert!(FogManifest::build(&scene, &values, &view, f32::INFINITY).is_err());
    }
}
