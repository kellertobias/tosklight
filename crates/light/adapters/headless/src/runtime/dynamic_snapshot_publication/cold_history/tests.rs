use super::*;
use light_core::{AttributeKey, FixtureId};
use light_dynamics::{
    ActivationPolicy, DynamicControl, DynamicController, DynamicControllerSource,
    DynamicDefinition, DynamicHeldPayload, DynamicSampleExpression, DynamicSpeed,
    DynamicSpeedTransport, DynamicStartRequest, DynamicTargetScope, Rational, ScalarSource,
    ScalarSourceResolver, SpeedGroup, TimedDynamicControl,
};

fn capacity(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

fn snapshot(revision: u64, definitions: Vec<DynamicDefinition>) -> Arc<EngineSnapshot> {
    Arc::new(EngineSnapshot {
        revision,
        dynamics: definitions.into(),
        ..Default::default()
    })
}

fn definition() -> DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "pool_number": 1, "revision": 1, "name": "Cold history",
        "target_binding": {"type": "targetless"},
        "lanes": [{
            "id": Uuid::new_v4(), "attribute": "intensity", "mode": "keyframes",
            "keyframes": {"points": [
                {"position": 0.0, "source": {"type": "value", "value": 0.0}, "interpolation": "linear"},
                {"position": 0.5, "source": {"type": "value", "value": 1.0}, "interpolation": "linear"}
            ]},
            "max_min": {"minimum": {"type": "value", "value": 0.0},
                "maximum": {"type": "value", "value": 1.0}, "function": "sinus"},
            "middle_amplitude": {"middle": {"type": "current"}, "amplitude": 0.5, "function": "sinus"},
            "speed_multiplier": {"numerator": 1, "denominator": 1}, "width": 1.0
        }],
        "phase": {"ordering": {"type": "selection"}, "offset_degrees": 0.0,
            "span_degrees": 0.0, "block_size": 1, "repeats": 1,
            "wings": false, "anchors_degrees": []},
        "speed": {"type": "fixed", "duration_millis": 1000},
        "default_activation": "start_now"
    }))
    .unwrap()
}

fn request(definition: &DynamicDefinition) -> DynamicStartRequest {
    DynamicStartRequest {
        definition_id: definition.id,
        controller: DynamicController {
            id: Uuid::new_v4(),
            source: DynamicControllerSource::Programmer {
                programmer_id: Uuid::new_v4(),
                instance_link: None,
            },
            priority: 1,
            activated_at_millis: 0,
            size: 1.0,
            speed_multiplier: 1.0,
            phase_offset_degrees: 0.0,
            paused: false,
        },
        target_scope: DynamicTargetScope {
            ordered_targets: vec![FixtureId::new()],
        },
        stage_positions: Default::default(),
        inherited_spatial_mapping: None,
        now_millis: 0,
        activation_delay_millis: 0,
        activation_duration_millis: 0,
        activation_policy_override: None,
        reuse_matching_targetless: false,
    }
}

fn apply(runtime: &mut DynamicRuntime, control: DynamicControl) {
    runtime
        .apply_recorded_control(TimedDynamicControl {
            at_millis: 150,
            control,
        })
        .unwrap();
}

fn publish_empty(
    publication: &DynamicSnapshotPublication,
    runtime: &DynamicRuntime,
    previous: &Arc<EngineSnapshot>,
    destination: &Arc<EngineSnapshot>,
) -> Arc<ColdGenerationEvent> {
    let boundary = publication.cold_boundary(runtime).unwrap();
    let from = boundary.from;
    let event = boundary.prepare(previous.clone(), destination.clone(), runtime);
    publication.installed_with_cold_event(destination.clone(), Some(event));
    publication.cold_generations_since(from).unwrap().remove(0)
}

