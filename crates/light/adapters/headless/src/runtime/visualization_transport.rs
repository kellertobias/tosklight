//! Dedicated Stage visualization WebSocket. Projection and serialization run in this
//! visualization-owned task, never on the output scheduler.

use super::{
    ApiError, AppState, Session, ShowContext, event_transport, visualization_snapshot_for_session,
};
use axum::{
    Router,
    extract::{State, WebSocketUpgrade, ws::Message, ws::WebSocket},
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use light_core::AttributeValue;
use light_wire::v2::{
    preload_values::{ProgrammingPreloadAttributeValue, ProgrammingPreloadColorXyz},
    visualization::{
        VISUALIZATION_MAX_RATE_HZ, VISUALIZATION_PROTOCOL_VERSION, VisualizationClientMessage,
        VisualizationLane, VisualizationLaneSnapshot, VisualizationScope,
        VisualizationServerMessage, VisualizationValue,
    },
};
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex, Notify};
use tokio::time::Instant;

const VISUALIZATION_STREAM_ROUTE: &str = "/api/v2/visualization/stream";
const MAX_VISUALIZATION_CLIENT_MESSAGE_BYTES: usize = 16 * 1024;
const VISUALIZATION_SEND_TIMEOUT: Duration = Duration::from_millis(500);

struct LatestOutgoing {
    output: super::OutputResource,
    batches: LatestBatch,
}

#[derive(Default)]
struct LatestBatch {
    pending: Mutex<Option<Vec<Message>>>,
    notify: Notify,
    closed: AtomicBool,
}

impl LatestOutgoing {
    fn new(output: super::OutputResource) -> Self {
        Self {
            output,
            batches: LatestBatch::default(),
        }
    }

    async fn replace(&self, messages: Vec<Message>, replacement: Vec<Message>) -> bool {
        let Some(replaced_pending) = self.batches.replace(messages, replacement).await else {
            return false;
        };
        self.output
            .record_visualization_stream_queue_push(replaced_pending);
        true
    }

    async fn next(&self) -> Option<Vec<Message>> {
        let messages = self.batches.next().await;
        if messages.is_some() {
            self.output.record_visualization_stream_queue_take();
        }
        messages
    }

    async fn close(&self) {
        if self.batches.close().await {
            self.output.record_visualization_stream_queue_take();
        }
    }

    async fn has_pending(&self) -> bool {
        self.batches.pending.lock().await.is_some()
    }
}

impl LatestBatch {
    async fn replace(&self, messages: Vec<Message>, replacement: Vec<Message>) -> Option<bool> {
        if self.closed.load(Ordering::Acquire) {
            return None;
        }
        let mut pending = self.pending.lock().await;
        if self.closed.load(Ordering::Acquire) {
            return None;
        }
        let replaced_pending = pending.is_some();
        *pending = Some(if replaced_pending {
            replacement
        } else {
            messages
        });
        drop(pending);
        self.notify.notify_one();
        Some(replaced_pending)
    }

    async fn next(&self) -> Option<Vec<Message>> {
        loop {
            let notified = self.notify.notified();
            if let Some(messages) = self.pending.lock().await.take() {
                return Some(messages);
            }
            if self.closed.load(Ordering::Acquire) {
                return None;
            }
            notified.await;
        }
    }

    async fn close(&self) -> bool {
        self.closed.store(true, Ordering::Release);
        let had_pending = self.pending.lock().await.take().is_some();
        self.notify.notify_waiters();
        had_pending
    }
}

struct ClientRateThrottle {
    period: Duration,
    next_allowed: Instant,
    observed_sequence: u64,
    pending: Option<Arc<super::visualization_frame::PublishedVisualizationFrame>>,
}

impl ClientRateThrottle {
    fn new() -> Self {
        Self {
            period: super::visualization_frame::VISUALIZATION_PUBLICATION_INTERVAL,
            next_allowed: Instant::now(),
            observed_sequence: 0,
            pending: None,
        }
    }

    fn set_rate(&mut self, rate_hz: u8) {
        self.period = if rate_hz == VISUALIZATION_MAX_RATE_HZ {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(1.0 / f64::from(rate_hz))
        };
        self.next_allowed = Instant::now();
        self.observed_sequence = 0;
        self.pending = None;
    }

