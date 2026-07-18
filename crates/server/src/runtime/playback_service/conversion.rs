use super::super::{ApiError, PoolPlaybackInput};
use light_application::{
    ActionSource, CueNumber, PendingPlaybackAction, PlaybackAction, PlaybackLevel, PlaybackSurface,
};

pub(super) fn parse_action(
    name: &str,
    input: &PoolPlaybackInput,
) -> Result<PlaybackAction, ApiError> {
    let pressed = input.pressed.unwrap_or(true);
    let action = match name {
        "go" | "go-plus" => PlaybackAction::Go { pressed },
        "go-minus" | "back" => PlaybackAction::Back { pressed },
        "pause" => PlaybackAction::Pause { pressed },
        "release" => PlaybackAction::Release,
        "on" => PlaybackAction::On { pressed },
        "off" => PlaybackAction::Off { pressed },
        "toggle" => PlaybackAction::Toggle { pressed },
        "fast-forward" => PlaybackAction::FastForward { pressed },
        "fast-rewind" => PlaybackAction::FastRewind { pressed },
        "flash" => PlaybackAction::Flash { pressed },
        "temp" => PlaybackAction::Temp { pressed },
        "swap" => PlaybackAction::Swap { pressed },
        "select" => PlaybackAction::Select { pressed },
        "select-contents" => PlaybackAction::SelectContents { pressed },
        "select-dereferenced" => PlaybackAction::SelectDereferenced { pressed },
        "learn" => PlaybackAction::Learn { pressed },
        "double" => PlaybackAction::Double { pressed },
        "half" => PlaybackAction::Half { pressed },
        "blackout" => PlaybackAction::Blackout { pressed },
        "pause-dynamics" => PlaybackAction::PauseDynamics { pressed },
        "none" => PlaybackAction::None { pressed },
        "master" | "fader" => parse_master(input)?,
        "go-to" => parse_cue_number(input, true)?,
        "load" => parse_cue_number(input, false)?,
        "xfade-on" => PlaybackAction::Crossfade { enabled: true },
        "xfade-off" => PlaybackAction::Crossfade { enabled: false },
        "temp-on" => PlaybackAction::Temporary {
            enabled: true,
            pressed,
        },
        "temp-off" => PlaybackAction::Temporary {
            enabled: false,
            pressed,
        },
        "button" => PlaybackAction::ConfiguredButton {
            number: input
                .button
                .ok_or_else(|| ApiError::bad_request("button number is required"))?,
            pressed,
        },
        _ => return Err(ApiError::not_found("playback action")),
    };
    Ok(action)
}

fn parse_master(input: &PoolPlaybackInput) -> Result<PlaybackAction, ApiError> {
    input
        .value
        .map(PlaybackLevel::new)
        .map(PlaybackAction::Master)
        .ok_or_else(|| ApiError::bad_request("master value is required"))
}

fn parse_cue_number(input: &PoolPlaybackInput, go_to: bool) -> Result<PlaybackAction, ApiError> {
    let number = input
        .cue_number
        .map(CueNumber::new)
        .ok_or_else(|| ApiError::bad_request("cue_number is required"))?;
    Ok(if go_to {
        PlaybackAction::GoTo(number)
    } else {
        PlaybackAction::Load(number)
    })
}

pub(super) fn legacy_action(
    action: PlaybackAction,
    surface: PlaybackSurface,
) -> (&'static str, PoolPlaybackInput) {
    let (name, value, cue_number, button) = structured_action(action);
    (
        name,
        PoolPlaybackInput {
            value,
            cue_number,
            pressed: action.pressed(),
            button,
            surface: Some(surface_name(surface).to_owned()),
        },
    )
}

fn structured_action(
    action: PlaybackAction,
) -> (&'static str, Option<f32>, Option<f64>, Option<u8>) {
    match action {
        PlaybackAction::Master(value) => ("master", Some(value.value()), None, None),
        PlaybackAction::GoTo(number) => ("go-to", None, Some(number.value()), None),
        PlaybackAction::Load(number) => ("load", None, Some(number.value()), None),
        PlaybackAction::ConfiguredButton { number, .. } => ("button", None, None, Some(number)),
        _ => (simple_action_name(action), None, None, None),
    }
}

