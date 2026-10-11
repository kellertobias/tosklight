use super::*;
use light_playback::{Cue, CueNumber, PlaybackDefinition, PlaybackPage};

fn setup() -> (AppState, std::path::PathBuf, ControlDesk) {
    let (state, path) = crate::runtime::tests::test_state();
    let ids = [light_core::CueListId::new(), light_core::CueListId::new()];
    let cue_lists = ids
        .iter()
        .map(|id| {
            serde_json::from_value::<light_playback::CueList>(serde_json::json!({
                "id":id,"name":"OSC holds","priority":0,"mode":"sequence","looped":true,
                "cues":[Cue::new(CueNumber::from(1_u8))]
            }))
            .unwrap()
        })
        .collect::<Vec<_>>();
    let pages = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let mut definition = PlaybackDefinition::new_cue_list("Held", *id);
            definition.auto_off = false;
            definition.buttons[0] = light_playback::PlaybackButtonAction::Flash;
            PlaybackPage {
                number: index as u8 + 1,
                name: "Page".into(),
                assignments: [(1, definition)].into(),
            }
        })
        .collect::<Vec<_>>();
    state
        .output
        .replace_snapshot(EngineSnapshot {
            cue_lists: cue_lists.into(),
            playback_pages: pages.into(),
            ..Default::default()
        })
        .unwrap();
    let desk = state.installation.desk().unwrap();
    (state, path, desk)
}
fn flash(state: &AppState, page: u8) -> bool {
    state
        .output
        .engine()
        .playback_runtime_status_at(light_playback::PlaybackAddress::new(page, 1).unwrap())
        .is_some_and(|status| status.playback.flash)
}
#[tokio::test]
async fn compact_osc_hold_release_retains_page_and_explicit_source_is_independent() {
    let (state, path, desk) = setup();
    assert!(handle_playback_osc(
        &state,
        "/light/main/playback/1/button/1",
        &[OscArgument::Int(1)],
        Some("127.0.0.1:19100")
    ));
    assert!(flash(&state, 1));
    state
        .installation
        .set_playback_surface_page(desk.id, "main", 2)
        .unwrap();
    assert!(handle_playback_osc(
        &state,
        "/light/main/playback/2/1/button/1",
        &[OscArgument::Int(1)],
        Some("127.0.0.1:19101")
    ));
    assert!(flash(&state, 2));
    assert!(handle_playback_osc(
        &state,
        "/light/main/playback/1/button/1",
        &[OscArgument::Int(0)],
        Some("127.0.0.1:19100")
    ));
    assert!(!flash(&state, 1));
    assert!(flash(&state, 2));
    assert!(handle_playback_osc(
        &state,
        "/light/main/playback/2/1/button/1",
        &[OscArgument::Int(0)],
        Some("127.0.0.1:19101")
    ));
    assert!(!flash(&state, 2));
    std::fs::remove_dir_all(path).unwrap();
}

#[tokio::test]
async fn osc_hold_original_action_survives_definition_edit_and_duplicate_press() {
    let (state, path, _desk) = setup();
    let route = "/light/main/playback/1/1/button/1";
    let source = Some("127.0.0.1:19100");
    assert!(handle_playback_osc(
        &state,
        route,
        &[OscArgument::Int(1), OscArgument::Int(20)],
        source
    ));
    let mut snapshot = (*state.output.snapshot()).clone();
    std::sync::Arc::make_mut(&mut snapshot.playback_pages)[0]
        .assignments
        .get_mut(&1)
        .unwrap()
        .buttons[0] = light_playback::PlaybackButtonAction::Go;
    let prepared = state.output.prepare_snapshot(snapshot).unwrap();
    state.output.install_prepared_snapshot(prepared);
    assert!(handle_playback_osc(
        &state,
        "/light/main/playback/2/1/button/1",
        &[OscArgument::Int(1), OscArgument::Int(20)],
        source
    ));
    assert!(
        !flash(&state, 2),
        "same physical control cannot acquire another hold before release"
    );
    assert!(handle_playback_osc(
        &state,
        "/light/main/playback/2/1/button/1",
        &[OscArgument::Int(0), OscArgument::Int(20)],
        source
    ));
    assert!(
        !flash(&state, 1),
        "release uses captured Flash despite changed configured Go"
    );
    assert!(
        !state
            .output
            .engine()
            .permanent_playback_enabled(light_playback::PlaybackAddress::new(2, 1).unwrap())
    );
    std::fs::remove_dir_all(path).unwrap();
}

