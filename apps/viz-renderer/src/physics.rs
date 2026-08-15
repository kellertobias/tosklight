//! Deterministic, bounded simulation for package-controlled scenic bodies.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use viz_effect::{PhysicsAction, PhysicsControlEngine, PhysicsRequest};
use viz_scene::{PhysicsFrame, RenderQuality, Scene, SceneValues};

const FIXED_STEP_SECONDS: f32 = 1.0 / 120.0;
const MAX_SIMULATION_SECONDS: f32 = 60.0;

#[derive(Default)]
struct Track {
    previous: Option<PhysicsAction>,
    release_time: Option<f32>,
    history: Vec<(f32, PhysicsAction)>,
    last_time: Option<f32>,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
struct SavedState {
    released_at_unix_millis: u64,
    settled: bool,
}

pub struct Physics {
    engine: Option<PhysicsControlEngine>,
    unavailable: Option<String>,
    tracks: HashMap<Uuid, Track>,
    reset_generation: u64,
    state_path: Option<PathBuf>,
    saved: HashMap<Uuid, SavedState>,
}

impl Physics {
    pub fn new() -> Self {
        let (engine, unavailable) = match PhysicsControlEngine::new() {
            Ok(engine) => (Some(engine), None),
            Err(reason) => (None, Some(reason)),
        };
        Self {
            engine,
            unavailable,
            tracks: HashMap::new(),
            reset_generation: 0,
            state_path: None,
            saved: HashMap::new(),
        }
    }

    pub fn with_state_path(path: Option<PathBuf>) -> Self {
        let mut physics = Self::new();
        physics.state_path = path;
        if let Some(path) = physics.state_path.as_deref()
            && let Ok(bytes) = std::fs::read(path)
        {
            physics.saved = serde_json::from_slice(&bytes).unwrap_or_default();
        }
        physics
    }

