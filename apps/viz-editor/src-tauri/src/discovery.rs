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
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use viz_document::{LiveDmxInputMapping, LiveDmxInputs};

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
    format!("ToskLight Rig Editor on {}", light_discovery::hostname())
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
pub async fn discovered_desks(discovery: tauri::State<'_, Discovery>) -> Answer<Vec<DeskPeer>> {
    let client = discovery_client(std::time::Duration::from_secs(2))?;
    let checks = discovery
        .desks()
        .into_iter()
        .filter(|desk| desk.show.is_some())
        .map(|desk| {
            let client = client.clone();
            tokio::spawn(async move {
                let reachable = reachable_base(&client, &desk).await.ok()?;
                Some(DeskPeer {
                    instance: desk.instance,
                    name: desk.name,
                    address: reachable.trim_start_matches("http://").to_owned(),
                    show: desk.show,
                })
            })
        })
        .collect::<Vec<_>>();
    let mut found = Vec::new();
    for check in checks {
        let Ok(Some(peer)) = check.await else {
            continue;
        };
        found.push(peer);
    }
    Ok(deduplicate_desk_peers(found))
}

fn deduplicate_desk_peers(peers: Vec<DeskPeer>) -> Vec<DeskPeer> {
    let mut instances = HashSet::new();
    let mut endpoints = HashSet::new();
    peers
        .into_iter()
        .filter(|peer| {
            instances.insert(peer.instance.clone()) && endpoints.insert(peer.address.clone())
        })
        .collect()
}

/// Take a copy of that desk's active show and open it here.
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
    crate::session::announce_document_change(&app, &window)?;
    Ok(summary)
}

/// Preview only the selected desk's compatible output routes as this document's live inputs.
///
/// Nothing is written here. The Show screen presents the result as a draft and its ordinary Apply
/// action is the only path that changes the current planning document.
#[tauri::command]
pub async fn take_live_dmx_inputs_from_desk(
    discovery: tauri::State<'_, Discovery>,
    instance: String,
) -> Answer<LiveDmxInputs> {
    let desk = discovery
        .desks()
        .into_iter()
        .find(|desk| desk.instance == instance)
        .ok_or("that desk is no longer on the network")?;
    fetch_live_dmx_inputs(&desk).await
}

async fn fetch_live_dmx_inputs(desk: &Peer) -> Answer<LiveDmxInputs> {
    let client = discovery_client(std::time::Duration::from_secs(20))?;
    let mut failures = Vec::new();
    for base in desk.base_urls() {
        match fetch_live_dmx_inputs_from(&client, &base).await {
            Ok(inputs) => return Ok(inputs),
            Err(error) => failures.push(format!("{base}: {error}")),
        }
    }
    Err(format!(
        "Could not read output routes from {}. {}",
        desk.name,
        failures.join("; ")
    ))
}

#[derive(Deserialize)]
struct RouteCollection {
    objects: Vec<RouteRecord>,
}

#[derive(Deserialize)]
struct RouteRecord {
    id: String,
    body: serde_json::Value,
}

async fn fetch_live_dmx_inputs_from(client: &reqwest::Client, base: &str) -> Answer<LiveDmxInputs> {
    let token = open_read_only_session(client, base).await?;
    let routes: RouteCollection = client
        .get(format!("{base}/api/v2/objects/route"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| format!("did not answer while reading output routes: {error}"))?
        .error_for_status()
        .map_err(|error| format!("refused its output routes: {error}"))?
        .json()
        .await
        .map_err(|error| format!("returned invalid output routes: {error}"))?;
    Ok(inputs_from_routes(routes.objects))
}

fn inputs_from_routes(routes: Vec<RouteRecord>) -> LiveDmxInputs {
    let mut logical = HashSet::new();
    let mappings = routes
        .into_iter()
        .filter_map(|route| {
            let protocol = match route.body.get("protocol")?.as_str()? {
                "art_net" | "artnet" => "artnet",
                "sacn" | "s_acn" => "sacn",
                _ => return None,
            };
            let logical_universe = u16::try_from(route.body.get("logical_universe")?.as_u64()?)
                .ok()
                .filter(|universe| *universe > 0)?;
            if !logical.insert(logical_universe) {
                // The first declared compatible route is the desk's deterministic winner.
                return None;
            }
            let destination_universe = route
                .body
                .get("destination_universe")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(logical_universe);
            let delivery = route
                .body
                .get("delivery_mode")
                .and_then(serde_json::Value::as_str)
                .filter(|value| {
                    matches!(
                        (protocol, *value),
                        ("artnet", "broadcast" | "unicast") | ("sacn", "multicast" | "unicast")
                    )
                })
                .unwrap_or(if protocol == "sacn" {
                    "multicast"
                } else {
                    "broadcast"
                });
            let default_port = if protocol == "sacn" { 5568 } else { 6454 };
            let port = route
                .body
                .get("destination")
                .and_then(serde_json::Value::as_str)
                .and_then(destination_port)
                .unwrap_or(default_port);
            Some(LiveDmxInputMapping {
                id: route.id,
                logical_universe,
                protocol: protocol.into(),
                destination_universe,
                port,
                enabled: route
                    .body
                    .get("enabled")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true),
                delivery: delivery.into(),
            })
        })
        .collect();
    LiveDmxInputs {
        schema_version: 1,
        mappings,
    }
}