#[tokio::test]
async fn osc_cancel_surface_clears_receipts_without_releasing_other_surface() {
    let (state, path, desk) = setup();
    let route = "/light/main/playback/1/1/flash";
    for source in ["127.0.0.1:19100", "127.0.0.1:19101"] {
        assert!(handle_playback_osc(
            &state,
            route,
            &[OscArgument::Int(1)],
            Some(source)
        ));
    }
    crate::runtime::playback_control_disconnect::cancel_for_desk(
        &state,
        desk.id,
        "osc:127.0.0.1:19100",
    )
    .unwrap();
    assert!(flash(&state, 1), "other surface hold remains authoritative");
    assert!(!handle_playback_osc(
        &state,
        route,
        &[OscArgument::Int(0)],
        Some("127.0.0.1:19100")
    ));
    assert!(flash(&state, 1));
    assert!(handle_playback_osc(
        &state,
        route,
        &[OscArgument::Int(0)],
        Some("127.0.0.1:19101")
    ));
    assert!(!flash(&state, 1));
    std::fs::remove_dir_all(path).unwrap();
}

#[tokio::test]
async fn accepted_same_id_cold_install_invalidates_previous_osc_release() {
    let (state, path, desk) = setup();
    let route = "/light/main/playback/1/1/flash";
    let source = Some("127.0.0.1:19100");
    assert!(handle_playback_osc(
        &state,
        route,
        &[OscArgument::Int(1)],
        source
    ));
    let snapshot = (*state.output.snapshot()).clone();
    let prepared = state.output.prepare_snapshot(snapshot).unwrap();
    let old_epoch = state.integrations.osc_playback_epoch();
    let prepared = state
        .output
        .prepare_destination_activation(
            prepared,
            crate::runtime::dynamic_source_origins::DynamicRuntimeSourceCheckpoint {
                runtime: Default::default(),
                origins: None,
            },
            &[],
            None,
        )
        .unwrap();
    state
        .output
        .install_destination_activation(&state.playback.render_capability(), prepared)
        .unwrap();
    // The show-activation commit tail increments this after the accepted cold pair install.
    state.integrations.replace_osc_playback_runtime();
    assert!(!flash(&state, 1));
    // A fresh runtime can be held by the same source, independently of the old ingress receipt.
    let control = light_playback::PlaybackControlSource {
        surface_id: "osc:127.0.0.1:19100".into(),
        control_id: 24,
    }
    .scoped_to_desk(desk.id)
    .unwrap();
    state
        .output
        .engine()
        .execute_playback(EnginePlaybackCommand::Playback {
            address: light_playback::PlaybackAddress::new(1, 1).unwrap(),
            action: light_engine::PlaybackAction::SetFlash(true),
            activation_origin: None,
            control_source: control,
        })
        .unwrap();
    let mut release = PoolPlaybackInput {
        pressed: Some(false),
        ..Default::default()
    };
    assert!(
        matches!(
            dispatch_osc_input(OscDispatch {
                state: &state,
                session: None,
                desk: Some(&desk),
                address: PlaybackAddress::Playback(
                    light_playback::PlaybackAddress::new(1, 1).unwrap()
                ),
                action: "flash",
                input: &mut release,
                control_source: light_playback::PlaybackControlSource {
                    surface_id: "osc:127.0.0.1:19100".into(),
                    control_id: 24
                },
                suppression_input: None,
                record_target: false,
                may_program: false,
                captured_release: true,
                hold_epoch: old_epoch,
            }),
            OscDispatchResult::Rejected
        ),
        "cold install between capture and dispatch rejects old epoch"
    );
    assert!(!handle_playback_osc(
        &state,
        route,
        &[OscArgument::Int(0)],
        source
    ));
    assert!(
        flash(&state, 1),
        "old release must never touch new same-ID runtime"
    );
    std::fs::remove_dir_all(path).unwrap();
}

