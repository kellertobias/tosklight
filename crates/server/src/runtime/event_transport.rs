//! Authenticated v2 filtered event delivery and authoritative Playback repair snapshots.

mod adapter;

use super::{ApiError, AppState, Session, authenticate, authenticate_token};
use axum::{
    Json, Router,
    extract::{State, WebSocketUpgrade, ws::Message, ws::WebSocket},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
    routing::get,
};
use light_application as application;
use light_wire::v2::events as wire;
use uuid::Uuid;

const DEFAULT_CAPACITY: usize = 256;
const MAX_CAPACITY: usize = 1_024;
const MAX_RATE_LIMITS: usize = 64;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/events", get(ws_events))
        .route("/api/v2/events/playback-snapshot", get(playback_snapshot))
}

async fn ws_events(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let session = authenticate_protocols(&state, &headers)?;
    Ok(ws
        .protocols(["light.events.v2", "light.v2"])
        .on_upgrade(move |socket| handle_socket(socket, state, session))
        .into_response())
}

pub(super) fn authenticate_protocols(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Session, ApiError> {
    let token = websocket_token(headers)
        .ok_or_else(|| ApiError::unauthorized("WebSocket session token protocol is missing"))?;
    authenticate_token(state, token)
}

fn websocket_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::SEC_WEBSOCKET_PROTOCOL)?
        .to_str()
        .ok()?
        .split(',')
        .map(str::trim)
        .find_map(|protocol| protocol.strip_prefix("light.token."))
}

async fn handle_socket(mut socket: WebSocket, state: AppState, session: Session) {
    let Some(request) = next_client_message(&mut socket).await else {
        return;
    };
    let mut stream =
        match EventStream::subscribe(&state.application_events, session.desk.id, request) {
            Ok(stream) => stream,
            Err(error) => {
                send_wire(&mut socket, wire::EventServerMessage::Error { error }).await;
                return;
            }
        };
    if !send_wire(&mut socket, stream.ready()).await {
        return;
    }
    event_loop(&mut socket, &mut stream).await;
}

async fn event_loop(socket: &mut WebSocket, stream: &mut EventStream) {
    loop {
        let item = tokio::select! {
            delivery = stream.next() => LoopItem::Delivery(delivery),
            request = next_client_message(socket) => LoopItem::Client(request),
        };
        if !handle_loop_item(socket, stream, item).await {
            return;
        }
    }
}

enum LoopItem {
    Delivery(Option<wire::EventServerMessage>),
    Client(Option<Result<wire::EventClientMessage, String>>),
}

async fn handle_loop_item(socket: &mut WebSocket, stream: &EventStream, item: LoopItem) -> bool {
    match item {
        LoopItem::Delivery(Some(message)) => send_wire(socket, message).await,
        LoopItem::Delivery(None) | LoopItem::Client(None) => false,
        LoopItem::Client(Some(Ok(message))) => handle_client_message(socket, stream, message).await,
        LoopItem::Client(Some(Err(error))) => {
            send_wire(socket, wire::EventServerMessage::Error { error }).await
        }
    }
}

async fn handle_client_message(
    socket: &mut WebSocket,
    stream: &EventStream,
    message: wire::EventClientMessage,
) -> bool {
    let response = match message {
        wire::EventClientMessage::Repair { cursor } => stream.repair(cursor),
        wire::EventClientMessage::Subscribe { .. } => wire::EventServerMessage::Error {
            error: "event subscription is already active".into(),
        },
    };
    send_wire(socket, response).await
}

async fn next_client_message(
    socket: &mut WebSocket,
) -> Option<Result<wire::EventClientMessage, String>> {
    loop {
        match socket.recv().await? {
            Ok(Message::Text(text)) => {
                return Some(serde_json::from_str(&text).map_err(|error| error.to_string()));
            }
            Ok(Message::Ping(value)) => {
                socket.send(Message::Pong(value)).await.ok()?;
            }
            Ok(Message::Close(_)) | Err(_) => return None,
            _ => {}
        }
    }
}

async fn send_wire(socket: &mut WebSocket, message: wire::EventServerMessage) -> bool {
    let Ok(json) = serde_json::to_string(&message) else {
        return false;
    };
    socket.send(Message::Text(json.into())).await.is_ok()
}

pub(super) struct EventStream {
    pub(super) bus: application::EventBus,
    pub(super) subscription: application::EventSubscription,
}

impl EventStream {
    pub(super) fn subscribe(
        bus: &application::EventBus,
        desk_id: Uuid,
        request: Result<wire::EventClientMessage, String>,
    ) -> Result<Self, String> {
        let message = request?;
        let wire::EventClientMessage::Subscribe {
            filter,
            after_sequence,
            capacity,
            rate_limits,
        } = message
        else {
            return Err("the first event message must subscribe".into());
        };
        validate_cursor(bus, after_sequence)?;
        let options = subscription_options(capacity, after_sequence, rate_limits)?;
        let subscription = bus.subscribe(adapter::application_filter(desk_id, filter), options);
        Ok(Self {
            bus: bus.clone(),
            subscription,
        })
    }

    pub(super) fn ready(&self) -> wire::EventServerMessage {
        wire::EventServerMessage::Ready {
            cursor: self.cursor(),
        }
    }

