//! Actual captured Cue sampler -> scalar compositor -> engine -> native words (TL-777).
use super::*;
use light_core::{
    CueListId, ManualClock, ReplacementHeadTarget, ReplacementProfileContext,
    ReplacementProgramProjection,
};
use light_engine::{CueListPlaybackAction, EnginePlaybackCommand, EngineSnapshot};
use light_fixture::{ChannelFunction, FixtureChannel, FixtureProfile, PatchedFixture, PatchedHead};
use light_playback::{Cue, CueDynamicChange, CueList, GroupCueChange};
use light_programmer::GroupDefinition;
use serde_json::json;

fn rig() -> (
    Vec<PatchedFixture>,
    Vec<FixtureId>,
    HashMap<FixtureId, ReplacementProgramProjection>,
) {
    let mut fixtures = Vec::new();
    let mut roots = Vec::new();
    let mut projections = HashMap::new();
    for rank in 0..4 {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "TL777 Test".into();
        profile.name = "Shared U16 or logical U8 dimmer".into();
        profile.revision = 1;
        let mode = &mut profile.modes[0];
        let logical = rank >= 2;
        mode.heads[0].master_shared = !logical;
        mode.splits[0].footprint = if logical { 1 } else { 2 };
        let head = mode.heads[0].id;
        let max = if logical { 255 } else { 65535 };
        let channel: FixtureChannel = serde_json::from_value(json!({
            "id":Uuid::new_v4(),"head_id":head,"split":1,
            "attribute":"intensity","fixture_attribute":"intensity",
            "resolution":if logical {"u8"} else {"u16"},
            "secondary_slots":if logical {vec![]} else {vec![2]},
            "default_raw":0,"highlight_raw":max,
            "functions":[ChannelFunction::continuous("Dimmer",AttributeKey::intensity(),max)]
        }))
        .unwrap();
        mode.channels = vec![channel];
        let context = ReplacementProfileContext {
            profile_id: profile.id,
            profile_revision: 1,
            mode_id: mode.id,
        };
        let root = FixtureId::new();
        let owner = if logical { FixtureId::new() } else { root };
        let mut fixture = physical_adapter::color::profiles::patched(&profile, root, 1 + rank * 3);
        fixture.fixture_number = Some(u32::from(rank) + 1);
        if logical {
            fixture.logical_heads = vec![PatchedHead {
                fixture_id: owner,
                head_index: 0,
                profile_head_id: Some(head),
            }];
        }
        projections.insert(
            root,
            ReplacementProgramProjection {
                source_owner: root,
                source_profile: ReplacementProfileContext {
                    profile_id: FixtureId::new(),
                    profile_revision: 1,
                    mode_id: Uuid::new_v4(),
                },
                source_head_id: Uuid::new_v4(),
                target_profile: context,
                targets: vec![ReplacementHeadTarget {
                    fixture_id: owner,
                    profile_head_id: head,
                }],
            },
        );
        roots.push(root);
        fixtures.push(fixture);
    }
    (fixtures, roots, projections)
}

#[derive(Clone, Copy, Debug)]
enum Case {
    Full,
    Zero,
    FreshMaster,
    DirectNoEnvelope,
    Empty,
    Dormant,
    OtherHtp,
    Programmer,
}

