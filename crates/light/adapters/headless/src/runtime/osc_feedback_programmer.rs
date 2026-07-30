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
    _desk: &ControlDesk,
    programmer: Option<&light_programmer::ProgrammerState>,
    fixtures: &[HighlightFixture],
    groups: &HashMap<String, light_programmer::GroupDefinition>,
) {
    let Some(session) = state.sessions.session(subscriber.session_id) else {
        return;
    };
    let Some(selection) = state.programming.selection(subscriber.session_id) else {
        return;
    };
    let context = programming_context(&session, light_application::ActionSource::Osc, None);
    let ports = highlight_service_adapter::HeadlessHighlightPorts::with_environment(
        state,
        &session,
        light_application::HighlightEnvironment {
            user_name: Some(session.user.name.clone()),
            selection,
            fixtures: fixtures.to_vec(),
            groups: groups.clone(),
            output_suppressed: programmer
                .is_some_and(|programmer| programmer.blind || programmer.preview),
        },
    );
    let Ok(highlight) = state.highlight.snapshot(&context, &ports) else {
        return;
    };
    let prefix = format!("/light/{}/feedback/highlight", subscriber.desk_alias);
    for (suffix, arguments) in highlight_arguments(&highlight) {
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
                .active_fixture
                .as_ref()
                .and_then(|fixture| fixture.name.clone())
                .unwrap_or_default(),
        )],
    );
}

fn send_dynamic_feedback(
    state: &AppState,
    subscriber: &OscSubscriber,
    programmer: Option<&light_programmer::ProgrammerState>,
) {
    let prefix = format!("/light/{}/feedback/dynamic", subscriber.desk_alias);
    send_runtime_dynamic_feedback(state, subscriber, &prefix);
    send_programmer_dynamic_feedback(state, subscriber, &prefix, programmer);
}

fn send_runtime_dynamic_feedback(state: &AppState, subscriber: &OscSubscriber, prefix: &str) {
    let runtime = state.output.dynamic_runtime_snapshot();
    let active_instances = runtime
        .instances
        .iter()
        .filter(|instance| !instance.completed)
        .collect::<Vec<_>>();
    send_osc(
        state,
        subscriber.target,
        format!("{prefix}/global-paused"),
        vec![OscArgument::Bool(runtime.global_paused)],
    );
    send_osc(
        state,
        subscriber.target,
        format!("{prefix}/runtime-count"),
        vec![OscArgument::Int(
            active_instances.len().min(i32::MAX as usize) as i32,
        )],
    );
    for instance in active_instances {
        let winner = instance
            .controllers
            .iter()
            .max_by_key(|controller| {
                (
                    controller.priority,
                    controller.activated_at_millis,
                    controller.id,
                )
            })
            .map(|controller| controller.id);
        let transitions = instance
            .controller_transitions
            .iter()
            .map(|transition| (transition.controller_id, transition))
            .collect::<HashMap<_, _>>();
        let runtime_prefix = format!("{prefix}/runtime/{}", instance.id);
        for (suffix, arguments) in [
            ("active", vec![OscArgument::Bool(true)]),
            (
                "pool-number",
                vec![OscArgument::Int(i32::from(instance.definition.pool_number))],
            ),
            (
                "name",
                vec![OscArgument::String(instance.definition.name.clone())],
            ),
            (
                "target-count",
                vec![OscArgument::Int(
                    instance.targets.len().min(i32::MAX as usize) as i32,
                )],
            ),
            (
                "controller-count",
                vec![OscArgument::Int(
                    instance.controllers.len().min(i32::MAX as usize) as i32,
                )],
            ),
            (
                "winning-controller",
                vec![OscArgument::String(
                    winner.map(|id| id.to_string()).unwrap_or_default(),
                )],
            ),
            (
                "paused",
                vec![OscArgument::Bool(
                    runtime.global_paused || instance.paused_at_millis.is_some(),
                )],
            ),
        ] {
            send_osc(
                state,
                subscriber.target,
                format!("{runtime_prefix}/{suffix}"),
                arguments,
            );
        }
        for controller in &instance.controllers {
            let controller_prefix = format!("{prefix}/controller/{}", controller.id);
            let source = match &controller.source {
                light_dynamics::DynamicControllerSource::Programmer { programmer_id } => {
                    format!("programmer:{programmer_id}")
                }
                light_dynamics::DynamicControllerSource::Cue {
                    cue_list_id,
                    instance_link,
                } => format!("cue:{cue_list_id}:{instance_link}"),
                light_dynamics::DynamicControllerSource::Playback { playback_number } => {
                    format!("playback:{playback_number}")
                }
            };
            for (suffix, arguments) in [
                (
                    "runtime-instance",
                    vec![OscArgument::String(instance.id.to_string())],
                ),
                ("source", vec![OscArgument::String(source)]),
                (
                    "priority",
                    vec![OscArgument::Int(i32::from(controller.priority))],
                ),
                ("size", vec![OscArgument::Float(controller.size)]),
                (
                    "speed",
                    vec![OscArgument::Float(controller.speed_multiplier)],
                ),
                (
                    "phase",
                    vec![OscArgument::Float(controller.phase_offset_degrees)],
                ),
                ("paused", vec![OscArgument::Bool(controller.paused)]),
                (
                    "winning",
                    vec![OscArgument::Bool(winner == Some(controller.id))],
                ),
                (
                    "releasing",
                    vec![OscArgument::Bool(
                        transitions.get(&controller.id).is_some_and(|transition| {
                            transition.release_started_at_millis.is_some()
                        }),
                    )],
                ),
            ] {
                send_osc(
                    state,
                    subscriber.target,
                    format!("{controller_prefix}/{suffix}"),
                    arguments,
                );
            }
        }
    }
}