fn destination_port(destination: &str) -> Option<u16> {
    destination
        .parse::<std::net::SocketAddr>()
        .ok()
        .map(|address| address.port())
        .or_else(|| destination.rsplit_once(':')?.1.parse().ok())
}

/// The desk's active show as a portable file, through the same API the visualizer uses.
///
/// A desk answers on every interface its machine has, and only one of them may be the network
/// this editor is on, so each is tried in turn rather than failing on the first.
async fn fetch_active_show(desk: &Peer) -> Answer<(String, Vec<u8>)> {
    let client = discovery_client(std::time::Duration::from_secs(60))?;
    let mut failures = Vec::new();
    for base in desk.base_urls() {
        match fetch_from(&client, desk, &base).await {
            Ok(show) => return Ok(show),
            Err(error) => failures.push(format!("{base}: {error}")),
        }
    }
    Err(if failures.is_empty() {
        format!(
            "{} has no advertised address this editor can reach",
            desk.name
        )
    } else {
        format!(
            "Could not connect to {} at any advertised address. Check that the desk is running and reachable. {}",
            desk.name,
            failures.join("; ")
        )
    })
}

fn discovery_client(timeout: std::time::Duration) -> Answer<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(2))
        .timeout(timeout)
        .build()
        .map_err(|error| error.to_string())
}

#[derive(Deserialize)]
struct Bootstrap {
    users: Vec<BootstrapUser>,
}

#[derive(Deserialize)]
struct BootstrapUser {
    name: String,
    enabled: bool,
}

async fn reachable_base(client: &reqwest::Client, desk: &Peer) -> Answer<String> {
    let mut failures = Vec::new();
    for base in desk.base_urls() {
        match enabled_session_user(client, &base).await {
            Ok(_) => return Ok(base),
            Err(error) => failures.push(format!("{base}: {error}")),
        }
    }
    Err(failures.join("; "))
}

async fn enabled_session_user(client: &reqwest::Client, base: &str) -> Answer<String> {
    let bootstrap: Bootstrap = client
        .get(format!("{base}/api/v2/bootstrap"))
        .send()
        .await
        .map_err(|error| format!("did not answer: {error}"))?
        .error_for_status()
        .map_err(|error| format!("did not provide API v2 bootstrap: {error}"))?
        .json()
        .await
        .map_err(|error| format!("returned an invalid API v2 bootstrap: {error}"))?;
    preferred_enabled_user(bootstrap.users)
}

async fn open_read_only_session(client: &reqwest::Client, base: &str) -> Answer<String> {
    let username = enabled_session_user(client, base).await?;
    let session: serde_json::Value = client
        .post(format!("{base}/api/v2/sessions"))
        .json(&serde_json::json!({"username": username, "role": "visualizer"}))
        .send()
        .await
        .map_err(|error| format!("did not answer while creating a read-only session: {error}"))?
        .error_for_status()
        .map_err(|error| format!("refused a read-only Visualizer session: {error}"))?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    session
        .get("token")
        .and_then(|token| token.as_str())
        .map(str::to_owned)
        .ok_or_else(|| "that desk answered without a session token".to_owned())
}

fn preferred_enabled_user(users: Vec<BootstrapUser>) -> Answer<String> {
    let mut enabled = users
        .into_iter()
        .filter(|user| user.enabled)
        .map(|user| user.name)
        .collect::<Vec<_>>();
    enabled.sort_by_key(|name| name.to_lowercase());
    enabled
        .iter()
        .find(|name| name.eq_ignore_ascii_case("Operator"))
        .cloned()
        .or_else(|| enabled.into_iter().next())
        .ok_or_else(|| "has no enabled user for a read-only Visualizer session".to_owned())
}

