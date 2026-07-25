use std::time::{Duration, UNIX_EPOCH};

use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use light_core::SessionId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use super::super::{ApiError, AppState, Session, authenticate};
use super::paths::{confined, mime_for, root};

const FILE_STREAM_TICKET_TTL: Duration = Duration::from_secs(8 * 60 * 60);

#[derive(Default, Deserialize)]
pub(super) struct ContentQuery {
    #[serde(default)]
    path: String,
    ticket: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct StreamTicketRequest {
    path: String,
}

#[derive(Serialize)]
pub(super) struct StreamTicketResponse {
    ticket: String,
    expires_in_millis: u128,
}

#[derive(Deserialize, Serialize)]
struct StreamTicketClaims {
    session_id: SessionId,
    root_id: String,
    path: String,
    expires_at_millis: u128,
    signature: String,
}

fn stream_ticket_signature(
    token: &str,
    session_id: SessionId,
    root_id: &str,
    path: &str,
    expires_at_millis: u128,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"tosklight-file-stream-v1\0");
    digest.update(session_id.0.as_bytes());
    digest.update(root_id.as_bytes());
    digest.update([0]);
    digest.update(path.as_bytes());
    digest.update(expires_at_millis.to_le_bytes());
    // A suffix secret avoids the length-extension weakness of a naive
    // secret-prefix SHA-256 construction. Active session and expiry checks
    // additionally bound every ticket.
    digest.update(token.as_bytes());
    URL_SAFE_NO_PAD.encode(digest.finalize())
}

fn now_millis() -> u128 {
    UNIX_EPOCH.elapsed().unwrap_or_default().as_millis()
}

fn validate_stream_ticket(
    state: &AppState,
    encoded: &str,
    root_id: &str,
    path: &str,
) -> Result<Session, ApiError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| ApiError::unauthorized("invalid file stream ticket"))?;
    let claims: StreamTicketClaims = serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::unauthorized("invalid file stream ticket"))?;
    validate_ticket_claims(&claims, root_id, path)?;
    let session = state
        .sessions
        .read()
        .get(&claims.session_id)
        .filter(|session| session.connected)
        .cloned()
        .ok_or_else(|| ApiError::unauthorized("file stream session is no longer active"))?;
    let expected = stream_ticket_signature(
        &session.token,
        session.id,
        root_id,
        path,
        claims.expires_at_millis,
    );
    if claims.signature != expected {
        return Err(ApiError::unauthorized("invalid file stream ticket"));
    }
    Ok(session)
}

fn validate_ticket_claims(
    claims: &StreamTicketClaims,
    root_id: &str,
    path: &str,
) -> Result<(), ApiError> {
    if claims.root_id != root_id || claims.path != path || claims.expires_at_millis < now_millis() {
        return Err(ApiError::unauthorized(
            "expired or mismatched file stream ticket",
        ));
    }
    Ok(())
}

pub(super) async fn stream_ticket(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<StreamTicketRequest>,
) -> Result<Json<StreamTicketResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let (root, _) = root(&state, &root_id)?;
    let path = confined(&root.path, &input.path, false)?;
    if !path.is_file() {
        return Err(ApiError::bad_request("path is not a file"));
    }
    let expires_at_millis = now_millis() + FILE_STREAM_TICKET_TTL.as_millis();
    let claims = StreamTicketClaims {
        signature: stream_ticket_signature(
            &session.token,
            session.id,
            &root_id,
            &input.path,
            expires_at_millis,
        ),
        session_id: session.id,
        root_id: root_id.clone(),
        path: input.path,
        expires_at_millis,
    };
    let ticket = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&claims).map_err(|error| ApiError::internal(error.to_string()))?,
    );
    Ok(Json(StreamTicketResponse {
        ticket,
        expires_in_millis: FILE_STREAM_TICKET_TTL.as_millis(),
    }))
}

pub(super) async fn content(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    Query(query): Query<ContentQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = if let Some(ticket) = query.ticket.as_deref() {
        validate_stream_ticket(&state, ticket, &root_id, &query.path)?
    } else {
        authenticate(&state, &headers)?
    };
    let (root, _) = root(&state, &root_id)?;
    let path = confined(&root.path, &query.path, false)?;
    let mut file = tokio::fs::File::open(&path).await.map_err(ApiError::io)?;
    let metadata = file.metadata().await.map_err(ApiError::io)?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request("path is not a file"));
    }
    let total = metadata.len();
    let (start, end, status) = parse_range(
        headers
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok()),
        total,
    )?;
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(ApiError::io)?;
    streaming_response(file, &path, start, end, total, status)
}

fn streaming_response(
    file: tokio::fs::File,
    path: &std::path::Path,
    start: u64,
    end: u64,
    total: u64,
    status: StatusCode,
) -> Result<Response, ApiError> {
    let length = end.saturating_sub(start);
    let stream = ReaderStream::new(file.take(length));
    let mut response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime_for(path))
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, length.to_string());
    if status == StatusCode::PARTIAL_CONTENT {
        response = response.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{}/{total}", end.saturating_sub(1)),
        );
    }
    response
        .body(Body::from_stream(stream))
        .map_err(|_| ApiError::internal("could not stream file"))
}

pub(super) fn parse_range(
    range: Option<&str>,
    total: u64,
) -> Result<(u64, u64, StatusCode), ApiError> {
    let Some(value) = range else {
        return Ok((0, total, StatusCode::OK));
    };
    let value = value
        .strip_prefix("bytes=")
        .ok_or_else(|| ApiError::bad_request("invalid range"))?;
    if value.contains(',') {
        return Err(ApiError::bad_request(
            "multiple byte ranges are not supported",
        ));
    }
    let (start, end) = value
        .split_once('-')
        .ok_or_else(|| ApiError::bad_request("invalid range"))?;
    if total == 0 {
        return Err(range_error("range is outside the empty file"));
    }
    let (start, end) = range_bounds(start, end, total)?;
    if start >= end || start >= total {
        return Err(range_error("range is outside the file"));
    }
    Ok((start, end, StatusCode::PARTIAL_CONTENT))
}

fn range_bounds(start: &str, end: &str, total: u64) -> Result<(u64, u64), ApiError> {
    if start.is_empty() {
        let suffix = end
            .parse::<u64>()
            .map_err(|_| ApiError::bad_request("invalid suffix range"))?;
        if suffix == 0 {
            return Err(range_error("suffix range must be greater than zero"));
        }
        return Ok((total.saturating_sub(suffix.min(total)), total));
    }
    let start = start
        .parse::<u64>()
        .map_err(|_| ApiError::bad_request("invalid range start"))?;
    let end = if end.is_empty() {
        total
    } else {
        end.parse::<u64>()
            .map_err(|_| ApiError::bad_request("invalid range end"))?
            .saturating_add(1)
            .min(total)
    };
    Ok((start, end))
}

fn range_error(message: impl Into<String>) -> ApiError {
    ApiError {
        status: StatusCode::RANGE_NOT_SATISFIABLE,
        message: message.into(),
    }
}
