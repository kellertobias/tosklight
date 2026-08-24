use super::*;
use anyhow::Context;
use std::{future::Future, pin::Pin};

mod mappings;
mod subscriptions;
use mappings::{apply_control_mappings, mapped_control_origin};
pub(super) use subscriptions::{disconnect_orphaned_osc_session, handle_subscription_osc};

pub(super) type ControlInputTask =
    Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>;

pub(super) fn control_input_tasks(
    state: &AppState,
    cancel: CancellationToken,
) -> Vec<ControlInputTask> {
    let configuration = state.installation.configuration();
    let mut tasks: Vec<ControlInputTask> = Vec::new();
    if !state.output.has_test_clock() {
        let feedback_state = state.clone();
        let feedback_cancel = cancel.clone();
        tasks.push(Box::pin(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            loop {
                tokio::select! {
                    _ = feedback_cancel.cancelled() => break,
                    _ = interval.tick() => send_osc_feedback(&feedback_state, false),
                }
            }
            Ok(())
        }));
        let refresh_state = state.clone();
        let refresh_cancel = cancel.clone();
        tasks.push(Box::pin(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50));
            loop {
                tokio::select! {
                    _ = refresh_cancel.cancelled() => break,
                    _ = interval.tick() => { refresh_speed_group_engine(&refresh_state); }
                }
            }
            Ok(())
        }));
    }
    for (address, protocol) in [
        (configuration.osc_bind, UdpInputProtocol::Osc),
        (
            configuration.art_timecode_bind,
            UdpInputProtocol::ArtTimeCode,
        ),
    ] {
        let Some(address) = address else {
            continue;
        };
        let state = state.clone();
        let cancel = cancel.clone();
        tasks.push(Box::pin(async move {
            let input = UdpControlInput::bind(address, protocol)
                .await
                .with_context(|| format!("control input could not bind {address}"))?;
            drive_control_input(state, cancel, input).await;
            Ok(())
        }));
    }
    tasks
}

pub(super) async fn drive_control_input<I: ControlInput>(
    state: AppState,
    cancel: CancellationToken,
    mut input: I,
) {
    loop {
        tokio::select! { _=cancel.cancelled()=>break,event=input.next_event()=>match event { Some(event)=>handle_control_event(&state,event),None=>break } }
    }
}

pub(super) fn handle_control_event(state: &AppState, event: ControlEvent) {
    if let ControlEvent::Timecode(timecode) = &event {
        ingest_timecode(state, timecode.clone());
    }
    let input_locked = if let ControlEvent::Osc { address, .. } = &event {
        let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
        parts
            .get(1)
            .and_then(|_| osc_control_desk(state))
            .is_some_and(|_| read_desk_lock(state).locked)
    } else {
        false
    };
    if let ControlEvent::Osc {
        address, arguments, ..
    } = &event
        && let Some(configuration) = &state.installation.configuration().osc_timecode
        && &configuration.address == address
        && let [
            OscArgument::Int(hours),
            OscArgument::Int(minutes),
            OscArgument::Int(seconds),
            OscArgument::Int(frames),
            ..,
        ] = arguments.as_slice()
        && (0..24).contains(hours)
        && (0..60).contains(minutes)
        && (0..60).contains(seconds)
        && (0..i32::from(configuration.rate.nominal_frames())).contains(frames)
    {
        ingest_timecode(
            state,
            SmpteTimecode {
                hours: *hours as u8,
                minutes: *minutes as u8,
                seconds: *seconds as u8,
                frames: *frames as u8,
                rate: configuration.rate,
                source: format!("osc:{address}"),
                received_at: chrono::Utc::now(),
            },
        );
    }
    if let ControlEvent::Osc {
        address,
        arguments,
        source,
    } = &event
    {
        let action_timing =
            begin_authenticated_osc_action_timing(state, address, arguments, source.as_deref());
        let mut measured_action_succeeded = false;
        if !handle_subscription_osc(state, address, arguments, source.as_deref()) && !input_locked {
            // A guest operates playback and nothing else. It never reaches the Programmer, so
            // there is no Record to capture, no Update to arm and no encoder to move — which is
            // the whole point of it being able to work beside somebody who is programming.
            measured_action_succeeded |=
                handle_playback_osc(state, address, arguments, source.as_deref());
            handle_timing_osc(state, address, arguments);
            if osc_source_capability(state, source.as_deref()).may_program() {
                handle_highlight_osc(state, address, arguments, source.as_deref());
                measured_action_succeeded |=
                    handle_dynamics_osc(state, address, arguments, source.as_deref());
                measured_action_succeeded |=
                    handle_programmer_osc(state, address, arguments, source.as_deref());
                handle_encoder_osc(state, address, arguments, source.as_deref());
            }
        }
        send_osc_feedback(state, false);
        if let Some(action_timing) = action_timing {
            action_timing.acknowledge(state, measured_action_succeeded);
        }
    }
    if input_locked {
        return;
    }
    let mappings = state.output.snapshot().control_mappings.clone();
    let origin = mapped_control_origin(state, &event);
    if mappings.iter().any(|mapping| mapping.matches(&event)) {
        match state.active_show.try_acquire() {
            Ok(_activation) => {
                apply_control_mappings(
                    state,
                    &origin,
                    mappings
                        .iter()
                        .filter(|mapping| mapping.matches(&event))
                        .map(|mapping| &mapping.action),
                );
            }
            Err(_) => tracing::warn!("mapped control action skipped during active show transition"),
        }
    }
    emit(
        state,
        "control_event",
        serde_json::to_value(event)
            .unwrap_or_else(|_| serde_json::json!({"error":"serialization failed"})),
    );
}