    fn observe(
        &mut self,
        source: Arc<super::visualization_frame::PublishedVisualizationFrame>,
        now: Instant,
    ) -> Option<Arc<super::visualization_frame::PublishedVisualizationFrame>> {
        self.observed_sequence = source.sequence;
        self.pending = Some(source);
        self.take_due(now)
    }

    fn take_due(
        &mut self,
        now: Instant,
    ) -> Option<Arc<super::visualization_frame::PublishedVisualizationFrame>> {
        (self.period.is_zero() || now >= self.next_allowed)
            .then(|| self.pending.take())
            .flatten()
    }

    fn mark_sent(&mut self, now: Instant) {
        if self.period.is_zero() {
            self.next_allowed = now;
            return;
        }
        self.next_allowed += self.period;
        while self.next_allowed <= now {
            self.next_allowed += self.period;
        }
    }

    fn queue_current(
        &mut self,
        source: Option<Arc<super::visualization_frame::PublishedVisualizationFrame>>,
    ) {
        self.pending = source;
    }
}

struct ClientPublicationState {
    outgoing_sequence: u64,
    last_normal_source: u64,
    last_preload_source: u64,
    last_normal_structure: Option<(VisualizationScope, u64)>,
    last_preload_structure: Option<(VisualizationScope, u64)>,
    last_heartbeat: Instant,
    force_snapshot: bool,
}

impl ClientPublicationState {
    fn new() -> Self {
        Self {
            outgoing_sequence: 0,
            last_normal_source: 0,
            last_preload_source: 0,
            last_normal_structure: None,
            last_preload_structure: None,
            last_heartbeat: Instant::now(),
            force_snapshot: false,
        }
    }

    fn reset_subscriptions(&mut self) {
        self.last_normal_source = 0;
        self.last_preload_source = 0;
        self.last_normal_structure = None;
        self.last_preload_structure = None;
    }

    async fn publish(
        &mut self,
        source: Arc<super::visualization_frame::PublishedVisualizationFrame>,
        subscribed: &SubscriptionClaims,
        state: &AppState,
        session: &Session,
        outgoing: &LatestOutgoing,
    ) -> bool {
        let mut sent_frame = false;
        let mut responses = Vec::new();
        let mut replacement_responses = Vec::new();
        for lane in [VisualizationLane::Normal, VisualizationLane::Preload] {
            if !subscribed.lanes.contains(&lane) {
                continue;
            }
            let previous = match lane {
                VisualizationLane::Normal => &mut self.last_normal_source,
                VisualizationLane::Preload => &mut self.last_preload_source,
            };
            let previous_structure = match lane {
                VisualizationLane::Normal => &mut self.last_normal_structure,
                VisualizationLane::Preload => &mut self.last_preload_structure,
            };
            if *previous == source.sequence {
                continue;
            }
            if *previous != 0
                && structural_scope_changed(*previous_structure, source.scope, source.show_revision)
            {
                let invalidation = VisualizationServerMessage::StructuralInvalidation {
                    scope: source.scope,
                    revision: source.show_revision,
                };
                responses.push(invalidation.clone());
                replacement_responses.push(invalidation);
                *previous = 0;
            }
            let key = subscribed.key(lane);
            let projection_state = state.clone();
            let projection_session = session.clone();
            let projection_source = Arc::clone(&source);
            let snapshot = match tokio::task::spawn_blocking(move || {
                projection_state
                    .output
                    .visualization_projection(key, &projection_source, || {
                        lane_snapshot(
                            &projection_state,
                            &projection_session,
                            lane,
                            &projection_source,
                        )
                    })
            })
            .await
            {
                Ok(Ok(snapshot)) => snapshot,
                Ok(Err(error)) => {
                    let response = VisualizationServerMessage::Error {
                        code: "projection_failed".into(),
                        message: error.message,
                    };
                    responses.push(response.clone());
                    replacement_responses.push(response);
                    self.force_snapshot = true;
                    continue;
                }
                Err(error) => {
                    let response = VisualizationServerMessage::Error {
                        code: "projection_failed".into(),
                        message: format!("visualization projection task failed: {error}"),
                    };
                    responses.push(response.clone());
                    replacement_responses.push(response);
                    self.force_snapshot = true;
                    continue;
                }
            };
            self.outgoing_sequence += 1;
            let published_at = chrono::Utc::now().to_rfc3339();
            let source_timestamp =
                chrono::DateTime::<chrono::Utc>::from(snapshot.source_generated_at).to_rfc3339();
            let snapshot_response = VisualizationServerMessage::Snapshot {
                lane,
                scope: source.scope,
                sequence: self.outgoing_sequence,
                source_frame: snapshot.lane_source_sequence,
                source_timestamp: source_timestamp.clone(),
                published_at: published_at.clone(),
                snapshot: snapshot.snapshot.as_ref().clone(),
            };
            let response = if !self.force_snapshot
                && *previous != 0
                && snapshot.previous_source_sequence == Some(*previous)
            {
                VisualizationServerMessage::Delta {
                    lane,
                    scope: source.scope,
                    sequence: self.outgoing_sequence,
                    source_frame: snapshot.lane_source_sequence,
                    source_timestamp,
                    published_at,
                    delta: snapshot.delta.as_ref().clone(),
                }
            } else {
                snapshot_response.clone()
            };
            responses.push(response);
            replacement_responses.push(snapshot_response);
            *previous = source.sequence;
            *previous_structure = Some((source.scope, source.show_revision));
            sent_frame = true;
            self.last_heartbeat = Instant::now();
        }
        if !responses.is_empty()
            && !enqueue(
                outgoing,
                &state.output,
                responses.iter(),
                replacement_responses.iter(),
            )
            .await
        {
            return false;
        }
        if sent_frame {
            self.force_snapshot = false;
        }
        true
    }

