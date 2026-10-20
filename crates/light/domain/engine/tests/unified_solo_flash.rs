use light_core::FixtureId;
use light_engine::{Engine, EnginePlaybackCommand, EngineSnapshot, PlaybackAction};
use light_playback::*;
use light_programmer::ProgrammerRegistry;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;
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

#[test]
fn solo_activation_does_not_promote_a_formerly_off_dynamic_after_flash_release() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let a = PlaybackAddress::new(1, 401).unwrap();
    let b = PlaybackAddress::new(2, 101).unwrap();
    let mut first = dynamic_playback_definition(17, DynamicPlaybackFaderMode::Master, false);
    if let PlaybackTarget::Dynamic { assignment } = &mut first.target {
        assignment.auto_off_flash_release = false;
    }
    let second = dynamic_playback_definition(18, DynamicPlaybackFaderMode::Master, false);
    engine
        .replace_snapshot(EngineSnapshot {
            playback_pages: Arc::new(vec![
                PlaybackPage {
                    number: 1,
                    name: "One".into(),
                    assignments: HashMap::from([(401, first)]),
                },
                PlaybackPage {
                    number: 2,
                    name: "Two".into(),
                    assignments: HashMap::from([(101, second)]),
                },
            ]),
            playback_solo_regions: Arc::new(vec![PlaybackSoloRegion {
                color: None,
                id: Uuid::new_v4(),
                name: "Across pages".into(),
                members: vec![a, b],
            }]),
            ..Default::default()
        })
        .unwrap();
    let source = PlaybackControlSource {
        surface_id: "main".into(),
        control_id: 7,
    };
    let run = |address, action| {
        engine
            .execute_playback(EnginePlaybackCommand::Playback {
                address,
                action,
                control_source: source.clone(),
                activation_origin: None,
            })
            .unwrap()
    };
    run(a, PlaybackAction::SetFlash(true));
    run(b, PlaybackAction::On);
    assert!(engine.active_dynamic_playback(a).unwrap().flash);
    run(a, PlaybackAction::SetFlash(false));
    assert!(
        !engine
            .active_dynamic_playback(a)
            .is_some_and(|runtime| runtime.enabled),
        "releasing Flash must not revive the old Solo peer"
    );
    assert!(engine.active_dynamic_playback(b).unwrap().enabled);
}
