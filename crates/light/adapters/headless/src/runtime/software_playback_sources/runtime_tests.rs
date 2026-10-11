use super::*;
use light_application::{
    ActionContext, ActionSource, PlaybackAction, PlaybackAddress, PlaybackCommand,
};
use light_playback::{Cue, CueList, PlaybackControlSource, PlaybackDefinition, PlaybackPage};
use std::{collections::HashMap, path::PathBuf};

fn fixture() -> (AppState, super::super::Session, PathBuf) {
    let (state, data_dir) = super::super::tests::test_state();
    let desk = state.installation.desk().unwrap();
    let session = super::super::Session {
        id: light_core::SessionId::new(),
        token: Uuid::new_v4().to_string(),
        connected: true,
        desk,
        capability: light_core::SurfaceCapability::Programming,
    };
    state.programming.start(session.id);
    super::super::attach_session_command_context(&state, &session);
    state.sessions.insert_session(session.clone());
    let mut lists = Vec::new();
    let mut assignments = HashMap::new();
    for position in 1..=3 {
        let id = light_core::CueListId::new();
        let list: CueList = serde_json::from_value(serde_json::json!({
            "id":id,"name":format!("CueList {position}"),"priority":0,"mode":"sequence","looped":false,
            "cues":[Cue::new("1".parse().unwrap())]
        })).unwrap();
        let mut definition = PlaybackDefinition::new_cue_list(format!("Playback {position}"), id);
        definition.flash_release = light_playback::FlashReleaseMode::ReleaseAll;
        assignments.insert(position, definition);
        lists.push(list);
    }
    state
        .output
        .replace_snapshot(light_engine::EngineSnapshot {
            cue_lists: lists.into(),
            playback_pages: vec![PlaybackPage {
                number: 1,
                name: "Main".into(),
                assignments,
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    (state, session, data_dir)
}
fn address(position: u16) -> light_playback::PlaybackAddress {
    light_playback::PlaybackAddress::new(1, position).unwrap()
}
fn execute(
    state: &AppState,
    session: &super::super::Session,
    socket: &SoftwarePlaybackSocket,
    position: u16,
    action: PlaybackAction,
    source: &str,
) -> Result<light_application::PlaybackResult, ApiError> {
    super::super::playback_service::execute_for_socket(
        state,
        Some(session),
        Some(&session.desk),
        socket,
        ActionContext::operator(session.desk.id, session.id.0, ActionSource::Http),
        PlaybackCommand {
            address: PlaybackAddress::Playback(address(position)),
            action,
            control_source: PlaybackControlSource {
                surface_id: source.into(),
                control_id: 1,
            },
        },
    )
}
fn held(state: &AppState, position: u16, action: &PlaybackAction) -> bool {
    state
        .output
        .playback_runtime_status_at(address(position))
        .is_some_and(|status| match action {
            PlaybackAction::Flash { .. } => status.playback.flash,
            PlaybackAction::Temp { .. } => status.temporary_active,
            PlaybackAction::Swap { .. } => status.swap_active,
            _ => panic!("expected held action"),
        })
}

#[test]
fn socket_close_releases_actual_flash_temp_swap_only_for_its_source_and_publishes_changes() {
    for hold in [
        PlaybackAction::Flash { pressed: true },
        PlaybackAction::Temp { pressed: true },
        PlaybackAction::Swap { pressed: true },
    ] {
        let (state, session, data_dir) = fixture();
        let main = state.software_playback_sources.open(session.desk.id);
        let sibling = state.software_playback_sources.open(session.desk.id);
        execute(
            &state,
            &session,
            &main,
            3,
            PlaybackAction::On { pressed: true },
            "main-instance",
        )
        .unwrap();
        execute(
            &state,
            &session,
            &sibling,
            2,
            PlaybackAction::Flash { pressed: true },
            "screen-instance",
        )
        .unwrap();
        execute(&state, &session, &main, 1, hold.clone(), "main-instance").unwrap();
        assert!(held(&state, 1, &hold));
        let cursor = state.events.latest_sequence();
        close_socket(&state, &main).unwrap();
        assert!(!held(&state, 1, &hold));
        assert!(held(&state, 2, &PlaybackAction::Flash { pressed: true }));
        assert!(
            state
                .output
                .playback_runtime_status_at(address(3))
                .unwrap()
                .playback
                .enabled,
            "permanent On survives source close"
        );
        assert!(
            state.events.latest_sequence() > cursor,
            "held output cancellation publishes authoritative repair events"
        );
        let light_application::EventReplay::Events(events) = state
            .events
            .replay(cursor, &light_application::EventFilter::default())
        else {
            panic!("expected runtime cancellation events")
        };
        assert!(events.iter().any(|event| matches!(&event.payload,light_application::ApplicationEvent::Playback(light_application::PlaybackEvent::RuntimeChanged(change)) if change.projection.identity == light_application::PlaybackRuntimeIdentity::Playback(address(1)))));
        close_socket(&state, &sibling).unwrap();
        assert!(!held(&state, 2, &PlaybackAction::Flash { pressed: true }));
        assert!(
            state
                .output
                .playback_runtime_status_at(address(3))
                .unwrap()
                .playback
                .enabled
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
#[test]
fn reconnect_takeover_and_late_old_close_preserve_new_actual_hold() {
    let (state, session, data_dir) = fixture();
    let old = state.software_playback_sources.open(session.desk.id);
    let new = state.software_playback_sources.open(session.desk.id);
    execute(
        &state,
        &session,
        &old,
        1,
        PlaybackAction::Flash { pressed: true },
        "instance",
    )
    .unwrap();
    execute(
        &state,
        &session,
        &new,
        1,
        PlaybackAction::Flash { pressed: true },
        "instance",
    )
    .unwrap();
    close_socket(&state, &old).unwrap();
    assert!(held(&state, 1, &PlaybackAction::Flash { pressed: true }));
    assert!(
        execute(
            &state,
            &session,
            &old,
            1,
            PlaybackAction::Flash { pressed: false },
            "instance"
        )
        .is_err()
    );
    assert!(held(&state, 1, &PlaybackAction::Flash { pressed: true }));
    close_socket(&state, &new).unwrap();
    assert!(!held(&state, 1, &PlaybackAction::Flash { pressed: true }));
    let _ = std::fs::remove_dir_all(data_dir);
}
#[test]
fn stale_socket_cannot_replay_preload_capture_and_queue_retains_raw_source_and_desk() {
    let (state, session, data_dir) = fixture();
    state
        .programming
        .set_modes(session.id, Some(true), None, None, None);
    let old = state.software_playback_sources.open(session.desk.id);
    let new = state.software_playback_sources.open(session.desk.id);
    execute(
        &state,
        &session,
        &new,
        1,
        PlaybackAction::Go { pressed: true },
        "raw-instance",
    )
    .unwrap();
    let queued = state
        .programming
        .preload_playback_actions(session.id)
        .unwrap();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].address, address(1));
    assert_eq!(
        queued[0].control_source,
        PlaybackControlSource {
            surface_id: "raw-instance".into(),
            control_id: 1
        }
    );
    assert_eq!(queued[0].origin_desk_id, Some(session.desk.id));
    close_socket(&state, &new).unwrap();
    let before = serde_json::to_value(&queued).unwrap();
    assert!(
        execute(
            &state,
            &session,
            &old,
            1,
            PlaybackAction::Go { pressed: true },
            "raw-instance"
        )
        .is_err()
    );
    assert_eq!(
        serde_json::to_value(
            state
                .programming
                .preload_playback_actions(session.id)
                .unwrap()
        )
        .unwrap(),
        before
    );
    assert!(
        !state
            .output
            .playback_runtime_status_at(address(1))
            .is_some_and(|status| status.playback.enabled)
    );
    close_socket(&state, &old).unwrap();
    let _ = std::fs::remove_dir_all(data_dir);
}