    async fn heartbeat(
        &mut self,
        source: &super::visualization_frame::PublishedVisualizationFrame,
        state: &AppState,
        outgoing: &LatestOutgoing,
    ) -> bool {
        if self.last_heartbeat.elapsed() < Duration::from_secs(2) || outgoing.has_pending().await {
            return true;
        }
        self.outgoing_sequence += 1;
        let heartbeat = VisualizationServerMessage::Heartbeat {
            scope: source.scope,
            sequence: self.outgoing_sequence,
            published_at: chrono::Utc::now().to_rfc3339(),
        };
        if !enqueue(outgoing, &state.output, [&heartbeat], [&heartbeat]).await {
            return false;
        }
        self.last_heartbeat = Instant::now();
        true
    }
}

struct SubscriptionClaims {
    output: super::OutputResource,
    session_id: uuid::Uuid,
    lanes: HashSet<VisualizationLane>,
}

impl SubscriptionClaims {
    fn new(output: super::OutputResource, session_id: uuid::Uuid) -> Self {
        Self {
            output,
            session_id,
            lanes: HashSet::new(),
        }
    }

    fn subscribe(&mut self, lanes: impl IntoIterator<Item = VisualizationLane>) {
        for lane in lanes {
            if self.lanes.insert(lane) {
                self.output.change_visualization_subscribers(lane, 1);
                self.output
                    .change_visualization_projection_claim(self.key(lane), 1);
            }
        }
    }

    fn unsubscribe(&mut self, lanes: impl IntoIterator<Item = VisualizationLane>) {
        for lane in lanes {
            if self.lanes.remove(&lane) {
                self.output.change_visualization_subscribers(lane, -1);
                self.output
                    .change_visualization_projection_claim(self.key(lane), -1);
            }
        }
    }

    fn key(
        &self,
        lane: VisualizationLane,
    ) -> super::visualization_frame::VisualizationProjectionKey {
        match lane {
            VisualizationLane::Normal => {
                super::visualization_frame::VisualizationProjectionKey::Normal
            }
            VisualizationLane::Preload => {
                super::visualization_frame::VisualizationProjectionKey::Preload(self.session_id)
            }
        }
    }
}

