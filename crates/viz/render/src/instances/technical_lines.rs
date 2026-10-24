//! CPU instance cache for technical Lines. No GPU ownership or rendered-light work lives here.
use super::*;

mod fingerprint;
use fingerprint::{Key, model_key, style_key};

#[derive(Default)]
struct Entry {
    key: Option<u64>,
    frame: FrameInstances,
    #[cfg(test)]
    builds: usize,
}

impl Entry {
    fn update(&mut self, key: u64, build: impl FnOnce() -> FrameInstances) -> bool {
        if self.key == Some(key) {
            return false;
        }
        self.frame = build();
        self.key = Some(key);
        #[cfg(test)]
        {
            self.builds += 1;
        }
        true
    }
}

/// At most one body bundle per current fixture and one aim bundle per current emitter.
/// Camera motion reuses all fixture transforms. Authored architectural cutaways retain their
/// existing camera semantics and conservatively rebuild the scenery bundle during an orbit.
#[derive(Default)]
pub struct TechnicalLinesInstances {
    scenery: Entry,
    bodies: Vec<Entry>,
    emitters: Vec<Entry>,
    frame: FrameInstances,
    revision: u64,
}

impl TechnicalLinesInstances {
    /// Changes only when CPU output was rebuilt, never because input clocks or a camera changed.
    /// Camera-dependent architectural cutaways are the documented exception.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn frame(&self) -> &FrameInstances {
        &self.frame
    }

    pub fn update(
        &mut self,
        scene: &Scene,
        values: &SceneValues,
        style: &FrameStyle,
    ) -> &FrameInstances {
        let mut style = style.clone();
        // Lines is quality-invariant and never constructs the simulated-light/crowd/media paths.
        style.quality = viz_scene::RenderQuality::Draft;
        style.plot = false;
        style.emitter_apertures = false;
        style.draw_beams = false;
        style.crowd_person_budget = 0;
        style.effect_particle_budget = 0;
        style.media_content = false;
        style.media_appearance.clear();
        let style = &style;
        let style_hash = style_key(style);
        let mut shared = Key::default();
        shared.add(&(
            scene.show_id,
            scene.revision,
            scene.source_show_revision,
            &scene.source_identity,
        ));
        shared.add(&style_hash);
        shared.add(&values.position_points);
        let shared = shared.finish();
        let models: Vec<_> = scene.models.iter().map(model_key).collect();
        let grouped = grouped_selection(scene, &values.selected_fixtures);
        let angles = head_angles(scene, values);

        let mut changed = self.bodies.len() != scene.fixtures.len()
            || self.emitters.len() != scene.emitters.len();
        self.bodies
            .resize_with(scene.fixtures.len(), Entry::default);
        self.emitters
            .resize_with(scene.emitters.len(), Entry::default);
        // Per-fixture lookup is calculated once rather than searching all emitters for every key.
        let mut first_emitter = vec![None; scene.fixtures.len()];
        for (index, emitter) in scene.emitters.iter().enumerate() {
            if let Some(first) = first_emitter.get_mut(emitter.fixture_index as usize) {
                first.get_or_insert(index);
            }
        }
        for (index, fixture) in scene.fixtures.iter().enumerate() {
            let mut key = fixture_key(scene, values, fixture, index, shared, &models);
            key.add(&values.selected_fixtures.contains(&fixture.fixture_id));
            key.add(&grouped.contains(&fixture.fixture_id));
            if let Some(emitter) = first_emitter[index] {
                key.add(&angles[emitter]);
            }
            changed |= self.bodies[index].update(key.finish(), || {
                let mut frame = FrameInstances::default();
                push_bodies(
                    &mut frame,
                    scene,
                    &angles,
                    values,
                    style,
                    &values.selected_fixtures,
                    &values.position_points,
                    Some(index),
                );
                // Lines uses its own global ink. These PBR channels never influence its passes,
                // and must not cause uploads when only a dimmer or source primary changes.
                for (_, instances) in &mut frame.meshes {
                    for instance in instances {
                        instance.base_colour = [0.0; 4];
                        instance.emissive = [0.0; 4];
                        instance.surface = [0.0; 4];
                    }
                }
                frame
            });
        }
        changed |= self.update_emitters(scene, values, style, &angles, shared, &models);
        changed |= self.update_scenery(scene, values, style, shared);
        if changed {
            self.rebuild_frame();
        }
        &self.frame
    }

    fn update_emitters(
        &mut self,
        scene: &Scene,
        values: &SceneValues,
        style: &FrameStyle,
        angles: &[(f32, f32)],
        shared: u64,
        models: &[u64],
    ) -> bool {
        let mut changed = false;
        for (index, emitter) in scene.emitters.iter().enumerate() {
            let mut key = Key::default();
            key.add(&(shared, emitter, angles[index]));
            if let Some(value) = values.emitters.get(index) {
                key.add(&(
                    value.physical_pose,
                    value
                        .physical_optics
                        .and_then(|optics| optics.zoom_shape_half_angle),
                    value.zoom,
                    value.iris,
                    value.frost,
                    value.physical_color.is_some(),
                ));
                key.add(&(
                    value.retained_visible_intensity(),
                    value.colour,
                    value.shutter,
                    &value.cells,
                ));
            } else {
                key.add(&false);
            }
            if let Some(fixture) = scene.fixtures.get(emitter.fixture_index as usize) {
                key.add(
                    &fixture_key(
                        scene,
                        values,
                        fixture,
                        emitter.fixture_index as usize,
                        shared,
                        models,
                    )
                    .finish(),
                );
            }
            changed |= self.emitters[index].update(key.finish(), || {
                let mut frame = FrameInstances::default();
                push_emitters(&mut frame, scene, values, angles, style, Some(index));
                frame
            });
        }
        changed
    }

    fn update_scenery(
        &mut self,
        scene: &Scene,
        values: &SceneValues,
        style: &FrameStyle,
        shared: u64,
    ) -> bool {
        let mut key = Key::default();
        key.add(&(
            shared,
            &scene.bounds,
            &scene.scenery,
            &scene.physics_scenery,
            &values.physics_frames,
        ));
        key.add(&(
            &scene.media_sections,
            &scene.media_sources,
            &scene.media_projectors,
        ));
        let mut selected: Vec<_> = values.selected_fixtures.iter().copied().collect();
        selected.sort_unstable();
        key.add(&(&selected, &scene.venue_groups));
        // Scenery selection maps logical fixture ids to physical instance ids.
        for fixture in &scene.fixtures {
            key.add(&(fixture.fixture_id, fixture.instance_id));
        }
        if scene
            .scenery
            .iter()
            .any(|object| object.architecture.is_some())
            || scene
                .physics_scenery
                .iter()
                .any(|body| body.scenery.architecture.is_some())
        {
            key.add(&style.camera_position);
        }
        self.scenery.update(key.finish(), || {
            let mut frame = FrameInstances::default();
            if style.floor_grid {
                floor_grid::push_floor_grid(&mut frame, scene, style);
            }
            scenery::push_scenery(&mut frame, scene, values, style);
            media::push_media(&mut frame, scene, style);
            frame
        })
    }

    fn rebuild_frame(&mut self) {
        let mut frame = FrameInstances::default();
        append(&mut frame, &self.scenery.frame);
        for body in &self.bodies {
            append(&mut frame, &body.frame);
        }
        for emitter in &self.emitters {
            append(&mut frame, &emitter.frame);
        }
        self.frame = frame;
        self.revision = self.revision.saturating_add(1);
    }
}

fn fixture_key(
    scene: &Scene,
    values: &SceneValues,
    fixture: &FixtureInstance,
    index: usize,
    shared: u64,
    models: &[u64],
) -> Key {
    let mut key = Key::default();
    key.add(&(shared, fixture));
    key.add(&fixture.model.and_then(|model| models.get(model as usize)));
    if let Some(physical) = scene
        .physical_position_indices
        .get(index)
        .copied()
        .flatten()
    {
        if let Some(plan) = scene.physical_positions.get(physical) {
            key.add(&(plan.instance_id, plan.model_scale, &plan.model_part_nodes));
        }
        if let Some(state) = values.physical_positions.get(physical) {
            key.add(&(state.instance_id, state.graph_id, &state.node_deltas));
        }
    }
    key
}

fn append(frame: &mut FrameInstances, source: &FrameInstances) {
    for (kind, instances) in &source.meshes {
        frame.mesh(*kind).extend_from_slice(instances);
    }
    frame.lines.extend_from_slice(&source.lines);
    frame.poses.extend_from_slice(&source.poses);
    frame
        .curtain_blockers
        .extend_from_slice(&source.curtain_blockers);
    frame.media_panels.extend_from_slice(&source.media_panels);
}

#[cfg(test)]
mod tests;