/// What the surface that sent this message is allowed to do.
///
/// An unknown source has not subscribed, so it has no surface and reaches nothing; treating it as
/// a guest keeps the programming path closed to anything that never announced itself.
fn osc_source_capability(state: &AppState, source: Option<&str>) -> light_core::SurfaceCapability {
    source
        .and_then(|source| source.parse().ok())
        .and_then(|source| state.integrations.osc_subscriber_for_source(source))
        .map_or(light_core::SurfaceCapability::PlaybackOnly, |subscriber| {
            subscriber.capability
        })
}

struct AuthenticatedOscActionTiming {
    receipt: ActionTimingReceipt,
    path: String,
    feedback_target: SocketAddr,
}

impl AuthenticatedOscActionTiming {
    fn acknowledge(self, state: &AppState, succeeded: bool) {
        let timing = self.receipt.acknowledge(succeeded);
        send_action_timing_feedback(state, &self.path, self.feedback_target, &timing);
    }
}

fn begin_authenticated_osc_action_timing(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) -> Option<AuthenticatedOscActionTiming> {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    let [light, ..] = parts.as_slice() else {
        return None;
    };
    if *light != "light" {
        return None;
    }
    let source = source?.parse::<SocketAddr>().ok()?;
    let subscriber = state.integrations.osc_subscriber_for_source(source)?;
    if state.sessions.session(subscriber.session_id).is_none() {
        return None;
    }
    let path = parts
        .get(1)
        .filter(|alias| !matches!(**alias, "playback" | "cuelist" | "qlist"))
        .copied()
        .unwrap_or(&subscriber.path);
    if !subscriber.path.eq_ignore_ascii_case(path) {
        return None;
    }
    let (action, may_change_output) = osc_action_timing(&parts, arguments)?;
    let request_id = arguments
        .get(1)
        .and_then(|argument| match argument {
            OscArgument::String(value) if !value.trim().is_empty() => Some(value.clone()),
            _ => None,
        })
        .unwrap_or_else(|| format!("osc-{}", Uuid::new_v4()));
    if action == "encoder" {
        state.action_timing.begin_causal_origin(
            subscriber.session_id.0.to_string(),
            "osc",
            request_id,
            state.output.frame_rate_hz(),
            OscActionFeedback {
                path: path.to_owned(),
                target: subscriber.target,
            },
        );
        return None;
    }
    Some(AuthenticatedOscActionTiming {
        receipt: state.action_timing.begin(
            "osc",
            action,
            request_id,
            state.output.frame_rate_hz(),
            may_change_output,
        ),
        path: path.to_owned(),
        feedback_target: subscriber.target,
    })
}

fn osc_action_timing(parts: &[&str], arguments: &[OscArgument]) -> Option<(String, bool)> {
    if let ["light", _, capability, rest @ ..] = parts {
        match *capability {
            "programmer" if !rest.is_empty() && osc_pressed(arguments) => {
                return Some((
                    "programmer_key".into(),
                    rest.first().is_some_and(|action| {
                        matches!(*action, "at" | "clear" | "enter" | "preload" | "undo")
                    }),
                ));
            }
            "dynamic" if !rest.is_empty() => return Some(("dynamic".into(), true)),
            "encode" if !rest.is_empty() => return Some(("encoder".into(), false)),
            "nav" => return Some(("encoder".into(), false)),
            _ => {}
        }
    }
    let (_, action_index) = osc_playback_address(parts)?;
    let action = match parts.get(action_index).copied()? {
        "go" => "playback_go",
        "flash" if osc_pressed(arguments) => "playback_flash_press",
        "flash" => "playback_flash_release",
        "fader" | "master" => "playback_master",
        "back" | "go-minus" => "playback_back",
        "pause" => "playback_pause",
        "release" => "playback_release",
        _ => "playback_action",
    };
    Some((action.into(), true))
}

#[cfg(test)]
mod action_timing_tests {
    use super::*;

    #[test]
    fn osc_playback_timing_distinguishes_press_release_and_fader() {
        let press = [OscArgument::Bool(true)];
        let release = [OscArgument::Bool(false)];
        assert_eq!(
            osc_action_timing(&["light", "playback", "1", "go"], &press),
            Some(("playback_go".into(), true))
        );
        assert_eq!(
            osc_action_timing(&["light", "playback", "1", "flash"], &press),
            Some(("playback_flash_press".into(), true))
        );
        assert_eq!(
            osc_action_timing(&["light", "playback", "1", "flash"], &release),
            Some(("playback_flash_release".into(), true))
        );
        assert_eq!(
            osc_action_timing(
                &["light", "main", "page-playback", "1", "fader"],
                &[OscArgument::Float(0.5)]
            ),
            Some(("playback_master".into(), true))
        );
    }
}