impl Drop for SubscriptionClaims {
    fn drop(&mut self) {
        for lane in self.lanes.drain() {
            self.output.change_visualization_subscribers(lane, -1);
            let key = match lane {
                VisualizationLane::Normal => {
                    super::visualization_frame::VisualizationProjectionKey::Normal
                }
                VisualizationLane::Preload => {
                    super::visualization_frame::VisualizationProjectionKey::Preload(self.session_id)
                }
            };
            self.output.change_visualization_projection_claim(key, -1);
        }
    }
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(VISUALIZATION_STREAM_ROUTE, get(stream))
}

async fn stream(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let session = event_transport::authenticate_protocols(&state, &headers)?;
    show.verify(&state)?;
    Ok(ws
        .max_message_size(MAX_VISUALIZATION_CLIENT_MESSAGE_BYTES)
        .protocols(["light.visualization.v1"])
        .on_upgrade(move |socket| handle_socket(socket, state, session))
        .into_response())
}

async fn handle_socket(mut socket: WebSocket, state: AppState, session: Session) {
    if !send_direct(
        &mut socket,
        &VisualizationServerMessage::Hello {
            protocol_version: VISUALIZATION_PROTOCOL_VERSION,
            max_rate_hz: VISUALIZATION_MAX_RATE_HZ,
            lanes: vec![VisualizationLane::Normal, VisualizationLane::Preload],
            scope: current_scope(&state),
        },
    )
    .await
    {
        return;
    }

    let (sender, receiver) = socket.split();
    let outgoing = Arc::new(LatestOutgoing::new(state.output.clone()));
    let writer_outgoing = Arc::clone(&outgoing);
    let writer = tokio::spawn(async move {
        run_writer(sender, writer_outgoing).await;
    });

    handle_socket_messages(receiver, &state, &session, &outgoing).await;
    outgoing.close().await;
    let _ = writer.await;
}

async fn handle_socket_messages(
    mut receiver: futures_util::stream::SplitStream<WebSocket>,
    state: &AppState,
    session: &Session,
    outgoing: &LatestOutgoing,
) {
    let mut subscribed = SubscriptionClaims::new(state.output.clone(), session.id.0);
    let mut throttle = ClientRateThrottle::new();
    let mut publication = ClientPublicationState::new();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(2));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    heartbeat.tick().await;

    loop {
        let mut ready_source = None;
        tokio::select! {
            message = receiver.next() => {
                let Some(Ok(message)) = message else { return };
                match message {
                    Message::Text(text) => {
                        let request = match decode_client_message(&text) {
                            Ok(request) => request,
                            Err(error) => {
                                let response = VisualizationServerMessage::Error {
                                    code: "invalid_message".into(),
                                    message: error.to_string(),
                                };
                                if !enqueue(outgoing, &state.output, [&response], [&response]).await { return; }
                                publication.force_snapshot = true;
                                continue;
                            }
                        };
                        match request {
                            VisualizationClientMessage::Subscribe { lanes, max_rate_hz } => {
                                if max_rate_hz == 0 || max_rate_hz > VISUALIZATION_MAX_RATE_HZ {
                                    let response = VisualizationServerMessage::Error {
                                        code: "invalid_rate".into(),
                                        message: format!(
                                            "max_rate_hz must be within 1-{VISUALIZATION_MAX_RATE_HZ}"
                                        ),
                                    };
                                    if !enqueue(outgoing, &state.output, [&response], [&response]).await { return; }
                                    publication.force_snapshot = true;
                                    continue;
                                }
                                subscribed.subscribe(lanes);
                                throttle.set_rate(max_rate_hz);
                                publication.reset_subscriptions();
                            }
                            VisualizationClientMessage::Unsubscribe { lanes } => {
                                subscribed.unsubscribe(lanes);
                                if subscribed.lanes.is_empty() {
                                    throttle.pending = None;
                                }
                            }
                            VisualizationClientMessage::Resynchronize { lane } => {
                                match lane {
                                    VisualizationLane::Normal => publication.last_normal_source = 0,
                                    VisualizationLane::Preload => publication.last_preload_source = 0,
                                }
                                publication.force_snapshot = true;
                                throttle.queue_current(state.output.sampled_visualization_frame());
                            }
                        }
                    }
                    Message::Ping(payload) => {
                        if !outgoing
                            .replace(
                                vec![Message::Pong(payload.clone())],
                                vec![Message::Pong(payload)],
                            )
                            .await
                        {
                            return;
                        }
                        publication.force_snapshot = true;
                    }
                    Message::Close(_) => return,
                    _ => {}
                }
            }
            source = state
                .output
                .wait_for_visualization_sample_after(throttle.observed_sequence),
                if !subscribed.lanes.is_empty() =>
            {
                ready_source = throttle.observe(source, Instant::now());
            }
            _ = tokio::time::sleep_until(throttle.next_allowed), if throttle.pending.is_some() => {
                ready_source = throttle.take_due(Instant::now());
            }
            _ = heartbeat.tick(), if !subscribed.lanes.is_empty() => {
                if let Some(source) = state.output.sampled_visualization_frame()
                    && !publication.heartbeat(&source, state, outgoing).await
                {
                    return;
                }
            }
        }
        if let Some(source) = ready_source {
            if !publication
                .publish(source, &subscribed, state, session, outgoing)
                .await
            {
                return;
            }
            throttle.mark_sent(Instant::now());
        }
    }
}

