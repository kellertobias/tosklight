use super::*;
use light_playback::{
    PlaybackAddress, PlaybackControlSource, TimecodeId, TimecodeTransportAction,
    TimecodeTransportState,
};

struct Rig {
    state: AppState,
    session: Session,
    data_dir: std::path::PathBuf,
    incoming: PlaybackAddress,
    peer: TimecodeId,
    unrelated: TimecodeId,
    list_id: light_core::CueListId,
    cue_id: Uuid,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

impl Rig {
    fn queue(&self, action: light_programmer::PreloadPlaybackQueueAction) {
        assert!(self.state.programming.arm_preload(self.session.id, true));
        assert!(
            self.state
                .programming
                .queue_preload_playback_action_with_origin(
                    self.session.id,
                    self.incoming,
                    action,
                    PlaybackControlSource {
                        surface_id: "main".into(),
                        control_id: 101
                    },
                    None,
                )
        );
    }

    fn command(&self, action: light_engine::PlaybackAction) {
        crate::runtime::playback_solo::execute_engine(
            &self.state,
            EnginePlaybackCommand::Playback {
                address: self.incoming,
                action,
                activation_origin: None,
                control_source: PlaybackControlSource {
                    surface_id: "main".into(),
                    control_id: 101,
                },
            },
        )
        .unwrap();
    }
}

fn rig() -> Rig {
    let (state, data_dir) = test_state();
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "preload-solo".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    let peer = TimecodeId(Uuid::new_v4());
    let unrelated = TimecodeId(Uuid::new_v4());
    for (id, number) in [(peer, 1), (unrelated, 2)] {
        state
            .timecodes
            .install(
                light_playback::TimecodeDefinition {
                    id,
                    number,
                    name: format!("Timecode {number}"),
                    duration: Some(light_playback::TimecodeFrame(440)),
                    transport_offset: light_playback::TimecodeFrame::ZERO,
                    auto_start: false,
                    audio: None,
                    markers: Vec::new(),
                    lanes: Vec::new(),
                },
                None,
            )
            .unwrap();
    }
    let list_id = light_core::CueListId::new();
    let cue = light_playback::Cue::new(light_playback::CueNumber::from(1_u8));
    let cue_id = cue.id;
    let list = serde_json::from_value(serde_json::json!({
        "id":list_id,"name":"Queued Cue","pool_number":1,"priority":0,
        "mode":"sequence","looped":false,"cues":[cue]
    }))
    .unwrap();
    let cue_definition = serde_json::from_value(serde_json::json!({
        "name":"Queued member","target":{"type":"cue_list","cue_list_id":list_id},"auto_off":false
    }))
    .unwrap();
    let timecode_definition = |id| {
        serde_json::from_value(serde_json::json!({
            "name":"Timecode member","target":{"type":"timecode","timecode_id":id},"has_fader":false
        }))
        .unwrap()
    };
    let incoming = PlaybackAddress::new(2, 101).unwrap();
    let peer_address = PlaybackAddress::new(2, 103).unwrap();
    state
        .output
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![list].into(),
            playback_pages: vec![light_playback::PlaybackPage {
                number: 2,
                name: "Solo preload".into(),
                assignments: HashMap::from([
                    (101, cue_definition),
                    (103, timecode_definition(peer)),
                    (104, timecode_definition(unrelated)),
                ]),
            }]
            .into(),
            playback_solo_regions: vec![light_playback::PlaybackSoloRegion {
                id: Uuid::new_v4(),
                name: "Exact members".into(),
                members: vec![incoming, peer_address],
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    state
        .installation
        .update_configuration(|configuration| configuration.programmer_fade_millis = 0);
    let show = state
        .installation
        .upsert_show(
            "Preload Solo",
            &data_dir
                .join("shows/preload-solo.show")
                .display()
                .to_string(),
            false,
        )
        .unwrap();
    std::fs::create_dir_all(data_dir.join("shows")).unwrap();
    let (store, _) = ShowStore::create(&show.path, "Preload Solo").unwrap();
    for (id, number) in [(peer, 1), (unrelated, 2)] {
        let definition = light_playback::TimecodeDefinition {
            id, number, name: format!("Timecode {number}"), duration: Some(light_playback::TimecodeFrame(440)),
            transport_offset: light_playback::TimecodeFrame::ZERO, auto_start: false,
            audio: None, markers: Vec::new(), lanes: Vec::new(),
        };
        store.put_object("timecode", &id.0.to_string(), &serde_json::to_value(definition).unwrap(), 0).unwrap();
    }
    state.active_show.replace_current(Some(show));
    state
        .timecodes
        .handle(peer, TimecodeTransportAction::Go)
        .unwrap();
    state
        .timecodes
        .handle(unrelated, TimecodeTransportAction::Go)
        .unwrap();
    Rig {
        state,
        session,
        data_dir,
        incoming,
        peer,
        unrelated,
        list_id,
        cue_id,
    }
}

#[tokio::test]
async fn preload_permanent_cue_go_stops_only_running_timecode_peers_in_exact_solo_region() {
    let rig = rig();
    rig.queue(light_programmer::PreloadPlaybackQueueAction::Go);
    assert_eq!(
        rig.state.timecodes.snapshot(rig.peer).unwrap().transport,
        TimecodeTransportState::Playing
    );
    assert!(
        !rig.state
            .output
            .engine()
            .permanent_playback_enabled(rig.incoming)
    );
    commit_preload(&rig.state, &rig.session).unwrap();
    assert!(
        rig.state
            .programming
            .preload_playback_actions(rig.session.id)
            .unwrap()
            .is_empty()
    );
    let runtime = rig
        .state
        .output
        .playback_runtime_status_for_cue_list(rig.list_id)
        .unwrap();
    assert!(runtime.playback.enabled);
    assert_eq!(runtime.playback.current_cue_id, Some(rig.cue_id));
    assert_eq!(
        rig.state.timecodes.snapshot(rig.peer).unwrap().transport,
        TimecodeTransportState::Stopped,
        "accepted Preload GO is an actual permanent member start and must stop its running Timecode peer"
    );
    assert_eq!(
        rig.state
            .timecodes
            .snapshot(rig.unrelated)
            .unwrap()
            .transport,
        TimecodeTransportState::Playing,
        "a Timecode outside the exact Solo Region must keep running"
    );
}

#[tokio::test]
async fn preload_solo_go_then_off_still_releases_timecode_after_actual_ordered_start() {
    let rig = rig();
    rig.queue(light_programmer::PreloadPlaybackQueueAction::Go);
    rig.queue(light_programmer::PreloadPlaybackQueueAction::Off);
    commit_preload(&rig.state, &rig.session).unwrap();
    assert!(
        !rig.state
            .output
            .engine()
            .permanent_playback_enabled(rig.incoming)
    );
    assert_eq!(
        rig.state.timecodes.snapshot(rig.peer).unwrap().transport,
        TimecodeTransportState::Stopped
    );
    assert_eq!(
        rig.state
            .timecodes
            .snapshot(rig.unrelated)
            .unwrap()
            .transport,
        TimecodeTransportState::Playing
    );
}

#[tokio::test]
async fn preload_solo_failed_batch_keeps_timecode_running_and_queue_unchanged() {
    let rig = rig();
    rig.queue(light_programmer::PreloadPlaybackQueueAction::Go);
    assert!(
        rig.state
            .programming
            .queue_preload_playback_action_with_origin(
                rig.session.id,
                PlaybackAddress::new(2, 999).unwrap(),
                light_programmer::PreloadPlaybackQueueAction::Go,
                PlaybackControlSource {
                    surface_id: "main".into(),
                    control_id: 999
                },
                None,
            )
    );
    let pending = rig
        .state
        .programming
        .preload_playback_actions(rig.session.id)
        .unwrap();
    let before = rig.state.timecodes.snapshot(rig.peer).unwrap();
    let error = commit_preload(&rig.state, &rig.session).unwrap_err();
    assert!(error.contains("2.999"), "{error}");
    assert_eq!(
        rig.state
            .programming
            .preload_playback_actions(rig.session.id)
            .unwrap(),
        pending
    );
    assert!(
        !rig.state
            .output
            .engine()
            .permanent_playback_enabled(rig.incoming)
    );
    let after = rig.state.timecodes.snapshot(rig.peer).unwrap();
    assert_eq!(after.transport, TimecodeTransportState::Playing);
    assert_eq!(after.revision, before.revision);
}

#[tokio::test]
async fn preload_solo_already_on_and_flash_do_not_stop_timecode() {
    let rig = rig();
    rig.command(light_engine::PlaybackAction::SetFlash(true));
    assert!(
        !rig.state
            .output
            .engine()
            .permanent_playback_enabled(rig.incoming)
    );
    assert!(rig.state.programming.arm_preload(rig.session.id, true));
    commit_preload(&rig.state, &rig.session).unwrap();
    rig.command(light_engine::PlaybackAction::SetFlash(false));
    assert_eq!(
        rig.state.timecodes.snapshot(rig.peer).unwrap().transport,
        TimecodeTransportState::Playing
    );
    rig.command(light_engine::PlaybackAction::On);
    // An editor/direct transport start has no Playback Solo membership of its own.
    rig.state
        .timecodes
        .handle(rig.peer, TimecodeTransportAction::Go)
        .unwrap();
    assert!(
        rig.state
            .output
            .engine()
            .permanent_playback_enabled(rig.incoming)
    );
    rig.queue(light_programmer::PreloadPlaybackQueueAction::On);
    commit_preload(&rig.state, &rig.session).unwrap();
    assert!(
        rig.state
            .output
            .engine()
            .permanent_playback_enabled(rig.incoming)
    );
    assert_eq!(
        rig.state.timecodes.snapshot(rig.peer).unwrap().transport,
        TimecodeTransportState::Playing
    );
}
