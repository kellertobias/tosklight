//! Native output oracles for the operator's edited Timecode schedule.
use super::*;
use light_playback::{
    TimecodeClipEnd, TimecodeFrame, TimecodeLaneContent, TimecodeTransportAction,
};

async fn render_levels(state: &AppState) -> [u8; 2] {
    // This isolated harness has no network sender, but the scheduler captures real DMX first.
    let error = output_scheduler::render_test_tick(state.clone())
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "network output is unavailable");
    let captured = state.output.dmx_snapshot().0;
    captured.universes[0].slots[..2].try_into().unwrap()
}

#[track_caller]
fn assert_levels(actual: [u8; 2], expected: [u8; 2]) {
    for (got, want) in actual.into_iter().zip(expected) {
        assert!(
            got.abs_diff(want) <= 1,
            "actual {actual:?}, expected {expected:?}"
        );
    }
}

fn install_fixtures(
    state: &AppState,
    fixtures: [light_core::FixtureId; 2],
    list: &light_playback::CueList,
) {
    let mut second = operational_fixture(fixtures[1]);
    second.address = Some(2);
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![operational_fixture(fixtures[0]), second].into(),
            cue_lists: vec![list.clone()].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
}

async fn edit_request(
    state: &AppState,
    app: &Router,
    token: &str,
    show: &str,
    path: &str,
    body: serde_json::Value,
) -> serde_json::Value {
    let active = state.active_show.current().unwrap();
    let revision = ShowStore::open(&active.path)
        .unwrap()
        .portable_document()
        .unwrap()
        .revision()
        .value();
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header(header::IF_MATCH, revision.to_string())
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-tosk-show", show)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let outcome = json(response).await;
    assert_eq!(status, StatusCode::OK, "{path}: {outcome}");
    outcome
}

