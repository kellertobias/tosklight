//! Discovered show browsing and folder saves through scoped, temporary desk sessions.
use super::{ActiveShowRepository, ApiError, AppState, authenticate};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use base64::Engine;
use light_discovery::{Peer, Role};
use light_wire::v2::{
    discovery::DiscoveredRole,
    show_library::{
        MvrExportSummary, ShowLibraryActionOutcome, ShowLibraryActionResult, ShowLibrarySnapshot,
    },
    show_network as wire,
};
use serde::Deserialize;
use std::time::Duration;
use uuid::Uuid;

// This separate gate allows a discovery-listed loopback desk to accept its forwarded
// local save while the caller still serializes remote-save request identity.
pub(super) static REMOTE_SAVE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/shows/network", get(catalog))
        .route(
            "/api/v2/shows/network/{instance}/folders",
            get(save_folders),
        )
}

async fn catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::NetworkShowCatalog>, ApiError> {
    authenticate(&state, &headers)?;
    let client = client(Duration::from_secs(8)).map_err(ApiError::internal)?;
    let mut pending = tokio::task::JoinSet::new();
    let own_instance = state.discovery.own_instance();
    for peer in remote_peers(state.discovery.peers(), own_instance.as_deref()) {
        let client = client.clone();
        pending.spawn(async move { project_peer(&client, peer).await });
    }
    let mut peers = Vec::new();
    while let Some(result) = pending.join_next().await {
        peers.push(
            result.map_err(|error| ApiError::internal(format!("network catalog task: {error}")))?,
        );
    }
    peers.sort_by(|a, b| a.name.cmp(&b.name).then(a.instance.cmp(&b.instance)));
    Ok(Json(wire::NetworkShowCatalog {
        browsing: state.discovery.is_browsing(),
        peers,
    }))
}

/// Filter by advertised identity, so another application on this machine remains available.
fn remote_peers(peers: Vec<Peer>, own_instance: Option<&str>) -> Vec<Peer> {
    peers
        .into_iter()
        .filter(|peer| !own_instance.is_some_and(|own| peer.instance.eq_ignore_ascii_case(own)))
        .collect()
}

fn client(timeout: Duration) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(timeout)
        // A discovered peer cannot redirect the server or its observer credential elsewhere.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("network show client: {error}"))
}

async fn project_peer(client: &reqwest::Client, peer: Peer) -> wire::NetworkShowPeer {
    let mut projected = wire::NetworkShowPeer {
        instance: peer.instance.clone(),
        name: peer.name.clone(),
        address: peer.address().to_owned(),
        role: if peer.role == Role::Desk {
            DiscoveredRole::Desk
        } else {
            DiscoveredRole::Editor
        },
        shows: Vec::new(),
        error: None,
    };
    if peer.role == Role::Editor {
        if let Some(name) = peer.show {
            projected.shows.push(wire::NetworkShow {
                id: None,
                name,
                updated_at: None,
                revisions: Vec::new(),
            });
        }
        return projected;
    }
    let mut failure = "No reachable address".to_owned();
    for base in peer.base_urls() {
        match desk_catalog(client, &base).await {
            Ok(snapshot) => {
                projected.shows = snapshot
                    .shows
                    .into_iter()
                    .map(|entry| wire::NetworkShow {
                        id: Some(entry.show.id),
                        name: entry.show.name,
                        updated_at: Some(entry.show.updated_at),
                        revisions: entry.revisions,
                    })
                    .collect();
                return projected;
            }
            Err(error) => failure = error,
        }
    }
    projected.error = Some(failure);
    projected
}

#[derive(Deserialize)]
struct ObserverSession {
    session_id: Uuid,
    token: String,
}

async fn observer(client: &reqwest::Client, base: &str) -> Result<ObserverSession, String> {
    client
        .post(format!("{base}/api/v2/sessions"))
        .json(&serde_json::json!({"role":"visualizer"}))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())
}

async fn close_observer(
    client: &reqwest::Client,
    base: &str,
    session: &ObserverSession,
) -> Result<(), String> {
    client
        .delete(format!("{base}/api/v2/sessions/{}", session.session_id))
        .bearer_auth(&session.token)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    Ok(())
}

