//! Finding the desk, and taking a copy of the show it is running.
//!
//! A rig planned here and a show running on a desk are the same rig. This is the short way
//! between them: the editor says it is here and what it has open, listens for desks doing the
//! same, and can pull one desk's active show down as an ordinary file to open.
//!
//! What crosses is always a copy. Opening a desk's show here does not join that desk's session,
//! does not hold its file open, and does not send anything back — patching afterwards is patching
//! this document, and the desk never notices.

use crate::session::{DocumentSummary, Session};
use light_discovery::{Advertisement, Advertiser, Browser, Peer, Role};
use parking_lot::Mutex;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// The editor's own announcement, and what it has heard.
#[derive(Default)]
pub struct Discovery {
    advertiser: Mutex<Option<Advertiser>>,
    browser: Mutex<Option<Browser>>,
    /// Where a show pulled from a desk is written. Set once the application knows its own data
    /// directory; without it there is nowhere to put a copy and the offer is not made.
    downloads: Mutex<Option<PathBuf>>,
}

/// What this editor calls itself on the network.
///
/// The host is part of the name because two editors on two machines are two entries, and an
/// operator picks between them by knowing which machine is which.
fn editor_name() -> String {
    format!("ToskLight Viz Editor on {}", light_discovery::hostname())
}

impl Discovery {
    /// Announce this editor on `port` and start looking for desks.
    ///
    /// Both halves fail quietly: a network with no mDNS costs the button and nothing else.
    pub fn start(&self, port: u16, show: Option<String>) {
        match Advertiser::start(Advertisement {
            role: Role::Editor,
            name: editor_name(),
            show,
            port,
        }) {
            Ok(advertiser) => *self.advertiser.lock() = Some(advertiser),
            Err(error) => eprintln!("this editor will not be discoverable: {error}"),
        }
        match Browser::start() {
            Ok(browser) => *self.browser.lock() = Some(browser),
            Err(error) => eprintln!("not looking for desks on the network: {error}"),
        }
    }

    pub fn set_downloads(&self, directory: PathBuf) {
        *self.downloads.lock() = Some(directory);
    }

    /// Publish the document now open, so a desk's menu names what this editor actually holds.
    pub fn announce_document(&self, name: Option<String>) {
        if let Some(advertiser) = self.advertiser.lock().as_ref() {
            advertiser.set_show(name);
        }
    }

    fn desks(&self) -> Vec<Peer> {
        self.browser
            .lock()
            .as_ref()
            .map(|browser| browser.peers_with_role(Role::Desk))
            .unwrap_or_default()
    }
}

/// One desk the operator could load from.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeskPeer {
    pub instance: String,
    pub name: String,
    /// The show it is running. A desk with none is listed and not offered.
    pub show: Option<String>,
    pub address: String,
}

type Answer<T> = Result<T, String>;

/// The desks on the network that are running a show.
#[tauri::command]
pub fn discovered_desks(discovery: tauri::State<'_, Discovery>) -> Vec<DeskPeer> {
    discovery
        .desks()
        .into_iter()
        .filter(|desk| desk.show.is_some())
        .map(|desk| DeskPeer {
            instance: desk.instance.clone(),
            name: desk.name.clone(),
            address: desk.address().to_owned(),
            show: desk.show,
        })
        .collect()
}

/// Take a copy of that desk's active show and open it here.
#[tauri::command]
pub async fn load_from_desk(
    discovery: tauri::State<'_, Discovery>,
    session: tauri::State<'_, Session>,
    instance: String,
) -> Answer<DocumentSummary> {
    let desk = discovery
        .desks()
        .into_iter()
        .find(|desk| desk.instance == instance)
        .ok_or("that desk is no longer on the network")?;
    let directory = discovery
        .downloads
        .lock()
        .clone()
        .ok_or("this editor has nowhere to keep a downloaded show")?;
    let (name, bytes) = fetch_active_show(&desk).await?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let path = unique_path(&directory, &name);
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    let summary = session.open(&path)?;
    discovery.announce_document(Some(summary.name.clone()));
    Ok(summary)
}