    pub(super) async fn next(&mut self) -> Option<wire::EventServerMessage> {
        self.subscription.next().await.map(adapter::wire_delivery)
    }

    pub(super) fn repair(&self, cursor: wire::EventSnapshotCursor) -> wire::EventServerMessage {
        if cursor.sequence > self.bus.latest_sequence() {
            return wire::EventServerMessage::Error {
                error: "snapshot cursor is newer than the event stream".into(),
            };
        }
        match self.subscription.repair_from_snapshot(cursor.sequence) {
            Ok(()) => wire::EventServerMessage::Repaired { cursor },
            Err(gap) => wire::EventServerMessage::Gap {
                gap: adapter::wire_gap(gap),
            },
        }
    }

    fn cursor(&self) -> wire::EventSnapshotCursor {
        wire::EventSnapshotCursor {
            sequence: self.bus.latest_sequence(),
        }
    }
}

fn validate_cursor(bus: &application::EventBus, cursor: Option<u64>) -> Result<(), String> {
    if cursor.is_some_and(|sequence| sequence > bus.latest_sequence()) {
        return Err("event cursor is newer than the event stream".into());
    }
    Ok(())
}

fn subscription_options(
    capacity: Option<u16>,
    after_sequence: Option<u64>,
    rate_limits: Vec<wire::EventRateLimit>,
) -> Result<application::SubscriptionOptions, String> {
    let capacity = capacity.map_or(DEFAULT_CAPACITY, usize::from);
    if !(1..=MAX_CAPACITY).contains(&capacity) {
        return Err(format!("event capacity must be within 1-{MAX_CAPACITY}"));
    }
    validate_rate_limits(&rate_limits)?;
    Ok(application::SubscriptionOptions {
        capacity,
        after_sequence,
        rate_limits: adapter::application_rate_limits(rate_limits),
    })
}

fn validate_rate_limits(limits: &[wire::EventRateLimit]) -> Result<(), String> {
    if limits.len() > MAX_RATE_LIMITS {
        return Err(format!(
            "at most {MAX_RATE_LIMITS} event rate limits are allowed"
        ));
    }
    for (index, limit) in limits.iter().enumerate() {
        validate_rate_limit(limit)?;
        if limits[..index]
            .iter()
            .any(|other| same_rate_topic(other, limit))
        {
            return Err("duplicate event rate-limit topic".into());
        }
    }
    Ok(())
}

fn validate_rate_limit(limit: &wire::EventRateLimit) -> Result<(), String> {
    if limit.min_interval_millis == 0 {
        return Err("event rate-limit intervals must be greater than zero".into());
    }
    if !matches!(
        limit.class,
        wire::EventClass::Projection | wire::EventClass::Telemetry
    ) {
        return Err("only replaceable projection and telemetry events may be rate-limited".into());
    }
    if limit
        .object
        .as_ref()
        .is_some_and(|object| object.capability != limit.capability)
    {
        return Err("event rate-limit object capability must match its topic".into());
    }
    Ok(())
}

fn same_rate_topic(left: &wire::EventRateLimit, right: &wire::EventRateLimit) -> bool {
    left.capability == right.capability && left.class == right.class && left.object == right.object
}

async fn playback_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::PlaybackEventSnapshot>, ApiError> {
    let session = authenticate(&state, &headers)?;
    Ok(Json(playback_snapshot_from(
        &state.application_events,
        session.desk.id,
        || {
            state
                .engine
                .playback()
                .read()
                .runtime_status()
                .into_iter()
                .map(playback_state)
                .collect()
        },
    )))
}

pub(super) fn playback_snapshot_from(
    bus: &application::EventBus,
    desk_id: Uuid,
    projection: impl FnOnce() -> Vec<wire::PlaybackStateSnapshot>,
) -> wire::PlaybackEventSnapshot {
    let cursor = wire::EventSnapshotCursor {
        sequence: bus.latest_sequence(),
    };
    wire::PlaybackEventSnapshot {
        desk_id,
        cursor,
        playbacks: projection(),
    }
}

fn playback_state(status: light_playback::PlaybackRuntimeStatus) -> wire::PlaybackStateSnapshot {
    let playback = status.playback;
    wire::PlaybackStateSnapshot {
        object: playback_object(playback.playback_number, playback.cue_list_id.0),
        playback_number: playback.playback_number,
        cue_list_id: playback.cue_list_id.0,
        current: cue(playback.current_cue_id, playback.current_cue_number),
        loaded: cue(playback.loaded_cue_id, playback.loaded_cue_number),
        paused: playback.paused,
        enabled: playback.enabled,
    }
}

fn playback_object(playback_number: Option<u16>, cue_list_id: Uuid) -> wire::EventObject {
    let id = playback_number.map_or_else(
        || format!("cuelist:{cue_list_id}"),
        |number| format!("playback:{number}"),
    );
    wire::EventObject {
        capability: wire::EventCapability::Playback,
        id,
    }
}

fn cue(id: Option<Uuid>, number: Option<f64>) -> Option<wire::CueReference> {
    id.zip(number)
        .map(|(id, number)| wire::CueReference { id, number })
}

#[cfg(test)]
#[path = "tests/event_transport_tests.rs"]
mod tests;