    pub fn state_path(preferences: Option<&Path>, target: &str) -> Option<PathBuf> {
        let parent = preferences?.parent()?;
        let safe_target: String = target
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || character == '-' {
                    character
                } else {
                    '-'
                }
            })
            .collect();
        Some(parent.join(format!("physics-state-{safe_target}.json")))
    }

    pub fn reset_all(&mut self, values: &mut SceneValues, generation: u64) {
        if generation <= self.reset_generation {
            return;
        }
        self.reset_generation = generation;
        for frame in &mut values.physics_frames {
            reset(frame);
        }
        for track in self.tracks.values_mut() {
            track.release_time = None;
            track.history.clear();
        }
        self.saved.clear();
        self.persist();
    }

    pub fn run(
        &mut self,
        scene: &Scene,
        values: &mut SceneValues,
        time: f32,
        quality: RenderQuality,
        reset_generation: u64,
    ) {
        values.resize_physics(scene.physics_scenery.len());
        self.reset_all(values, reset_generation);
        let Some(engine) = self.engine.as_mut() else {
            let reason = self
                .unavailable
                .clone()
                .unwrap_or_else(|| "no physics runtime".into());
            for frame in &mut values.physics_frames {
                frame.fault = Some(reason.clone());
            }
            return;
        };
        let mut saved_changed = false;
        for (index, body) in scene.physics_scenery.iter().enumerate() {
            let (prior_frames, current_and_later) = values.physics_frames.split_at_mut(index);
            let frame = &mut current_and_later[0];
            let Some(source) = body.program.script.as_deref() else {
                frame.fault = Some("this physics fixture ships no control script".into());
                continue;
            };
            if !self.tracks.contains_key(&body.fixture_instance_id) {
                let mut track = Track::default();
                if let Some(saved) = self.saved.get(&body.fixture_instance_id).copied() {
                    let wall_elapsed =
                        now_unix_millis().saturating_sub(saved.released_at_unix_millis) as f32
                            / 1_000.0;
                    track.release_time = Some(
                        time - if saved.settled {
                            MAX_SIMULATION_SECONDS
                        } else {
                            wall_elapsed.min(MAX_SIMULATION_SECONDS)
                        },
                    );
                    frame.released = true;
                    frame.settled = saved.settled;
                }
                self.tracks.insert(body.fixture_instance_id, track);
            }
            let track = self
                .tracks
                .get_mut(&body.fixture_instance_id)
                .expect("the physics track was inserted");
            let result = engine.run(
                index,
                &PhysicsRequest {
                    source,
                    source_key: body.program.script_key,
                    result_version: body.program.result_version,
                    slots: &frame.slots,
                    time_seconds: f64::from(time),
                    elapsed_seconds: f64::from(
                        track.last_time.map_or(0.0, |last| (time - last).max(0.0)),
                    ),
                    fixture_identity: &body.fixture_instance_id.to_string(),
                    released: frame.released,
                    settled: frame.settled,
                    timeline_seconds: frame.timeline_seconds,
                },
            );
            track.last_time = Some(time);
            frame.version = result.version;
            frame.fault = result.fault;
            if frame.fault.is_some() {
                continue;
            }
            if track.previous != Some(result.action) {
                track.history.push((time, result.action));
                if track.history.len() > 256 {
                    track.history.drain(..128);
                }
                match result.action {
                    PhysicsAction::Release if !frame.released => {
                        frame.released = true;
                        frame.settled = false;
                        track.release_time = Some(time);
                        self.saved.insert(
                            body.fixture_instance_id,
                            SavedState {
                                released_at_unix_millis: now_unix_millis(),
                                settled: false,
                            },
                        );
                        saved_changed = true;
                    }
                    PhysicsAction::Reset => {
                        reset(frame);
                        track.release_time = None;
                        self.saved.remove(&body.fixture_instance_id);
                        saved_changed = true;
                    }
                    PhysicsAction::Hold | PhysicsAction::Release => {}
                }
            }
            track.previous = Some(result.action);

            // A backwards timecode seek reconstructs the last discrete release/reset history.
            if track.release_time.is_some_and(|released| time < released) {
                reset(frame);
                track.release_time = None;
                for &(event_time, action) in &track.history {
                    if event_time > time {
                        break;
                    }
                    match action {
                        PhysicsAction::Release => {
                            frame.released = true;
                            track.release_time = Some(event_time);
                        }
                        PhysicsAction::Reset => {
                            reset(frame);
                            track.release_time = None;
                        }
                        PhysicsAction::Hold => {}
                    }
                }
            }
            let Some(released) = track.release_time else {
                continue;
            };
            let elapsed = (time - released).max(0.0).min(MAX_SIMULATION_SECONDS);
            let elapsed = (elapsed / FIXED_STEP_SECONDS).floor() * FIXED_STEP_SECONDS;
            frame.timeline_seconds = elapsed;
            let gravity = body.body.gravity_metres_per_second_squared;
            let authored_bottom = body.scenery.position.y - body.scenery.size.y * 0.5;
            let mut floor = body.constraints.floor_y_metres;
            if quality >= RenderQuality::High && body.constraints.scenery_collision {
                for object in &scene.scenery {
                    let half = object.size * 0.5;
                    let p = body.scenery.position;
                    if p.x >= object.position.x - half.x
                        && p.x <= object.position.x + half.x
                        && p.z >= object.position.z - half.z
                        && p.z <= object.position.z + half.z
                        && object.position.y + half.y <= authored_bottom
                    {
                        floor = floor.max(object.position.y + half.y);
                    }
                }
            }
            if quality >= RenderQuality::High && body.constraints.self_collision {
                for (other, other_state) in scene.physics_scenery[..index].iter().zip(prior_frames)
                {
                    let other_position = other.scenery.position
                        + glam::Vec3::from_array(other_state.position_offset);
                    let other_half = other.scenery.size * 0.5;
                    let p = body.scenery.position;
                    if p.x >= other_position.x - other_half.x
                        && p.x <= other_position.x + other_half.x
                        && p.z >= other_position.z - other_half.z
                        && p.z <= other_position.z + other_half.z
                        && other_position.y + other_half.y <= authored_bottom
                    {
                        floor = floor.max(other_position.y + other_half.y);
                    }
                }
            }
            let maximum_drop = (authored_bottom - floor).max(0.0);
            let drop = (0.5 * gravity * elapsed * elapsed).min(maximum_drop);
            frame.position_offset = [0.0, -drop, 0.0];
            frame.velocity = [
                0.0,
                if drop >= maximum_drop {
                    0.0
                } else {
                    -gravity * elapsed
                },
                0.0,
            ];
            let was_settled = frame.settled;
            frame.settled = drop >= maximum_drop;
            if frame.settled && !was_settled {
                if let Some(saved) = self.saved.get_mut(&body.fixture_instance_id) {
                    saved.settled = true;
                    saved_changed = true;
                }
            }
        }
        let live: HashSet<_> = scene
            .physics_scenery
            .iter()
            .map(|body| body.fixture_instance_id)
            .collect();
        self.tracks.retain(|id, _| live.contains(id));
        engine.retain(|index| index < scene.physics_scenery.len());
        let saved_before_retain = self.saved.len();
        self.saved.retain(|id, _| live.contains(id));
        saved_changed |= self.saved.len() != saved_before_retain;
        if saved_changed {
            self.persist();
        }
    }

    pub fn fault(&self, values: &SceneValues) -> Option<String> {
        let faults: Vec<_> = values
            .physics_frames
            .iter()
            .filter_map(|frame| frame.fault.as_deref())
            .collect();
        faults.first().map(|first| {
            if faults.len() == 1 {
                format!("physics: {first}")
            } else {
                format!("physics: {first} (and {} more)", faults.len() - 1)
            }
        })
    }
}

fn now_unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().min(u64::MAX as u128) as u64
        })
}

