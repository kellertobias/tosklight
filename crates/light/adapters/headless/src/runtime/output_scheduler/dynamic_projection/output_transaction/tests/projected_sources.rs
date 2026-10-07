use super::*;
use crate::runtime::dynamic_source_origins::{
    DynamicRuntimeSourceCheckpoint, DynamicSourceBinding, DynamicSourceOrigin, DynamicSourceOrigins,
};
use light_playback::{
    ActiveCueDynamicValue, ActiveDynamicPlayback, CueDynamicSourceKey as Key, SequenceMasterSource,
    TemporaryPlaybackKind,
};

struct Harness {
    engine: Engine,
    snapshot: Arc<light_engine::EngineSnapshot>,
    runtime: DynamicRuntime,
    origins: DynamicSourceOrigins,
}

impl Harness {
    fn new(snapshot: light_engine::EngineSnapshot) -> Self {
        let mut runtime = DynamicRuntime::default();
        runtime
            .install_definitions(snapshot.dynamics.iter().cloned())
            .unwrap();
        Self {
            engine: Engine::new(ProgrammerRegistry::default()),
            snapshot: Arc::new(snapshot),
            runtime,
            origins: Default::default(),
        }
    }

    fn sample(
        &mut self,
        millis: i64,
        cues: &[ActiveCueDynamicValue],
        playbacks: &[ActiveDynamicPlayback],
        paused: bool,
    ) -> CapturedDynamicSample {
        let addresser = self.engine.frame_addresser();
        let programmers = Arc::new(vec![]);
        let speeds = capture_dynamic_speed_transports(
            &Mutex::new(std::array::from_fn(|_| {
                light_control::speed::SpeedGroupController::new(120.0, Default::default()).unwrap()
            })),
            millis as u64,
        );
        let inputs = CapturedDynamicInputs {
            now: chrono::DateTime::from_timestamp_millis(millis).unwrap(),
            speed_transports: &speeds,
            rate: 40,
            snapshot: &self.snapshot,
            programmer_values: &programmers,
            programmer_rows: None,
            cue_values: cues,
            dynamic_playbacks: playbacks,
            playback_paused: paused,
            addresser: &addresser,
            extra_programmer_values: &[],
            programmer_reconciliation_cache: None,
            force_source_reconciliation: false,
        };
        let sources = TickSources::new(&self.engine);
        let origins = &mut self.origins;
        let sample = sample_captured_dynamic_inputs_with(
            &mut self.runtime,
            &inputs,
            |runtime, now, interval, assignments, _| {
                source_bindings::bind_captured_sources(origins, runtime, &inputs, assignments)
                    .map_err(|error| DynamicRuntimeError::InvalidSample(error.to_string()))?;
                runtime.sample_all_programming_addressed(
                    now,
                    interval,
                    &speeds,
                    &sources,
                    &source_bindings::AuthoredDynamicSources(origins),
                    Some(&addresser),
                )
            },
        )
        .unwrap();
        source_bindings::retire_removed_controllers(
            origins,
            &sample.before_runtime,
            &sample.after_runtime,
        );
        DynamicRuntimeSourceCheckpoint::capture(self.runtime.snapshot(), origins).unwrap();
        sample
    }
}

fn occurrence(sample: &DynamicRuntimeSample) -> DynamicSourceOccurrenceId {
    let mut ids = HashSet::new();
    sample
        .expression
        .visit_source_occurrences(&mut |id| {
            ids.insert(id);
        })
        .unwrap();
    assert_eq!(ids.len(), 1);
    *ids.iter().next().unwrap()
}

fn normal_key() -> Key {
    Key::Normal {
        source: SequenceMasterSource {
            playback_number: Some(1),
            playback_identity: Some(PlaybackIdentity::physical(1).unwrap()),
            cue_list_id: light_core::CueListId::new(),
            temporary: false,
        },
    }
}

