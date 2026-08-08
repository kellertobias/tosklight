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
use crate::wire::{AudioView, TelemetryFrame};

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
/// Nothing is read from the socket except its closure: this direction carries telemetry, and a
/// client that wants to change something uses the API. Anything the client does send is drained so
/// a chatty one cannot fill a buffer.
async fn push(mut socket: WebSocket, state: ApiState) {
    let mut ticks = tokio::time::interval(FRAME_INTERVAL);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = ticks.tick() => {
                let frame = TelemetryFrame {
                    audio: AudioView::of(&(state.diagnostics.audio)()),
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
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}