impl Physics {
    fn persist(&self) {
        let Some(path) = self.state_path.as_deref() else {
            return;
        };
        let Ok(bytes) = serde_json::to_vec(&self.saved) else {
            return;
        };
        let Some(parent) = path.parent() else { return };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let temporary = path.with_extension("json.tmp");
        if std::fs::write(&temporary, bytes).is_ok() {
            let _ = std::fs::rename(temporary, path);
        }
    }
}

fn reset(frame: &mut PhysicsFrame) {
    let slots = std::mem::take(&mut frame.slots);
    *frame = PhysicsFrame {
        slots,
        ..PhysicsFrame::default()
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use std::sync::Arc;
    use viz_scene::{PhysicsProgram, PhysicsSceneryObject, SceneryKind, SceneryObject};

    fn scene() -> Scene {
        let id = Uuid::new_v4();
        Scene {
            physics_scenery: vec![PhysicsSceneryObject {
                fixture_instance_id: id,
                scenery: SceneryObject {
                    id,
                    name: "Kabuki".into(),
                    position: Vec3::new(0.0, 10.0, 0.0),
                    rotation_degrees: Vec3::ZERO,
                    size: Vec3::new(6.0, 2.0, 0.1),
                    colour: [0.3, 0.1, 0.1],
                    roughness: 0.8,
                    kind: SceneryKind::Curtain,
                    chords: 1,
                },
                program: PhysicsProgram {
                    script: Some(Arc::from(
                        "export function physics(input){return {version:1,action:input.dmx[0]>=192?'release':input.dmx[0]<=31?'reset':'hold'}}",
                    )),
                    script_key: 1,
                    result_version: 1,
                },
                body: viz_scene::PhysicsBody {
                    mass_kilograms: 10.0,
                    gravity_metres_per_second_squared: 9.806_65,
                },
                constraints: viz_scene::PhysicsConstraints {
                    floor_y_metres: 0.0,
                    scenery_collision: true,
                    self_collision: false,
                },
            }],
            ..Scene::default()
        }
    }

    #[test]
    fn release_is_latched_until_an_edge_reset_and_always_settles_on_the_floor() {
        let scene = scene();
        let mut values = SceneValues::default();
        values.resize_physics(1);
        values.physics_frames[0].slots = vec![255];
        let mut physics = Physics::new();
        physics.run(&scene, &mut values, 1.0, RenderQuality::Draft, 0);
        physics.run(&scene, &mut values, 2.0, RenderQuality::Draft, 0);
        assert!(values.physics_frames[0].released);
        assert!(values.physics_frames[0].position_offset[1] < -4.0);
        physics.run(&scene, &mut values, 3.0, RenderQuality::Draft, 0);
        assert_eq!(values.physics_frames[0].position_offset[1], -9.0);
        assert!(values.physics_frames[0].settled);

        values.physics_frames[0].slots[0] = 0;
        physics.run(&scene, &mut values, 4.0, RenderQuality::Draft, 0);
        assert!(!values.physics_frames[0].released);
        assert_eq!(values.physics_frames[0].position_offset, [0.0; 3]);
    }

    #[test]
    fn authoritative_reset_generation_is_applied_once() {
        let scene = scene();
        let mut values = SceneValues::default();
        values.resize_physics(1);
        values.physics_frames[0].slots = vec![255];
        let mut physics = Physics::new();
        physics.run(&scene, &mut values, 1.0, RenderQuality::High, 0);
        physics.run(&scene, &mut values, 2.0, RenderQuality::High, 0);
        physics.run(&scene, &mut values, 2.0, RenderQuality::High, 1);
        assert!(!values.physics_frames[0].released);
        // The held Release level cannot immediately undo the reset; it needs another action edge.
        physics.run(&scene, &mut values, 2.5, RenderQuality::High, 1);
        assert!(!values.physics_frames[0].released);
    }

    #[test]
    fn restart_restores_the_latched_snapshot_without_replaying_from_zero() {
        let scene = scene();
        let path = std::env::temp_dir().join(format!("tosklight-physics-{}.json", Uuid::new_v4()));
        let mut values = SceneValues::default();
        values.resize_physics(1);
        values.physics_frames[0].slots = vec![255];
        let mut first = Physics::with_state_path(Some(path.clone()));
        first.run(&scene, &mut values, 1.0, RenderQuality::High, 0);
        first.run(&scene, &mut values, 3.0, RenderQuality::High, 0);
        assert!(values.physics_frames[0].settled);
        drop(first);

        let mut restored_values = SceneValues::default();
        restored_values.resize_physics(1);
        restored_values.physics_frames[0].slots = vec![64];
        let mut restarted = Physics::with_state_path(Some(path.clone()));
        restarted.run(&scene, &mut restored_values, 0.0, RenderQuality::High, 0);
        assert!(restored_values.physics_frames[0].released);
        assert!(restored_values.physics_frames[0].settled);
        assert_eq!(restored_values.physics_frames[0].position_offset[1], -9.0);
        let _ = std::fs::remove_file(path);
    }
}
