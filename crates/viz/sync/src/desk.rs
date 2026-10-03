//! The desk side of synchronization: Control's HTTP routes and its event socket.

use futures_util::{SinkExt, StreamExt};
use light_wire::v2::show_sync::{
    ShowSyncCommit, ShowSyncErrorResponse, ShowSyncGap, ShowSyncTransactionOutcome,
    ShowSyncTransactionRequest,
};
use serde_json::Value;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use uuid::Uuid;

const SHOW_HEADER: &str = "x-tosk-show";

/// Why a desk request did not produce an answer.
#[derive(Clone, Debug, PartialEq)]
pub enum DeskError {
    /// The desk could not be reached, or did not answer in time. Retry the same request.
    Unreachable(String),
    /// The desk answered with a status this client does not act on.
    Http { status: u16, body: String },
}

impl std::fmt::Display for DeskError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(reason) => write!(formatter, "Control is unreachable: {reason}"),
            Self::Http { status, body } => write!(formatter, "Control answered {status}: {body}"),
        }
    }
}

fn unreachable(error: impl std::fmt::Display) -> DeskError {
    DeskError::Unreachable(error.to_string())
}

/// What a desk says about itself and its open show.
#[derive(Clone, Debug, PartialEq)]
pub struct Readiness {
    pub desk_identity: Option<Uuid>,
    pub active_show: Option<Uuid>,
}

/// The desk's answer to one sync transaction.
#[derive(Clone, Debug)]
pub enum TransactionReply {
    Outcome(Box<ShowSyncTransactionOutcome>),
    Refused {
        status: u16,
        error: ShowSyncErrorResponse,
    },
}

/// One message from the sync feed this client acts on.
#[derive(Clone, Debug)]
pub enum FeedMessage {
    Committed(Box<ShowSyncCommit>),
    Gap(ShowSyncGap),
    /// The event stream lost events the client never saw; snapshots must be re-read.
    StreamGap,
    /// Anything else on the socket, including heartbeats.
    Other,
}

pub struct DeskClient {
    base: String,
    http: reqwest::Client,
    session: Mutex<Option<(String, String)>>,
}

impl DeskClient {
    pub fn new(base: impl Into<String>) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            base: base.into().trim_end_matches('/').to_owned(),
            http,
            session: Mutex::new(None),
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// The editing session's token, opening one when there is none.
    async fn token(&self) -> Result<String, DeskError> {
        let mut session = self.session.lock().await;
        if let Some((token, _)) = session.as_ref() {
            return Ok(token.clone());
        }
        let credentials: Value = self
            .http
            .post(format!("{}/api/v2/sessions", self.base))
            .json(&serde_json::json!({"role": "operator"}))
            .send()
            .await
            .map_err(unreachable)?
            .error_for_status()
            .map_err(unreachable)?
            .json()
            .await
            .map_err(unreachable)?;
        let token = credentials["token"]
            .as_str()
            .ok_or_else(|| unreachable("the desk returned no session token"))?
            .to_owned();
        let id = credentials["session_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        *session = Some((token.clone(), id));
        Ok(token)
    }

    /// Forgets a session the desk no longer knows, such as after a desk restart.
    async fn forget_session(&self) {
        *self.session.lock().await = None;
    }

    /// Closes the editing session, if one is open.
    pub async fn close(&self) {
        let session = self.session.lock().await.take();
        if let Some((token, id)) = session {
            let _ = self
                .http
                .delete(format!("{}/api/v2/sessions/{id}", self.base))
                .bearer_auth(token)
                .send()
                .await;
        }
    }

    async fn get(&self, path: &str, show: Option<Uuid>) -> Result<reqwest::Response, DeskError> {
        for attempt in 0..2 {
            let token = self.token().await?;
            let mut request = self
                .http
                .get(format!("{}{path}", self.base))
                .bearer_auth(token);
            if let Some(show) = show {
                request = request.header(SHOW_HEADER, show.to_string());
            }
            let response = request.send().await.map_err(unreachable)?;
            if response.status() == reqwest::StatusCode::UNAUTHORIZED && attempt == 0 {
                self.forget_session().await;
                continue;
            }
            return Ok(response);
        }
        Err(unreachable("the desk refused every session"))
    }

    pub async fn readiness(&self) -> Result<Readiness, DeskError> {
        let readiness: Value = self
            .http
            .get(format!("{}/api/v2/readiness", self.base))
            .send()
            .await
            .map_err(unreachable)?
            .json()
            .await
            .map_err(unreachable)?;
        let uuid = |field: &str| {
            readiness[field]
                .as_str()
                .and_then(|value| Uuid::parse_str(value).ok())
        };
        Ok(Readiness {
            desk_identity: uuid("desk_identity"),
            active_show: uuid("active_show"),
        })
    }

    /// The active show's revision, read through the cheapest snapshot route. `None` when the
    /// show is no longer the active one.
    pub async fn show_revision(&self, show: Uuid) -> Result<Option<u64>, DeskError> {
        let response = self
            .get("/api/v2/objects/cad_drawing_tree", Some(show))
            .await?;
        if response.status() == reqwest::StatusCode::CONFLICT {
            return Ok(None);
        }
        let body = checked(response).await?;
        Ok(body["show_revision"].as_u64())
    }

    /// One stored object as the desk holds it now, with its revision.
    pub async fn object(
        &self,
        show: Uuid,
        kind: &str,
        id: &str,
    ) -> Result<Option<(u64, Value)>, DeskError> {
        let response = self
            .get(&format!("/api/v2/objects/{kind}/{id}"), Some(show))
            .await?;
        let body = checked(response).await?;
        let object = &body["object"];
        if object.is_null() {
            return Ok(None);
        }
        Ok(Some((
            object["revision"].as_u64().unwrap_or_default(),
            object["body"].clone(),
        )))
    }

    /// The whole show file, for a snapshot re-read after a gap.
    pub async fn download(&self, show: Uuid) -> Result<Vec<u8>, DeskError> {
        let response = self
            .get(&format!("/api/v2/shows/{show}/download"), None)
            .await?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(DeskError::Http { status, body });
        }
        Ok(response.bytes().await.map_err(unreachable)?.to_vec())
    }