#[test]
fn captured_cue_origins_keep_authors_temporary_kinds_and_held_history_distinct() {
    let mut definition = definition();
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    let fixture = FixtureId::new();
    let link = Uuid::new_v4();
    let normal = normal_key();
    let temporary = SequenceMasterSource {
        temporary: true,
        ..normal.source()
    };
    let keys = [
        normal,
        Key::Temporary {
            source: temporary,
            kind: TemporaryPlaybackKind::TempButton,
        },
        Key::Temporary {
            source: temporary,
            kind: TemporaryPlaybackKind::TempFader,
        },
    ];
    let mut rows = keys.map(|key| cue_dynamic_row(&definition, fixture, link, key));
    rows[0].changed_at += chrono::Duration::microseconds(123);
    let mut harness = Harness::new(light_engine::EngineSnapshot {
        dynamics: vec![definition].into(),
        ..Default::default()
    });
    let first = harness.sample(1100, &rows, &[], false);
    assert_eq!(first.samples.len(), 3);
    let mut ids = HashMap::new();
    for sample in &first.samples {
        let id = occurrence(sample);
        ids.insert(sample.controller_id, id);
        let row = rows
            .iter()
            .find(|row| row.source_key.controller_id(link) == sample.controller_id)
            .unwrap();
        let DynamicSourceOrigin::Cue {
            source,
            temporary_kind,
            cue_id,
            changed_at,
            transition_ordinal,
            ..
        } = &harness.origins.get(id).unwrap().origin
        else {
            panic!("expected Cue origin")
        };
        assert_eq!(*source, row.source.into());
        assert_eq!(*cue_id, row.authored_cue_id);
        assert_ne!(*cue_id, row.current_cue_id);
        assert_eq!(*changed_at, row.changed_at);
        assert_eq!(*transition_ordinal, row.transition_ordinal);
        assert_eq!(
            *temporary_kind,
            match row.source_key {
                Key::Normal { .. } => None,
                Key::Temporary { kind, .. } => Some(kind.into()),
            }
        );
    }
    assert_eq!(ids.values().copied().collect::<HashSet<_>>().len(), 3);
    let unchanged = harness.origins.clone();
    harness.sample(1100, &rows, &[], false);
    assert!(unchanged.shares_storage(&harness.origins));

    harness.sample(1100, &rows, &[], true);
    // Advancing to a sparse tracked Cue must not rewrite the previously held leaf.
    rows[0].current_cue_id = Uuid::new_v4();
    rows[0].changed_at += chrono::Duration::milliseconds(200);
    rows[0].changed_at_millis += 200;
    rows[0].transition_ordinal += 1;
    let held = harness.sample(1300, &rows, &[], true);
    for sample in &held.samples {
        assert_eq!(occurrence(sample), ids[&sample.controller_id]);
    }
    let sample = held
        .samples
        .iter()
        .find(|sample| sample.controller_id == keys[0].controller_id(link))
        .unwrap();
    let active = harness
        .origins
        .binding(&DynamicSourceBinding::Authored {
            instance_id: sample.instance_id,
            controller_id: sample.controller_id,
            target: fixture,
            lane_id: sample.lane_id,
        })
        .unwrap();
    assert_ne!(active, ids[&sample.controller_id]);
    assert!(
        matches!(harness.origins.get(active).unwrap().origin, DynamicSourceOrigin::Cue { cue_id, .. } if cue_id == rows[0].authored_cue_id)
    );
    let checkpoint =
        DynamicRuntimeSourceCheckpoint::capture(harness.runtime.snapshot(), &harness.origins)
            .unwrap();
    let bytes = serde_json::to_vec(&checkpoint).unwrap();
    let (restored_runtime, restored_origins) =
        serde_json::from_slice::<DynamicRuntimeSourceCheckpoint>(&bytes)
            .unwrap()
            .restore()
            .unwrap();
    assert_eq!(restored_runtime, checkpoint.runtime);
    assert_eq!(restored_origins.snapshot(), harness.origins.snapshot());
}

#[test]
fn fading_off_keeps_original_occurrence_until_controller_retires() {
    let definition = definition();
    let fixture = FixtureId::new();
    let link = Uuid::new_v4();
    let mut row = cue_dynamic_row(&definition, fixture, link, normal_key());
    let mut harness = Harness::new(light_engine::EngineSnapshot {
        dynamics: vec![definition].into(),
        ..Default::default()
    });
    let first = harness.sample(1100, std::slice::from_ref(&row), &[], false);
    let sample = &first.samples[0];
    let id = occurrence(sample);
    let binding = DynamicSourceBinding::Authored {
        instance_id: sample.instance_id,
        controller_id: sample.controller_id,
        target: fixture,
        lane_id: sample.lane_id,
    };
    row.value = DynamicSemanticValue::DynamicOff {
        instance_link: link,
        timing: DynamicValueTiming {
            fade_millis: Some(1000),
            delay_millis: None,
        },
    };
    harness.sample(1200, std::slice::from_ref(&row), &[], false);
    let halfway = harness.sample(1700, std::slice::from_ref(&row), &[], false);
    assert_eq!(halfway.samples.len(), 1);
    assert_eq!(occurrence(&halfway.samples[0]), id);
    assert_eq!(harness.origins.binding(&binding), Some(id));
    let ended = harness.sample(2200, std::slice::from_ref(&row), &[], false);
    assert!(ended.samples.is_empty());
    assert!(ended.after_runtime.instances.is_empty());
    assert_eq!(harness.origins.binding(&binding), None);
    assert!(
        harness.origins.get(id).is_some(),
        "immutable history remains until cold pruning"
    );
    harness
        .origins
        .prune_runtime(&harness.runtime.snapshot())
        .unwrap();
    assert!(harness.origins.get(id).is_none());
}