/// The desk's active show as a portable file, through the same API the visualizer uses.
///
/// A desk answers on every interface its machine has, and only one of them may be the network
/// this editor is on, so each is tried in turn rather than failing on the first.
async fn fetch_active_show(desk: &Peer) -> Answer<(String, Vec<u8>)> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|error| error.to_string())?;
    let mut failure = None;
    for base in desk.base_urls() {
        match fetch_from(&client, desk, &base).await {
            Ok(show) => return Ok(show),
            Err(error) => failure = Some(error),
        }
    }
    Err(failure.unwrap_or_else(|| format!("{} has no address this editor can reach", desk.name)))
}

async fn fetch_from(
    client: &reqwest::Client,
    desk: &Peer,
    base: &str,
) -> Answer<(String, Vec<u8>)> {
    // A read-only session, which is what this is: the editor takes a copy and issues nothing else.
    let session: serde_json::Value = client
        .post(format!("{base}/api/v2/sessions"))
        .json(&serde_json::json!({"username": "ToskLight Viz Editor", "role": "visualizer"}))
        .send()
        .await
        .map_err(|error| format!("{} did not answer: {error}", desk.name))?
        .error_for_status()
        .map_err(|error| format!("{} refused a session: {error}", desk.name))?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    let token = session
        .get("token")
        .and_then(|token| token.as_str())
        .ok_or("that desk answered without a session token")?;
    let readiness: serde_json::Value = client
        .get(format!("{base}/api/v2/readiness"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    let show_id = readiness
        .get("active_show")
        .and_then(|show| show.as_str())
        .ok_or("that desk has no show open")?;
    let response = client
        .get(format!("{base}/api/v2/shows/{show_id}/download"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "{} answered {} for its show",
            desk.name,
            response.status()
        ));
    }
    let name = file_name(&response).unwrap_or_else(|| desk.show.clone().unwrap_or_default());
    let bytes = response
        .bytes()
        .await
        .map_err(|error| error.to_string())?
        .to_vec();
    if bytes.is_empty() {
        return Err(format!("{} sent an empty show", desk.name));
    }
    Ok((sanitised(&name), bytes))
}

/// The name the desk gave the file, so the copy is called what the show is called.
fn file_name(response: &reqwest::Response) -> Option<String> {
    let disposition = response
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)?
        .to_str()
        .ok()?;
    let quoted = disposition.split("filename=").nth(1)?.trim();
    let name = quoted.trim_matches('"');
    Some(name.trim_end_matches(".show").to_owned())
}

fn sanitised(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | ' ') {
                character
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim().to_owned();
    if trimmed.is_empty() {
        "Show from desk".to_owned()
    } else {
        trimmed
    }
}

/// A second copy of the same show does not overwrite the first: the operator may still be working
/// in it, and a download is not an instruction to discard anything.
fn unique_path(directory: &Path, name: &str) -> PathBuf {
    let candidate = directory.join(format!("{name}.show"));
    if !candidate.exists() {
        return candidate;
    }
    for suffix in 2..1_000 {
        let candidate = directory.join(format!("{name} {suffix}.show"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("{name} {}.show", uuid::Uuid::new_v4()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_downloaded_show_never_overwrites_one_already_there() {
        let directory = std::env::temp_dir().join(format!("viz-editor-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let first = unique_path(&directory, "Summer Tour");
        assert!(first.ends_with("Summer Tour.show"));
        std::fs::write(&first, b"show").unwrap();
        let second = unique_path(&directory, "Summer Tour");
        assert!(second.ends_with("Summer Tour 2.show"));
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn a_desk_name_that_is_not_a_file_name_still_becomes_one() {
        assert_eq!(sanitised("Summer/Tour 2026"), "Summer-Tour 2026");
        assert_eq!(sanitised("   "), "Show from desk");
    }
}