fn send_programmer_dynamic_feedback(
    state: &AppState,
    subscriber: &OscSubscriber,
    prefix: &str,
    programmer: Option<&light_programmer::ProgrammerState>,
) {
    let mut instances =
        HashMap::<Uuid, (u16, String, light_dynamics::DynamicInstanceOverrides, usize)>::new();
    let mut fix_at_count = 0_usize;
    if let Some(programmer) = programmer {
        for stored in programmer.dynamic_values.iter() {
            match &stored.value {
                light_dynamics::DynamicSemanticValue::DynamicOn {
                    instance_link,
                    dynamic,
                    overrides,
                    ..
                } => {
                    let entry = instances.entry(*instance_link).or_insert_with(|| {
                        (
                            dynamic.last_known_pool_number,
                            dynamic.embedded_fallback.definition.name.clone(),
                            overrides.clone(),
                            0,
                        )
                    });
                    entry.3 += 1;
                }
                light_dynamics::DynamicSemanticValue::FixAt { .. } => fix_at_count += 1,
                _ => {}
            }
        }
    }
    send_osc(
        state,
        subscriber.target,
        format!("{prefix}/count"),
        vec![OscArgument::Int(
            instances.len().min(i32::MAX as usize) as i32
        )],
    );
    send_osc(
        state,
        subscriber.target,
        format!("{prefix}/fix-at/count"),
        vec![OscArgument::Int(fix_at_count.min(i32::MAX as usize) as i32)],
    );
    let mut instances = instances.into_iter().collect::<Vec<_>>();
    instances.sort_by_key(|(id, _)| *id);
    for (instance_id, (pool_number, name, overrides, address_count)) in instances {
        let instance_prefix = format!("{prefix}/instance/{instance_id}");
        send_osc(
            state,
            subscriber.target,
            format!("{instance_prefix}/active"),
            vec![OscArgument::Bool(true)],
        );
        send_osc(
            state,
            subscriber.target,
            format!("{instance_prefix}/pool-number"),
            vec![OscArgument::Int(i32::from(pool_number))],
        );
        send_osc(
            state,
            subscriber.target,
            format!("{instance_prefix}/name"),
            vec![OscArgument::String(name)],
        );
        send_osc(
            state,
            subscriber.target,
            format!("{instance_prefix}/size"),
            vec![OscArgument::Float(overrides.size)],
        );
        send_osc(
            state,
            subscriber.target,
            format!("{instance_prefix}/speed"),
            vec![OscArgument::Float(
                overrides.speed_multiplier.factor() as f32
            )],
        );
        send_osc(
            state,
            subscriber.target,
            format!("{instance_prefix}/phase"),
            vec![OscArgument::Float(overrides.phase_offset_degrees)],
        );
        send_osc(
            state,
            subscriber.target,
            format!("{instance_prefix}/address-count"),
            vec![OscArgument::Int(address_count.min(i32::MAX as usize) as i32)],
        );
        send_osc(
            state,
            subscriber.target,
            format!("{prefix}/{pool_number}/active"),
            vec![
                OscArgument::Bool(true),
                OscArgument::String(instance_id.to_string()),
            ],
        );
    }
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
    let programmer = state.programming.get(subscriber.session_id);
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
    send_dynamic_feedback(state, subscriber, programmer.as_ref());
    send_highlight_feedback(
        state,
        subscriber,
        desk,
        programmer.as_ref(),
        highlight_fixtures,
        highlight_groups,
    );
}