async fn desk_catalog(client: &reqwest::Client, base: &str) -> Result<ShowLibrarySnapshot, String> {
    let session = observer(client, base).await?;
    let result = async {
        client
            .get(format!("{base}/api/v2/shows"))
            .bearer_auth(&session.token)
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json()
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    let cleanup = close_observer(client, base, &session).await;
    match result {
        Err(error) => Err(error),
        Ok(snapshot) => cleanup.map(|()| snapshot),
    }
}

/// Fetch only a library UUID from the selected discovery-authorized desk; never accept a host body.
pub(super) async fn fetch_desk_document(
    state: &AppState,
    instance: &str,
    show_id: Uuid,
    revision: Option<u64>,
) -> Result<(String, Vec<u8>), ApiError> {
    let peer = state
        .discovery
        .peer(instance)
        .ok_or_else(|| ApiError::not_found("network desk"))?;
    if peer.role != Role::Desk {
        return Err(ApiError::bad_request("the selected source is not a desk"));
    }
    if show_id.is_nil() {
        return Err(ApiError::bad_request("show_id must not be nil"));
    }
    let client = client(Duration::from_secs(60)).map_err(ApiError::internal)?;
    let mut failure = "No reachable address".to_owned();
    for base in peer.base_urls() {
        match desk_document(&client, &base, show_id, revision).await {
            Ok(document) => return Ok(document),
            Err(error) => failure = error,
        }
    }
    Err(ApiError::bad_gateway(format!("{}: {failure}", peer.name)))
}

async fn desk_document(
    client: &reqwest::Client,
    base: &str,
    show_id: Uuid,
    revision: Option<u64>,
) -> Result<(String, Vec<u8>), String> {
    let session = observer(client, base).await?;
    let result = async {
        let snapshot: ShowLibrarySnapshot = client
            .get(format!("{base}/api/v2/shows"))
            .bearer_auth(&session.token)
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json()
            .await
            .map_err(|error| error.to_string())?;
        let entry = snapshot
            .shows
            .into_iter()
            .find(|entry| entry.show.id == show_id)
            .ok_or_else(|| "The selected desk show is no longer available".to_owned())?;
        if let Some(revision) = revision {
            if !entry
                .revisions
                .iter()
                .any(|saved| saved.revision == revision)
            {
                return Err("The selected named revision is no longer available".into());
            }
        }
        let name = entry.show.name;
        let url = if let Some(revision) = revision {
            format!("{base}/api/v2/shows/{show_id}/revisions/{revision}/download")
        } else {
            format!("{base}/api/v2/shows/{show_id}/download")
        };
        let response = client
            .get(url)
            .bearer_auth(&session.token)
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?;
        let bytes = response
            .bytes()
            .await
            .map_err(|error| error.to_string())?
            .to_vec();
        if bytes.is_empty() {
            return Err("The desk sent an empty show".into());
        }
        Ok((name, bytes))
    }
    .await;
    let cleanup = close_observer(client, base, &session).await;
    match result {
        Err(error) => Err(error),
        Ok(document) => cleanup.map(|()| document),
    }
}

#[cfg(test)]
mod tests;

#[derive(Default, Deserialize)]
struct FolderQuery {
    root_id: Option<String>,
    #[serde(default)]
    path: String,
}

fn discovered_desk(state: &AppState, instance: &str) -> Result<Peer, ApiError> {
    selected_desk(state.discovery.peer(instance))
}

fn selected_desk(peer: Option<Peer>) -> Result<Peer, ApiError> {
    let peer = peer.ok_or_else(|| ApiError::not_found("network desk"))?;
    if peer.role != Role::Desk {
        return Err(ApiError::bad_request(
            "the selected destination is not a control desk",
        ));
    }
    Ok(peer)
}

async fn save_folders(
    State(state): State<AppState>,
    Path(instance): Path<String>,
    Query(query): Query<FolderQuery>,
    headers: HeaderMap,
) -> Result<Json<wire::NetworkSaveFolders>, ApiError> {
    authenticate(&state, &headers)?;
    let peer = discovered_desk(&state, &instance)?;
    let client = client(Duration::from_secs(15)).map_err(ApiError::internal)?;
    let mut failure = ApiError::bad_gateway("No reachable address");
    for base in peer.base_urls() {
        match desk_save_folders(&client, &base, &query).await {
            Ok(listing) => return Ok(Json(listing)),
            Err(error) if error.status.is_client_error() => return Err(error),
            Err(error) => failure = error,
        }
    }
    Err(failure)
}

fn endpoint(base: &str, parts: &[&str]) -> Result<reqwest::Url, ApiError> {
    let mut url =
        reqwest::Url::parse(base).map_err(|error| ApiError::internal(error.to_string()))?;
    let mut segments = url
        .path_segments_mut()
        .map_err(|()| ApiError::bad_gateway("invalid peer address"))?;
    segments.pop_if_empty();
    for part in parts {
        segments.push(part);
    }
    drop(segments);
    Ok(url)
}

async fn checked(response: reqwest::Response) -> Result<reqwest::Response, ApiError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.json::<serde_json::Value>().await.ok();
    let message = body
        .as_ref()
        .and_then(|value| value.get("error"))
        .and_then(|value| value.as_str())
        .unwrap_or("The destination desk rejected the request")
        .to_owned();
    Err(ApiError {
        status: if status.is_client_error() {
            status
        } else {
            StatusCode::BAD_GATEWAY
        },
        message,
    })
}

