//! Browse desk libraries and save documents to their originating library entry.

use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct DeskSource {
    pub base: String,
    pub name: String,
    pub show_id: String,
    pub revision: u64,
}

#[derive(Serialize, Deserialize)]
pub struct DeskShow {
    pub id: String,
    pub name: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
struct DeskCatalog {
    shows: Vec<DeskShow>,
}

#[tauri::command]
pub async fn desk_shows(address: String) -> Answer<Vec<DeskShow>> {
    let base = desk_base(&address)?;
    let client = discovery_client(std::time::Duration::from_secs(20))?;
    let (_, bytes) = read_desk_resource(&client, &base, "/api/v2/shows").await?;
    let catalog: DeskCatalog = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    Ok(catalog.shows)
}

async fn read_desk_resource(
    client: &reqwest::Client,
    base: &str,
    path: &str,
) -> Answer<(reqwest::header::HeaderMap, Vec<u8>)> {
    let credentials: serde_json::Value = client
        .post(format!("{base}/api/v2/sessions"))
        .json(&serde_json::json!({"role":"visualizer"}))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let token = credentials["token"]
        .as_str()
        .ok_or("Desk returned no read token")?;
    let id = credentials["session_id"]
        .as_str()
        .ok_or("Desk returned no session identity")?;
    let result = async {
        let response = client
            .get(format!("{base}{path}"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let headers = response.headers().clone();
        let bytes = response.bytes().await.map_err(|e| e.to_string())?.to_vec();
        Ok((headers, bytes))
    }
    .await;
    let _ = client
        .delete(format!("{base}/api/v2/sessions/{id}"))
        .bearer_auth(token)
        .send()
        .await;
    result
}

fn desk_base(address: &str) -> Answer<String> {
    let base = if address.contains("://") {
        address.to_owned()
    } else {
        format!("http://{address}")
    };
    let url = reqwest::Url::parse(&base).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err("Enter a desk host and port, without a path or credentials".into());
    }
    Ok(base.trim_end_matches('/').to_owned())
}

/// Open the selected library show, keeping its originating desk as the save destination.
#[tauri::command]
pub async fn load_desk_show(
    app: tauri::AppHandle,
    window: tauri::Window,
    discovery: tauri::State<'_, Discovery>,
    session: tauri::State<'_, Session>,
    address: String,
    name: String,
    show_id: String,
) -> Answer<DocumentSummary> {
    uuid::Uuid::parse_str(&show_id).map_err(|e| e.to_string())?;
    let base = desk_base(&address)?;
    let client = discovery_client(std::time::Duration::from_secs(60))?;
    let (headers, bytes) =
        read_desk_resource(&client, &base, &format!("/api/v2/shows/{show_id}/download")).await?;
    let show_name = headers
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|header| header.to_str().ok())
        .and_then(|value| value.split("filename=").nth(1))
        .map(|name| {
            name.trim()
                .trim_matches('"')
                .trim_end_matches(".show")
                .to_owned()
        })
        .unwrap_or_else(|| "Desk Show".into());
    let directory = discovery
        .downloads
        .lock()
        .clone()
        .ok_or("No download directory configured")?;
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let path = unique_path(&directory, &sanitised(&show_name));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    let summary = session.open_from_desk(
        &path,
        DeskSource {
            base,
            name,
            show_id,
            revision: 0,
        },
    )?;
    discovery.announce_document(Some(summary.name.clone()));
    crate::session::announce_document_change(&app, &window)?;
    Ok(summary)
}

/// Keep the older active-show command for existing callers.
#[tauri::command]
pub async fn load_from_desk(
    app: tauri::AppHandle,
    window: tauri::Window,
    discovery: tauri::State<'_, Discovery>,
    session: tauri::State<'_, Session>,
    instance: String,
) -> Answer<DocumentSummary> {
    let desk = discovery
        .desks()
        .into_iter()
        .find(|desk| desk.instance == instance)
        .ok_or("That desk is no longer on the network")?;
    let client = discovery_client(std::time::Duration::from_secs(20))?;
    let base = reachable_base(&client, &desk).await?;
    let readiness: serde_json::Value = client
        .get(format!("{base}/api/v2/readiness"))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let show_id = readiness["active_show"]
        .as_str()
        .ok_or("That desk has no show open")?
        .to_owned();
    load_desk_show(app, window, discovery, session, base, desk.name, show_id).await
}

#[tauri::command]
pub fn source_desk(session: tauri::State<'_, Session>) -> Option<String> {
    session
        .desk_source
        .lock()
        .as_ref()
        .map(|source| source.name.clone())
}

#[derive(Clone)]
pub(crate) struct PendingDeskSave {
    generation: u64,
    source: DeskSource,
    token: String,
    session_id: String,
    request: serde_json::Value,
    local_revision: u64,
}

async fn close_edit_session(client: &reqwest::Client, pending: &PendingDeskSave) {
    let _ = client
        .delete(format!(
            "{}/api/v2/sessions/{}",
            pending.source.base, pending.session_id
        ))
        .bearer_auth(&pending.token)
        .send()
        .await;
}

#[tauri::command]
pub async fn save_to_source_desk(session: tauri::State<'_, Session>) -> Answer<String> {
    save_to_desk(&session).await
}

async fn save_to_desk(session: &Session) -> Answer<String> {
    use base64::Engine;
    let _save = session.desk_save_gate.lock().await;
    let (source, generation, bytes, local_revision) = session.desk_save_snapshot()?;
    let client = discovery_client(std::time::Duration::from_secs(60))?;
    let previous = session.pending_desk_save.lock().clone();
    let pending = if let Some(pending) = previous {
        pending
    } else {
        let credentials: serde_json::Value = client
            .post(format!("{}/api/v2/sessions", source.base))
            .json(&serde_json::json!({"role":"operator"}))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| format!("Desk refused an editing session: {e}"))?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let pending = PendingDeskSave {
            generation,
            local_revision,
            source: source.clone(),
            token: credentials["token"]
                .as_str()
                .ok_or("Desk returned no editing token")?
                .into(),
            session_id: credentials["session_id"]
                .as_str()
                .ok_or("Desk returned no session identity")?
                .into(),
            request: serde_json::json!({"request_id":uuid::Uuid::new_v4().to_string(),"action":{
                "type":"update_document","destination_show_id":source.show_id,"expected_revision":source.revision,
                "data_base64":base64::engine::general_purpose::STANDARD.encode(bytes)
            }}),
        };
        *session.pending_desk_save.lock() = Some(pending.clone());
        pending
    };
    let response = client
        .post(format!("{}/api/v2/shows", pending.source.base))
        .bearer_auth(&pending.token)
        .json(&pending.request)
        .send()
        .await
        .map_err(|e| {
            format!("Desk save is unconfirmed. Press Save again to recover the same request: {e}")
        })?;
    let status = response.status();
    let outcome: serde_json::Value = response.json().await.map_err(|e| {
        format!("Desk save is unconfirmed. Press Save again to recover the same request: {e}")
    })?;
    if !status.is_success() {
        // A server-side failure may occur after commit. Retain identity for a safe retry.
        if status.is_client_error() {
            *session.pending_desk_save.lock() = None;
            close_edit_session(&client, &pending).await;
        }
        return Err(format!("Desk save failed ({status}): {outcome}"));
    }
    let revision = outcome["result"]["document_revision"]
        .as_u64()
        .ok_or("Desk did not confirm the saved revision; retry Save")?;
    session.confirm_desk_save(pending.generation, revision)?;
    *session.pending_desk_save.lock() = None;
    close_edit_session(&client, &pending).await;
    if pending.generation != generation {
        Ok(format!(
            "Confirmed the previous save to {}; the current document has not been saved to its desk",
            pending.source.name
        ))
    } else if session.with(|document| document.portable_revision().map_err(|e| e.to_string()))?
        != pending.local_revision
    {
        Ok(format!(
            "Saved the earlier snapshot to {}; press Save again to send newer local edits",
            pending.source.name
        ))
    } else {
        Ok(format!("Saved to {}", pending.source.name))
    }
}

#[cfg(test)]
#[path = "show_library_tests.rs"]
mod tests;
