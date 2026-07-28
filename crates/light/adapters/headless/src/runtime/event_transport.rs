//! Authenticated v2 filtered event delivery and authoritative Playback repair snapshots.

mod adapter;

use super::{
    ApiError, AppState, EventResource, Session, WsResponse, authenticate_token,
    dispatch_live_action_live,
};
use axum::{
    Router,
    extract::{State, WebSocketUpgrade, ws::Message, ws::WebSocket},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
    routing::get,
};
use light_application as application;
use light_wire::v2::events as wire;
use light_wire::v2::live_action::LiveActionFrame;
use serde::Serialize;
use uuid::Uuid;

const DEFAULT_CAPACITY: usize = 256;
const MAX_CAPACITY: usize = 1_024;
const MAX_RATE_LIMITS: usize = 64;

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v2/events", get(ws_events))
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
    let request = match request {
        ClientMessage::Event(request) => Ok(request),
        ClientMessage::Action(_) => Err("the first event message must subscribe".into()),
        ClientMessage::Invalid { error, .. } => Err(error),
    };
    let mut stream = match EventStream::subscribe(&state.events, &session, request) {
        Ok(stream) => stream,
        Err(error) => {
            send_wire(&mut socket, wire::EventServerMessage::Error { error }).await;
            return;
        }
    };
    if !send_wire(&mut socket, stream.ready()).await {
        return;
    }
    event_loop(&mut socket, &mut stream, &state, &session).await;
}

async fn event_loop(
    socket: &mut WebSocket,
    stream: &mut EventStream,
    state: &AppState,
    session: &Session,
) {
    loop {
        let item = tokio::select! {
            delivery = stream.next() => LoopItem::Delivery(delivery),
            request = next_client_message(socket) => LoopItem::Client(request),
        };
        if !handle_loop_item(socket, stream, state, session, item).await {
            return;
        }
    }
}

enum LoopItem {
    Delivery(Option<wire::EventServerMessage>),
    Client(Option<ClientMessage>),
}

async fn handle_loop_item(
    socket: &mut WebSocket,
    stream: &EventStream,
    state: &AppState,
    session: &Session,
    item: LoopItem,
) -> bool {
    match item {
        LoopItem::Delivery(Some(message)) => send_wire(socket, message).await,
        LoopItem::Delivery(None) | LoopItem::Client(None) => false,
        LoopItem::Client(Some(message)) => {
            send_server_message(
                socket,
                client_response(stream, state, session, message).await,
            )
            .await
        }
    }
}

pub(super) async fn client_response(
    stream: &EventStream,
    state: &AppState,
    session: &Session,
    message: ClientMessage,
) -> ServerMessage {
    match message {
        ClientMessage::Event(wire::EventClientMessage::Repair { cursor }) => {
            ServerMessage::Event(stream.repair(cursor))
        }
        ClientMessage::Event(wire::EventClientMessage::Subscribe { .. }) => {
            ServerMessage::Event(wire::EventServerMessage::Error {
                error: "event subscription is already active".into(),
            })
        }
        ClientMessage::Action(action) => {
            ServerMessage::Command(dispatch_live_action_live(state, session, action).await)
        }
        ClientMessage::Invalid {
            request_id: Some(request_id),
            error,
        } => ServerMessage::Command(command_error(state, request_id, error)),
        ClientMessage::Invalid {
            request_id: None,
            error,
        } => ServerMessage::Event(wire::EventServerMessage::Error { error }),
    }
}

async fn next_client_message(socket: &mut WebSocket) -> Option<ClientMessage> {
    loop {
        match socket.recv().await? {
            Ok(Message::Text(text)) => {
                return Some(parse_client_message(&text));
            }
            Ok(Message::Ping(value)) => {
                socket.send(Message::Pong(value)).await.ok()?;
            }
            Ok(Message::Close(_)) | Err(_) => return None,
            _ => {}
        }
    }
}

pub(super) enum ClientMessage {
    Event(wire::EventClientMessage),
    Action(LiveActionFrame),
    Invalid {
        request_id: Option<String>,
        error: String,
    },
}

pub(super) enum ServerMessage {
    Event(wire::EventServerMessage),
    Command(WsResponse),
}