async fn desk_save_folders(
    client: &reqwest::Client,
    base: &str,
    query: &FolderQuery,
) -> Result<wire::NetworkSaveFolders, ApiError> {
    let session = observer(client, base)
        .await
        .map_err(ApiError::bad_gateway)?;
    let result = async {
        let response = client
            .get(endpoint(base, &["api", "v2", "files", "roots"])?)
            .bearer_auth(&session.token)
            .send()
            .await
            .map_err(|error| ApiError::bad_gateway(error.to_string()))?;
        let mut roots: Vec<wire::NetworkSaveRoot> = checked(response)
            .await?
            .json()
            .await
            .map_err(|error| ApiError::bad_gateway(error.to_string()))?;
        roots.retain(|root| root.writable);
        let Some(root_id) = query.root_id.as_ref() else {
            return Ok(wire::NetworkSaveFolders {
                roots,
                root_id: None,
                path: String::new(),
                entries: Vec::new(),
            });
        };
        if !roots.iter().any(|root| &root.id == root_id) {
            return Err(ApiError::forbidden(
                "The selected remote root is unavailable or read-only",
            ));
        }
        #[derive(Deserialize)]
        struct Listing {
            path: String,
            entries: Vec<wire::NetworkSaveEntry>,
        }
        let response = client
            .get(endpoint(base, &["api", "v2", "files", root_id, "entries"])?)
            .query(&[("path", &query.path)])
            .bearer_auth(&session.token)
            .send()
            .await
            .map_err(|error| ApiError::bad_gateway(error.to_string()))?;
        let mut listing: Listing = checked(response)
            .await?
            .json()
            .await
            .map_err(|error| ApiError::bad_gateway(error.to_string()))?;
        listing
            .entries
            .retain(|entry| entry.kind == wire::NetworkSaveEntryKind::Folder && entry.writable);
        Ok(wire::NetworkSaveFolders {
            roots,
            root_id: Some(root_id.clone()),
            path: listing.path,
            entries: listing.entries,
        })
    }
    .await;
    let cleanup = close_observer(client, base, &session).await;
    match result {
        Err(error) => Err(error),
        Ok(value) => cleanup.map(|()| value).map_err(ApiError::bad_gateway),
    }
}

