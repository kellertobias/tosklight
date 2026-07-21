use super::*;
use light_playback::{
    Cue, CueList, CueListMode, FlashReleaseMode, IntensityPriorityMode, PlaybackButtonAction,
    PlaybackDefinition, PlaybackEngine, PlaybackFaderMode, RestartMode, WrapMode,
};

#[test]
fn direct_cuelist_action_projection_is_not_replaced_by_assigned_playbacks() {
    let cue_list = cue_list();
    let cue_list_id = cue_list.id;
    let first = definition(1, cue_list_id);
    let second = definition(2, cue_list_id);
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.go(cue_list_id).unwrap();
    let direct_runtime = engine.active().pop().unwrap();
    engine.register_definition(first.clone()).unwrap();
    engine.register_definition(second.clone()).unwrap();
    engine.goto_playback(1, 2.0).unwrap();
    engine.goto_playback(2, 3.0).unwrap();
    engine.restore_active([direct_runtime]);
    let runtime = engine.runtime_status();
    let requested = PlaybackRuntimeIdentity::CueList(cue_list_id);
    let scope = test_scope();

    let direct = cue_list_projection(
        scope,
        requested.clone(),
        None,
        cue_list_id,
        direct_cue_list_runtime(&runtime, cue_list_id),
    );
    assert_eq!(direct.playback_number, None);
    assert_eq!(direct.current_cue().map(|cue| cue.number), Some(1.0));

    let snapshot = EngineSnapshot {
        playbacks: vec![first, second],
        ..EngineSnapshot::default()
    };
    let mut repair = Vec::new();
    project_cue_list(
        scope,
        &snapshot,
        &runtime,
        requested,
        cue_list_id,
        &mut repair,
    );
    assert_eq!(repair.len(), 3);
    assert_eq!(
        repair
            .iter()
            .map(|projection| (
                projection.playback_number,
                projection.current_cue().map(|cue| cue.number)
            ))
            .collect::<Vec<_>>(),
        vec![
            (Some(1), Some(2.0)),
            (Some(2), Some(3.0)),
            (None, Some(1.0))
        ]
    );
}

fn test_scope() -> PlaybackShowScope {
    PlaybackShowScope {
        show_id: uuid::Uuid::from_u128(1),
        show_revision: 3,
    }
}

fn cue_list() -> CueList {
    CueList {
        id: CueListId::new(),
        name: "Shared".into(),
        priority: 0,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Off),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![Cue::new(1.0), Cue::new(2.0), Cue::new(3.0)],
    }
}

fn definition(number: u16, cue_list_id: CueListId) -> PlaybackDefinition {
    PlaybackDefinition {
        number,
        name: format!("Playback {number}"),
        target: light_playback::PlaybackTarget::CueList { cue_list_id },
        buttons: [
            PlaybackButtonAction::GoMinus,
            PlaybackButtonAction::Go,
            PlaybackButtonAction::Flash,
        ],
        button_count: 3,
        fader: PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}
