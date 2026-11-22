//! Pushed telemetry.
//!
//! Volatile state is pushed, never polled. An audio meter changes many times a second, and a panel
//! that asked for it over and over would spend a show's worth of requests learning that the room
//! is quiet. So the socket sends frames on its own cadence and the client just draws them.
//!
//! The cadence is the server's decision because the server knows what the analysis costs. It is
//! deliberately slower than the analysis itself: an operator reads a meter, they do not measure it,
//! and a browser cannot usefully draw faster than this anyway.

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;

use crate::routes::ApiState;
use crate::wire::{
    AudioView, DeskIdentityView, DmxIngressView, ImportJobView, SpeedGroupReceptionView,
    TelemetryFrame,
};

/// Twenty frames a second: fast enough that a beat reads as a flash, slow enough that a browser
/// keeps up on a machine that is also compositing.
const FRAME_INTERVAL: Duration = Duration::from_millis(50);

pub(super) async fn telemetry(
    upgrade: WebSocketUpgrade,
    State(state): State<ApiState>,
) -> Response {
    upgrade.on_upgrade(move |socket| push(socket, state))
}

/// Sends frames until the client goes away.
///
/// Live actions share this ordered connection. Acknowledgements carry an action identity for
/// feedback only; neither end queues or replays actions after reconnection.
async fn push(mut socket: WebSocket, state: ApiState) {
    let mut ticks = tokio::time::interval(FRAME_INTERVAL);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = ticks.tick() => {
                let (_, jobs) = (state.diagnostics.imports.state)();
                let frame = TelemetryFrame {
                    automation_revision: state.diagnostics.automation.snapshot.load().automation_revision,
                    automation: state.diagnostics.automation.snapshot.load().status.clone(),
                    audio: AudioView::of(&(state.diagnostics.audio)()),
                    imports: jobs.iter().map(ImportJobView::of).collect(),
                    dmx: (state.diagnostics.dmx)().iter().map(DmxIngressView::of).collect(),
                    desk_identity: (state.diagnostics.desk_identity)()
                        .as_ref()
                        .map(DeskIdentityView::of),
                    speed_groups: SpeedGroupReceptionView::of(&(state.diagnostics.speed_groups)()),
                };
                let Ok(serialized) = serde_json::to_string(&frame) else {
                    continue;
                };
                if socket.send(Message::Text(serialized.into())).await.is_err() {
                    return;
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    // The client closed, or the connection broke. Either way this is over.
                    None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                    Some(Ok(Message::Text(body))) => {
                        let answer = action(&state, &body).await;
                        if let Ok(serialized) = serde_json::to_string(&answer)
                            && socket.send(Message::Text(serialized.into())).await.is_err() { return; }
                    }
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

async fn action(state: &ApiState, body: &str) -> crate::wire::AutomationActionResult {
    let parsed: Result<crate::wire::AutomationActionFrame, _> = serde_ignored::deserialize(
        &mut serde_json::Deserializer::from_str(body),
        |path| tracing::warn!(route = "/api/v2/telemetry", field = %path, "unknown action field"),
    );
    let mut preview = None;
    let (action_id, result) = match parsed {
        Ok(frame) if !frame.action_id.is_empty() && frame.action_id.len() <= 256 => {
            let result = match frame.action {
                crate::wire::AutomationAction::AutomationPreviewStep { request } => {
                    super::playlist_preview::show_action(state, &request).map(|result| {
                        preview = Some(result);
                    })
                }
                crate::wire::AutomationAction::AutomationStopPreview { request } => {
                    super::playlist_preview::stop_action(state, &request).map(|result| {
                        preview = Some(result);
                    })
                }
                crate::wire::AutomationAction::AutomationApplyNow { output_id } => {
                    super::automation::apply_now_action(state, &output_id)
                }
                crate::wire::AutomationAction::AutomationSetEnabled { output_id, enabled } => {
                    super::automation::set_enabled_action(state, &output_id, enabled).await
                }
            };
            (frame.action_id, result)
        }
        _ => (
            String::new(),
            Err(crate::ApiError::bad_request(
                "invalid-action",
                "Send a valid automation action with an actionId",
            )),
        ),
    };
    crate::wire::AutomationActionResult {
        preview,
        kind: "automation-action-result".into(),
        action_id,
        ok: result.is_ok(),
        message: result.err().map(|error| error.body().message.clone()),
    }
}