async fn fetch_from(
    client: &reqwest::Client,
    desk: &Peer,
    base: &str,
) -> Answer<(String, Vec<u8>)> {
    // A read-only session, which is what this is: the editor takes a copy and issues nothing else.
    let token = open_read_only_session(client, base).await?;
    let readiness: serde_json::Value = client
        .get(format!("{base}/api/v2/readiness"))
        .bearer_auth(&token)
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
        .bearer_auth(&token)
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn answer_http(
        listener: &tokio::net::TcpListener,
        expected: &str,
        body: &[u8],
        extra_headers: &str,
    ) -> String {
        let (mut stream, _) = listener.accept().await.expect("request");
        let mut request = Vec::new();
        loop {
            let mut chunk = [0_u8; 2048];
            let count = stream.read(&mut chunk).await.expect("request bytes");
            if count == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..count]);
            let text = String::from_utf8_lossy(&request);
            let Some(headers_end) = text.find("\r\n\r\n") else {
                continue;
            };
            let content_length = text[..headers_end]
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")?
                        .trim()
                        .parse::<usize>()
                        .ok()
                })
                .unwrap_or(0);
            if request.len() >= headers_end + 4 + content_length {
                break;
            }
        }
        let request = String::from_utf8(request).expect("UTF-8 HTTP request");
        assert!(request.starts_with(expected), "{request}");
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n{extra_headers}\r\n{}",
                    body.len(),
                    String::from_utf8_lossy(body)
                )
                .as_bytes(),
            )
            .await
            .expect("response");
        request
    }

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

    #[test]
    fn the_clean_install_operator_is_used_for_the_read_only_session() {
        let selected = preferred_enabled_user(vec![
            BootstrapUser {
                name: "Disabled".into(),
                enabled: false,
            },
            BootstrapUser {
                name: "Operator".into(),
                enabled: true,
            },
        ])
        .expect("clean installation has an enabled user");
        assert_eq!(selected, "Operator");
        assert!(preferred_enabled_user(Vec::new()).is_err());
    }

    #[test]
    fn one_advertised_desk_is_deduplicated_across_address_representations() {
        let peer = |address: &str| DeskPeer {
            instance: "tosklight-desk-kmp5._tosklight._tcp.local.".into(),
            name: "kmp5".into(),
            show: Some("Tour".into()),
            address: address.into(),
        };
        let found = deduplicate_desk_peers(vec![
            peer("[::1]:5000"),
            peer("127.0.0.1:5000"),
            peer("kmp5:5000"),
        ]);

        assert_eq!(found.len(), 1);
    }

    #[test]
    fn desk_routes_become_portable_inputs_without_machine_interfaces() {
        let inputs = inputs_from_routes(vec![
            RouteRecord {
                id: "artnet-u1".into(),
                body: serde_json::json!({
                    "protocol":"art_net","logical_universe":1,"destination_universe":11,
                    "delivery_mode":"unicast","destination":"10.0.0.9:6455","enabled":true
                }),
            },
            RouteRecord {
                id: "sacn-u2".into(),
                body: serde_json::json!({
                    "protocol":"sacn","logical_universe":2,"destination_universe":22,
                    "delivery_mode":"multicast","enabled":false
                }),
            },
        ]);
        assert_eq!(inputs.mappings.len(), 2);
        assert_eq!(inputs.mappings[0].protocol, "artnet");
        assert_eq!(inputs.mappings[0].port, 6455);
        assert_eq!(inputs.mappings[1].protocol, "sacn");
        assert_eq!(inputs.mappings[1].port, 5568);
        assert!(!inputs.mappings[1].enabled);
        inputs.validate().expect("desk routes produce valid inputs");
    }

    #[test]
    fn first_compatible_desk_route_wins_one_logical_universe() {
        let inputs = inputs_from_routes(vec![
            RouteRecord {
                id: "first".into(),
                body: serde_json::json!({"protocol":"art_net","logical_universe":1,"destination_universe":1}),
            },
            RouteRecord {
                id: "second".into(),
                body: serde_json::json!({"protocol":"sacn","logical_universe":1,"destination_universe":1}),
            },
        ]);
        assert_eq!(inputs.mappings.len(), 1);
        assert_eq!(inputs.mappings[0].id, "first");
    }

    #[tokio::test]
    async fn an_ipv6_then_ipv4_peer_falls_back_to_the_ipv4_only_desk() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("IPv4 listener");
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("bootstrap request");
            let mut request = [0_u8; 2048];
            let count = stream.read(&mut request).await.expect("request bytes");
            assert!(
                String::from_utf8_lossy(&request[..count]).starts_with("GET /api/v2/bootstrap")
            );
            let body = r#"{"users":[{"name":"Operator","enabled":true}]}"#;
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .expect("bootstrap response");
        });
        let peer = Peer {
            role: Role::Desk,
            name: "IPv4 desk".into(),
            show: Some("Show".into()),
            addresses: vec![format!("[::1]:{port}"), format!("127.0.0.1:{port}")],
            instance: "desk.local".into(),
        };

        let base = reachable_base(
            &discovery_client(std::time::Duration::from_secs(2)).unwrap(),
            &peer,
        )
        .await
        .expect("IPv4 fallback reaches the desk");
        assert_eq!(base, format!("http://127.0.0.1:{port}"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn an_ipv4_only_clean_desk_creates_a_visualizer_session_and_downloads() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("IPv4 listener");
        let port = listener.local_addr().unwrap().port();
        let show_id = uuid::Uuid::new_v4();
        let server = tokio::spawn(async move {
            answer_http(
                &listener,
                "GET /api/v2/bootstrap",
                br#"{"users":[{"name":"Operator","enabled":true}]}"#,
                "content-type: application/json\r\n",
            )
            .await;
            let session = answer_http(
                &listener,
                "POST /api/v2/sessions",
                br#"{"token":"visualizer-token"}"#,
                "content-type: application/json\r\n",
            )
            .await;
            assert!(session.contains(r#""username":"Operator""#));
            assert!(session.contains(r#""role":"visualizer""#));
            answer_http(
                &listener,
                "GET /api/v2/readiness",
                format!(r#"{{"active_show":"{show_id}"}}"#).as_bytes(),
                "content-type: application/json\r\n",
            )
            .await;
            answer_http(
                &listener,
                &format!("GET /api/v2/shows/{show_id}/download"),
                b"portable-show",
                "content-type: application/octet-stream\r\ncontent-disposition: attachment; filename=\"IPv4 Tour.show\"\r\n",
            )
            .await;
        });
        let peer = Peer {
            role: Role::Desk,
            name: "Clean desk".into(),
            show: Some("IPv4 Tour".into()),
            addresses: vec![format!("[::1]:{port}"), format!("127.0.0.1:{port}")],
            instance: "clean.local".into(),
        };

        let (name, bytes) = fetch_active_show(&peer).await.expect("downloaded show");
        assert_eq!(name, "IPv4 Tour");
        assert_eq!(bytes, b"portable-show");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn an_ipv4_only_clean_desk_previews_its_routes_read_only() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("IPv4 listener");
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            answer_http(
                &listener,
                "GET /api/v2/bootstrap",
                br#"{"users":[{"name":"Operator","enabled":true}]}"#,
                "content-type: application/json\r\n",
            )
            .await;
            let session = answer_http(
                &listener,
                "POST /api/v2/sessions",
                br#"{"token":"visualizer-token"}"#,
                "content-type: application/json\r\n",
            )
            .await;
            assert!(session.contains(r#""role":"visualizer""#));
            let routes = answer_http(
                &listener,
                "GET /api/v2/objects/route",
                br#"{"objects":[{"id":"route-1","body":{"protocol":"art_net","logical_universe":1,"destination_universe":12,"delivery_mode":"broadcast","enabled":true}}]}"#,
                "content-type: application/json\r\n",
            )
            .await;
            assert!(
                routes
                    .to_ascii_lowercase()
                    .contains("authorization: bearer visualizer-token")
            );
        });
        let peer = Peer {
            role: Role::Desk,
            name: "Clean desk".into(),
            show: Some("IPv4 Tour".into()),
            addresses: vec![format!("127.0.0.1:{port}")],
            instance: "clean.local".into(),
        };

        let inputs = fetch_live_dmx_inputs(&peer).await.expect("route preview");
        assert_eq!(inputs.mappings.len(), 1);
        assert_eq!(inputs.mappings[0].destination_universe, 12);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn an_endpoint_without_a_listener_is_not_reachable() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let peer = Peer {
            role: Role::Desk,
            name: "Stale desk".into(),
            show: Some("Old show".into()),
            addresses: vec![format!("127.0.0.1:{port}")],
            instance: "stale.local".into(),
        };

        assert!(
            reachable_base(
                &discovery_client(std::time::Duration::from_millis(500)).unwrap(),
                &peer,
            )
            .await
            .is_err()
        );
    }
}