async fn run_writer(
    mut sender: futures_util::stream::SplitSink<WebSocket, Message>,
    outgoing: Arc<LatestOutgoing>,
) {
    while let Some(messages) = outgoing.next().await {
        let started = Instant::now();
        let succeeded = tokio::time::timeout(VISUALIZATION_SEND_TIMEOUT, async {
            for message in messages {
                sender.send(message).await?;
            }
            Ok::<(), axum::Error>(())
        })
        .await
        .is_ok_and(|result| result.is_ok());
        outgoing
            .output
            .record_visualization_stream_send(started.elapsed(), succeeded);
        if !succeeded {
            outgoing.close().await;
            return;
        }
    }
}

async fn enqueue<'a>(
    outgoing: &LatestOutgoing,
    output: &super::OutputResource,
    messages: impl IntoIterator<Item = &'a VisualizationServerMessage>,
    replacement: impl IntoIterator<Item = &'a VisualizationServerMessage>,
) -> bool {
    let messages = messages.into_iter().cloned().collect::<Vec<_>>();
    let replacement = replacement.into_iter().cloned().collect::<Vec<_>>();
    let serialized = tokio::task::spawn_blocking(move || {
        let started = Instant::now();
        let messages = messages
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>();
        let replacement = replacement
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>();
        (started.elapsed(), messages, replacement)
    })
    .await;
    let Ok((duration, Ok(messages), Ok(replacement))) = serialized else {
        return false;
    };
    let payload_bytes = messages.iter().map(String::len).sum::<usize>() as u64
        + replacement.iter().map(String::len).sum::<usize>() as u64;
    output.record_visualization_stream_serialization(duration, payload_bytes);
    outgoing
        .replace(
            messages
                .into_iter()
                .map(|message| Message::Text(message.into()))
                .collect(),
            replacement
                .into_iter()
                .map(|message| Message::Text(message.into()))
                .collect(),
        )
        .await
}

fn lane_snapshot(
    state: &AppState,
    session: &Session,
    lane: VisualizationLane,
    source: &super::visualization_frame::PublishedVisualizationFrame,
) -> Result<VisualizationLaneSnapshot, ApiError> {
    if lane == VisualizationLane::Preload {
        let mut snapshot: VisualizationLaneSnapshot =
            serde_json::from_value(visualization_snapshot_for_session(state, session, true)?)
                .map_err(|error| ApiError::internal(error.to_string()))?;
        snapshot.scope = source.scope;
        return Ok(snapshot);
    }
    let profile_output_values = state
        .output
        .profile_visualization_values(&source.values, source.options)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let mut snapshot: VisualizationLaneSnapshot =
        serde_json::from_value(visualization_snapshot_for_session(state, session, false)?)
            .map_err(|error| ApiError::internal(error.to_string()))?;
    snapshot.scope = source.scope;
    snapshot.revision = source.show_revision;
    snapshot.generated_at = chrono::DateTime::<chrono::Utc>::from(source.generated_at).to_rfc3339();
    snapshot.grand_master = source.options.grand_master;
    snapshot.blackout = source.options.blackout;
    snapshot.values = ordered_values(&source.values);
    snapshot.profile_output_values = ordered_values(&profile_output_values);
    Ok(snapshot)
}

