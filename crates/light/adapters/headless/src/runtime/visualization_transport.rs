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
use light_core::AttributeValue;
use light_wire::v2::{
    preload_values::{ProgrammingPreloadAttributeValue, ProgrammingPreloadColorXyz},
    visualization::{
        VISUALIZATION_MAX_RATE_HZ, VISUALIZATION_PROTOCOL_VERSION, VisualizationClientMessage,
        VisualizationLane, VisualizationLaneSnapshot, VisualizationServerMessage,
        VisualizationValue,
    },
};
use serde::Serialize;
use std::{collections::HashSet, time::Duration};

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v2/visualization/stream", get(stream))
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
        .protocols(["light.visualization.v1"])
        .on_upgrade(move |socket| handle_socket(socket, state, session))
        .into_response())
}

async fn handle_socket(mut socket: WebSocket, state: AppState, session: Session) {
    if !send(
        &mut socket,
        &VisualizationServerMessage::Hello {
            protocol_version: VISUALIZATION_PROTOCOL_VERSION,
            max_rate_hz: VISUALIZATION_MAX_RATE_HZ,
            lanes: vec![VisualizationLane::Normal, VisualizationLane::Preload],
        },
    )
    .await
    {
        return;
    }

    let mut subscribed = HashSet::new();
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut outgoing_sequence = 0_u64;
    let mut last_normal_source = 0_u64;
    let mut last_preload_source = 0_u64;

    loop {
        tokio::select! {
            message = socket.recv() => {
                let Some(Ok(message)) = message else { return };
                match message {
                    Message::Text(text) => {
                        let request = match serde_json::from_str::<VisualizationClientMessage>(&text) {
                            Ok(request) => request,
                            Err(error) => {
                                let response = VisualizationServerMessage::Error {
                                    code: "invalid_message".into(),
                                    message: error.to_string(),
                                };
                                if !send(&mut socket, &response).await { return; }
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
                                    if !send(&mut socket, &response).await { return; }
                                    continue;
                                }
                                subscribed.extend(lanes);
                                let millis = 1_000_u64 / u64::from(max_rate_hz);
                                interval = tokio::time::interval(Duration::from_millis(millis));
                                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                                last_normal_source = 0;
                                last_preload_source = 0;
                            }
                            VisualizationClientMessage::Unsubscribe { lanes } => {
                                for lane in lanes { subscribed.remove(&lane); }
                            }
                            VisualizationClientMessage::Resynchronize { lane } => match lane {
                                VisualizationLane::Normal => last_normal_source = 0,
                                VisualizationLane::Preload => last_preload_source = 0,
                            },
                        }
                    }
                    Message::Ping(payload) => {
                        if socket.send(Message::Pong(payload)).await.is_err() { return; }
                    }
                    Message::Close(_) => return,
                    _ => {}
                }
            }
            _ = interval.tick(), if !subscribed.is_empty() => {
                let Some(source) = state.output.latest_visualization_frame() else { continue };
                for lane in [VisualizationLane::Normal, VisualizationLane::Preload] {
                    if !subscribed.contains(&lane) { continue; }
                    let previous = match lane {
                        VisualizationLane::Normal => &mut last_normal_source,
                        VisualizationLane::Preload => &mut last_preload_source,
                    };
                    if *previous == source.sequence { continue; }
                    let snapshot = match lane_snapshot(&state, &session, lane, &source) {
                        Ok(snapshot) => snapshot,
                        Err(error) => {
                            let response = VisualizationServerMessage::Error {
                                code: "projection_failed".into(),
                                message: error.message,
                            };
                            if !send(&mut socket, &response).await { return; }
                            continue;
                        }
                    };
                    outgoing_sequence += 1;
                    let published_at = chrono::Utc::now().to_rfc3339();
                    let response = VisualizationServerMessage::Snapshot {
                        lane,
                        sequence: outgoing_sequence,
                        source_frame: source.sequence,
                        source_timestamp: chrono::DateTime::<chrono::Utc>::from(source.generated_at)
                            .to_rfc3339(),
                        published_at,
                        snapshot,
                    };
                    if !send(&mut socket, &response).await { return; }
                    *previous = source.sequence;
                }
            }
        }
    }
}

fn lane_snapshot(
    state: &AppState,
    session: &Session,
    lane: VisualizationLane,
    source: &super::visualization_frame::PublishedVisualizationFrame,
) -> Result<VisualizationLaneSnapshot, ApiError> {
    if lane == VisualizationLane::Preload {
        return serde_json::from_value(visualization_snapshot_for_session(state, session, true)?)
            .map_err(|error| ApiError::internal(error.to_string()));
    }
    let profile_output_values = state
        .output
        .profile_visualization_values(&source.values, source.options)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let mut snapshot: VisualizationLaneSnapshot =
        serde_json::from_value(visualization_snapshot_for_session(state, session, false)?)
            .map_err(|error| ApiError::internal(error.to_string()))?;
    snapshot.revision = source.show_revision;
    snapshot.generated_at = chrono::DateTime::<chrono::Utc>::from(source.generated_at).to_rfc3339();
    snapshot.grand_master = source.options.grand_master;
    snapshot.blackout = source.options.blackout;
    snapshot.values = ordered_values(&source.values);
    snapshot.profile_output_values = ordered_values(&profile_output_values);
    Ok(snapshot)
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

async fn send(socket: &mut WebSocket, message: &impl Serialize) -> bool {
    let Ok(json) = serde_json::to_string(message) else {
        return false;
    };
    tokio::time::timeout(
        Duration::from_millis(500),
        socket.send(Message::Text(json.into())),
    )
    .await
    .is_ok_and(|result| result.is_ok())
}