fn parse_client_message(text: &str) -> ClientMessage {
    let value = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => value,
        Err(error) => {
            return ClientMessage::Invalid {
                request_id: None,
                error: format!("invalid WebSocket message: {error}"),
            };
        }
    };
    if value.get("type").and_then(serde_json::Value::as_str) == Some("action") {
        let request_id = value
            .get("request_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        crate::tolerant_json::log_unknown_value_fields::<LiveActionFrame>(
            "/api/v2/events action",
            &value,
        );
        return match serde_json::from_value(value) {
            Ok(action) => ClientMessage::Action(action),
            Err(error) => ClientMessage::Invalid {
                request_id,
                error: format!("invalid action frame: {error}"),
            },
        };
    }
    if value.get("type").is_some() {
        return match serde_json::from_value(value) {
            Ok(message) => ClientMessage::Event(message),
            Err(error) => ClientMessage::Invalid {
                request_id: None,
                error: format!("invalid event control message: {error}"),
            },
        };
    }
    let request_id = value
        .get("request_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    ClientMessage::Invalid {
        request_id,
        error: "WebSocket actions require an explicit type: action frame".into(),
    }
}

fn command_error(state: &AppState, request_id: String, error: String) -> WsResponse {
    WsResponse {
        protocol_version: 2,
        request_id,
        ok: false,
        revision: state.output.snapshot().revision,
        payload: None,
        error: Some(error),
    }
}

async fn send_wire(socket: &mut WebSocket, message: wire::EventServerMessage) -> bool {
    send_json(socket, &message).await
}

async fn send_server_message(socket: &mut WebSocket, message: ServerMessage) -> bool {
    match message {
        ServerMessage::Event(message) => send_wire(socket, message).await,
        ServerMessage::Command(message) => send_json(socket, &message).await,
    }
}

async fn send_json<T: Serialize>(socket: &mut WebSocket, message: &T) -> bool {
    let Ok(json) = serde_json::to_string(message) else {
        return false;
    };
    socket.send(Message::Text(json.into())).await.is_ok()
}

pub(super) struct EventStream {
    events: EventResource,
    pub(super) subscription: application::EventSubscription,
}

impl EventStream {
    pub(super) fn subscribe(
        events: &EventResource,
        session: &Session,
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
        validate_cursor(events, after_sequence)?;
        validate_programming_scope(session, &filter, &rate_limits)?;
        let options = subscription_options(capacity, after_sequence, rate_limits)?;
        let subscription = events.subscribe(adapter::application_filter(session, filter), options);
        Ok(Self {
            events: events.clone(),
            subscription,
        })
    }

    pub(super) fn ready(&self) -> wire::EventServerMessage {
        wire::EventServerMessage::Ready {
            cursor: self.cursor(),
        }
    }

    pub(super) async fn next(&mut self) -> Option<wire::EventServerMessage> {
        loop {
            let delivery = self.subscription.next().await?;
            if let Some(message) = adapter::wire_delivery(delivery) {
                return Some(message);
            }
        }
    }

    pub(super) fn repair(&self, cursor: wire::EventSnapshotCursor) -> wire::EventServerMessage {
        if cursor.sequence > self.events.latest_sequence() {
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
            sequence: self.events.latest_sequence(),
        }
    }
}

fn validate_programming_scope(
    session: &Session,
    filter: &wire::EventSubscriptionFilter,
    rate_limits: &[wire::EventRateLimit],
) -> Result<(), String> {
    for object in filter
        .objects
        .iter()
        .chain(rate_limits.iter().filter_map(|limit| limit.object.as_ref()))
    {
        validate_programming_object(session, object)?;
    }
    Ok(())
}

fn validate_programming_object(
    session: &Session,
    object: &wire::EventObject,
) -> Result<(), String> {
    if object.capability != wire::EventCapability::Programmer {
        return Ok(());
    }
    if object.id == "programming-lifecycle" {
        return Ok(());
    }
    let user = object
        .id
        .strip_prefix("programming-values:")
        .or_else(|| object.id.strip_prefix("programming-priority:"))
        .or_else(|| object.id.strip_prefix("programming-preload-values:"))
        .or_else(|| {
            object
                .id
                .strip_prefix("programming-preload-playback-queue:")
        })
        .or_else(|| object.id.strip_prefix("programming-capture-mode:"));
    let Some(user) = user else {
        return Err("unknown Programmer event object".into());
    };
    let user = Uuid::parse_str(user)
        .map_err(|_| "user-scoped Programmer event objects require a valid user UUID".to_owned())?;
    if user != session.user.id.0 {
        return Err("Programmer subscriptions may only address the authenticated user".into());
    }
    Ok(())
}

fn validate_cursor(events: &EventResource, cursor: Option<u64>) -> Result<(), String> {
    if cursor.is_some_and(|sequence| sequence > events.latest_sequence()) {
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

#[cfg(test)]
#[path = "tests/event_transport_tests.rs"]
mod tests;