#[tokio::test]
async fn osc_subscription_replacement_and_unsubscribe_cancel_captured_controls() {
    let (state, path, _desk) = setup();
    let subscribe = |client: &str, source: &str| {
        crate::runtime::handle_subscription_osc(
            &state,
            "/light/subscribe",
            &[
                OscArgument::String(client.into()),
                OscArgument::String("main".into()),
                OscArgument::Int(19150),
            ],
            Some(source),
        )
    };
    let route = "/light/main/playback/1/button/1";
    assert!(subscribe("held-client", "127.0.0.1:19100"));
    assert!(handle_playback_osc(
        &state,
        route,
        &[OscArgument::Int(1)],
        Some("127.0.0.1:19100")
    ));
    assert!(flash(&state, 1));
    assert!(subscribe("held-client", "127.0.0.1:19101"));
    assert!(!flash(&state, 1), "replacement cancels old socket hold");
    assert!(!handle_playback_osc(
        &state,
        route,
        &[OscArgument::Int(0)],
        Some("127.0.0.1:19101")
    ));
    assert!(handle_playback_osc(
        &state,
        route,
        &[OscArgument::Int(1)],
        Some("127.0.0.1:19101")
    ));
    assert!(flash(&state, 1));
    assert!(crate::runtime::handle_subscription_osc(
        &state,
        "/light/unsubscribe",
        &[OscArgument::String("held-client".into())],
        Some("127.0.0.1:19101")
    ));
    assert!(!flash(&state, 1));
    std::fs::remove_dir_all(path).unwrap();
}

#[tokio::test]
async fn direct_osc_flash_and_swap_without_control_ids_are_independent() {
    let (state, path, _desk) = setup();
    let source = Some("127.0.0.1:19100");
    for control in ["flash", "swap"] {
        assert!(handle_playback_osc(
            &state,
            &format!("/light/main/playback/1/1/{control}"),
            &[OscArgument::Int(1)],
            source
        ));
    }
    let address = light_playback::PlaybackAddress::new(1, 1).unwrap();
    assert!(
        state
            .output
            .engine()
            .playback_runtime_status_at(address)
            .unwrap()
            .swap_active
    );
    assert!(handle_playback_osc(
        &state,
        "/light/main/playback/1/1/flash",
        &[OscArgument::Int(0)],
        source
    ));
    assert!(
        state
            .output
            .engine()
            .playback_runtime_status_at(address)
            .unwrap()
            .swap_active,
        "releasing Flash cannot release independent Swap"
    );
    assert!(handle_playback_osc(
        &state,
        "/light/main/playback/1/1/swap",
        &[OscArgument::Int(0)],
        source
    ));
    assert!(
        !state
            .output
            .engine()
            .playback_runtime_status_at(address)
            .unwrap()
            .swap_active
    );
    std::fs::remove_dir_all(path).unwrap();
}

#[tokio::test]
async fn same_osc_source_explicit_pages_without_ids_hold_independently() {
    let (state, path, _desk) = setup();
    let source = Some("127.0.0.1:19100");
    for page in [1, 2] {
        assert!(handle_playback_osc(
            &state,
            &format!("/light/main/playback/{page}/1/flash"),
            &[OscArgument::Int(1)],
            source
        ));
    }
    assert!(flash(&state, 1));
    assert!(flash(&state, 2));
    assert!(handle_playback_osc(
        &state,
        "/light/main/playback/1/1/flash",
        &[OscArgument::Int(0)],
        source
    ));
    assert!(!flash(&state, 1));
    assert!(flash(&state, 2));
    assert!(handle_playback_osc(
        &state,
        "/light/main/playback/2/1/flash",
        &[OscArgument::Int(0)],
        source
    ));
    assert!(!flash(&state, 2));
    std::fs::remove_dir_all(path).unwrap();
}
