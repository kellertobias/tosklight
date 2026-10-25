//! Draft instance preparation identity. Clocks/uniform-only changes do not rebuild CPU arrays.
//! Raw model/projection buffers are owned by the existing content-keyed asset reconciliation.
use crate::instances::FrameStyle;
use crate::renderer::runtime_hash::RuntimeHasher;
use std::{hash::Hasher, io::Write};
use viz_scene::{Scene, SceneValues};

#[cfg(test)]
use std::hash::DefaultHasher;

#[derive(Default)]
pub(in crate::renderer) struct Preparation {
    key: Option<u64>,
}
impl Preparation {
    pub(in crate::renderer) fn update(
        &mut self,
        scene: &Scene,
        values: &SceneValues,
        style: &FrameStyle,
        assets_changed: bool,
    ) -> bool {
        let next = fingerprint(scene, values, style);
        let changed = assets_changed || self.key != Some(next);
        self.key = Some(next);
        changed
    }
}
#[derive(Default)]
struct Key<H: Hasher = RuntimeHasher>(H);
impl<H: Hasher> Write for Key<H> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.write(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<H: Hasher> Key<H> {
    fn add(&mut self, value: &impl serde::Serialize) {
        postcard::to_io(value, &mut *self).expect("Draft instance metadata is serializable");
        self.0.write_u8(0xff);
    }
}
fn fingerprint(scene: &Scene, values: &SceneValues, style: &FrameStyle) -> u64 {
    fingerprint_with::<RuntimeHasher>(scene, values, style)
}
fn fingerprint_with<H: Hasher + Default>(
    scene: &Scene,
    values: &SceneValues,
    style: &FrameStyle,
) -> u64 {
    let mut key = Key::<H>::default();
    authored(&mut key, scene);
    live(&mut key, values);
    appearance(&mut key, style);
    key.0.finish()
}
fn authored(key: &mut Key<impl Hasher>, scene: &Scene) {
    // Exhaustive destructuring makes newly added scene fields an explicit cache-policy decision.
    let Scene {
        physical_positions,
        physical_position_indices,
        revision,
        show_id,
        source_show_revision,
        show_name,
        source_identity,
        fixtures,
        emitters,
        emitter_ids,
        scenery,
        physics_scenery,
        crowds,
        media_sources,
        media_sections,
        media_projectors,
        models,
        plan_artwork: _,
        fixture_plan,
        gobo_artwork: _,
        venue_groups,
        bounds,
    } = scene;
    key.add(&(
        revision,
        show_id,
        source_show_revision,
        show_name,
        source_identity,
    ));
    key.add(&(
        physical_positions,
        physical_position_indices,
        fixtures,
        emitters,
        emitter_ids,
    ));
    key.add(&(
        scenery,
        physics_scenery,
        crowds,
        media_sections,
        media_projectors,
    ));
    key.add(&media_sources.len());
    for source in media_sources {
        // Pixels are already represented by the adopted atlas appearance below. Hashing
        // their 512-square RGBA payload here would turn a held video into per-frame CPU work.
        let viz_scene::MediaSourceBinding {
            id,
            server_id,
            host,
            port,
            advertised_source_id,
            name,
            aspect_ratio,
            fallback_rgba: _,
        } = source;
        key.add(&(
            id,
            server_id,
            host,
            port,
            advertised_source_id,
            name,
            aspect_ratio,
        ));
    }
    key.add(&(fixture_plan, venue_groups, bounds));
    key.add(&models.len());
    for model in models {
        key.add(&(
            model.extent,
            model.head_pivot,
            model.emitter_anchor,
            model.emitter_size,
            model.emitter_axis,
            model.has_head,
            model.bracket_hinge,
            model.parts.len(),
        ));
        for part in &model.parts {
            key.add(&(
                &part.name,
                part.kind,
                part.geometry_node_id,
                &part.node_ancestry,
                part.colour,
                part.roughness,
                part.metallic,
            ));
        }
    }
    // Gobo pixels are sampled through the atlas, which is still updated on every frame.
    // Geometry content changes force update() through AssetCache before this key is considered.
}
fn live(key: &mut Key<impl Hasher>, values: &SceneValues) {
    let SceneValues {
        physical_positions,
        emitters,
        laser_scans,
        effect_frames,
        physics_frames,
        atmosphere: _,
        frame: _,
        newest_input_micros: _,
        external_camera: _,
        position_points,
        selected_fixtures,
    } = values;
    key.add(&(
        physical_positions,
        emitters,
        laser_scans,
        effect_frames,
        physics_frames,
        position_points,
    ));
    let mut selected: Vec<_> = selected_fixtures.iter().collect();
    selected.sort_unstable();
    key.add(&selected);
    // Haze density/time are shader uniforms; effect timelines and physical motion above are not.
    // The external camera has already been resolved into the actual style camera by the caller.
}
fn appearance(key: &mut Key<impl Hasher>, style: &FrameStyle) {
    key.add(&(
        style.camera_position,
        style.quality,
        style.draw_beams,
        style.draw_aim_lines,
        style.plot,
        style.plot_right,
        style.plot_up,
        style.projection_view,
        style.symbol_metres,
    ));
    key.add(&(
        style.ink,
        style.faint_ink,
        style.beam_ink,
        style.symbol_ink,
        style.selected_ink,
        style.group_selected_ink,
    ));
    key.add(&(
        style.fixture_models,
        style.emitter_apertures,
        style.scenery_surfaces,
        style.aim_guides,
        style.floor_grid,
        style.scenery as usize,
    ));
    key.add(&(
        style.crowd_amount,
        style.detailed_people && style.quality == viz_scene::RenderQuality::Ultra,
        style.crowd_person_budget,
        style.effect_particle_budget,
        style.media_content,
    ));
    key.add(&style.media_appearance.len());
    for (id, source) in &style.media_appearance {
        key.add(&(id, source.average, source.flicker));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use viz_scene::{EffectFrame, EmitterValues, FixtureInstance, ModelPart};

    #[test]
    fn retained_media_pixels_do_not_enter_instance_preparation_identity() {
        let id = viz_scene::uuid::Uuid::new_v4();
        let mut scene = Scene::default();
        scene.media_sources.push(viz_scene::MediaSourceBinding {
            id,
            server_id: id,
            host: String::new(),
            port: 0,
            advertised_source_id: 0,
            name: "Fallback".into(),
            aspect_ratio: Some(1.0),
            fallback_rgba: Some(vec![0; 512 * 512 * 4]),
        });
        let values = SceneValues::default();
        let mut style = FrameStyle::default();
        let mut cache = Preparation::default();
        assert!(cache.update(&scene, &values, &style, false));
        scene.media_sources[0]
            .fallback_rgba
            .as_mut()
            .unwrap()
            .fill(255);
        assert!(!cache.update(&scene, &values, &style, false));
        style.media_appearance.insert(
            id,
            crate::instances::MediaAppearance {
                average: Vec3::ONE,
                flicker: 0.0,
            },
        );
        assert!(cache.update(&scene, &values, &style, false));
    }

    #[test]
    fn unchanged_inputs_and_transport_clock_updates_reuse_preparation() {
        let scene = Scene::default();
        let mut values = SceneValues::default();
        let style = FrameStyle::default();
        let mut cache = Preparation::default();
        assert!(cache.update(&scene, &values, &style, false));
        values.frame += 10;
        values.newest_input_micros += 10_000;
        values.atmosphere.density = 0.8;
        assert!(!cache.update(&scene, &values, &style, false));
    }
    #[test]
    fn actual_pose_optics_physics_and_effect_timeline_changes_invalidate() {
        let mut scene = Scene::default();
        scene.fixtures.push(FixtureInstance::default());
        let mut values = SceneValues::default();
        values.emitters.push(EmitterValues::default());
        let mut style = FrameStyle::default();
        let mut cache = Preparation::default();
        cache.update(&scene, &values, &style, false);
        scene.fixtures[0].position.x = 0.5;
        assert!(cache.update(&scene, &values, &style, false));
        values.emitters[0].pan = 0.8;
        assert!(cache.update(&scene, &values, &style, false));
        values.effect_frames.push(EffectFrame::default());
        assert!(cache.update(&scene, &values, &style, false));
        values.effect_frames[0].timeline_seconds = 0.25;
        assert!(cache.update(&scene, &values, &style, false));
        style.camera_position = Some(Vec3::new(0.0, 2.0, 5.0));
        assert!(cache.update(&scene, &values, &style, false));
        assert!(!cache.update(&scene, &values, &style, false));
    }
    #[test]
    fn model_material_and_asset_replacement_refresh_while_pixel_clocks_do_not() {
        let mut scene = Scene::default();
        scene.models.push(viz_scene::FixtureModel {
            parts: vec![ModelPart::default()],
            ..Default::default()
        });
        let values = SceneValues::default();
        let mut style = FrameStyle::default();
        let mut cache = Preparation::default();
        cache.update(&scene, &values, &style, false);
        scene.models[0].parts[0].colour = [0.2, 0.7, 0.1];
        assert!(cache.update(&scene, &values, &style, false));
        assert!(!cache.update(&scene, &values, &style, false));
        assert!(
            cache.update(&scene, &values, &style, true),
            "content reconciliation invalidates unchanged metadata"
        );
        let id = viz_scene::uuid::Uuid::from_u128(1);
        style.media_appearance.insert(
            id,
            crate::instances::MediaAppearance {
                average: Vec3::X,
                flicker: 0.1,
            },
        );
        assert!(cache.update(&scene, &values, &style, false));
        style.media_appearance.get_mut(&id).unwrap().average = Vec3::Z;
        assert!(cache.update(&scene, &values, &style, false));
        style.media_content = false;
        assert!(cache.update(&scene, &values, &style, false));
    }
    #[test]
    fn selection_order_is_irrelevant_and_render_budgets_remain_dependencies() {
        let scene = Scene::default();
        let mut values = SceneValues::default();
        let mut style = FrameStyle::default();
        let ids = [
            viz_scene::uuid::Uuid::from_u128(1),
            viz_scene::uuid::Uuid::from_u128(2),
        ];
        values.selected_fixtures.extend(ids);
        let first = fingerprint(&scene, &values, &style);
        values.selected_fixtures.clear();
        values.selected_fixtures.extend(ids.into_iter().rev());
        assert_eq!(first, fingerprint(&scene, &values, &style));
        style.effect_particle_budget += 1;
        assert_ne!(first, fingerprint(&scene, &values, &style));
    }
    #[test]
    fn live_physics_and_laser_paths_cannot_reuse_stale_instance_arrays() {
        let scene = Scene::default();
        let mut values = SceneValues::default();
        let style = FrameStyle::default();
        values
            .physics_frames
            .push(viz_scene::PhysicsFrame::default());
        values.laser_scans.push(viz_scene::LaserScan::default());
        let mut cache = Preparation::default();
        cache.update(&scene, &values, &style, false);
        values.physics_frames[0].position_offset[1] = -0.2;
        assert!(cache.update(&scene, &values, &style, false));
        values.physics_frames[0].timeline_seconds += 0.1;
        assert!(cache.update(&scene, &values, &style, false));
        values.laser_scans[0].fault = Some("script stopped".into());
        assert!(cache.update(&scene, &values, &style, false));
        assert!(!cache.update(&scene, &values, &style, false));
    }
}

#[cfg(test)]
#[path = "preparation/benchmark.rs"]
mod benchmark;

#[cfg(test)]
mod detailed_people_tests {
    use super::*;
    #[test]
    fn requested_detail_changes_only_effective_ultra_preparation_identity() {
        let scene = Scene::default();
        let values = SceneValues::default();
        let mut style = FrameStyle {
            quality: viz_scene::RenderQuality::Draft,
            ..Default::default()
        };
        let off = fingerprint(&scene, &values, &style);
        style.detailed_people = true;
        assert_eq!(off, fingerprint(&scene, &values, &style));
        style.quality = viz_scene::RenderQuality::Ultra;
        let detailed = fingerprint(&scene, &values, &style);
        style.detailed_people = false;
        assert_ne!(detailed, fingerprint(&scene, &values, &style));
    }
}