#[test]
fn cue_on_during_release_preserves_the_running_instance_past_the_old_deadline() {
    let definition = definition();
    let fixture = FixtureId::new();
    let link = Uuid::new_v4();
    let mut on = cue_dynamic_row(&definition, fixture, link, normal_key());
    let mut harness = Harness::new(light_engine::EngineSnapshot {
        dynamics: vec![definition].into(),
        ..Default::default()
    });
    let first = harness.sample(1100, std::slice::from_ref(&on), &[], false);
    let original = first.samples[0].instance_id;
    let original_occurrence = occurrence(&first.samples[0]);
    let mut off = on.clone();
    off.value = DynamicSemanticValue::DynamicOff {
        instance_link: link,
        timing: DynamicValueTiming {
            fade_millis: Some(1000),
            delay_millis: None,
        },
    };
    harness.sample(1200, &[off], &[], false);
    on.authored_cue_id = Uuid::new_v4();
    on.changed_at += chrono::Duration::milliseconds(700);
    on.changed_at_millis += 700;
    on.transition_ordinal += 1;
    let resumed = harness.sample(1700, std::slice::from_ref(&on), &[], false);
    assert_eq!(resumed.samples[0].instance_id, original);
    assert_ne!(occurrence(&resumed.samples[0]), original_occurrence);
    let later = harness.sample(2300, &[on], &[], false);
    assert_eq!(later.samples.len(), 1);
    assert_eq!(later.samples[0].instance_id, original);
    assert_eq!(later.after_runtime.instances[0].started_at_millis, 1000);
}

fn playback(
    definition: &DynamicDefinition,
    fixture: FixtureId,
    number: u16,
) -> light_playback::PlaybackDefinition {
    serde_json::from_value(serde_json::json!({
        "number": number, "name": "Source playback", "target": {"type": "dynamic", "assignment": {
            "dynamic": {"dynamic_id": definition.id, "last_known_pool_number": definition.pool_number,
                "embedded_fallback": {"definition": definition}},
            "target_scope": {"type": "frozen_targets", "targets": [fixture]}
        }}
    })).unwrap()
}

fn active_playback(definition: &DynamicDefinition, number: u16) -> ActiveDynamicPlayback {
    serde_json::from_value(serde_json::json!({
        "dynamic_id": definition.id, "playback_number": number, "enabled": true, "paused": false,
        "activated_at": chrono::DateTime::from_timestamp_millis(1000).unwrap()
    }))
    .unwrap()
}

#[test]
fn captured_playback_origins_keep_physical_and_virtual_identity_after_disabled_rows() {
    let physical = definition();
    let mut virtual_definition = definition();
    virtual_definition.pool_number = 2;
    let fixture = FixtureId::new();
    let virtual_identity =
        PlaybackIdentity::Virtual(light_playback::VirtualPlaybackAddress::new(3, 1601).unwrap());
    let mut disabled = active_playback(&physical, 9);
    disabled.enabled = false;
    let physical_row = active_playback(&physical, 1);
    let mut virtual_row = active_playback(&virtual_definition, 1601);
    virtual_row.playback_identity = Some(virtual_identity);
    virtual_row.activated_at += chrono::Duration::microseconds(987);
    let mut harness = Harness::new(light_engine::EngineSnapshot {
        dynamics: vec![physical.clone(), virtual_definition.clone()].into(),
        playbacks: vec![playback(&physical, fixture, 1)].into(),
        playback_pages: vec![light_playback::PlaybackPage {
            number: 3,
            name: "Virtual".into(),
            slots: Default::default(),
            virtual_playbacks: [(1601, playback(&virtual_definition, fixture, 1601))].into(),
        }]
        .into(),
        ..Default::default()
    });
    let rows = [disabled, physical_row, virtual_row];
    let output = harness.sample(1100, &[], &rows, false);
    assert_eq!(output.samples.len(), 2);
    for sample in output.samples {
        let id = occurrence(&sample);
        let expected = if sample.controller_id
            == light_playback::dynamic_playback_controller_id(physical.id)
        {
            &rows[1]
        } else {
            &rows[2]
        };
        assert_eq!(
            harness.origins.get(id).unwrap().origin,
            DynamicSourceOrigin::Playback {
                identity: expected
                    .playback_identity
                    .unwrap_or_else(|| PlaybackIdentity::physical(1).unwrap()),
                activated_at: expected.activated_at,
            }
        );
    }
}

