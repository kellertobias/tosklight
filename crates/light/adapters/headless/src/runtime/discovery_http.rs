//! The other ToskLights on the network, and loading a rig from one of them.
//!
//! The desk says who it is and what it is running, and it listens for anything else saying the
//! same. That is all discovery is here: a list the Load Show menu can offer a Viz editor's open
//! document from, and an entry in every other ToskLight's list so the traffic goes both ways.
//!
//! None of it is required. A network without mDNS, a firewall that eats it, or a responder that
//! will not start costs the offer and nothing else — the desk runs exactly as it did before.

use super::{ApiError, AppState};
use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};
use light_discovery::{Advertisement, Advertiser, Browser, Peer, Role};
use light_wire::v2::discovery as wire;
use std::sync::Arc;

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v2/discovery/peers", get(peers))
}

/// What this desk publishes and what it has heard.
///
/// Both halves are optional and independent: a desk that could not advertise can still browse,
/// and a desk that could not browse still tells the network it is here.
#[derive(Default)]
pub(super) struct DeskDiscovery {
    advertiser: Option<Advertiser>,
    browser: Option<Browser>,
}

impl DeskDiscovery {
    /// Start announcing this desk on `port` and looking for the others.
    pub(super) fn start(port: u16, show: Option<String>) -> Self {
        let name = light_discovery::hostname();
        let advertiser = match Advertiser::start(Advertisement {
            role: Role::Desk,
            name,
            show,
            port,
        }) {
            Ok(advertiser) => Some(advertiser),
            Err(error) => {
                tracing::info!(%error, "this desk will not be discoverable on the network");
                None
            }
        };
        let browser = match Browser::start() {
            Ok(browser) => Some(browser),
            Err(error) => {
                tracing::info!(%error, "not looking for other ToskLights on the network");
                None
            }
        };
        Self {
            advertiser,
            browser,
        }
    }

    /// Publish the show the desk is now running, so a peer's menu names what is actually loaded.
    pub(super) fn announce_show(&self, show: Option<String>) {
        if let Some(advertiser) = &self.advertiser {
            advertiser.set_show(show);
        }
    }

    pub(super) fn peers(&self) -> Vec<Peer> {
        self.browser
            .as_ref()
            .map(Browser::peers)
            .unwrap_or_default()
    }

    pub(super) fn is_browsing(&self) -> bool {
        self.browser.is_some()
    }

    pub(super) fn peer(&self, instance: &str) -> Option<Peer> {
        self.peers()
            .into_iter()
            .find(|peer| peer.instance == instance)
    }
}

/// The desk's discovery, as the rest of the runtime holds it.
///
/// The responder and the browse stay inside: what leaves is a list of peers and an announcement
/// of the show, never the daemons themselves.
#[derive(Clone, Default)]
pub(super) struct DiscoveryResource(Arc<DeskDiscovery>);

impl DiscoveryResource {
    pub(super) fn start(port: u16, show: Option<String>) -> Self {
        Self(Arc::new(DeskDiscovery::start(port, show)))
    }

    pub(super) fn announce_show(&self, show: Option<String>) {
        self.0.announce_show(show);
    }

    pub(super) fn peers(&self) -> Vec<Peer> {
        self.0.peers()
    }

    pub(super) fn is_browsing(&self) -> bool {
        self.0.is_browsing()
    }

    pub(super) fn peer(&self, instance: &str) -> Option<Peer> {
        self.0.peer(instance)
    }
}

async fn peers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::DiscoverySnapshot>, ApiError> {
    super::authenticate(&state, &headers)?;
    Ok(Json(wire::DiscoverySnapshot {
        browsing: state.discovery.is_browsing(),
        peers: state
            .discovery
            .peers()
            .into_iter()
            .map(projection)
            .collect(),
    }))
}

fn projection(peer: Peer) -> wire::DiscoveredPeer {
    wire::DiscoveredPeer {
        role: match peer.role {
            Role::Desk => wire::DiscoveredRole::Desk,
            Role::Editor => wire::DiscoveredRole::Editor,
        },
        address: peer.address().to_owned(),
        name: peer.name,
        show: peer.show,
        instance: peer.instance,
    }
}

/// The open document of the Viz editor advertising `instance`, as a portable show file.
///
/// A copy is what crosses: the desk imports these bytes into its own library, and neither side
/// becomes a live link to the other. A peer that has gone since the menu was drawn is reported as
/// gone rather than substituted with another.
pub(super) async fn fetch_visualizer_document(
    state: &AppState,
    instance: &str,
) -> Result<(String, Vec<u8>), ApiError> {
    let peer = state
        .discovery
        .peer(instance)
        .ok_or_else(|| ApiError::not_found("visualizer"))?;
    if peer.role != Role::Editor {
        return Err(ApiError::bad_request(
            "only a Viz editor holds a document to load",
        ));
    }
    let name = peer
        .show
        .clone()
        .ok_or_else(|| ApiError::bad_request("that visualizer has no document open"))?;
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|error| ApiError::internal(format!("http client: {error}")))?;
    // An editor answers on every interface its machine has, and only one of them may be the
    // network this desk is on. Each is tried in turn rather than failing on the first.
    let mut failure = None;
    for base in peer.base_urls() {
        match download(&client, &base).await {
            Ok(bytes) if bytes.is_empty() => {
                failure = Some(format!("{} sent an empty show", peer.name));
            }
            Ok(bytes) => return Ok((name, bytes)),
            Err(error) => failure = Some(format!("{}: {error}", peer.name)),
        }
    }
    Err(ApiError::bad_gateway(failure.unwrap_or_else(|| {
        format!("{} has no address this desk can reach", peer.name)
    })))
}

async fn download(client: &reqwest::Client, base: &str) -> Result<Vec<u8>, String> {
    let response = client
        .get(format!("{base}/api/v2/document/download"))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("answered {} for its document", response.status()));
    }
    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| error.to_string())
}