fn exercise(case: Case) {
    let static_base = !matches!(case, Case::Zero | Case::FreshMaster);
    let (fixtures, roots, mut projections) = rig();
    let owners: Vec<_> = roots
        .iter()
        .map(|root| projections[root].targets[0].fixture_id)
        .collect();
    if matches!(case, Case::FreshMaster) {
        projections.clear();
    }
    if matches!(case, Case::Empty) {
        for projection in projections.values_mut() {
            projection.targets.clear();
        }
    }
    if matches!(case, Case::Dormant) {
        for projection in projections.values_mut() {
            projection.target_profile.profile_revision += 1;
        }
    }
    let started = chrono::DateTime::from_timestamp_millis(1_000_000).unwrap()
        + chrono::Duration::microseconds(140);
    let clock = Arc::new(ManualClock::new(started));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = light_core::SessionId::new();
    programmers.start(session);
    let engine = Engine::new(programmers.clone());
    let mut wire_definition = serde_json::to_value(super::definition()).unwrap();
    wire_definition["lanes"][0]["mode"] = json!("max_min");
    wire_definition["speed"]["duration_millis"] = json!(4000);
    wire_definition["phase"]["span_degrees"] = json!(360);
    let definition: DynamicDefinition = serde_json::from_value(wire_definition).unwrap();
    let id = CueListId::new();
    let link = Uuid::new_v4();
    let mut cue = Cue::new(2_u16.into());
    cue.group_changes.push(GroupCueChange {
        group_id: "roots".into(),
        attribute: AttributeKey::intensity(),
        value: Some(AttributeValue::Normalized(if static_base {
            1.
        } else {
            0.
        })),
        replacement_projections: projections,
        preset_reference: None,
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    });
    if matches!(case, Case::DirectNoEnvelope) {
        cue.changes = roots
            .iter()
            .map(|root| {
                light_playback::CueChange::set(
                    *root,
                    AttributeKey::intensity(),
                    AttributeValue::Normalized(0.),
                )
            })
            .collect();
    }
    cue.dynamic_changes = roots
        .iter()
        .map(|root| CueDynamicChange {
            fixture_id: *root,
            attribute: AttributeKey::intensity(),
            automatic_restore: false,
            value: cue_dynamic_row(
                &definition,
                *root,
                link,
                light_playback::CueDynamicSourceKey::Normal {
                    source: light_playback::SequenceMasterSource {
                        playback_address: None,
                        control_source_id: None,
                        cue_list_id: id,
                        temporary: false,
                    },
                },
            )
            .value,
        })
        .collect();
    let list: CueList = serde_json::from_value(json!({"id":id,"name":"Recorded Group + Dynamic",
        "priority":0,"mode":"sequence","looped":false,"cues":[cue]}))
    .unwrap();
    let mut lists = vec![list];
    let other_id = CueListId::new();
    if matches!(case, Case::OtherHtp) {
        let mut other = Cue::new(1_u16.into());
        other.changes = owners
            .iter()
            .map(|owner| {
                light_playback::CueChange::set(
                    *owner,
                    AttributeKey::intensity(),
                    AttributeValue::Normalized(0.75),
                )
            })
            .collect();
        lists.push(
            serde_json::from_value(json!({"id":other_id,"name":"Independent owner",
            "priority":0,"mode":"sequence","looped":false,"cues":[other]}))
            .unwrap(),
        );
    }
    engine
        .replace_snapshot(EngineSnapshot {
            required_programming_contract: 4,
            fixtures: fixtures.clone().into(),
            groups: vec![GroupDefinition {
                id: "roots".into(),
                fixtures: roots.clone(),
                ..Default::default()
            }]
            .into(),
            cue_lists: lists.into(),
            dynamics: vec![definition.clone()].into(),
            ..Default::default()
        })
        .unwrap();
    engine
        .execute_playback(EnginePlaybackCommand::CueList {
            id,
            action: CueListPlaybackAction::GoAt(started),
        })
        .unwrap();
    if matches!(case, Case::OtherHtp) {
        engine
            .execute_playback(EnginePlaybackCommand::CueList {
                id: other_id,
                action: CueListPlaybackAction::GoAt(started + chrono::Duration::microseconds(1)),
            })
            .unwrap();
    }
    if matches!(case, Case::Programmer) {
        clock.set(started + chrono::Duration::milliseconds(1));
        for owner in &owners {
            programmers.set(
                session,
                *owner,
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.25),
            );
        }
    }
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    for elapsed in [0, 1000, 2000, 3000, 4000] {
        clock.set(started + chrono::Duration::milliseconds(elapsed));
        let capture = engine.prepare_output_frame(Default::default());
        let snapshot = capture.snapshot();
        let addresser = capture.frame_addresser();
        let speeds = [DynamicSpeedTransport {
            effective_bpm: 120.,
            phase_origin_millis: 0,
            phase_reference_millis: 0,
            beat_phase: 0.,
            phase_advancing: true,
        }; 5];
        let inputs = CapturedDynamicInputs {
            now: capture.sampled_at(),
            speed_transports: &speeds,
            rate: 44,
            snapshot: &snapshot,
            programmer_values: capture.dynamic_programmer_values(),
            programmer_rows: Some(capture.dynamic_programmer_rows()),
            cue_values: capture.cue_dynamic_values(),
            dynamic_playbacks: capture.dynamic_playbacks(),
            playback_paused: false,
            addresser: &addresser,
            extra_programmer_values: &[],
            programmer_reconciliation_cache: None,
            force_source_reconciliation: false,
        };
        let sources = TickSources::prepared(&engine, &capture, &[]);
        let sampled = sample_captured_dynamic_inputs(&mut runtime, &inputs, &sources);
        assert_eq!(sampled.after_runtime.instances.len(), 1);
        assert_eq!(sampled.samples.len(), 4);
        let batch = project_captured_dynamic_sample(&inputs, &sampled, &sources);
        if matches!(case, Case::Full | Case::Empty) {
            let live_before = serde_json::to_value(runtime.output_projection_snapshot()).unwrap();
            let preload = engine.prepare_preload_frame(&capture, None);
            let state = light_engine::PreloadFrameState::default();
            let preview_sources = PreloadTickSources {
                engine: &engine,
                input: &preload,
                state: &state,
                before_release: false,
                baseline_samples: &[],
                values: OnceLock::new(),
            };
            let mut preview_runtime = runtime.fork_for_preview();
            let preview_sample =
                sample_captured_dynamic_inputs(&mut preview_runtime, &inputs, &preview_sources);
            let preview_batch =
                project_captured_dynamic_sample(&inputs, &preview_sample, &preview_sources);
            let values = engine.observe_prepared_preload_values(
                &preload,
                &preview_batch,
                &state,
                light_engine::PreloadBranch::AfterRelease,
            );
            for (rank, owner) in owners.iter().enumerate() {
                let expected = if matches!(case, Case::Empty) {
                    0.
                } else {
                    (std::f64::consts::TAU * (elapsed as f64 / 4000. + rank as f64 / 4.)).sin()
                        * 0.5
                        + 0.5
                };
                let actual = values
                    .value(*owner, &AttributeKey::intensity())
                    .and_then(AttributeValue::normalized)
                    .unwrap_or(0.);
                assert!(
                    (f64::from(actual) - expected).abs() < 1e-6,
                    "Preload case={case:?} rank={rank}"
                );
            }
            assert_eq!(
                serde_json::to_value(runtime.output_projection_snapshot()).unwrap(),
                live_before,
                "speculative Preload must not change the live Dynamic clock/ownership"
            );
        }
        let rendered = engine.render_prepared(&capture, &batch).unwrap();
        let bytes = &rendered.universes[&1];
        for rank in 0..4 {
            let offset = rank * 3;
            let actual = if rank < 2 {
                u32::from(bytes[offset]) * 256 + u32::from(bytes[offset + 1])
            } else {
                u32::from(bytes[offset])
            };
            let mut normalized =
                (std::f64::consts::TAU * (elapsed as f64 / 4000. + rank as f64 / 4.)).sin() * 0.5
                    + 0.5;
            if matches!(case, Case::OtherHtp) {
                normalized = normalized.max(0.75);
            }
            if matches!(case, Case::Programmer) {
                normalized = 0.25;
            }
            if matches!(case, Case::Empty | Case::Dormant)
                || (matches!(case, Case::FreshMaster | Case::DirectNoEnvelope) && rank >= 2)
            {
                normalized = 0.;
            }
            let expected = (normalized * if rank < 2 { 65535. } else { 255. }).round() as u32;
            assert!(
                actual.abs_diff(expected) <= 1,
                "case={case:?} elapsed={elapsed} rank={rank} actual={actual} expected={expected}"
            );
        }
        assert_eq!(engine.snapshot().groups[0].fixtures, roots);
    }
}

#[test]
fn tl777_same_cue_group_full_intensity_and_dynamic_emit_native_quadrature() {
    exercise(Case::Full);
}

#[test]
fn tl777_dynamic_routes_original_root_with_captured_consent_and_zero_group_base() {
    exercise(Case::Zero);
}

#[test]
fn tl777_fresh_master_dynamic_does_not_inherit_old_replacement_consent() {
    exercise(Case::FreshMaster);
}
#[test]
fn tl777_explicit_empty_projection_remains_dormant() {
    exercise(Case::Empty);
}
#[test]
fn tl777_stale_profile_projection_remains_dormant() {
    exercise(Case::Dormant);
}
#[test]
fn tl777_independent_playback_intensity_still_competes_by_htp() {
    exercise(Case::OtherHtp);
}
#[test]
fn tl777_programmer_intensity_keeps_ltp_over_sampled_cue() {
    exercise(Case::Programmer);
}

#[test]
fn tl777_direct_current_master_assignment_does_not_borrow_group_replacement_consent() {
    exercise(Case::DirectNoEnvelope);
}
