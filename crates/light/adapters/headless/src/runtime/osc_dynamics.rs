use super::dynamics_adapter::ServerDynamicsPorts;
use super::*;
use light_application::{
    ActionSource, DynamicControllerUpdate, DynamicFixAtCommand, DynamicStartCommand,
};
use light_core::AttributeKey;
use light_dynamics::{DynamicInstanceOverrides, DynamicValueTiming, Rational};
use std::net::SocketAddr;
use uuid::Uuid;

pub(super) fn handle_dynamics_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) -> bool {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    if parts.first() != Some(&"light") {
        return false;
    }
    let Some((session, desk_alias, feedback_target)) = osc_session(state, &parts, source) else {
        return false;
    };
    let operation = if parts.len() == 5 && parts[2] == "dynamic" {
        pool_operation(state, &session, parts[3], parts[4], arguments)
    } else if parts.len() == 6 && parts[2] == "dynamic" && parts[3] == "instance" {
        instance_operation(state, &session, parts[4], parts[5], arguments)
    } else if parts.len() == 4 && parts[2] == "programmer" && parts[3] == "fix-at" {
        fix_at_operation(state, &session, arguments)
    } else {
        return false;
    };

    match operation {
        Ok(changed) => {
            if changed {
                if let Err(error) = persist_programmer(state, &session) {
                    emit_rejection(
                        state,
                        &session,
                        &desk_alias,
                        feedback_target,
                        address,
                        &error.message,
                    );
                    return false;
                }
                if let Err(error) = persist_output_runtime(state) {
                    emit_rejection(
                        state,
                        &session,
                        &desk_alias,
                        feedback_target,
                        address,
                        &error.message,
                    );
                    return false;
                }
            }
            emit(
                state,
                "dynamic_action",
                serde_json::json!({
                    "desk_alias": desk_alias,
                    "session_id": session.id,
                    "address": address,
                    "changed": changed,
                    "source": "osc",
                }),
            );
            changed
        }
        Err(message) => {
            emit_rejection(
                state,
                &session,
                &desk_alias,
                feedback_target,
                address,
                &message,
            );
            false
        }
    }
}

fn osc_session(
    state: &AppState,
    parts: &[&str],
    source: Option<&str>,
) -> Option<(Session, String, SocketAddr)> {
    let desk_alias = parts.get(1)?.to_string();
    let source = source?.parse::<SocketAddr>().ok()?;
    let subscriber = state.integrations.osc_subscriber_for_source(source)?;
    if !subscriber.desk_alias.eq_ignore_ascii_case(&desk_alias) {
        return None;
    }
    let session = state.sessions.session(subscriber.session_id)?;
    attach_session_command_context(state, &session);
    Some((session, desk_alias, subscriber.target))
}

fn pool_operation(
    state: &AppState,
    session: &Session,
    pool_number: &str,
    action: &str,
    arguments: &[OscArgument],
) -> Result<bool, String> {
    if !osc_pressed(arguments) {
        return Ok(false);
    }
    let pool_number = pool_number
        .parse::<u16>()
        .ok()
        .filter(|number| (1..=9_999).contains(number))
        .ok_or_else(|| "Dynamic pool number must be from 1 through 9999".to_owned())?;
    let dynamic_id = state
        .output
        .snapshot()
        .dynamics
        .iter()
        .find(|dynamic| dynamic.pool_number == pool_number)
        .map(|dynamic| dynamic.id)
        .ok_or_else(|| format!("Dynamic {pool_number} does not exist"))?;
    let _activation = state
        .active_show
        .try_acquire()
        .map_err(|_| "the active show is changing; retry the Dynamic action".to_owned())?;
    let context = programming_context(session, ActionSource::Osc, None);
    let ports = ServerDynamicsPorts { state, session };
    let command = DynamicStartCommand {
        dynamic_id,
        targets: Vec::new(),
        overrides: DynamicInstanceOverrides {
            size: 1.0,
            speed_multiplier: Rational::ONE,
            phase_offset_degrees: 0.0,
        },
        timing: DynamicValueTiming::default(),
        undo_group: None,
    };
    match action {
        "toggle" => state
            .dynamics
            .toggle(&context, command, &ports)
            .map(|_| true)
            .map_err(|error| error.message),
        "off" => state
            .dynamics
            .off_matching(&context, command, &ports)
            .map(|outcome| outcome.is_some())
            .map_err(|error| error.message),
        _ => Err("unsupported Dynamic pool action".to_owned()),
    }
}

