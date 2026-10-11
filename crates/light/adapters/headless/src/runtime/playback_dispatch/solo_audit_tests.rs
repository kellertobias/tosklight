use super::*;
use light_playback::{
    Cue, CueNumber, PlaybackAddress, PlaybackControlSource, PlaybackDefinition, PlaybackPage,
    PlaybackSoloRegion,
};

fn address(page: u8, position: u16) -> PlaybackAddress {
    PlaybackAddress::new(page, position).unwrap()
}

#[tokio::test]
async fn solo_dispatch_audit_reports_actual_released_aliases_and_osc_source() {
    let (state, path) = super::super::tests::test_state();
    let ids = [light_core::CueListId::new(), light_core::CueListId::new()];
    let cue_lists =
        ids.iter()
            .map(|id| {
                serde_json::from_value::<light_playback::CueList>(serde_json::json!({
        "id": id, "name": "Audit Cue", "priority": 0, "mode": "sequence", "looped": true,
        "cues": [Cue::new(CueNumber::from(1_u8))]
    })).unwrap()
            })
            .collect::<Vec<_>>();
    let incoming_cue_id = cue_lists[1].cues[0].id;
    let definitions = ids.map(|id| {
        let mut definition = PlaybackDefinition::new_cue_list("Audit Playback", id);
        definition.auto_off = false;
        definition
    });
    state
        .output
        .replace_snapshot(EngineSnapshot {
            cue_lists: cue_lists.into(),
            playback_pages: vec![
                PlaybackPage {
                    number: 1,
                    name: "One".into(),
                    assignments: [(1, definitions[0].clone()), (2, definitions[1].clone())].into(),
                },
                PlaybackPage {
                    number: 3,
                    name: "Alias".into(),
                    assignments: [(401, definitions[0].clone())].into(),
                },
            ]
            .into(),
            playback_solo_regions: vec![PlaybackSoloRegion {
                id: Uuid::new_v4(),
                name: "Pair".into(),
                members: vec![address(1, 1), address(1, 2)],
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    let source = PlaybackControlSource {
        surface_id: "hardware:audit".into(),
        control_id: 17,
    };
    state
        .output
        .engine()
        .execute_playback(EnginePlaybackCommand::Playback {
            address: address(1, 1),
            action: light_engine::PlaybackAction::On,
            activation_origin: None,
            control_source: source.clone(),
        })
        .unwrap();
    let desk = state.installation.desk().unwrap();
    let action =
        light_application::ActionContext::system(desk.id, light_application::ActionSource::Osc);
    let dispatch = |name: &str, pressed| {
        dispatch_playback_action(
            &state,
            address(1, 2),
            &definitions[1],
            name,
            &PoolPlaybackInput {
                pressed: Some(pressed),
                ..Default::default()
            },
            PlaybackDispatchContext {
                action: &action,
                session: None,
                desk: Some(&desk),
                source: "osc",
                control_source: &source,
                activation_origin: None,
            },
        )
    };
    // A direct Timecode Cue start does not activate an assigned Solo member. An
    // already-running target's GO must report only changes the central engine made.
    state
        .output
        .engine()
        .execute_playback(EnginePlaybackCommand::CueList {
            id: ids[1],
            action: light_engine::CueListPlaybackAction::ExecuteCueId(incoming_cue_id),
        })
        .unwrap();
    assert!(
        state
            .output
            .engine()
            .permanent_playback_enabled(address(1, 2))
    );
    dispatch("go", true).unwrap();
    assert!(
        state
            .output
            .engine()
            .permanent_playback_enabled(address(1, 1))
    );
    assert!(
        state
            .output
            .engine()
            .permanent_playback_enabled(address(1, 2))
    );
    dispatch("off", true).unwrap();
    assert!(
        state
            .output
            .engine()
            .permanent_playback_enabled(address(1, 1))
    );
    assert!(
        !state
            .events
            .audit_events()
            .iter()
            .any(|event| event.kind == "playback_exclusion_applied")
    );
    dispatch("flash", true).unwrap();
    assert!(
        state
            .output
            .engine()
            .permanent_playback_enabled(address(1, 1))
    );
    dispatch("flash", false).unwrap();
    assert!(
        !state
            .events
            .audit_events()
            .iter()
            .any(|event| event.kind == "playback_exclusion_applied")
    );
    dispatch("go", true).unwrap();
    assert!(
        !state
            .output
            .engine()
            .permanent_playback_enabled(address(1, 1))
    );
    assert!(
        !state
            .output
            .engine()
            .permanent_playback_enabled(address(3, 401))
    );
    let events = state.events.audit_events();
    let solo = events
        .iter()
        .filter(|event| event.kind == "playback_exclusion_applied")
        .collect::<Vec<_>>();
    assert_eq!(solo.len(), 1);
    assert_eq!(solo[0].payload["source"], "osc");
    assert_eq!(
        solo[0].payload["activated_playback"],
        serde_json::json!({"page":1,"position":2})
    );
    assert_eq!(
        solo[0].payload["released_playbacks"],
        serde_json::json!([{"page":1,"position":1},{"page":3,"position":401}])
    );
    dispatch("go", true).unwrap();
    assert_eq!(
        state
            .events
            .audit_events()
            .iter()
            .filter(|event| event.kind == "playback_exclusion_applied")
            .count(),
        1
    );
    std::fs::remove_dir_all(path).unwrap();
}