#[tokio::test]
async fn persisted_trigger_and_independent_fade_edits_drive_actual_cues_and_dmx() {
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let (state, data_dir) = test_state_with_clock(clock.clone());
    let app = router(state.clone());
    let (token, _) = login(&app, "Timecode edit proof").await;
    let show = create_show(&app, &token, "Edited timeline output").await;
    let show_id = show["id"].as_str().unwrap();
    assert_eq!(
        app.clone()
            .oneshot(open_show_request(&token, show_id))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let fixtures = [light_core::FixtureId::new(), light_core::FixtureId::new()];
    let mut list = asymmetric_timeline_cues(fixtures);
    install_fixtures(&state, fixtures, &list);
    let list_id = list.id;
    let save = |request_id: &str, expected_revision: u64, list: &light_playback::CueList| {
        serde_json::json!({
            "request_id": request_id,
            "action": {"type":"save_cue_list", "cue_list_id":list_id,
                "expected_revision":expected_revision,
                "expected_object_id":if expected_revision == 0 { None } else { Some(list_id.0.to_string()) },
                "body":list}
        })
    };
    edit_request(
        &state,
        &app,
        &token,
        show_id,
        "/api/v2/playback-topology/actions",
        save("initial-cues", 0, &list),
    )
    .await;
    // Deliberately save different initial timing so playback proves the accepted edit, not a seed.
    list.cues[1].delay_millis = 250;
    list.cues[1].fade_millis = 1_000;
    list.cues[0].out_delay_millis = Some(0);
    list.cues[0].out_fade_millis = Some(500);
    edit_request(
        &state,
        &app,
        &token,
        show_id,
        "/api/v2/playback-topology/actions",
        save("edited-cues", 1, &list),
    )
    .await;
    let id = light_playback::TimecodeId(Uuid::new_v4());
    let mut lanes = serde_json::json!([{"id":Uuid::new_v4(),"name":"Edited Cues","content":{
    "kind":"cue_list", "cue_list_id":list_id,"clips":[{
        "id":Uuid::new_v4(),"start_frame":0,"end_frame":264,
        "start_cue_id":list.cues[0].id,"end_cue_id":list.cues[1].id,
        "start_behavior":"state","end_behavior":"hold",
        "cue_starts":[{"cue_id":list.cues[1].id,"offset_frame":44}],
        "in_fade_frames":0,"out_fade_frames":0
    }]}}]);
    edit_request(
        &state,
        &app,
        &token,
        show_id,
        "/api/v2/timecodes/actions",
        serde_json::json!({
            "request_id":"initial-timecode", "action":{"type":"create","definition":{
                "id":id,"number":1,"name":"Edited Cues","duration_frame":440,
                "transport_offset_frame":0,"auto_start":false,"markers":[],"lanes":lanes
            }}
        }),
    )
    .await;
    lanes[0]["content"]["clips"][0]["cue_starts"][0]["offset_frame"] = serde_json::json!(66);
    edit_request(
        &state,
        &app,
        &token,
        show_id,
        "/api/v2/timecodes/actions",
        serde_json::json!({
            "request_id":"move-manual-trigger", "action":{"type":"update","timecode_id":id,
                "expected_revision":1,"patch":{"lanes":lanes}}
        }),
    )
    .await;
    let active = state.active_show.current().unwrap();
    let repository = ActiveShowRepository::open(&active.path).unwrap();
    let stored_list = repository
        .object_with_portable_revision("cue_list", &list_id.0.to_string())
        .unwrap()
        .1
        .unwrap();
    let stored_list: light_playback::CueList = serde_json::from_value(stored_list.body).unwrap();
    assert_eq!(stored_list.cues, list.cues);
    let stored_timeline = repository
        .object_with_portable_revision("timecode", &id.0.to_string())
        .unwrap()
        .1
        .unwrap();
    let stored_timeline: light_playback::TimecodeDefinition =
        serde_json::from_value(stored_timeline.body).unwrap();
    let TimecodeLaneContent::CueList { clips, .. } = &stored_timeline.lanes[0].content else {
        panic!("Cuelist lane");
    };
    assert_eq!(clips[0].cue_starts[0].offset_frame, TimecodeFrame(66));
    assert_eq!(
        stored_list.cues[1].trigger,
        light_playback::CueTrigger::Manual
    );
    // Reinstall the persisted definitions to rule out a UI/runtime-only optimistic value.
    install_fixtures(&state, fixtures, &stored_list);
    state.timecodes.install(stored_timeline, None).unwrap();
    state
        .timecodes
        .handle(id, TimecodeTransportAction::Go)
        .unwrap();
    state.timecodes.reconcile_cue_lists(state.output.engine());
    clock.advance_millis(1_499);
    assert_levels(render_levels(&state).await, [51, 204]);
    assert_eq!(
        state
            .output
            .engine()
            .playback_runtime_status_for_cue_list(list_id)
            .unwrap()
            .playback
            .current_cue_id,
        Some(list.cues[0].id)
    );
    clock.advance_millis(1);
    assert_levels(render_levels(&state).await, [51, 204]);
    assert_eq!(
        state
            .output
            .engine()
            .playback_runtime_status_for_cue_list(list_id)
            .unwrap()
            .playback
            .current_cue_id,
        Some(list.cues[1].id)
    );
    // At .5s after the edited trigger: In waits .25s then rises a quarter over1s;
    // outgoing .8 -> .2 falls fully over .5s, independently of that In.
    clock.advance_millis(500);
    assert_levels(render_levels(&state).await, [89, 51]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn continuous_paused_seek_backward_seek_resume_and_loop_have_the_same_output() {
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let (state, data_dir) = test_state_with_clock(clock.clone());
    let fixtures = [light_core::FixtureId::new(), light_core::FixtureId::new()];
    let list = asymmetric_timeline_cues(fixtures);
    let definition = asymmetric_timeline_definition(&list);
    install_fixtures(&state, fixtures, &list);
    state.timecodes.install(definition.clone(), None).unwrap();
    let id = definition.id;
    state
        .timecodes
        .handle(id, TimecodeTransportAction::Go)
        .unwrap();
    state.timecodes.reconcile_cue_lists(state.output.engine());
    assert_levels(render_levels(&state).await, [51, 204]);
    clock.advance_millis(1_000);
    let _ = render_levels(&state).await;
    clock.advance_millis(750);
    let continuous = render_levels(&state).await;
    assert_levels(continuous, [70, 128]);
    state
        .timecodes
        .handle(id, TimecodeTransportAction::Pause)
        .unwrap();
    state.timecodes.reconcile_cue_lists(state.output.engine());
    let paused = render_levels(&state).await;
    let ordinal = state
        .output
        .engine()
        .playback_runtime_status_for_cue_list(list.id)
        .unwrap()
        .playback
        .transition_ordinal;
    clock.advance_millis(5_000);
    assert_eq!(render_levels(&state).await, paused);
    assert_eq!(
        state.timecodes.snapshot(id).unwrap().frame,
        TimecodeFrame(77)
    );
    assert_eq!(
        state
            .output
            .engine()
            .playback_runtime_status_for_cue_list(list.id)
            .unwrap()
            .playback
            .transition_ordinal,
        ordinal
    );
    state
        .timecodes
        .handle(
            id,
            TimecodeTransportAction::Seek {
                frame: TimecodeFrame(110),
            },
        )
        .unwrap();
    state.timecodes.reconcile_cue_lists(state.output.engine());
    assert_levels(render_levels(&state).await, [128, 51]);
    state
        .timecodes
        .handle(
            id,
            TimecodeTransportAction::Seek {
                frame: TimecodeFrame(77),
            },
        )
        .unwrap();
    state.timecodes.reconcile_cue_lists(state.output.engine());
    assert_eq!(render_levels(&state).await, continuous);
    state
        .timecodes
        .handle(id, TimecodeTransportAction::Pause)
        .unwrap();
    state.timecodes.reconcile_cue_lists(state.output.engine());
    clock.advance_millis(750);
    assert_levels(render_levels(&state).await, [128, 51]);
    state
        .timecodes
        .handle(id, TimecodeTransportAction::Rewind)
        .unwrap();
    state.timecodes.reconcile_cue_lists(state.output.engine());
    assert_levels(render_levels(&state).await, [51, 204]);
    clock.advance_millis(10_000);
    assert_levels(render_levels(&state).await, [51, 204]);
    clock.advance_millis(1_000);
    let _ = render_levels(&state).await;
    clock.advance_millis(750);
    assert_eq!(render_levels(&state).await, continuous);
    assert_eq!(
        state.timecodes.snapshot(id).unwrap().frame,
        TimecodeFrame(77)
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn single_and_multi_cue_hold_release_at_the_exact_clip_end_without_final_out_tail() {
    for single in [false, true] {
        for ending in [TimecodeClipEnd::Hold, TimecodeClipEnd::Release] {
            let clock = Arc::new(ManualClock::new(fixed_test_time()));
            let (state, data_dir) = test_state_with_clock(clock.clone());
            let fixtures = [light_core::FixtureId::new(), light_core::FixtureId::new()];
            let mut list = asymmetric_timeline_cues(fixtures);
            let mut definition = asymmetric_timeline_definition(&list);
            if single {
                list.cues.truncate(1);
            }
            list.cues.last_mut().unwrap().out_delay_millis = Some(5_000);
            list.cues.last_mut().unwrap().out_fade_millis = Some(10_000);
            let TimecodeLaneContent::CueList { clips, .. } = &mut definition.lanes[0].content
            else {
                unreachable!()
            };
            clips[0].end_cue_id = list.cues.last().unwrap().id;
            clips[0].end_behavior = ending;
            if single {
                clips[0].cue_starts.clear();
            }
            install_fixtures(&state, fixtures, &list);
            state.timecodes.install(definition.clone(), None).unwrap();
            state
                .timecodes
                .handle(definition.id, TimecodeTransportAction::Go)
                .unwrap();
            state.timecodes.reconcile_cue_lists(state.output.engine());
            let _ = render_levels(&state).await;
            clock.advance_millis(1_000);
            let _ = render_levels(&state).await;
            clock.advance_millis(4_999);
            let expected = if single { [51, 204] } else { [204, 51] };
            assert_levels(render_levels(&state).await, expected);
            assert_eq!(
                state.timecodes.snapshot(definition.id).unwrap().frame,
                TimecodeFrame(263)
            );
            clock.advance_millis(1);
            let actual = render_levels(&state).await;
            assert_eq!(
                state.timecodes.snapshot(definition.id).unwrap().frame,
                TimecodeFrame(264)
            );
            let owner = state
                .output
                .engine()
                .playback_runtime_status_for_cue_list(list.id);
            match ending {
                TimecodeClipEnd::Hold => {
                    assert_levels(actual, expected);
                    assert_eq!(
                        owner.unwrap().playback.current_cue_id,
                        Some(list.cues.last().unwrap().id)
                    );
                }
                TimecodeClipEnd::Release => {
                    assert_eq!(actual, [0, 0]);
                    assert!(owner.is_none());
                }
            }
            clock.advance_millis(1_000);
            assert_eq!(render_levels(&state).await, actual);
            let _ = std::fs::remove_dir_all(data_dir);
        }
    }
}

#[tokio::test]
async fn late_scheduler_tick_reconstructs_elapsed_cue_fades_at_its_authoritative_frame() {
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let (state, data_dir) = test_state_with_clock(clock.clone());
    let fixtures = [light_core::FixtureId::new(), light_core::FixtureId::new()];
    let list = asymmetric_timeline_cues(fixtures);
    let definition = asymmetric_timeline_definition(&list);
    install_fixtures(&state, fixtures, &list);
    state.timecodes.install(definition.clone(), None).unwrap();
    state
        .timecodes
        .handle(definition.id, TimecodeTransportAction::Go)
        .unwrap();
    state.timecodes.reconcile_cue_lists(state.output.engine());
    assert_levels(render_levels(&state).await, [51, 204]);
    clock.advance_millis(1_750);
    let actual = render_levels(&state).await;
    assert_eq!(
        state.timecodes.snapshot(definition.id).unwrap().frame,
        TimecodeFrame(77)
    );
    let status = state
        .output
        .engine()
        .playback_runtime_status_for_cue_list(list.id)
        .unwrap();
    assert_eq!(status.playback.current_cue_id, Some(list.cues[1].id));
    assert_levels(actual, [70, 128]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn late_forward_crossing_keeps_final_cue_actions_claimable_exactly_once() {
    for elapsed in [1_000, 1_750] {
        let clock = Arc::new(ManualClock::new(fixed_test_time()));
        let (state, data_dir) = test_state_with_clock(clock.clone());
        let fixtures = [light_core::FixtureId::new(), light_core::FixtureId::new()];
        let mut list = asymmetric_timeline_cues(fixtures);
        let action = light_playback::CueAction::TimecodeStop {
            timecode_id: light_playback::TimecodeId(Uuid::new_v4()),
        };
        list.cues[1].actions.push(action.clone());
        let definition = asymmetric_timeline_definition(&list);
        install_fixtures(&state, fixtures, &list);
        state.timecodes.install(definition.clone(), None).unwrap();
        state
            .timecodes
            .handle(definition.id, TimecodeTransportAction::Go)
            .unwrap();
        state.timecodes.reconcile_cue_lists(state.output.engine());
        let _ = render_levels(&state).await;
        clock.advance_millis(elapsed);
        state.timecodes.tick();
        state.timecodes.reconcile_cue_lists(state.output.engine());
        let frame = state
            .output
            .engine()
            .prepare_output_frame(RenderOptions::default());
        assert_eq!(
            output_scheduler::claim_automatic_cue_action_batches(&frame),
            vec![(list.id, vec![action])],
            "elapsed {elapsed}ms: exact and late live crossings must both dispatch actions"
        );
        assert!(output_scheduler::claim_automatic_cue_action_batches(&frame).is_empty());
        state.timecodes.reconcile_cue_lists(state.output.engine());
        let repeated = state
            .output
            .engine()
            .prepare_output_frame(RenderOptions::default());
        assert!(output_scheduler::claim_automatic_cue_action_batches(&repeated).is_empty());
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