fn source_bytes(state: &AppState, source_id: Uuid) -> Result<Vec<u8>, ApiError> {
    if state
        .active_show
        .current()
        .as_ref()
        .is_none_or(|show| show.id.0 != source_id)
    {
        return Err(ApiError::conflict(
            "The active show changed; reopen Save As",
        ));
    }
    let entry = state
        .installation
        .show(light_core::ShowId(source_id))
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("source show"))?;
    let export = state
        .installation
        .data_dir()
        .join(format!(".peer-save-{}.show", Uuid::new_v4()));
    let result = (|| {
        ActiveShowRepository::open(&entry.path)
            .map_err(ApiError::store)?
            .backup_to(&export)
            .map_err(ApiError::store)?;
        std::fs::read(&export).map_err(ApiError::io)
    })();
    let _ = std::fs::remove_file(export);
    result
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn save_copy_to_peer(
    state: &AppState,
    request_id: &str,
    instance: &str,
    source_show_id: Uuid,
    name: &str,
    root_id: &str,
    path: &str,
    is_base_show: bool,
) -> Result<light_wire::v2::runtime::RuntimeShowEntry, ApiError> {
    let peer = discovered_desk(state, instance)?;
    let data =
        base64::engine::general_purpose::STANDARD.encode(source_bytes(state, source_show_id)?);
    let result = peer_save(
        &peer,
        request_id,
        serde_json::json!({"type":"save_copy", "source_show_id":null,
        "data_base64":data,"name":name,"root_id":root_id,"path":path,"is_base_show":is_base_show}),
    )
    .await?;
    match result {
        ShowLibraryActionResult::Show { show } => Ok(show),
        _ => Err(ApiError::bad_gateway(
            "The destination desk returned an unexpected save result",
        )),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn export_mvr_to_peer(
    state: &AppState,
    request_id: &str,
    instance: &str,
    show_id: Uuid,
    name: &str,
    root_id: &str,
    path: &str,
) -> Result<(String, String, MvrExportSummary), ApiError> {
    let peer = discovered_desk(state, instance)?;
    // Guard the source even though the export reads a library operand.
    if state
        .active_show
        .current()
        .as_ref()
        .is_none_or(|show| show.id.0 != show_id)
    {
        return Err(ApiError::conflict(
            "The active show changed; reopen Save As",
        ));
    }
    let (_, document, summary) = super::build_mvr_export(state, show_id)?;
    let data =
        light_mvr::write(&document).map_err(|error| ApiError::internal(error.to_string()))?;
    let result = peer_save(&peer,request_id,serde_json::json!({"type":"export_mvr_file","show_id":null,
        "data_base64":base64::engine::general_purpose::STANDARD.encode(data),"name":name,"root_id":root_id,"path":path})).await?;
    match result {
        ShowLibraryActionResult::FileSaved { root_id, path } => Ok((root_id, path, summary)),
        _ => Err(ApiError::bad_gateway(
            "The destination desk returned an unexpected export result",
        )),
    }
}

async fn peer_save(
    peer: &Peer,
    request_id: &str,
    action: serde_json::Value,
) -> Result<ShowLibraryActionResult, ApiError> {
    let client = client(Duration::from_secs(60)).map_err(ApiError::internal)?;
    let mut failure = ApiError::bad_gateway("No reachable address");
    for base in peer.base_urls() {
        // Address fallback is safe only before a write has been sent.
        let session = match writable_session(&client, &base).await {
            Ok(session) => session,
            Err(error) if error.status.is_client_error() => return Err(error),
            Err(error) => {
                failure = error;
                continue;
            }
        };
        return save_with_session(&client, &base, &session, request_id, &action).await;
    }
    Err(failure)
}

async fn writable_session(
    client: &reqwest::Client,
    base: &str,
) -> Result<ObserverSession, ApiError> {
    let response = client
        .post(endpoint(base, &["api", "v2", "sessions"])?)
        .json(&serde_json::json!({"role":"operator"}))
        .send()
        .await
        .map_err(|error| ApiError::bad_gateway(error.to_string()))?;
    checked(response)
        .await?
        .json()
        .await
        .map_err(|error| ApiError::bad_gateway(error.to_string()))
}

async fn save_with_session(
    client: &reqwest::Client,
    base: &str,
    session: &ObserverSession,
    request_id: &str,
    action: &serde_json::Value,
) -> Result<ShowLibraryActionResult, ApiError> {
    let payload = serde_json::json!({"request_id":request_id,"action":action});
    let result = async {
        let mut failure = ApiError::bad_gateway("The remote save is unconfirmed; refresh its folder before retrying");
        for _ in 0..3 {
            let response = match client.post(endpoint(base,&["api","v2","shows"])?).bearer_auth(&session.token).json(&payload).send().await {
                Ok(response) => response,
                Err(error) => {failure=ApiError::bad_gateway(format!("Remote save is unconfirmed: {error}; refresh the destination folder before retrying")); continue;}
            };
            if response.status().is_server_error() {
                failure = checked(response).await.err().unwrap_or_else(|| ApiError::bad_gateway("Remote save unconfirmed"));
                continue;
            }
            let response = checked(response).await?;
            match response.json::<ShowLibraryActionOutcome>().await {
                Ok(outcome) => return Ok(outcome.result),
                Err(error) => failure=ApiError::bad_gateway(format!("Remote save is unconfirmed: {error}; refresh the destination folder before retrying")),
            }
        }
        Err(failure)
    }.await;
    let cleanup = close_observer(client, base, session).await;
    match result {
        Err(error) => Err(error),
        Ok(value) => cleanup.map(|()| value).map_err(ApiError::bad_gateway),
    }
}