#[test]
fn virtual_playback_release_uses_its_fade_and_reenable_cancels_retirement() {
    let definition = definition();
    let fixture = FixtureId::new();
    let mut assignment = playback(&definition, fixture, 1601);
    assignment.xfade_millis = 1000;
    let mut active = active_playback(&definition, 1601);
    active.playback_identity = Some(PlaybackIdentity::Virtual(
        light_playback::VirtualPlaybackAddress::new(3, 1601).unwrap(),
    ));
    let mut harness = Harness::new(light_engine::EngineSnapshot {
        dynamics: vec![definition].into(),
        playback_pages: vec![light_playback::PlaybackPage {
            number: 3,
            name: "Virtual".into(),
            slots: Default::default(),
            virtual_playbacks: [(1601, assignment)].into(),
        }]
        .into(),
        ..Default::default()
    });
    let first = harness.sample(2100, &[], std::slice::from_ref(&active), false);
    let sample = &first.samples[0];
    let id = occurrence(sample);
    let instance = sample.instance_id;
    harness.sample(2200, &[], &[], false);
    let halfway = harness.sample(2700, &[], &[], false);
    assert_eq!(halfway.samples.len(), 1);
    assert_eq!(occurrence(&halfway.samples[0]), id);
    assert!((halfway.samples[0].activation_mix - 0.5).abs() < 0.001);
    harness.sample(2800, &[], std::slice::from_ref(&active), false);
    let resumed = harness.sample(3300, &[], std::slice::from_ref(&active), false);
    assert_eq!(resumed.samples.len(), 1);
    assert_eq!(resumed.samples[0].instance_id, instance);
    assert_eq!(occurrence(&resumed.samples[0]), id);
    assert_eq!(resumed.after_runtime.instances[0].started_at_millis, 1000);
    // Disabling the still-captured row must act like absence; it cannot retain a live vote.
    active.enabled = false;
    harness.sample(3400, &[], std::slice::from_ref(&active), false);
    let ended = harness.sample(4400, &[], &[active], false);
    assert!(ended.samples.is_empty());
    assert!(ended.after_runtime.instances.is_empty());
}

#[test]
fn blind_definition_edit_keeps_effective_cue_and_playback_lane_origins() {
    let definition = definition();
    let fixture = FixtureId::new();
    let cue = cue_dynamic_row(&definition, fixture, Uuid::new_v4(), normal_key());
    let active = active_playback(&definition, 1);
    let mut harness = Harness::new(light_engine::EngineSnapshot {
        dynamics: vec![definition.clone()].into(),
        playbacks: vec![playback(&definition, fixture, 1)].into(),
        ..Default::default()
    });
    let first = harness.sample(
        1100,
        std::slice::from_ref(&cue),
        std::slice::from_ref(&active),
        false,
    );
    assert_eq!(first.samples.len(), 2);
    let before = first
        .samples
        .iter()
        .map(|sample| {
            (
                sample.controller_id,
                (sample.instance_id, occurrence(sample)),
            )
        })
        .collect::<HashMap<_, _>>();
    harness.runtime.set_definitions_pinned(true);
    let mut edited = definition.clone();
    edited.revision += 1;
    edited.lanes[0].id = Uuid::new_v4();
    harness
        .runtime
        .install_definitions([edited.clone()])
        .unwrap();
    Arc::make_mut(&mut harness.snapshot).dynamics = vec![edited].into();
    let pinned = harness.sample(1400, &[cue], std::slice::from_ref(&active), false);
    assert_eq!(pinned.samples.len(), 2);
    for sample in pinned.samples {
        assert_eq!(sample.lane_id, definition.lanes[0].id);
        assert_eq!(
            (sample.instance_id, occurrence(&sample)),
            before[&sample.controller_id]
        );
    }
    let stored = harness.origins.snapshot();
    assert!(
        stored
            .bindings
            .iter()
            .all(|binding| matches!(binding.binding,
        DynamicSourceBinding::Authored { lane_id, .. } if lane_id == definition.lanes[0].id))
    );
    // Committing the definition installs the new lane without restarting the Playback clock.
    harness.runtime.set_definitions_pinned(false);
    let committed = harness.sample(1500, &[], &[active], false);
    assert_eq!(committed.samples.len(), 1);
    let sample = &committed.samples[0];
    assert_ne!(sample.lane_id, definition.lanes[0].id);
    assert_eq!(sample.instance_id, before[&sample.controller_id].0);
    assert_ne!(occurrence(sample), before[&sample.controller_id].1);
}