fn instance_operation(
    state: &AppState,
    session: &Session,
    runtime_instance_id: &str,
    field: &str,
    arguments: &[OscArgument],
) -> Result<bool, String> {
    let runtime_instance_id = Uuid::parse_str(runtime_instance_id)
        .map_err(|_| "invalid Dynamic instance UUID".to_owned())?;
    let controller_id = super::dynamics_adapter::controller_for_runtime_instance(
        state,
        session,
        runtime_instance_id,
    )?;
    let value =
        numeric_argument(arguments).ok_or_else(|| "Dynamic value must be numeric".to_owned())?;
    let (size, speed_multiplier, phase_offset_degrees) = match field {
        "size" => (Some(value), None, None),
        "speed" => (None, Some(value), None),
        "phase" => (None, None, Some(value)),
        _ => return Err("unsupported Dynamic instance action".to_owned()),
    };
    let _activation = state
        .active_show
        .try_acquire()
        .map_err(|_| "the active show is changing; retry the Dynamic action".to_owned())?;
    let context = programming_context(session, ActionSource::Osc, None);
    let ports = ServerDynamicsPorts { state, session };
    state
        .dynamics
        .update_controller(
            &context,
            DynamicControllerUpdate {
                controller_id,
                size,
                speed_multiplier,
                phase_offset_degrees,
                undo_group: None,
            },
            &ports,
        )
        .map(|_| true)
        .map_err(|error| error.message)
}

fn fix_at_operation(
    state: &AppState,
    session: &Session,
    arguments: &[OscArgument],
) -> Result<bool, String> {
    let attribute = match arguments.first() {
        Some(OscArgument::String(value)) if !value.trim().is_empty() => {
            AttributeKey(value.trim().into())
        }
        _ => return Err("FixAT requires an attribute name as its first argument".to_owned()),
    };
    let value = numeric_argument(&arguments[1..])
        .ok_or_else(|| "FixAT requires a numeric value as its second argument".to_owned())?;
    let _activation = state
        .active_show
        .try_acquire()
        .map_err(|_| "the active show is changing; retry FixAT".to_owned())?;
    let context = programming_context(session, ActionSource::Osc, None);
    let ports = ServerDynamicsPorts { state, session };
    state
        .dynamics
        .fix_at(
            &context,
            DynamicFixAtCommand {
                targets: Vec::new(),
                attribute,
                value,
                timing: DynamicValueTiming::default(),
            },
            &ports,
        )
        .map(|_| true)
        .map_err(|error| error.message)
}

fn numeric_argument(arguments: &[OscArgument]) -> Option<f32> {
    arguments.first().and_then(|argument| match argument {
        OscArgument::Float(value) => Some(*value),
        OscArgument::Int(value) => Some(*value as f32),
        OscArgument::String(value) => value.parse().ok(),
        OscArgument::Bool(_) => None,
    })
}

fn emit_rejection(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    target: SocketAddr,
    address: &str,
    message: &str,
) {
    send_osc(
        state,
        target,
        format!("/light/{desk_alias}/feedback/dynamic/error"),
        vec![
            OscArgument::String(address.to_owned()),
            OscArgument::String(message.to_owned()),
        ],
    );
    emit(
        state,
        "dynamic_rejected",
        serde_json::json!({
            "desk_id": session.desk.id,
            "session_id": session.id,
            "address": address,
            "message": message,
            "source": "osc",
        }),
    );
}
