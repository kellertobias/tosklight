//! The configuration event stream a desk connection follows once its scene is up.

use super::{
    Command, ConnectionState, DeskConnection, Message, ProviderDiagnostics, read_scene, read_view,
    replaces_the_show, scene_affecting, view_affecting,
};
use crate::client::DeskClient;
use futures_util::StreamExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

type EventSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Subscribe to revisioned configuration changes and apply them to the running scene.
///
/// A configuration change is re-read over the connection that is already open and sent on as a
/// delta: the socket stays up, the session stays open, the receivers keep delivering, and the
/// values are carried across. Only a change of show — or a re-read that fails — goes back through
/// the full reconnect, because then the values genuinely belong to something else.
pub(super) async fn watch(
    client: &DeskClient,
    endpoint: &str,
    connection: &DeskConnection,
    outbox: &Sender<Message>,
    orders: &mut tokio::sync::mpsc::UnboundedReceiver<Command>,
    stop: &AtomicBool,
) {
    let Some(mut socket) = open_event_stream(client, endpoint, outbox).await else {
        return;
    };
    // Close the GET-to-WebSocket subscription gap: a settings write between the initial scene
    // read and this subscription is still observed as the newest authoritative snapshot.
    if let Some(settings) = client.renderer_settings().await {
        let _ = outbox.send(Message::RendererSettings(Box::new(settings)));
    }

    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        if connection.values_from_desk_output {
            if let Some(output) = client.output_dmx().await {
                // Older servers retain their normalized overlay. Native lanes include the
                // desk's complete Preload and require no second volatile poll.
                if output.native_protocol == 0
                    && let Some(preload) = client.preload_projection().await
                {
                    let _ = outbox.send(Message::Preload2(Box::new(preload)));
                }
                let _ = outbox.send(Message::DeskOutput(Box::new(output)));
            }
        }
        let poll = if connection.values_from_desk_output {
            // Stage's source contract is 10 Hz. Rendering remains independent and can interpolate
            // physical motion between these authoritative desk-output snapshots.
            100
        } else {
            500
        };
        let next = tokio::select! {
            command = orders.recv() => {
                match command {
                    None => return,
                    Some(Command::Resync) => {
                        let _ = outbox.send(Message::Resync("operator requested".into()));
                        return;
                    }
                    Some(Command::RendererSettings(intent)) => {
                        if let Ok(update) = client.update_renderer_settings(&intent).await {
                            let _ = outbox.send(Message::RendererSettings(Box::new(update)));
                        }
                        continue;
                    }
                }
            }
            message = socket.next() => message,
            _ = tokio::time::sleep(Duration::from_millis(poll)) => continue,
        };
        let Some(Ok(message)) = next else {
            let _ = outbox.send(Message::Connection(ConnectionState::Stale {
                endpoint: endpoint.to_owned(),
                reason: "the configuration event stream closed".into(),
            }));
            return;
        };
        let Ok(text) = message.into_text() else {
            continue;
        };
        let Some(frame) = crate::wire::EventFrame::parse(&text) else {
            continue;
        };
        if frame.kind == "renderer_settings_changed" {
            if let Some(settings) = frame.renderer_settings {
                let _ = outbox.send(Message::RendererSettings(Box::new(settings)));
            } else if let Some(settings) = client.renderer_settings().await {
                let _ = outbox.send(Message::RendererSettings(Box::new(settings)));
            }
            continue;
        }
        // A different show is a different scene: its values, mappings and identity all change
        // together, so it is staged as a whole rather than merged into what is displayed.
        if replaces_the_show(&frame.kind) {
            let _ = outbox.send(Message::Resync(format!("{} changed the scene", frame.kind)));
            return;
        }
        // A preview change is one small re-read, not a scene resynchronisation: the rig has not
        // moved, only what it is being lit with.
        if frame.kind == "preview_values_changed" {
            if let Some(preview) = client.preview_values().await {
                let _ = outbox.send(Message::Preview(Box::new(preview)));
            }
            continue;
        }
        if frame.kind == "visualizer_selection_changed" {
            if let Some(selection) = client.selection().await {
                let _ = outbox.send(Message::Selection(Box::new(selection)));
            }
            continue;
        }
        // The desk moving a camera is not a change of rig: nothing is re-read but the view.
        if view_affecting(&frame.kind) {
            if let Some(view) = read_view(client, connection).await {
                let _ = outbox.send(Message::View(Box::new(view)));
            }
            continue;
        }
        if !scene_affecting(&frame.kind) {
            continue;
        }
        match read_scene(client, endpoint, connection).await {
            Ok((plan, mappings, diagnostics)) => {
                let _ = outbox.send(Message::delta(plan, mappings, diagnostics));
            }
            Err(error) => {
                // The re-read is the only thing that failed; the displayed scene is still the
                // last good one, so this asks for the full path rather than pretending.
                let _ = outbox.send(Message::Resync(format!(
                    "{} changed the scene, and re-reading it failed: {}",
                    frame.kind, error.detail
                )));
                return;
            }
        }
    }
}

/// Open the desk's event stream and subscribe to the projections the renderer follows.
///
/// Answers `None` once the failure has been reported, or silently when there is no session.
async fn open_event_stream(
    client: &DeskClient,
    endpoint: &str,
    outbox: &Sender<Message>,
) -> Option<EventSocket> {
    use futures_util::SinkExt;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL;

    let token = client.token()?;
    let url = endpoint
        .replacen("http://", "ws://", 1)
        .replacen("https://", "wss://", 1);
    let Ok(mut request) = format!("{url}/api/v2/events").into_client_request() else {
        return None;
    };
    let protocols = format!("light.events.v2, light.v2, light.token.{token}");
    if let Ok(value) = protocols.parse() {
        request.headers_mut().insert(SEC_WEBSOCKET_PROTOCOL, value);
    }
    let Ok((mut socket, _)) = tokio_tungstenite::connect_async(request).await else {
        let _ = outbox.send(Message::Diagnostics(Box::new(ProviderDiagnostics {
            endpoint: endpoint.to_owned(),
            warnings: vec![
                "The configuration event stream is unavailable; press R to resynchronise.".into(),
            ],
            ..ProviderDiagnostics::default()
        })));
        return None;
    };

    // A desk delivers events to a subscriber, not to whoever opens the socket: the first frame
    // has to say what this client wants, or the desk answers with an error and closes. Everything
    // the renderer follows is a projection of the show or of the desk's own configuration.
    let subscribe = serde_json::json!({
        "type": "subscribe",
        "filter": {"capabilities": ["show", "desk"], "classes": ["projection"]},
        "capacity": 128,
        "rate_limits": [],
    });
    if socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            subscribe.to_string().into(),
        ))
        .await
        .is_err()
    {
        let _ = outbox.send(Message::Connection(ConnectionState::Stale {
            endpoint: endpoint.to_owned(),
            reason: "the configuration event stream would not take a subscription".into(),
        }));
        return None;
    }
    Some(socket)
}
