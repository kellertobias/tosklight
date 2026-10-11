use super::super::{ApiError, PoolPlaybackInput};
use light_application::{
    ActionSource, CueNumber, PendingPlaybackAction, PlaybackAction, PlaybackLevel,
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
        "dynamic-restart" => PlaybackAction::DynamicRestart { pressed },
        "dynamic-double-speed" => PlaybackAction::DynamicDoubleSpeed { pressed },
        "dynamic-half-speed" => PlaybackAction::DynamicHalfSpeed { pressed },
        "dynamic-learn-speed" => PlaybackAction::DynamicLearnSpeed { pressed },
        "none" => PlaybackAction::None { pressed },
        "master" | "fader" => parse_master(input)?,
        "master-absolute" => PlaybackAction::MasterAbsolute(
            input
                .value
                .map(PlaybackLevel::new)
                .ok_or_else(|| ApiError::bad_request("master value is required"))?,
        ),
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
        "configured-fader" => PlaybackAction::ConfiguredFader {
            number: input
                .button
                .ok_or_else(|| ApiError::bad_request("fader number is required"))?,
            level: input
                .value
                .map(PlaybackLevel::new)
                .ok_or_else(|| ApiError::bad_request("fader value is required"))?,
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
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("cue_number is required"))?
        .parse::<CueNumber>()
        .map_err(ApiError::bad_request)?;
    Ok(if go_to {
        PlaybackAction::GoTo(number)
    } else {
        PlaybackAction::Load(number)
    })
}

pub(super) fn dispatch_action_input(action: PlaybackAction) -> (&'static str, PoolPlaybackInput) {
    let pressed = action.clone().pressed();
    let (name, value, cue_number, button) = structured_action(action);
    (
        name,
        PoolPlaybackInput {
            value,
            cue_number,
            pressed,
            button,
        },
    )
}

fn structured_action(
    action: PlaybackAction,
) -> (&'static str, Option<f32>, Option<String>, Option<u8>) {
    match action {
        PlaybackAction::MasterAbsolute(value) => {
            ("master-absolute", Some(value.value()), None, None)
        }
        PlaybackAction::Master(value) => ("master", Some(value.value()), None, None),
        PlaybackAction::MasterTransition { .. } => {
            unreachable!("master transitions use the typed Playback boundary")
        }
        PlaybackAction::GoTo(number) => ("go-to", None, Some(number.to_string()), None),
        PlaybackAction::Load(number) => ("load", None, Some(number.to_string()), None),
        PlaybackAction::ConfiguredButton { number, .. } => ("button", None, None, Some(number)),
        PlaybackAction::ConfiguredFader { .. } | PlaybackAction::ConfiguredFaderAbsolute { .. } => {
            unreachable!("configured faders are resolved before assignment dispatch")
        }
        _ => (simple_action_name(action), None, None, None),
    }
}

fn simple_action_name(action: PlaybackAction) -> &'static str {
    match action {
        PlaybackAction::Go { .. } => "go",
        PlaybackAction::Back { .. } => "go-minus",
        PlaybackAction::Pause { .. } => "pause",
        // The numbered control dispatcher exposes the immediate release as Off.
        // Direct Cuelist runtime handles Release as a typed action.
        PlaybackAction::Release => "off",
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
        PlaybackAction::DynamicRestart { .. } => "dynamic-restart",
        PlaybackAction::DynamicDoubleSpeed { .. } => "dynamic-double-speed",
        PlaybackAction::DynamicHalfSpeed { .. } => "dynamic-half-speed",
        PlaybackAction::DynamicLearnSpeed { .. } => "dynamic-learn-speed",
        PlaybackAction::None { .. } => "none",
        PlaybackAction::Crossfade { enabled: true } => "xfade-on",
        PlaybackAction::Crossfade { enabled: false } => "xfade-off",
        PlaybackAction::Temporary { enabled: true, .. } => "temp-on",
        PlaybackAction::Temporary { enabled: false, .. } => "temp-off",
        PlaybackAction::RuntimeStopDynamic { .. }
        | PlaybackAction::Master(_)
        | PlaybackAction::MasterAbsolute(_)
        | PlaybackAction::MasterTransition { .. }
        | PlaybackAction::GoTo(_)
        | PlaybackAction::Load(_)
        | PlaybackAction::ConfiguredButton { .. }
        | PlaybackAction::ConfiguredFader { .. }
        | PlaybackAction::ConfiguredFaderAbsolute { .. } => unreachable!("structured action"),
    }
}

pub(super) const fn activation_surface(
    source: ActionSource,
) -> light_playback::PlaybackActivationSurface {
    match source {
        ActionSource::Osc => light_playback::PlaybackActivationSurface::Osc,
        ActionSource::Matter => light_playback::PlaybackActivationSurface::Matter,
        _ => light_playback::PlaybackActivationSurface::Virtual,
    }
}

pub(super) fn parse_pending(
    action: light_programmer::PreloadPlaybackQueueAction,
) -> PendingPlaybackAction {
    use light_programmer::PreloadPlaybackQueueAction as Queued;
    match action {
        Queued::Toggle => PendingPlaybackAction::Toggle,
        Queued::Go => PendingPlaybackAction::Go,
        Queued::Back => PendingPlaybackAction::Back,
        Queued::Off => PendingPlaybackAction::Off,
        Queued::On => PendingPlaybackAction::On,
        Queued::TemporaryOn => PendingPlaybackAction::TemporaryOn,
        Queued::TemporaryOff => PendingPlaybackAction::TemporaryOff,
        Queued::DynamicPause => PendingPlaybackAction::DynamicPause,
        Queued::DynamicRestart => PendingPlaybackAction::DynamicRestart,
        Queued::DynamicDoubleSpeed => PendingPlaybackAction::DynamicDoubleSpeed,
        Queued::DynamicHalfSpeed => PendingPlaybackAction::DynamicHalfSpeed,
        Queued::DynamicLearnSpeed => PendingPlaybackAction::DynamicLearnSpeed,
    }
}

pub(super) fn action_touched(action: PlaybackAction) -> bool {
    matches!(
        action,
        PlaybackAction::Master(_)
            | PlaybackAction::MasterAbsolute(_)
            | PlaybackAction::MasterTransition { .. }
            | PlaybackAction::ConfiguredFader { .. }
            | PlaybackAction::ConfiguredFaderAbsolute { .. }
    ) || action.pressed().unwrap_or(true)
}

pub(super) const fn source_name(source: ActionSource) -> &'static str {
    match source {
        ActionSource::Osc => "osc",
        ActionSource::Matter => "matter",
        ActionSource::Extension => "extension",
        ActionSource::Keyboard => "keyboard",
        ActionSource::UserInterface | ActionSource::Http => "ui",
        _ => "application",
    }
}