    pub async fn transaction(
        &self,
        request: &ShowSyncTransactionRequest,
    ) -> Result<TransactionReply, DeskError> {
        for attempt in 0..2 {
            let token = self.token().await?;
            let response = self
                .http
                .post(format!("{}/api/v2/show-sync/transactions", self.base))
                .bearer_auth(token)
                .header(SHOW_HEADER, request.show_id.to_string())
                .json(request)
                .send()
                .await
                .map_err(unreachable)?;
            let status = response.status();
            if status == reqwest::StatusCode::UNAUTHORIZED && attempt == 0 {
                self.forget_session().await;
                continue;
            }
            let body = response.text().await.map_err(unreachable)?;
            if status.is_success() {
                let outcome = serde_json::from_str(&body).map_err(|error| DeskError::Http {
                    status: status.as_u16(),
                    body: format!("unreadable outcome: {error}"),
                })?;
                return Ok(TransactionReply::Outcome(Box::new(outcome)));
            }
            return match serde_json::from_str::<ShowSyncErrorResponse>(&body) {
                Ok(error) => Ok(TransactionReply::Refused {
                    status: status.as_u16(),
                    error,
                }),
                Err(_) if status.is_server_error() => Err(DeskError::Unreachable(body)),
                Err(_) => Err(DeskError::Http {
                    status: status.as_u16(),
                    body,
                }),
            };
        }
        Err(unreachable("the desk refused every session"))
    }

