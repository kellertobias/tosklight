use light_core::{CueListId, FixtureId};
use light_playback::*;
use std::sync::Arc;
use uuid::Uuid;
fn address(page: u8, position: u16) -> PlaybackAddress {
    PlaybackAddress::new(page, position).unwrap()
}
fn source(id: &str) -> PlaybackControlSource {
    PlaybackControlSource {
        surface_id: id.into(),
        control_id: 7,
    }
}
fn list(cues: Vec<Cue>) -> CueList {
    CueList {
        pool_number: None,
        legacy_pool_aliases: Vec::new(),
        id: CueListId::new(),
        name: "Main".into(),
        priority: 10,
        mode: CueListMode::Sequence,
        looped: false,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Off),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_step_millis: 1_000,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_group: None,
        speed_multiplier: 1.0,
        cues,
    }
}
fn dynamic_playback_definition(
    number: u16,
    fader_mode: DynamicPlaybackFaderMode,
    targetless: bool,
) -> PlaybackDefinition {
    let dynamic_id = Uuid::new_v4();
    let definition = light_dynamics::DynamicDefinition {
        id: dynamic_id,
        pool_number: number,
        revision: 1,
        name: format!("Dynamic {number}"),
        color: None,
        icon: None,
        overall_speed_multiplier: light_dynamics::Rational::ONE,
        target_binding: if targetless {
            light_dynamics::DynamicTargetBinding::Targetless
        } else {
            light_dynamics::DynamicTargetBinding::FrozenTargets {
                targets: vec![FixtureId::new()],
            }
        },
        lanes: Vec::new(),
        random_groups: Vec::new(),
        phase_spread_mode: light_dynamics::DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: light_dynamics::DynamicSpatialMappingOverride::default(),
        phase: light_dynamics::PhaseDistribution {
            ordering: light_dynamics::PhaseOrdering::Selection,
            offset_degrees: 0.0,
            span_degrees: 360.0,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: Vec::new(),
        },
        speed: light_dynamics::DynamicSpeed::Fixed {
            duration_millis: 1_000,
        },
        run_mode: light_dynamics::DynamicRunMode::Loop,
        default_activation: light_dynamics::ActivationPolicy::StartNow,
        activation_boundary: light_dynamics::ActivationBoundary::Beat,
    };
    let target = PlaybackTarget::Dynamic {
        assignment: DynamicPlaybackAssignment {
            dynamic: light_dynamics::DynamicReference {
                dynamic_id: Some(dynamic_id),
                last_known_pool_number: number,
                embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                    definition: Arc::new(definition),
                },
            },
            revision: 1,
            target_scope: targetless.then(|| DynamicPlaybackTargetScope::FrozenTargets {
                targets: vec![FixtureId::new()],
            }),
            fader_mode,
            priority: 0,
            activation_override: None,
            resume_policy: DynamicPlaybackResumePolicy::FollowDynamic,
            local_speed_multiplier: light_dynamics::Rational::ONE,
            learned_duration_millis: None,
            crossfade_non_intensity: false,
            auto_off_at_zero: true,
            auto_off_flash_release: true,
            auto_off_full_control: true,
        },
    };
    PlaybackDefinition {
        name: format!("Dynamic Playback {number}"),
        buttons: PlaybackDefinition::default_buttons(&target),
        button_count: 3,
        fader: PlaybackFaderMode::Master,
        has_fader: true,
        footprint: PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
        target,
    }
}
fn cue_engine() -> (PlaybackEngine, PlaybackAddress, PlaybackAddress) {
    let list = list(vec![
        Cue::new(CueNumber::from(1_u8)),
        Cue::new(CueNumber::from(2_u8)),
    ]);
    let id = list.id;
    let a = address(1, 1);
    let b = address(99, 1000);
    let mut engine = PlaybackEngine::default();
    engine.register(list).unwrap();
    engine
        .register_definition(a, PlaybackDefinition::new_cue_list("First", id))
        .unwrap();
    engine
        .register_definition(b, PlaybackDefinition::new_cue_list("Last", id))
        .unwrap();
    (engine, a, b)
}
#[test]
fn address_validation_rejects_invalid_and_retired_shapes() {
    assert_eq!(address(99, 1000).to_string(), "99.1000");
    for (page, position) in [(0, 1), (100, 1), (1, 0), (1, 1001)] {
        assert!(PlaybackAddress::new(page, position).is_err());
    }
    assert!(serde_json::from_str::<PlaybackAddress>(r#"{"page":100,"position":1}"#).is_err());
    assert!(serde_json::from_str::<PlaybackAddress>(r#"{"page":1,"number":1001}"#).is_err());
}
#[test]
fn preload_ranges_default_all_pages_and_restrict_explicitly() {
    assert!(PlaybackPreloadScope::default().contains(address(99, 1000)));
    let scope = PlaybackPreloadScope {
        position_ranges: vec![
            PlaybackPositionRange { start: 3, end: 5 },
            PlaybackPositionRange {
                start: 900,
                end: 1000,
            },
        ],
        pages: Some(vec![2, 99]),
    };
    scope.validate().unwrap();
    assert!(scope.contains(address(99, 1000)));
    assert!(scope.contains(address(2, 4)));
    assert!(!scope.contains(address(1, 4)));
    assert!(!scope.contains(address(2, 6)));
    assert!(
        PlaybackPreloadScope {
            position_ranges: vec![PlaybackPositionRange { start: 5, end: 3 }],
            pages: None
        }
        .validate()
        .is_err()
    );
}
#[test]
fn page_aliases_share_one_cuelist_runtime() {
    let (mut engine, a, b) = cue_engine();
    engine.on(a).unwrap();
    engine.go_playback(b).unwrap();
    assert!(engine.same_target(a, b));
    assert_eq!(engine.runtime().len(), 1);
    assert_eq!(
        engine.playback_runtime(a).unwrap().current_cue_number,
        Some(CueNumber::from(2_u8))
    );
    engine.off(b).unwrap();
    assert!(!engine.permanent_enabled(a));
}
#[test]
fn same_address_flash_press_release_is_independent_per_source() {
    let (mut engine, a, _) = cue_engine();
    let one = source("main");
    let two = source("pane:second");
    engine.set_flash(a, &one, true).unwrap();
    engine.set_flash(a, &one, true).unwrap();
    engine.set_flash(a, &two, true).unwrap();
    assert_eq!(engine.active().len(), 2);
    engine.set_flash(a, &one, false).unwrap();
    assert_eq!(engine.active().len(), 1);
    assert!(
        engine
            .runtime_status_for_playback(a)
            .unwrap()
            .playback
            .flash
    );
    engine.set_flash(a, &one, false).unwrap();
    assert_eq!(engine.active().len(), 1);
    engine.set_flash(a, &two, false).unwrap();
    assert!(engine.active().is_empty());
}
#[test]
fn disconnect_cancels_only_owned_holds_and_preserves_permanent_state() {
    let (mut engine, a, _) = cue_engine();
    let one = source("main");
    let two = source("pane:second");
    engine.on(a).unwrap();
    engine.set_flash(a, &one, true).unwrap();
    engine.set_flash(a, &two, true).unwrap();
    engine.cancel_control_surface("main");
    assert_eq!(engine.active().len(), 2);
    assert!(engine.permanent_enabled(a));
    engine.cancel_control_source(&two);
    assert_eq!(engine.active().len(), 1);
    assert!(engine.permanent_enabled(a));
}
#[test]
fn pickup_is_independent_for_controls_on_same_assignment() {
    let (mut engine, a, _) = cue_engine();
    let one = source("hardware:first");
    let two = source("hardware:second");
    engine.on(a).unwrap();
    engine.set_master_mutation(a, &one, 0.0).unwrap();
    engine.set_master_mutation(a, &two, 1.0).unwrap();
    assert!(engine.control_state(a, &one).fader_pickup_required);
    assert!(!engine.control_state(a, &two).fader_pickup_required);
    engine.set_master_mutation(a, &one, 1.0).unwrap();
    engine.set_master_mutation(a, &two, 0.5).unwrap();
    assert!(engine.control_state(a, &one).fader_pickup_required);
    assert_eq!(engine.playback_runtime(a).unwrap().master, 0.5);
}
#[test]
fn dynamic_permanent_off_during_flash_cannot_revive_on_release() {
    let a = address(2, 55);
    let mut definition = dynamic_playback_definition(17, DynamicPlaybackFaderMode::Master, false);
    let PlaybackTarget::Dynamic { assignment } = &mut definition.target else {
        unreachable!()
    };
    assignment.auto_off_flash_release = false;
    let mut engine = PlaybackEngine::default();
    engine.register_definition(a, definition).unwrap();
    engine.on_dynamic_mutation(a).unwrap();
    let one = source("main");
    let two = source("hardware:second");
    engine.set_dynamic_flash_mutation(a, &one, true).unwrap();
    engine.set_dynamic_flash_mutation(a, &two, true).unwrap();
    engine.release_permanent_mutation(a).unwrap();
    assert!(!engine.permanent_enabled(a));
    assert!(engine.active_dynamic_playback(a).unwrap().enabled);
    engine.set_dynamic_flash_mutation(a, &one, false).unwrap();
    assert!(!engine.permanent_enabled(a));
    engine.set_dynamic_flash_mutation(a, &two, false).unwrap();
    assert!(!engine.active_dynamic_playback(a).unwrap().enabled);
}
#[test]
fn dynamic_aliases_preserve_activation_receipt_and_rotate_on_off_on() {
    let a = address(1, 3);
    let b = address(99, 3);
    let definition = dynamic_playback_definition(18, DynamicPlaybackFaderMode::Master, false);
    let PlaybackTarget::Dynamic { assignment } = &definition.target else {
        unreachable!()
    };
    let target = assignment.target_id();
    let mut engine = PlaybackEngine::default();
    engine.register_definition(a, definition.clone()).unwrap();
    engine.register_definition(b, definition).unwrap();
    engine.on_dynamic_mutation(a).unwrap();
    let first = engine
        .active_dynamic_playback(b)
        .unwrap()
        .start_state
        .incarnation;
    engine.acknowledge_dynamic_playback_start(target, first, DynamicPlaybackStartOutcome::Accepted);
    engine.on_dynamic_mutation(b).unwrap();
    assert_eq!(
        engine
            .active_dynamic_playback(a)
            .unwrap()
            .start_state
            .incarnation,
        first
    );
    engine.off_dynamic_mutation(a).unwrap();
    engine.on_dynamic_mutation(b).unwrap();
    assert_ne!(
        engine
            .active_dynamic_playback(a)
            .unwrap()
            .start_state
            .incarnation,
        first
    );
    assert!(!engine.acknowledge_dynamic_playback_start(
        target,
        first,
        DynamicPlaybackStartOutcome::Accepted
    ));
}

#[test]
fn volatile_telemetry_fans_out_every_shared_assignment() {
    let (mut engine, a, b) = cue_engine();
    engine.on(a).unwrap();
    let samples = engine.telemetry_samples_at(chrono::Utc::now());
    assert_eq!(samples.len(), 2);
    assert_eq!(
        samples
            .iter()
            .map(|sample| sample.address)
            .collect::<Vec<_>>(),
        vec![a, b]
    );
    assert!(samples.iter().all(|sample| sample.enabled));
}

#[test]
fn cue_off_during_formerly_off_intensity_only_flash_cannot_promote_on_release() {
    let address = address(1, 70);
    let cue_list = list(vec![Cue::new(CueNumber::from(1_u8))]);
    let mut definition = PlaybackDefinition::new_cue_list("Flash", cue_list.id);
    definition.flash_release = FlashReleaseMode::ReleaseIntensityOnly;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(address, definition).unwrap();
    let source = source("main");
    engine.set_flash(address, &source, true).unwrap();
    assert!(!engine.permanent_enabled(address));
    engine.off(address).unwrap();
    assert!(
        engine
            .runtime_status_for_playback(address)
            .unwrap()
            .temporary_active
    );
    engine.set_flash(address, &source, false).unwrap();
    assert!(!engine.permanent_enabled(address));
}

#[test]
fn solo_inhibition_preserves_formerly_off_dynamic_flash_until_release() {
    let address = address(2, 71);
    let mut definition = dynamic_playback_definition(17, DynamicPlaybackFaderMode::Master, false);
    let PlaybackTarget::Dynamic { assignment } = &mut definition.target else {
        unreachable!()
    };
    assignment.auto_off_flash_release = false;
    let alias = self::address(3, 72);
    let mut engine = PlaybackEngine::default();
    engine
        .register_definition(alias, definition.clone())
        .unwrap();
    engine.register_definition(address, definition).unwrap();
    let source = source("main");
    engine
        .set_dynamic_flash_mutation(address, &source, true)
        .unwrap();
    assert!(!engine.permanent_enabled(address));
    let before_inhibition = engine.clone();
    assert_eq!(
        engine
            .inhibit_held_release_activation(address)
            .unwrap()
            .effect,
        PlaybackRuntimeEffect::Transient
    );
    assert_eq!(
        engine.retained_runtime_effect_since(&before_inhibition),
        PlaybackRuntimeEffect::Transient
    );
    assert_eq!(
        engine.numbered_runtime_effects_since(&before_inhibition),
        vec![
            (address, PlaybackRuntimeEffect::Transient),
            (alias, PlaybackRuntimeEffect::Transient),
        ]
    );
    let held = engine.active_dynamic_playback(address).unwrap();
    assert!(held.enabled && held.flash);
    assert_eq!(held.master, 1.0);
    engine
        .set_dynamic_flash_mutation(address, &source, false)
        .unwrap();
    assert!(!engine.active_dynamic_playback(address).unwrap().enabled);
    assert!(!engine.permanent_enabled(address));
}

#[test]
fn restoring_permanent_runtime_does_not_restore_held_control_sources() {
    let address = address(2, 72);
    let cue_list = list(vec![Cue::new(CueNumber::from(1_u8))]);
    let definition = PlaybackDefinition::new_cue_list("Restore", cue_list.id);
    let source = source("pane:held");
    let mut live = PlaybackEngine::default();
    live.register(cue_list.clone()).unwrap();
    live.register_definition(address, definition.clone())
        .unwrap();
    live.on(address).unwrap();
    live.set_flash(address, &source, true).unwrap();
    assert_eq!(live.active().len(), 2);
    let stored: Vec<ActivePlayback> =
        serde_json::from_value(serde_json::to_value(live.runtime()).unwrap()).unwrap();
    let mut restored = PlaybackEngine::default();
    restored.register(cue_list).unwrap();
    restored.register_definition(address, definition).unwrap();
    restored.restore_active(stored);
    let status = restored.runtime_status_for_playback(address).unwrap();
    assert!(status.playback.enabled);
    assert!(!status.temporary_active && !status.playback.flash);
    assert_eq!(restored.active().len(), 1);
    restored.set_flash(address, &source, false).unwrap();
    assert!(restored.permanent_enabled(address));
}
