//! Dedicated Stage visualization WebSocket. Projection and serialization run in this
//! visualization-owned task, never on the output scheduler.

use super::{
    ApiError, AppState, Session, ShowContext, event_transport,
    visualization_snapshot_for_session_content_from_resolved,
};
use axum::{
    Router,
    extract::{State, WebSocketUpgrade, ws::Message, ws::WebSocket},
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use light_wire::v2::visualization::{
    VISUALIZATION_MAX_RATE_HZ, VISUALIZATION_PROTOCOL_VERSION, VisualizationClientMessage,
    VisualizationLane, VisualizationLaneSnapshot, VisualizationScope, VisualizationServerMessage,
    VisualizationStackEntryType,
};
use std::{
    collections::{HashSet, VecDeque},
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
// Keep backpressure on the server side, where the latest publication replaces
// stale work. Allowing several frames into WebKit's WebSocket event queue makes
// old visualization values contend with the operator UI before they can be
// acknowledged or discarded.
const MAX_UNACKNOWLEDGED_PUBLICATIONS: usize = 1;

struct LatestOutgoing {
    output: super::OutputResource,
    batches: LatestBatch,
    batched_messages: AtomicBool,
}

#[derive(Clone, Debug, PartialEq)]
enum VisualizationOutgoingMessage {
    Server(VisualizationServerMessage),
    Raw(Message),
}

#[derive(Default)]
struct LatestBatch {
    pending: Mutex<Option<Vec<VisualizationOutgoingMessage>>>,
    notify: Notify,
    closed: AtomicBool,
}

impl LatestOutgoing {
    fn new(output: super::OutputResource) -> Self {
        Self {
            output,
            batches: LatestBatch::default(),
            batched_messages: AtomicBool::new(false),
        }
    }

    async fn replace(
        &self,
        messages: Vec<VisualizationOutgoingMessage>,
        replacement: Vec<VisualizationOutgoingMessage>,
    ) -> bool {
        let Some(replaced_pending) = self.batches.replace(messages, replacement).await else {
            return false;
        };
        self.output
            .record_visualization_stream_queue_push(replaced_pending);
        true
    }

    async fn next(&self) -> Option<Vec<VisualizationOutgoingMessage>> {
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

    fn set_batched_messages(&self, enabled: bool) {
        self.batched_messages.store(enabled, Ordering::Release);
    }

    fn batched_messages(&self) -> bool {
        self.batched_messages.load(Ordering::Acquire)
    }
}

impl LatestBatch {
    async fn replace(
        &self,
        messages: Vec<VisualizationOutgoingMessage>,
        replacement: Vec<VisualizationOutgoingMessage>,
    ) -> Option<bool> {
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

    async fn next(&self) -> Option<Vec<VisualizationOutgoingMessage>> {
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
            period: Duration::from_secs_f64(1.0 / f64::from(VISUALIZATION_MAX_RATE_HZ)),
            next_allowed: Instant::now(),
            observed_sequence: 0,
            pending: None,
        }
    }

    fn set_rate(&mut self, rate_hz: u8) {
        self.period = Duration::from_secs_f64(1.0 / f64::from(rate_hz));
        self.next_allowed = Instant::now();
        self.observed_sequence = 0;
        self.pending = None;
    }

    #[cfg(test)]
    fn observe(
        &mut self,
        source: Arc<super::visualization_frame::PublishedVisualizationFrame>,
        now: Instant,
    ) -> Option<Arc<super::visualization_frame::PublishedVisualizationFrame>> {
        self.observed_sequence = source.sequence;
        self.pending = Some(source);
        self.take_due(now)
    }

    fn queue_observed(
        &mut self,
        source: Arc<super::visualization_frame::PublishedVisualizationFrame>,
    ) {
        self.observed_sequence = source.sequence;
        self.pending = Some(source);
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
    last_normal_snapshot: Option<Arc<VisualizationLaneSnapshot>>,
    last_preload_snapshot: Option<Arc<VisualizationLaneSnapshot>>,
    last_normal_structure: Option<(VisualizationScope, u64)>,
    last_preload_structure: Option<(VisualizationScope, u64)>,
    last_heartbeat: Instant,
    force_snapshot: bool,
    acknowledgements: bool,
    unacknowledged_sequences: VecDeque<u64>,
}

impl ClientPublicationState {
    fn new() -> Self {
        Self {
            outgoing_sequence: 0,
            last_normal_source: 0,
            last_preload_source: 0,
            last_normal_snapshot: None,
            last_preload_snapshot: None,
            last_normal_structure: None,
            last_preload_structure: None,
            last_heartbeat: Instant::now(),
            force_snapshot: false,
            acknowledgements: false,
            unacknowledged_sequences: VecDeque::new(),
        }
    }

    fn set_acknowledgements(&mut self, acknowledgements: bool) {
        self.acknowledgements = acknowledgements;
        if !acknowledgements {
            self.unacknowledged_sequences.clear();
        }
    }

    fn acknowledge(&mut self, sequence: u64) {
        while self
            .unacknowledged_sequences
            .front()
            .is_some_and(|awaiting| sequence >= *awaiting)
        {
            self.unacknowledged_sequences.pop_front();
        }
    }

    fn can_publish(&self) -> bool {
        !self.acknowledgements
            || self.unacknowledged_sequences.len() < MAX_UNACKNOWLEDGED_PUBLICATIONS
    }

    fn reset_subscriptions(&mut self) {
        self.last_normal_source = 0;
        self.last_preload_source = 0;
        self.last_normal_snapshot = None;
        self.last_preload_snapshot = None;
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
        let projected = project_subscribed_lanes(&source, subscribed, state, session).await;
        for (lane, projected) in projected {
            let previous = match lane {
                VisualizationLane::Normal => &mut self.last_normal_source,
                VisualizationLane::Preload => &mut self.last_preload_source,
            };
            let previous_snapshot = match lane {
                VisualizationLane::Normal => &mut self.last_normal_snapshot,
                VisualizationLane::Preload => &mut self.last_preload_snapshot,
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
                super::visualization_readouts::evict_on_invalidation(state, session, lane);
                *previous = 0;
                *previous_snapshot = None;
            }
            let snapshot = match projected {
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
            let stamp = super::visualization_readouts::lane_stamp(state, session, lane, &source);
            let snapshot_response = VisualizationServerMessage::Snapshot {
                lane,
                scope: source.scope,
                sequence: self.outgoing_sequence,
                source_frame: stamp.source_frame.unwrap_or(snapshot.lane_source_sequence),
                source_timestamp: source_timestamp.clone(),
                published_at: published_at.clone(),
                snapshot: snapshot.snapshot.as_ref().clone(),
                lease: stamp.lease,
                pending: stamp.pending.clone(),
            };
            let response = if !self.force_snapshot && *previous != 0 && previous_snapshot.is_some()
            {
                let mut delta = if snapshot.previous_source_sequence == Some(*previous) {
                    snapshot.delta.as_ref().clone()
                } else {
                    super::visualization_frame::lane_delta(
                        previous_snapshot.as_deref(),
                        snapshot.snapshot.as_ref(),
                    )
                };
                if !subscribed.sparse_dynamic_stack && delta.dynamic_stack.is_none() {
                    delta.dynamic_stack = Some(snapshot.snapshot.dynamic_stack.clone());
                }
                VisualizationServerMessage::Delta {
                    lane,
                    scope: source.scope,
                    sequence: self.outgoing_sequence,
                    source_frame: stamp.source_frame.unwrap_or(snapshot.lane_source_sequence),
                    source_timestamp,
                    published_at,
                    delta,
                    lease: stamp.lease,
                    pending: stamp.pending.clone(),
                }
            } else {
                snapshot_response.clone()
            };
            responses.push(response);
            replacement_responses.push(snapshot_response);
            let normal_lease = stamp.lease.filter(|_| lane == VisualizationLane::Normal);
            if let Some(readouts) = normal_lease.and_then(|lease| {
                super::visualization_readouts::readouts_message(
                    state,
                    subscribed.readouts.as_deref(),
                    &source,
                    lease,
                    &mut self.outgoing_sequence,
                )
            }) {
                responses.push(readouts.clone());
                replacement_responses.push(readouts);
            }
            *previous = source.sequence;
            *previous_snapshot = Some(Arc::clone(&snapshot.snapshot));
            *previous_structure = Some((source.scope, source.show_revision));
            sent_frame = true;
            self.last_heartbeat = Instant::now();
        }
        if !enqueue_publication_responses(
            outgoing,
            &state.output,
            &responses,
            &replacement_responses,
        )
        .await
        {
            return false;
        }
        if sent_frame {
            self.force_snapshot = false;
            if self.acknowledgements {
                self.unacknowledged_sequences
                    .push_back(self.outgoing_sequence);
            }
        }
        true
    }

    async fn heartbeat(
        &mut self,
        source: &super::visualization_frame::PublishedVisualizationFrame,
        state: &AppState,
        outgoing: &LatestOutgoing,
    ) -> bool {
        if !self.can_publish()
            || self.last_heartbeat.elapsed() < Duration::from_secs(2)
            || outgoing.has_pending().await
        {
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
        if self.acknowledgements {
            self.unacknowledged_sequences
                .push_back(self.outgoing_sequence);
        }
        self.last_heartbeat = Instant::now();
        true
    }
}

type ProjectedLane = (
    VisualizationLane,
    Result<
        Result<Arc<super::visualization_frame::ProjectedVisualizationFrame>, ApiError>,
        tokio::task::JoinError,
    >,
);

async fn project_subscribed_lanes(
    source: &Arc<super::visualization_frame::PublishedVisualizationFrame>,
    subscribed: &SubscriptionClaims,
    state: &AppState,
    session: &Session,
) -> Vec<ProjectedLane> {
    futures_util::future::join_all(
        [VisualizationLane::Normal, VisualizationLane::Preload]
            .into_iter()
            .filter(|lane| subscribed.lanes.contains(lane))
            .map(|lane| {
                let key = subscribed.key(lane);
                let projection_state = state.clone();
                let projection_session = session.clone();
                let projection_source = Arc::clone(source);
                async move {
                    let snapshot = tokio::task::spawn_blocking(move || {
                        projection_state.output.visualization_projection(
                            key,
                            &projection_source,
                            |refresh_dynamic_stack| {
                                claimed_lane_snapshot(key, refresh_dynamic_stack, |refresh| {
                                    lane_snapshot(
                                        &projection_state,
                                        &projection_session,
                                        lane,
                                        &projection_source,
                                        refresh,
                                    )
                                })
                            },
                        )
                    })
                    .await;
                    (lane, snapshot)
                }
            }),
    )
    .await
}

async fn enqueue_publication_responses(
    outgoing: &LatestOutgoing,
    output: &super::OutputResource,
    responses: &[VisualizationServerMessage],
    replacement_responses: &[VisualizationServerMessage],
) -> bool {
    responses.is_empty()
        || enqueue(
            outgoing,
            output,
            responses.iter(),
            replacement_responses.iter(),
        )
        .await
}

struct SubscriptionClaims {
    output: super::OutputResource,
    session_id: uuid::Uuid,
    lanes: HashSet<VisualizationLane>,
    include_dynamic_stack: bool,
    /// Every resolved attribute rather than only those the Stage draws (Preset pools).
    complete_values: bool,
    sparse_dynamic_stack: bool,
    /// TL-594 readout claim (Normal lane); replaced by every Subscribe.
    readouts: Option<Vec<light_core::FixtureId>>,
}

impl SubscriptionClaims {
    fn new(output: super::OutputResource, session_id: uuid::Uuid) -> Self {
        Self {
            output,
            session_id,
            lanes: HashSet::new(),
            include_dynamic_stack: false,
            complete_values: false,
            sparse_dynamic_stack: false,
            readouts: None,
        }
    }

    /// Both details select the shared projection, so a change moves every claimed lane's claim.
    fn set_projection_detail(&mut self, include_dynamic_stack: bool, complete_values: bool) {
        if self.include_dynamic_stack == include_dynamic_stack
            && self.complete_values == complete_values
        {
            return;
        }
        let lanes = self.lanes.iter().copied().collect::<Vec<_>>();
        for lane in &lanes {
            self.output
                .change_visualization_projection_claim(self.key(*lane), -1);
        }
        self.include_dynamic_stack = include_dynamic_stack;
        self.complete_values = complete_values;
        for lane in lanes {
            self.output
                .change_visualization_projection_claim(self.key(lane), 1);
        }
    }

    fn set_sparse_dynamic_stack(&mut self, sparse_dynamic_stack: bool) {
        self.sparse_dynamic_stack = sparse_dynamic_stack;
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
                super::visualization_frame::VisualizationProjectionKey::Normal {
                    include_dynamic_stack: self.include_dynamic_stack,
                    complete_values: self.complete_values,
                }
            }
            VisualizationLane::Preload => {
                super::visualization_frame::VisualizationProjectionKey::Preload {
                    session_id: self.session_id,
                    include_dynamic_stack: self.include_dynamic_stack,
                    complete_values: self.complete_values,
                }
            }
        }
    }
}

impl Drop for SubscriptionClaims {
    fn drop(&mut self) {
        let lanes = self.lanes.drain().collect::<Vec<_>>();
        for lane in lanes {
            self.output.change_visualization_subscribers(lane, -1);
            let key = self.key(lane);
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
                            VisualizationClientMessage::Subscribe {
                                lanes,
                                max_rate_hz,
                                acknowledgements,
                                include_dynamic_stack,
                                complete_values,
                                sparse_dynamic_stack,
                                batched_messages,
                                readouts,
                            } => {
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
                                subscribed
                                    .set_projection_detail(include_dynamic_stack, complete_values);
                                subscribed.set_sparse_dynamic_stack(sparse_dynamic_stack);
                                subscribed.readouts =
                                    super::visualization_readouts::claimed_owners(readouts);
                                outgoing.set_batched_messages(batched_messages);
                                subscribed.subscribe(lanes);
                                publication.set_acknowledgements(acknowledgements);
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
                                    VisualizationLane::Normal => {
                                        publication.last_normal_source = 0;
                                        publication.last_normal_snapshot = None;
                                    }
                                    VisualizationLane::Preload => {
                                        publication.last_preload_source = 0;
                                        publication.last_preload_snapshot = None;
                                    }
                                }
                                publication.force_snapshot = true;
                                throttle.queue_current(state.output.sampled_visualization_frame());
                            }
                            VisualizationClientMessage::Acknowledge { sequence } => {
                                publication.acknowledge(sequence);
                                if publication.can_publish() {
                                    ready_source = throttle.take_due(Instant::now());
                                }
                            }
                        }
                    }
                    Message::Ping(payload) => {
                        if !outgoing
                            .replace(
                                vec![VisualizationOutgoingMessage::Raw(Message::Pong(
                                    payload.clone(),
                                ))],
                                vec![VisualizationOutgoingMessage::Raw(Message::Pong(payload))],
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
                throttle.queue_observed(source);
                if publication.can_publish() {
                    ready_source = throttle.take_due(Instant::now());
                }
            }
            _ = tokio::time::sleep_until(throttle.next_allowed),
                if throttle.pending.is_some() && publication.can_publish() =>
            {
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
        let batched_messages = outgoing.batched_messages();
        let serialized = tokio::task::spawn_blocking(move || {
            let started = Instant::now();
            let messages = serialize_outgoing_messages(messages, batched_messages);
            (started.elapsed(), messages)
        })
        .await;
        let Ok((serialization_duration, Ok(messages))) = serialized else {
            outgoing.close().await;
            return;
        };
        let payload_bytes = messages
            .iter()
            .map(|message| match message {
                Message::Text(message) => message.len(),
                Message::Binary(message) | Message::Ping(message) | Message::Pong(message) => {
                    message.len()
                }
                Message::Close(_) => 0,
            })
            .sum::<usize>() as u64;
        outgoing
            .output
            .record_visualization_stream_serialization(serialization_duration, payload_bytes);
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

fn serialize_outgoing_messages(
    messages: Vec<VisualizationOutgoingMessage>,
    batched_messages: bool,
) -> Result<Vec<Message>, serde_json::Error> {
    if batched_messages
        && messages.len() > 1
        && messages
            .iter()
            .all(|message| matches!(message, VisualizationOutgoingMessage::Server(_)))
    {
        let messages = messages
            .into_iter()
            .filter_map(|message| match message {
                VisualizationOutgoingMessage::Server(message) => Some(message),
                VisualizationOutgoingMessage::Raw(_) => None,
            })
            .collect::<Vec<_>>();
        return serde_json::to_string(&messages)
            .map(|messages| vec![Message::Text(messages.into())]);
    }
    messages
        .into_iter()
        .map(|message| match message {
            VisualizationOutgoingMessage::Server(message) => {
                serde_json::to_string(&message).map(|message| Message::Text(message.into()))
            }
            VisualizationOutgoingMessage::Raw(message) => Ok(message),
        })
        .collect()
}

async fn enqueue<'a>(
    outgoing: &LatestOutgoing,
    _output: &super::OutputResource,
    messages: impl IntoIterator<Item = &'a VisualizationServerMessage>,
    replacement: impl IntoIterator<Item = &'a VisualizationServerMessage>,
) -> bool {
    let messages = messages
        .into_iter()
        .cloned()
        .map(VisualizationOutgoingMessage::Server)
        .collect::<Vec<_>>();
    let replacement = replacement
        .into_iter()
        .cloned()
        .map(VisualizationOutgoingMessage::Server)
        .collect::<Vec<_>>();
    outgoing.replace(messages, replacement).await
}

fn lane_snapshot(
    state: &AppState,
    session: &Session,
    lane: VisualizationLane,
    source: &super::visualization_frame::PublishedVisualizationFrame,
    include_dynamic_stack: bool,
) -> Result<VisualizationLaneSnapshot, ApiError> {
    let preload = lane == VisualizationLane::Preload;
    // TL-594: gated Preload content and its message stamp come from ONE publication read.
    let content = match preload
        .then(|| super::pending_preload_readers::published_preload(state))
        .flatten()
    {
        Some(published) => {
            super::visualization_readouts::gated_preload_content(state, session, source, published)
        }
        None => visualization_snapshot_for_session_content_from_resolved(
            state,
            session,
            preload,
            include_dynamic_stack,
            true,
            Some(source),
        )?,
    };
    let mut snapshot: VisualizationLaneSnapshot =
        serde_json::from_value(content).map_err(|error| ApiError::internal(error.to_string()))?;
    if !preload {
        snapshot.scope = source.scope;
        snapshot.revision = source.show_revision;
        snapshot.generated_at =
            chrono::DateTime::<chrono::Utc>::from(source.generated_at).to_rfc3339();
        snapshot.grand_master = source.options.grand_master;
        snapshot.blackout = source.options.blackout;
    }
    Ok(snapshot)
}

/// One lane publication for one claim. `refresh_dynamic_stack` only decides whether this frame
/// re-evaluates the Dynamic stack (the projection reuses the previous stack otherwise); what the
/// client receives is decided by its claim alone, so values never come and go with the refresh
/// cadence.
fn claimed_lane_snapshot(
    claim: super::visualization_frame::VisualizationProjectionKey,
    refresh_dynamic_stack: bool,
    build: impl FnOnce(bool) -> Result<VisualizationLaneSnapshot, ApiError>,
) -> Result<VisualizationLaneSnapshot, ApiError> {
    let mut snapshot = build(refresh_dynamic_stack)?;
    trim_lane_snapshot(&mut snapshot, claim);
    Ok(snapshot)
}

/// What one lane publication carries for its claim.
fn trim_lane_snapshot(
    snapshot: &mut VisualizationLaneSnapshot,
    claim: super::visualization_frame::VisualizationProjectionKey,
) {
    if claim.includes_dynamic_stack() {
        // Fixture Sheet consumes Dynamic identity and state; live sampled and
        // resolved values belong to the DMX/output view. Do not make every
        // Stage publication carry ordinary static entries or duplicate values.
        snapshot
            .dynamic_stack
            .retain(|entry| entry.entry_type != VisualizationStackEntryType::OrdinaryStatic);
        for entry in &mut snapshot.dynamic_stack {
            entry.value = None;
            entry.resolved_value = None;
            entry.activation_mix = None;
        }
    }
    // A Preset pool compares every stored attribute, including the semantic Position owner and
    // Beam attributes; the Stage draws only these. A Dynamic-stack claim alone does not widen it.
    if !claim.complete_values() {
        snapshot
            .values
            .retain(|entry| stage_visualization_attribute(&entry.attribute));
    }
    snapshot
        .profile_output_values
        .retain(|entry| stage_visualization_attribute(&entry.attribute));
}

fn stage_visualization_attribute(attribute: &str) -> bool {
    matches!(
        attribute,
        "intensity" | "pan" | "tilt" | "zoom" | "focus" | "beam.zoom" | "beam.focus" | "gobo"
    ) || attribute == "color"
        || attribute.starts_with("color.")
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
mod tests;