    /// Opens the event socket subscribed to the sync feed and waits for the desk's `ready`.
    pub async fn subscribe(&self) -> Result<FeedSocket, DeskError> {
        let token = self.token().await?;
        let url = self
            .base
            .replacen("http://", "ws://", 1)
            .replacen("https://", "wss://", 1);
        let mut request = format!("{url}/api/v2/events")
            .into_client_request()
            .map_err(unreachable)?;
        let protocols = format!("light.events.v2, light.v2, light.token.{token}");
        request.headers_mut().insert(
            "sec-websocket-protocol",
            protocols.parse().map_err(unreachable)?,
        );
        let (mut socket, _) = tokio::time::timeout(
            Duration::from_secs(5),
            tokio_tungstenite::connect_async(request),
        )
        .await
        .map_err(|_| unreachable("the event socket did not open in time"))?
        .map_err(unreachable)?;
        let subscribe = serde_json::json!({
            "type": "subscribe",
            "filter": {"capabilities": ["show"], "classes": ["projection"], "topics": ["show_sync"]},
            "capacity": 1024,
            "rate_limits": [],
        });
        socket
            .send(Message::Text(subscribe.to_string().into()))
            .await
            .map_err(unreachable)?;
        loop {
            let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
                .await
                .map_err(|_| unreachable("the event socket did not confirm the subscription"))?;
            let Some(Ok(Message::Text(text))) = message else {
                match message {
                    Some(Ok(_)) => continue,
                    _ => return Err(unreachable("the event socket closed")),
                }
            };
            let value: Value = serde_json::from_str(&text).map_err(unreachable)?;
            match value["type"].as_str() {
                Some("ready") => return Ok(FeedSocket { socket }),
                Some("error") => {
                    return Err(DeskError::Http {
                        status: 400,
                        body: value["error"].to_string(),
                    });
                }
                _ => {}
            }
        }
    }
}

async fn checked(response: reqwest::Response) -> Result<Value, DeskError> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(if status.is_server_error() {
            DeskError::Unreachable(body)
        } else {
            DeskError::Http {
                status: status.as_u16(),
                body,
            }
        });
    }
    response.json().await.map_err(unreachable)
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// The subscribed event socket.
pub struct FeedSocket {
    socket: Socket,
}

impl FeedSocket {
    /// The next message, or `None` when the socket closed.
    pub async fn next(&mut self) -> Option<FeedMessage> {
        loop {
            let message = self.socket.next().await?;
            let text = match message {
                Ok(Message::Text(text)) => text,
                Ok(Message::Ping(payload)) => {
                    let _ = self.socket.send(Message::Pong(payload)).await;
                    continue;
                }
                Ok(Message::Close(_)) | Err(_) => return None,
                Ok(_) => continue,
            };
            return Some(parse_feed(&text));
        }
    }
}

/// Reads one server frame, decoding only the payloads the sync feed defines. Every other event
/// on the subscribed stream is `Other`, so a payload this build does not know never breaks it.
pub fn parse_feed(text: &str) -> FeedMessage {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return FeedMessage::Other;
    };
    match value["type"].as_str() {
        Some("gap") => FeedMessage::StreamGap,
        Some("event") => {
            let payload = &value["event"]["payload"];
            match payload["type"].as_str() {
                Some("show_sync_committed") => serde_json::from_value(payload["change"].clone())
                    .map_or(FeedMessage::StreamGap, |change| {
                        FeedMessage::Committed(Box::new(change))
                    }),
                Some("show_sync_gap") => serde_json::from_value(payload["gap"].clone())
                    .map_or(FeedMessage::StreamGap, FeedMessage::Gap),
                _ => FeedMessage::Other,
            }
        }
        _ => FeedMessage::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_feed_decodes_sync_payloads_and_ignores_everything_else() {
        let show = Uuid::new_v4();
        let committed = serde_json::json!({"type": "event", "event": {"payload": {
            "type": "show_sync_committed",
            "change": {"show_id": show, "show_revision": 5, "previous_show_revision": 4,
                "patch_revision": 1, "request_id": null, "association_id": null, "objects": [],
                "metadata": [], "profile_revisions": [], "desk_only_changes": 0}
        }}});
        assert!(matches!(
            parse_feed(&committed.to_string()),
            FeedMessage::Committed(change) if change.show_revision == 5
        ));
        let gap = serde_json::json!({"type": "event", "event": {"payload": {
            "type": "show_sync_gap",
            "gap": {"show_id": show, "show_revision": 9, "reason": "show_replaced"}
        }}});
        assert!(
            matches!(parse_feed(&gap.to_string()), FeedMessage::Gap(gap) if gap.show_revision == 9)
        );
        assert!(matches!(
            parse_feed(
                r#"{"type":"gap","gap":{"after_sequence":1,"oldest_available":4,"latest_sequence":9}}"#
            ),
            FeedMessage::StreamGap
        ));
        let other =
            r#"{"type":"event","event":{"payload":{"type":"a_payload_from_a_newer_desk"}}}"#;
        assert!(matches!(parse_feed(other), FeedMessage::Other));
    }
}