fn current_scope(state: &AppState) -> VisualizationScope {
    VisualizationScope {
        show_id: state.active_show.current().map(|show| show.id.0),
    }
}

fn structural_scope_changed(
    previous: Option<(VisualizationScope, u64)>,
    scope: VisualizationScope,
    revision: u64,
) -> bool {
    previous.is_some_and(|previous| previous != (scope, revision))
}

fn decode_client_message(text: &str) -> Result<VisualizationClientMessage, serde_json::Error> {
    let raw = serde_json::from_str::<serde_json::Value>(text)?;
    let message = serde_json::from_value::<VisualizationClientMessage>(raw.clone())?;
    crate::tolerant_json::log_unknown_value_fields::<VisualizationClientMessage>(
        VISUALIZATION_STREAM_ROUTE,
        &raw,
    );
    Ok(message)
}

fn ordered_values(
    values: &std::collections::HashMap<
        (light_core::FixtureId, light_core::AttributeKey),
        AttributeValue,
    >,
) -> Vec<VisualizationValue> {
    let mut values = values
        .iter()
        .map(|((fixture_id, attribute), value)| VisualizationValue {
            fixture_id: fixture_id.0,
            attribute: attribute.0.clone(),
            value: wire_value(value),
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left.fixture_id
            .cmp(&right.fixture_id)
            .then_with(|| left.attribute.cmp(&right.attribute))
    });
    values
}

fn wire_value(value: &AttributeValue) -> ProgrammingPreloadAttributeValue {
    match value {
        AttributeValue::Normalized(value) => ProgrammingPreloadAttributeValue::Normalized(*value),
        AttributeValue::Spread(value) => ProgrammingPreloadAttributeValue::Spread(value.clone()),
        AttributeValue::Discrete(value) => {
            ProgrammingPreloadAttributeValue::Discrete(value.clone())
        }
        AttributeValue::ColorXyz(value) => {
            ProgrammingPreloadAttributeValue::ColorXyz(ProgrammingPreloadColorXyz {
                x: value.x,
                y: value.y,
                z: value.z,
            })
        }
        AttributeValue::RawDmx(value) => ProgrammingPreloadAttributeValue::RawDmx(*value),
        AttributeValue::RawDmxExact(value) => ProgrammingPreloadAttributeValue::RawDmxExact(*value),
    }
}

