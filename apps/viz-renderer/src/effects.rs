//! Running package-owned Effect fixture programs once per displayed frame.

use std::collections::HashMap;
use uuid::Uuid;
use viz_effect::{EffectEngine, EffectRequest};
use viz_scene::{EmitterKind, Scene, SceneValues};

pub struct Effects {
    engine: Option<EffectEngine>,
    unavailable: Option<String>,
    clocks: HashMap<Uuid, (f32, f32)>,
}

impl Effects {
    pub fn new() -> Self {
        let (engine, unavailable) = match EffectEngine::new() {
            Ok(engine) => (Some(engine), None),
            Err(reason) => (None, Some(reason)),
        };
        Self {
            engine,
            unavailable,
            clocks: HashMap::new(),
        }
    }

    pub fn run(&mut self, scene: &Scene, values: &mut SceneValues, time: f32) {
        if !scene
            .emitters
            .iter()
            .any(|emitter| emitter.kind == EmitterKind::Effect)
        {
            return;
        }
        let Some(engine) = self.engine.as_mut() else {
            let reason = self
                .unavailable
                .clone()
                .unwrap_or_else(|| "no effect runtime".into());
            for (index, emitter) in scene.emitters.iter().enumerate() {
                if emitter.kind == EmitterKind::Effect
                    && let Some(frame) = values.effect_frames.get_mut(index)
                {
                    frame.emitters.clear();
                    frame.fault = Some(reason.clone());
                }
            }
            return;
        };
        for (index, emitter) in scene.emitters.iter().enumerate() {
            if emitter.kind != EmitterKind::Effect {
                continue;
            }
            let Some(program) = emitter.effect.as_ref() else {
                continue;
            };
            let Some(frame) = values.effect_frames.get_mut(index) else {
                continue;
            };
            let Some(source) = program.script.as_ref() else {
                frame.emitters.clear();
                frame.fault = Some("this Effect fixture's profile ships no effect script".into());
                continue;
            };
            let fixture = scene.fixtures.get(emitter.fixture_index as usize);
            let fixture_id = fixture
                .map(|fixture| fixture.instance_id)
                .unwrap_or_default();
            let (last, started) = self.clocks.entry(fixture_id).or_insert((time, time));
            let elapsed = (time - *last).max(0.0);
            let live_timeline = (time - *started).max(0.0);
            *last = time;
            let identity = fixture
                .map(|fixture| fixture.instance_id.to_string())
                .unwrap_or_default();
            let seed = fixture.map_or(0, |fixture| {
                u64::from_le_bytes(
                    fixture.instance_id.as_bytes()[..8]
                        .try_into()
                        .unwrap_or([0; 8]),
                )
            });
            let produced = engine.run(
                index,
                &EffectRequest {
                    source,
                    source_key: program.script_key,
                    result_version: program.result_version,
                    slots: &frame.slots,
                    time_seconds: f64::from(time),
                    elapsed_seconds: f64::from(elapsed),
                    intensity: values
                        .emitters
                        .get(index)
                        .map_or(0.0, |value| value.intensity),
                    fixture_identity: &identity,
                    capture_seed: seed,
                },
            );
            frame.version = produced.version;
            // Live simulation starts at zero after a renderer restart. Deterministic capture is a
            // separate path and supplies its explicit timeline directly in SceneValues.
            frame.timeline_seconds = live_timeline;
            frame.emitters = produced.emitters;
            frame.fault = produced.fault;
        }
        let live = scene.emitters.len();
        engine.retain(|index| index < live);
        let live_ids: std::collections::HashSet<_> = scene
            .fixtures
            .iter()
            .map(|fixture| fixture.instance_id)
            .collect();
        self.clocks.retain(|id, _| live_ids.contains(id));
    }

    pub fn fault(&self, values: &SceneValues) -> Option<String> {
        if let Some(reason) = &self.unavailable {
            return Some(format!("effect scripts unavailable: {reason}"));
        }
        let faults: Vec<_> = values
            .effect_frames
            .iter()
            .filter_map(|frame| frame.fault.as_deref())
            .collect();
        let first = faults.first()?;
        Some(if faults.len() == 1 {
            format!("effect: {first}")
        } else {
            format!("effect: {first} (and {} more)", faults.len() - 1)
        })
    }
}
