use light_application::{ActionContext, SpeedGroupCommand, SpeedGroupOutcome, SpeedGroupResult};

use super::super::{AppState, Session};

pub(super) fn execute(
    state: &AppState,
    session: &Session,
    context: &ActionContext,
    command: SpeedGroupCommand,
) -> Result<SpeedGroupResult, String> {
    super::super::speed_group_service::execute_action(
        state,
        Some(session),
        context.clone(),
        command,
    )
    .map_err(|error| error.message)
}

pub(super) fn emit_compatibility_change(
    state: &AppState,
    command: &str,
    parsed: SpeedGroupCommand,
    result: &SpeedGroupResult,
) -> Result<(), String> {
    let addressed = super::speed_group_command::addressed(parsed);
    let controllers = state.speed_groups.lock();
    let snapshots = addressed
        .iter()
        .map(|group| controllers[group.index()].snapshot(result.applied_at_millis))
        .collect::<Vec<_>>();
    drop(controllers);
    super::super::emit(
        state,
        "speed_group_command",
        serde_json::json!({
            "command":super::speed_group_command::normalized(command)?,
            "groups":addressed.iter().map(|group| group_name(*group)).collect::<Vec<_>>(),
            "snapshots":snapshots,
        }),
    );
    Ok(())
}

pub(crate) fn execute_compatibility(
    state: &AppState,
    session: &Session,
    context: &ActionContext,
    command: &str,
) -> Result<usize, String> {
    let parsed =
        super::speed_group_command::parse(command)?.ok_or("expected a Speed Group command")?;
    let addressed = super::speed_group_command::addressed(parsed).len();
    let result = execute(state, session, context, parsed)?;
    if result.outcome == SpeedGroupOutcome::Applied && !result.replayed {
        emit_compatibility_change(state, command, parsed, &result)?;
    }
    Ok(addressed)
}

fn group_name(group: light_application::SpeedGroupId) -> String {
    char::from(b'A' + group.index() as u8).to_string()
}
