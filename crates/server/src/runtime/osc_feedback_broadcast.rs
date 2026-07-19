use super::*;

fn active_osc_subscribers(state: &AppState) -> Vec<OscSubscriber> {
    let now = Instant::now();
    let mut subscribers = state.osc_subscribers.lock();
    let before = subscribers.len();
    subscribers
        .retain(|_, subscriber| now.duration_since(subscriber.last_seen) < Duration::from_secs(20));
    if before != subscribers.len() {
        emit(
            state,
            "hardware_connection_changed",
            serde_json::json!({"connected":!subscribers.is_empty()}),
        );
    }
    subscribers.values().cloned().collect()
}

pub(super) fn send_osc_feedback(state: &AppState, _full: bool) {
    let subscribers = active_osc_subscribers(state);
    let Some(show) = state.active_show.read().clone() else {
        return;
    };
    let snapshot = state.engine.snapshot();
    let runtime = state.engine.playback_runtime_status();
    let speed_groups = {
        let now = application_millis(state);
        let controllers = state.speed_groups.lock();
        std::array::from_fn(|index| controllers[index].snapshot(now))
    };
    let highlight_fixtures = highlight_fixture_summaries(&snapshot.fixtures);
    let groups = highlight_groups(&snapshot);
    for subscriber in subscribers {
        let Ok(Some(desk)) = state
            .desk
            .lock()
            .control_desk_by_alias(&subscriber.desk_alias)
        else {
            continue;
        };
        let page = state.desk.lock().desk_page(desk.id, show.id).unwrap_or(1);
        let selected = state
            .desk
            .lock()
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
