use super::*;

fn send_command_key_feedback(state: &AppState, subscriber: &OscSubscriber, command_line: &str) {
    for key in [
        "group", "at", "thru", "plus", "minus", "time", "delay", "cue", "record", "clear", "enter",
        "preload",
    ] {
        let token = match key {
            "group" => "GROUP".to_owned(),
            "thru" => "THRU".to_owned(),
            "plus" => "+".to_owned(),
            "minus" => "-".to_owned(),
            "record" => "RECORD".to_owned(),
            other => other.to_ascii_uppercase(),
        };
        send_osc(
            state,
            subscriber.target,
            format!("/light/{}/feedback/programmer/{key}", subscriber.desk_alias),
            vec![OscArgument::Bool(
                command_line.split_whitespace().any(|part| part == token),
            )],
        );
    }
}

fn highlight_arguments(highlight: &HighlightState) -> [(&'static str, Vec<OscArgument>); 9] {
    [
        ("active", vec![OscArgument::Bool(highlight.active)]),
        ("output", vec![OscArgument::Bool(highlight.output_enabled)]),
        (
            "mode",
            vec![OscArgument::String(
                match highlight.mode {
                    HighlightMode::Selection => "selection",
                    HighlightMode::Step => "step",
                }
                .into(),
            )],
        ),
        (
            "index",
            vec![OscArgument::Int(
                highlight
                    .active_index
                    .map(|index| index.saturating_add(1) as i32)
                    .unwrap_or(0),
            )],
        ),
        (
            "total",
            vec![OscArgument::Int(
                highlight.remembered.len().min(i32::MAX as usize) as i32,
            )],
        ),
        ("can-next", vec![OscArgument::Bool(highlight.can_next)]),
        (
            "can-previous",
            vec![OscArgument::Bool(highlight.can_previous)],
        ),
        (
            "fixture/id",
            vec![OscArgument::String(
                highlight
                    .active_fixture
                    .as_ref()
                    .map(|fixture| fixture.fixture_id.0.to_string())
                    .unwrap_or_default(),
            )],
        ),
        (
            "fixture/number",
            vec![OscArgument::Int(
                highlight
                    .active_fixture
                    .as_ref()
                    .and_then(|fixture| fixture.number)
                    .and_then(|number| i32::try_from(number).ok())
                    .unwrap_or(0),
            )],
        ),
    ]
}

fn send_highlight_feedback(
    state: &AppState,
    subscriber: &OscSubscriber,
    desk: &ControlDesk,
    programmer: Option<&light_programmer::ProgrammerState>,
    fixtures: &[HighlightFixture],
    groups: &HashMap<String, light_programmer::GroupDefinition>,
) {
    let Some(session) = state.sessions.read().get(&subscriber.session_id).cloned() else {
        return;
    };
    let Some(selection) = state.programmers.selection(subscriber.session_id) else {
        return;
    };
    let highlight = state.highlight.status(
        desk.id,
        session.user.id,
        Some(&session.user.name),
        &selection,
        fixtures,
        groups,
        programmer.is_some_and(|programmer| programmer.blind || programmer.preview),
    );
    let prefix = format!("/light/{}/feedback/highlight", subscriber.desk_alias);
    for (suffix, arguments) in highlight_arguments(&highlight.state) {
        send_osc(
            state,
            subscriber.target,
            format!("{prefix}/{suffix}"),
            arguments,
        );
    }
    send_osc(
        state,
        subscriber.target,
        format!("{prefix}/fixture/name"),
        vec![OscArgument::String(
            highlight
                .state
                .active_fixture
                .as_ref()
                .and_then(|fixture| fixture.name.clone())
                .unwrap_or_default(),
        )],
    );
}

pub(super) fn send_programmer_osc_feedback(
    state: &AppState,
    subscriber: &OscSubscriber,
    desk: &ControlDesk,
    page: u8,
    highlight_fixtures: &[HighlightFixture],
    highlight_groups: &HashMap<String, light_programmer::GroupDefinition>,
) {
    let prefix = format!("/light/{}/feedback", subscriber.desk_alias);
    send_osc(
        state,
        subscriber.target,
        format!("{prefix}/locked"),
        vec![OscArgument::Bool(read_desk_lock(state, desk.id).locked)],
    );
    send_osc(
        state,
        subscriber.target,
        format!("{prefix}/page"),
        vec![OscArgument::Int(i32::from(page))],
    );
    let programmer = state.programmers.get(subscriber.session_id);
    let command_line = programmer
        .as_ref()
        .map(|programmer| programmer.command_line.clone())
        .unwrap_or_default();
    send_osc(
        state,
        subscriber.target,
        format!("{prefix}/command-line"),
        vec![OscArgument::String(command_line.clone())],
    );
    send_osc(
        state,
        subscriber.target,
        format!("{prefix}/update/armed"),
        vec![OscArgument::Bool(command_line_arms_update(&command_line))],
    );
    send_command_key_feedback(state, subscriber, &command_line);
    send_highlight_feedback(
        state,
        subscriber,
        desk,
        programmer.as_ref(),
        highlight_fixtures,
        highlight_groups,
    );
}
