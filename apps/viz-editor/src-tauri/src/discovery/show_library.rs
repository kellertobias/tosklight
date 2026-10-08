//! Browse desk libraries and open a desk's show as a document bound to it.
//!
//! A document opened here stays synchronized with its desk show automatically (`crate::sync`);
//! there is no separate save to the desk.

use super::*;
use crate::sync::SyncBinding;

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
    let show_uuid = uuid::Uuid::parse_str(&show_id).map_err(|e| e.to_string())?;
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
    let desk_identity = desk_identity(&client, &base).await;
    let summary = session.open_from_desk(
        &path,
        SyncBinding::new(desk_identity, show_uuid, base, name, 0),
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

/// The desk installation's identity, from its readiness. A desk that predates identities, or one
/// that does not answer, leaves the binding to adopt the identity on its first confirmed contact.
async fn desk_identity(client: &reqwest::Client, base: &str) -> Option<uuid::Uuid> {
    let readiness: serde_json::Value = client
        .get(format!("{base}/api/v2/readiness"))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    readiness["desk_identity"]
        .as_str()
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
}

#[tauri::command]
pub fn source_desk(session: tauri::State<'_, Session>) -> Option<String> {
    session
        .binding
        .lock()
        .as_ref()
        .map(|binding| binding.desk_name.clone())
}

/// Publishes the open document to a desk's show library as a new show, then continues with the
/// desk's copy, bound to it.
///
/// This is the deliberate way to associate a document with a desk: a standalone show, or a copy
/// made with Save As, which has its own identity and is bound to nothing. The local file stays
/// as it was; the document that opens afterwards is the desk's show, in step with it from then on.
#[tauri::command]
pub async fn publish_to_desk(
    app: tauri::AppHandle,
    window: tauri::Window,
    discovery: tauri::State<'_, Discovery>,
    session: tauri::State<'_, Session>,
    instance: String,
) -> Answer<DocumentSummary> {
    use base64::Engine;
    if session.binding.lock().is_some() {
        return Err("This show already follows a desk. Save As first to publish a copy.".into());
    }
    let desk = discovery
        .desks()
        .into_iter()
        .find(|desk| desk.instance == instance)
        .ok_or("That desk is no longer on the network")?;
    let client = discovery_client(std::time::Duration::from_secs(60))?;
    let base = reachable_base(&client, &desk).await?;
    let (name, bytes) = session.with(|document| {
        let staged = document
            .path()
            .with_file_name(format!(".publish-{}.show", uuid::Uuid::new_v4()));
        let bytes = document
            .save_as(&staged)
            .map_err(|e| e.to_string())
            .and_then(|_| std::fs::read(&staged).map_err(|e| e.to_string()));
        let _ = std::fs::remove_file(&staged);
        Ok((document.name().map_err(|e| e.to_string())?, bytes?))
    })?;
    let credentials: serde_json::Value = client
        .post(format!("{base}/api/v2/sessions"))
        .json(&serde_json::json!({"role": "operator"}))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| format!("{} refused an editing session: {e}", desk.name))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let token = credentials["token"].as_str().unwrap_or_default().to_owned();
    let session_id = credentials["session_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let created = client
        .post(format!("{base}/api/v2/shows"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "request_id": uuid::Uuid::new_v4().to_string(),
            "action": {"type": "create", "name": name, "overwrite": false,
                "data_base64": base64::engine::general_purpose::STANDARD.encode(bytes)},
        }))
        .send()
        .await;
    let _ = client
        .delete(format!("{base}/api/v2/sessions/{session_id}"))
        .bearer_auth(&token)
        .send()
        .await;
    let created = created.map_err(|e| e.to_string())?;
    let status = created.status();
    let body: serde_json::Value = created.json().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "{} did not take the show ({status}): {}",
            desk.name,
            body["error"].as_str().unwrap_or("no reason given")
        ));
    }
    let show_id = body["result"]["show"]["id"]
        .as_str()
        .ok_or("The desk did not say which show it created")?
        .to_owned();
    load_desk_show(app, window, discovery, session, base, desk.name, show_id).await
}
