//! OSC absolute family writes (TL-544 G3): `/light/{desk}/programmer/family/{component}` with
//! one number (an absolute value) or several (an ordered `[THRU]` spread), in the component's
//! display units: percent for Red, Green, Blue, White Blend and Focus, degrees for Pan, Tilt and
//! Zoom. The write is the command line's typed family value against the desk's current ordered
//! selection, through the same Programmer values transaction as the encoders.
use super::programmer_family_values::{
    CommandFamily, FamilyValueSource, FamilyValues, apply_family_values_to_selection,
    parse_family_values,
};
use super::*;
use std::net::SocketAddr;

/// `None`: not a family write (the caller keeps routing). `Some(applied)` otherwise.
pub(super) fn handle_family_values_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) -> Option<bool> {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    let [light, path, programmer, family, component] = parts.as_slice() else {
        return None;
    };
    if *light != "light" || *programmer != "programmer" || *family != "family" {
        return None;
    }
    let source = source.and_then(|value| value.parse::<SocketAddr>().ok());
    let (subscriber, session) = programmer_osc_session(state, source)?;
    if !subscriber.path.eq_ignore_ascii_case(path) || read_desk_lock(state).locked {
        return Some(false);
    }
    attach_session_command_context(state, &session);
    let result = osc_family_values(component, arguments).and_then(|values| {
        let _activation = state
            .active_show
            .try_acquire()
            .map_err(|_| "the active show is changing; retry the family value".to_owned())?;
        apply_family_values_to_selection(
            state,
            &session,
            &values,
            programmer_value_timing(state, CommandTiming::default()),
            FamilyValueSource::Osc,
        )
    });
    match result {
        Ok(applied) => Some(applied > 0),
        Err(message) => {
            send_osc(
                state,
                subscriber.target,
                format!("/light/{path}/feedback/programmer/error"),
                vec![
                    OscArgument::String(address.to_owned()),
                    OscArgument::String(message.clone()),
                ],
            );
            emit(
                state,
                "programmer_family_value_rejected",
                serde_json::json!({
                    "desk_id": session.desk.id,
                    "session_id": session.id,
                    "address": address,
                    "message": message,
                    "source": "osc",
                }),
            );
            Some(false)
        }
    }
}

/// The OSC component name and its family; names follow the encoder slot labels.
fn component_family(component: &str) -> Option<(CommandFamily, usize)> {
    Some(match component {
        "red" => (CommandFamily::Color, 0),
        "green" => (CommandFamily::Color, 1),
        "blue" => (CommandFamily::Color, 2),
        "white-blend" => (CommandFamily::Color, 3),
        "pan" => (CommandFamily::Position, 0),
        "tilt" => (CommandFamily::Position, 1),
        "focus" => (CommandFamily::Focus, 0),
        "zoom" => (CommandFamily::Focus, 1),
        _ => return None,
    })
}

/// Builds the same token stream the command line parses: the value in its encoder slot, with
/// `THRU` between spread points and `- -` for a negative absolute value.
pub(super) fn osc_family_values(
    component: &str,
    arguments: &[OscArgument],
) -> Result<FamilyValues, String> {
    let (family, index) = component_family(component)
        .ok_or_else(|| format!("unknown family component `{component}`"))?;
    let numbers = arguments
        .iter()
        .map(|argument| match argument {
            OscArgument::Float(value) => Some(*value),
            OscArgument::Int(value) => Some(*value as f32),
            OscArgument::String(value) => value.trim().parse().ok(),
            OscArgument::Bool(_) => None,
        })
        .collect::<Option<Vec<_>>>()
        .filter(|numbers| !numbers.is_empty() && numbers.iter().all(|value| value.is_finite()))
        .ok_or_else(|| "a family value takes one or more finite numbers".to_owned())?;
    let mut tokens = Vec::new();
    for (point, value) in numbers.iter().enumerate() {
        if point > 0 {
            tokens.push("THRU".to_owned());
        }
        tokens.extend(std::iter::repeat_n("DIV".to_owned(), index));
        if *value < 0.0 {
            tokens.extend(["-".to_owned(), "-".to_owned()]);
        }
        tokens.push(value.abs().to_string());
    }
    parse_family_values(family, &tokens)
}
