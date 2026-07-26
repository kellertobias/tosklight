use super::*;

fn active_osc_subscribers(state: &AppState) -> Vec<OscSubscriber> {
    let now = Instant::now();
    let (active, expired, changed) = state
        .integrations
        .active_osc_subscribers(now, Duration::from_secs(20));
    for (session_id, source) in expired {
        state
            .integrations
            .remove_suppression_source(session_id, source);
        disconnect_orphaned_osc_session(state, session_id);
    }
    if changed {
        emit(
            state,
            "hardware_connection_changed",
            serde_json::json!({"connected":state.integrations.hardware_connected()}),
        );
    }
    active
}

pub(super) fn send_osc_feedback(state: &AppState, _full: bool) {
    let subscribers = active_osc_subscribers(state);
    let Some(show) = state.active_show.current().clone() else {
        return;
    };
    let snapshot = state.output.snapshot();
    let runtime = state.output.playback_runtime_status();
    let speed_groups = state
        .output
        .speed_group_snapshots(application_millis(state));
    let highlight_fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    for subscriber in subscribers {
        let Ok(Some(desk)) = state
            .installation
            .control_desk_by_alias(&subscriber.desk_alias)
        else {
            continue;
        };
        let page = state.installation.desk_page(desk.id, show.id).unwrap_or(1);
        let selected = state
            .installation
            .selected_playback(desk.id, show.id)
            .ok()
            .flatten();
        send_programmer_osc_feedback(
            state,
            &subscriber,
            &desk,
            page,
            &highlight_fixtures,
            &groups,
        );
        send_playback_osc_feedback(OscPlaybackFeedback {
            state,
            subscriber: &subscriber,
            desk: &desk,
            page,
            selected_playback: selected,
            snapshot: &snapshot,
            runtime: &runtime,
            speed_groups: &speed_groups,
        });
    }
    sync_highlight_output(state);
}