fn simple_action_name(action: PlaybackAction) -> &'static str {
    match action {
        PlaybackAction::Go { .. } => "go",
        PlaybackAction::Back { .. } => "go-minus",
        PlaybackAction::Pause { .. } => "pause",
        PlaybackAction::Release => "release",
        PlaybackAction::On { .. } => "on",
        PlaybackAction::Off { .. } => "off",
        PlaybackAction::Toggle { .. } => "toggle",
        PlaybackAction::FastForward { .. } => "fast-forward",
        PlaybackAction::FastRewind { .. } => "fast-rewind",
        PlaybackAction::Flash { .. } => "flash",
        PlaybackAction::Temp { .. } => "temp",
        PlaybackAction::Swap { .. } => "swap",
        PlaybackAction::Select { .. } => "select",
        PlaybackAction::SelectContents { .. } => "select-contents",
        PlaybackAction::SelectDereferenced { .. } => "select-dereferenced",
        PlaybackAction::Learn { .. } => "learn",
        PlaybackAction::Double { .. } => "double",
        PlaybackAction::Half { .. } => "half",
        PlaybackAction::Blackout { .. } => "blackout",
        PlaybackAction::PauseDynamics { .. } => "pause-dynamics",
        PlaybackAction::None { .. } => "none",
        PlaybackAction::Crossfade { enabled: true } => "xfade-on",
        PlaybackAction::Crossfade { enabled: false } => "xfade-off",
        PlaybackAction::Temporary { enabled: true, .. } => "temp-on",
        PlaybackAction::Temporary { enabled: false, .. } => "temp-off",
        PlaybackAction::Master(_)
        | PlaybackAction::GoTo(_)
        | PlaybackAction::Load(_)
        | PlaybackAction::ConfiguredButton { .. } => unreachable!("structured action"),
    }
}

pub(super) fn parse_surface(surface: Option<&str>) -> PlaybackSurface {
    match surface {
        Some("virtual") => PlaybackSurface::Virtual,
        Some("osc") => PlaybackSurface::Osc,
        Some("matter") => PlaybackSurface::Matter,
        _ => PlaybackSurface::Physical,
    }
}

pub(super) const fn surface_name(surface: PlaybackSurface) -> &'static str {
    match surface {
        PlaybackSurface::Physical => "physical",
        PlaybackSurface::Virtual => "virtual",
        PlaybackSurface::Osc => "osc",
        PlaybackSurface::Matter => "matter",
    }
}

pub(super) fn parse_pending(action: &str) -> PendingPlaybackAction {
    match action {
        "toggle" => PendingPlaybackAction::Toggle,
        "go" => PendingPlaybackAction::Go,
        "go-minus" => PendingPlaybackAction::Back,
        "off" => PendingPlaybackAction::Off,
        "on" => PendingPlaybackAction::On,
        "temp-on" => PendingPlaybackAction::TemporaryOn,
        "temp-off" => PendingPlaybackAction::TemporaryOff,
        _ => unreachable!("preload returned unsupported action"),
    }
}

pub(super) const fn pending_name(action: PendingPlaybackAction) -> &'static str {
    match action {
        PendingPlaybackAction::Toggle => "toggle",
        PendingPlaybackAction::Go => "go",
        PendingPlaybackAction::Back => "go-minus",
        PendingPlaybackAction::Off => "off",
        PendingPlaybackAction::On => "on",
        PendingPlaybackAction::TemporaryOn => "temp-on",
        PendingPlaybackAction::TemporaryOff => "temp-off",
    }
}

pub(super) fn action_touched(action: PlaybackAction) -> bool {
    matches!(action, PlaybackAction::Master(_)) || action.pressed().unwrap_or(true)
}

pub(super) const fn source_name(source: ActionSource) -> &'static str {
    match source {
        ActionSource::Osc => "osc",
        ActionSource::Matter => "matter",
        ActionSource::Midi => "midi",
        ActionSource::Keyboard => "keyboard",
        ActionSource::UserInterface | ActionSource::Http => "ui",
        _ => "application",
    }
}