async fn send_direct(socket: &mut WebSocket, message: &impl serde::Serialize) -> bool {
    let Ok(json) = serde_json::to_string(message) else {
        return false;
    };
    tokio::time::timeout(
        VISUALIZATION_SEND_TIMEOUT,
        socket.send(Message::Text(json.into())),
    )
    .await
    .is_ok_and(|result| result.is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(
        sequence: u64,
    ) -> Arc<super::super::visualization_frame::PublishedVisualizationFrame> {
        Arc::new(
            super::super::visualization_frame::PublishedVisualizationFrame {
                sequence,
                generated_at: std::time::SystemTime::now(),
                scope: VisualizationScope { show_id: None },
                show_revision: sequence,
                options: light_engine::RenderOptions::default(),
                values: Arc::new(std::collections::HashMap::new()),
            },
        )
    }

    fn scope(id: &str) -> VisualizationScope {
        VisualizationScope {
            show_id: Some(uuid::Uuid::parse_str(id).unwrap()),
        }
    }

    #[test]
    fn same_revision_show_switch_requires_structural_invalidation() {
        let first = scope("11111111-1111-4111-8111-111111111111");
        let replacement = scope("22222222-2222-4222-8222-222222222222");

        assert!(!structural_scope_changed(Some((first, 7)), first, 7));
        assert!(structural_scope_changed(Some((first, 7)), replacement, 7));
        assert!(structural_scope_changed(Some((first, 7)), first, 8));
    }

    #[test]
    fn client_messages_tolerate_unknown_fields_through_the_logged_decoder() {
        let message = decode_client_message(
            r#"{
                "type":"subscribe",
                "lanes":["normal"],
                "max_rate_hz":10,
                "future":{"secret":"must-not-be-logged"}
            }"#,
        )
        .unwrap();

        assert_eq!(
            message,
            VisualizationClientMessage::Subscribe {
                lanes: vec![VisualizationLane::Normal],
                max_rate_hz: 10,
            }
        );
    }

    #[test]
    fn visualization_client_messages_have_a_small_transport_bound() {
        assert_eq!(MAX_VISUALIZATION_CLIENT_MESSAGE_BYTES, 16 * 1024);
    }

    #[tokio::test]
    async fn stalled_client_queue_retains_only_the_latest_complete_batch() {
        let queue = LatestBatch::default();
        assert_eq!(
            queue
                .replace(
                    vec![Message::Text("stale".into())],
                    vec![Message::Text("first replacement".into())],
                )
                .await,
            Some(false)
        );
        assert_eq!(
            queue
                .replace(
                    vec![Message::Text("incoherent delta".into())],
                    vec![Message::Text("latest snapshot".into())],
                )
                .await,
            Some(true)
        );

        let batch = queue.next().await.unwrap();
        assert_eq!(batch, vec![Message::Text("latest snapshot".into())]);
        assert!(queue.pending.lock().await.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn ten_hz_notification_uses_the_shared_sampler_as_its_only_throttle() {
        let mut throttle = ClientRateThrottle::new();
        throttle.set_rate(10);
        let first = throttle.observe(source(1), Instant::now()).unwrap();
        assert_eq!(first.sequence, 1);
        throttle.mark_sent(Instant::now());

        tokio::time::advance(Duration::from_millis(1)).await;
        let second = throttle.observe(source(2), Instant::now()).unwrap();
        assert_eq!(second.sequence, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn lower_rate_throttle_does_not_accumulate_projection_delay() {
        let mut throttle = ClientRateThrottle::new();
        throttle.set_rate(5);
        assert_eq!(
            throttle
                .observe(source(1), Instant::now())
                .unwrap()
                .sequence,
            1
        );
        throttle.mark_sent(Instant::now());

        tokio::time::advance(Duration::from_millis(210)).await;
        assert_eq!(
            throttle
                .observe(source(2), Instant::now())
                .unwrap()
                .sequence,
            2
        );
        throttle.mark_sent(Instant::now());

        tokio::time::advance(Duration::from_millis(189)).await;
        assert!(throttle.observe(source(3), Instant::now()).is_none());
        tokio::time::advance(Duration::from_millis(1)).await;
        assert_eq!(throttle.take_due(Instant::now()).unwrap().sequence, 3);
    }

    #[tokio::test(start_paused = true)]
    async fn lower_rate_throttle_keeps_the_newest_notified_source_until_due() {
        let mut throttle = ClientRateThrottle::new();
        throttle.set_rate(2);
        assert_eq!(
            throttle
                .observe(source(1), Instant::now())
                .unwrap()
                .sequence,
            1
        );
        throttle.mark_sent(Instant::now());

        tokio::time::advance(Duration::from_millis(100)).await;
        assert!(throttle.observe(source(2), Instant::now()).is_none());
        tokio::time::advance(Duration::from_millis(100)).await;
        assert!(throttle.observe(source(3), Instant::now()).is_none());
        tokio::time::advance(Duration::from_millis(299)).await;
        assert!(throttle.take_due(Instant::now()).is_none());
        tokio::time::advance(Duration::from_millis(1)).await;
        assert_eq!(throttle.take_due(Instant::now()).unwrap().sequence, 3);
    }

    #[tokio::test(start_paused = true)]
    async fn non_divisor_rate_never_exceeds_the_requested_maximum() {
        let mut throttle = ClientRateThrottle::new();
        throttle.set_rate(3);
        assert_eq!(
            throttle
                .observe(source(1), Instant::now())
                .unwrap()
                .sequence,
            1
        );
        throttle.mark_sent(Instant::now());

        tokio::time::advance(Duration::from_millis(333)).await;
        assert!(throttle.observe(source(2), Instant::now()).is_none());
        tokio::time::advance(Duration::from_micros(334)).await;
        assert_eq!(throttle.take_due(Instant::now()).unwrap().sequence, 2);
    }
}