#[test]
fn beginning_history_is_idempotent_and_requires_exact_installed_snapshot() {
    let initial = snapshot(1, vec![]);
    let publication = DynamicSnapshotPublication::new(initial.clone());
    let mut runtime = DynamicRuntime::default();
    assert!(
        publication
            .begin_retained_history(&mut runtime, &Arc::new((*initial).clone()), capacity(4))
            .is_err()
    );
    assert_eq!(runtime.control_cursor(), None);
    let first = publication
        .begin_retained_history(&mut runtime, &initial, capacity(4))
        .unwrap();
    assert_eq!(
        publication
            .begin_retained_history(&mut runtime, &initial, capacity(1))
            .unwrap(),
        first
    );
    assert!(
        publication
            .cold_generations_since(first.0)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn successive_generations_order_empty_control_intervals_and_exact_snapshot_arcs() {
    let initial = snapshot(1, vec![]);
    let second = snapshot(2, vec![]);
    let third = snapshot(3, vec![]);
    let publication = DynamicSnapshotPublication::new(initial.clone());
    let mut live = DynamicRuntime::default();
    let (mut generations, mut controls) = publication
        .begin_retained_history(&mut live, &initial, capacity(4))
        .unwrap();
    let seed = generations;
    let original_controls = controls;
    let mut pending = live.fork_for_pending_preview();
    let mut current = initial.clone();
    let mut scratch = DynamicOutputFrameScratch::default();
    let first = publish_empty(&publication, &live, &initial, &second);
    let next = publish_empty(&publication, &live, &second, &third);
    assert_eq!(first.to, next.from);
    assert_ne!(first.from, first.to);
    assert!(Arc::ptr_eq(&first.previous, &initial));
    assert!(Arc::ptr_eq(&first.destination, &second));
    assert!(Arc::ptr_eq(&next.previous, &second));
    assert!(Arc::ptr_eq(&next.destination, &third));
    assert_eq!(publication.cold_generations_since(seed).unwrap().len(), 2);
    assert!(
        next.apply(
            &mut pending,
            &mut current,
            &mut controls,
            &mut generations,
            &mut scratch
        )
        .is_err()
    );
    assert_eq!(generations, seed);
    for event in [&first, &next] {
        assert!(event.controls.as_ref().unwrap().is_empty());
        event
            .apply(
                &mut pending,
                &mut current,
                &mut controls,
                &mut generations,
                &mut scratch,
            )
            .unwrap();
    }
    assert!(Arc::ptr_eq(&current, &third));
    assert_eq!(controls, original_controls);
    assert!(
        next.apply(
            &mut pending,
            &mut current,
            &mut controls,
            &mut generations,
            &mut scratch
        )
        .is_err()
    );
    assert_eq!(generations, next.to);
}

#[test]
fn bounded_eviction_preserves_retained_event_and_ordinary_install_invalidates_epoch() {
    let initial = snapshot(1, vec![]);
    let second = snapshot(2, vec![]);
    let third = snapshot(3, vec![]);
    let publication = DynamicSnapshotPublication::new(initial.clone());
    let mut live = DynamicRuntime::default();
    let (mut generations, mut controls) = publication
        .begin_retained_history(&mut live, &initial, capacity(1))
        .unwrap();
    let seed = generations;
    let retained = publish_empty(&publication, &live, &initial, &second);
    let next = publish_empty(&publication, &live, &second, &third);
    assert!(matches!(
        publication.cold_generations_since(seed),
        Err(ColdGenerationReadError::HistoryLost)
    ));
    assert!(Arc::ptr_eq(
        &publication.cold_generations_since(retained.to).unwrap()[0],
        &next
    ));
    let mut pending = live.fork_for_pending_preview();
    let mut current = initial;
    retained
        .apply(
            &mut pending,
            &mut current,
            &mut controls,
            &mut generations,
            &mut DynamicOutputFrameScratch::default(),
        )
        .unwrap();
    assert!(Arc::ptr_eq(&current, &second));
    publication.installed(third.clone());
    assert!(matches!(
        publication.cold_generations_since(next.to),
        Err(ColdGenerationReadError::WrongEpoch)
    ));
    let fresh = publication
        .begin_retained_history(&mut live, &third, capacity(1))
        .unwrap();
    assert_ne!(fresh.0.epoch, next.to.epoch);
}

#[test]
fn missing_or_evicted_control_history_is_passive_and_does_not_mutate_pending() {
    for missing in [false, true] {
        let initial = snapshot(1, vec![]);
        let destination = snapshot(2, vec![]);
        let publication = DynamicSnapshotPublication::new(initial.clone());
        let mut live = DynamicRuntime::default();
        let (mut generations, mut controls) = publication
            .begin_retained_history(&mut live, &initial, capacity(1))
            .unwrap();
        let seed = (generations, controls);
        let mut pending = live.fork_for_pending_preview();
        let before = pending.snapshot();
        let boundary = publication.cold_boundary(&live).unwrap();
        if missing {
            live.end_control_recording();
        } else {
            apply(&mut live, DynamicControl::GlobalPause(true));
            apply(&mut live, DynamicControl::GlobalPause(false));
        }
        let event = boundary.prepare(initial.clone(), destination.clone(), &live);
        publication.installed_with_cold_event(destination.clone(), Some(event));
        assert!(
            publication.matches(&destination),
            "a preview gap cannot veto Live publication"
        );
        let event = publication
            .cold_generations_since(seed.0)
            .unwrap()
            .remove(0);
        assert_eq!(event.control_boundary().unwrap(), seed.1);
        assert_eq!(event.before_controls, Some(seed.1));
        assert_eq!(event.after_controls, live.control_cursor());
        assert_eq!(
            event.controls.as_ref().err().copied(),
            Some(if missing {
                DynamicControlLogError::WrongEpoch
            } else {
                DynamicControlLogError::HistoryLost
            })
        );
        let mut current = initial.clone();
        assert!(
            event
                .apply(
                    &mut pending,
                    &mut current,
                    &mut controls,
                    &mut generations,
                    &mut DynamicOutputFrameScratch::default()
                )
                .is_err()
        );
        assert_eq!(pending.snapshot(), before);
        assert!(Arc::ptr_eq(&current, &initial));
        assert_eq!((generations, controls), seed);
    }
}

#[test]
fn cold_definitions_are_installed_before_replaying_new_start() {
    let definition = definition();
    let request = request(&definition);
    let controller = request.controller.id;
    let initial = snapshot(1, vec![]);
    let destination = snapshot(2, vec![definition.clone()]);
    let publication = DynamicSnapshotPublication::new(initial.clone());
    let mut live = DynamicRuntime::default();
    let (mut generations, mut controls) = publication
        .begin_retained_history(&mut live, &initial, capacity(8))
        .unwrap();
    let mut pending = live.fork_for_pending_preview();
    let boundary = publication.cold_boundary(&live).unwrap();
    let mut candidate = live.fork_for_cold_install();
    candidate.install_definitions([definition.clone()]).unwrap();
    apply(&mut candidate, DynamicControl::Start(Box::new(request)));
    let expected = candidate.controller(controller).unwrap().0;
    let event = boundary.prepare(initial.clone(), destination.clone(), &candidate);
    publication.installed_with_cold_event(destination.clone(), Some(event));
    let event = publication
        .cold_generations_since(generations)
        .unwrap()
        .remove(0);
    let mut current = initial;
    event
        .apply(
            &mut pending,
            &mut current,
            &mut controls,
            &mut generations,
            &mut DynamicOutputFrameScratch::default(),
        )
        .unwrap();
    assert_eq!(pending.controller(controller).unwrap().0, expected);
    assert_eq!(
        pending.instance_definition(expected).unwrap().as_ref(),
        &definition
    );
    assert!(Arc::ptr_eq(&current, &destination));
}

#[test]
fn replay_requires_preceding_controls_and_exact_previous_snapshot() {
    let initial = snapshot(1, vec![]);
    let destination = snapshot(2, vec![]);
    let publication = DynamicSnapshotPublication::new(initial.clone());
    let mut live = DynamicRuntime::default();
    let (mut generations, mut controls) = publication
        .begin_retained_history(&mut live, &initial, capacity(8))
        .unwrap();
    let seed = (generations, controls);
    let mut pending = live.fork_for_pending_preview();
    apply(&mut live, DynamicControl::GlobalPause(true));
    let preceding = live.controls_since(controls).unwrap().unwrap();
    let event = publish_empty(&publication, &live, &initial, &destination);
    let mut scratch = DynamicOutputFrameScratch::default();
    let mut current = initial.clone();
    let before = pending.snapshot();
    assert!(
        event
            .apply(
                &mut pending,
                &mut current,
                &mut controls,
                &mut generations,
                &mut scratch
            )
            .is_err()
    );
    assert_eq!(pending.snapshot(), before);
    assert_eq!((generations, controls), seed);
    replay_dynamic_controls(&mut pending, &mut scratch, &mut controls, &preceding, None).unwrap();
    let accepted = pending.snapshot();
    let accepted_controls = controls;
    current = Arc::new((*initial).clone());
    assert!(
        event
            .apply(
                &mut pending,
                &mut current,
                &mut controls,
                &mut generations,
                &mut scratch
            )
            .is_err()
    );
    assert_eq!(pending.snapshot(), accepted);
    assert_eq!(controls, accepted_controls);
    assert_eq!(generations, seed.0);
    current = initial;
    event
        .apply(
            &mut pending,
            &mut current,
            &mut controls,
            &mut generations,
            &mut scratch,
        )
        .unwrap();
    assert!(pending.snapshot().global_paused);
    assert!(Arc::ptr_eq(&current, &destination));
    assert_eq!(controls, accepted_controls);
}

#[test]
fn failed_replay_does_not_install_definitions_snapshot_or_advance_either_cursor() {
    let original = definition();
    let mut edited = original.clone();
    edited.name = "Uncommitted pending replacement".into();
    let initial = snapshot(1, vec![original.clone()]);
    let destination = snapshot(2, vec![edited.clone()]);
    let publication = DynamicSnapshotPublication::new(initial.clone());
    let mut live = DynamicRuntime::default();
    live.install_definitions([original.clone()]).unwrap();
    let (mut generations, mut controls) = publication
        .begin_retained_history(&mut live, &initial, capacity(8))
        .unwrap();
    let seed = (generations, controls);
    let mut request = request(&original);
    // Reusable targetless scope with a different identity is a real replay conflict;
    // an ordinary targetless Start is allowed to create another independent instance.
    request.reuse_matching_targetless = true;
    let mut pending = live.fork_for_pending_preview();
    let id = pending.start(request.clone()).unwrap();
    let before = pending.snapshot();
    let boundary = publication.cold_boundary(&live).unwrap();
    let mut candidate = live.fork_for_cold_install();
    candidate.install_definitions([edited]).unwrap();
    apply(&mut candidate, DynamicControl::Start(Box::new(request)));
    let event = boundary.prepare(initial.clone(), destination.clone(), &candidate);
    publication.installed_with_cold_event(destination, Some(event));
    let event = publication
        .cold_generations_since(generations)
        .unwrap()
        .remove(0);
    let mut current = initial.clone();
    assert!(
        event
            .apply(
                &mut pending,
                &mut current,
                &mut controls,
                &mut generations,
                &mut DynamicOutputFrameScratch::default()
            )
            .is_err()
    );
    assert_eq!(pending.snapshot(), before);
    assert_eq!(pending.instance_definition(id).unwrap().as_ref(), &original);
    assert!(Arc::ptr_eq(&current, &initial));
    assert_eq!((generations, controls), seed);
}

struct Sources(f32);

impl ScalarSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        Some(self.0)
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}

fn assert_held_current(runtime: &DynamicRuntime, expected: f32) {
    let state = runtime.snapshot();
    let instance = &state.instances[0];
    assert!(!instance.synchronized_hold_values.is_empty());
    for held in &instance.synchronized_hold_values {
        let DynamicHeldPayload::TapeRoot { tape_root } = held.payload else {
            panic!("expected retained Current expression")
        };
        let expression = DynamicSampleExpression::Retained {
            tape: instance.expression_tape.as_ref().unwrap().clone(),
            root: tape_root,
        };
        let mut values = Vec::new();
        expression.visit_legacy_contributions(|_, value, _| values.push(value));
        assert!(!values.is_empty());
        assert!(values.iter().all(|value| (*value - expected).abs() < 1e-6));
    }
}

#[test]
fn cold_pause_uses_pending_sample_history_without_copying_live_held_values() {
    let mut definition = definition();
    for point in &mut definition.lanes[0].legacy_mut().unwrap().keyframes.points {
        point.source = ScalarSource::Current;
    }
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    let initial = snapshot(1, vec![definition.clone()]);
    let destination = snapshot(2, vec![definition.clone()]);
    let publication = DynamicSnapshotPublication::new(initial.clone());
    let mut live = DynamicRuntime::default();
    live.install_definitions([definition.clone()]).unwrap();
    live.start(request(&definition)).unwrap();
    let (mut generations, mut controls) = publication
        .begin_retained_history(&mut live, &initial, capacity(8))
        .unwrap();
    let mut pending = live.fork_for_pending_preview();
    let transports = [DynamicSpeedTransport {
        effective_bpm: 60.0,
        phase_origin_millis: 0,
        phase_reference_millis: 100,
        beat_phase: 0.1,
        phase_advancing: true,
    }; 5];
    live.sample_all_addressed(100, 10, &transports, &Sources(0.2), None);
    pending.sample_all_addressed(100, 10, &transports, &Sources(0.8), None);
    let pending_sample = pending.committed_sample_boundary();
    assert_ne!(live.committed_sample_boundary(), pending_sample);
    let boundary = publication.cold_boundary(&live).unwrap();
    let mut candidate = live.fork_for_cold_install();
    apply(&mut candidate, DynamicControl::GlobalPause(true));
    assert_held_current(&candidate, 0.2);
    let event = boundary.prepare(initial.clone(), destination.clone(), &candidate);
    publication.installed_with_cold_event(destination, Some(event));
    let event = publication
        .cold_generations_since(generations)
        .unwrap()
        .remove(0);
    let mut current = initial;
    event
        .apply(
            &mut pending,
            &mut current,
            &mut controls,
            &mut generations,
            &mut DynamicOutputFrameScratch::default(),
        )
        .unwrap();
    assert_eq!(pending.committed_sample_boundary(), pending_sample);
    assert_held_current(&pending, 0.8);
    assert_held_current(&candidate, 0.2);
}
