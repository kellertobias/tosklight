//! Discovering ToskLight Media Servers and reading their current output configuration.
//!
//! The desk patches only what the current Media Server contract describes: an output with one of
//! the two current personalities. Anything else is still listed, with the reason the desk cannot
//! patch it and what the operator should do, so a server that needs an update is never silently
//! patched with a guessed personality.

use super::*;
use light_wire::v2::output_control::{
    DiscoveredMediaAddressUpdateRequest, DiscoveredMediaOutput, DiscoveredMediaServer,
    MediaServerDiscovery,
};

/// The Media Server personalities and the Media Server fixture mode each one is patched with.
pub(super) const MEDIA_PERSONALITY_MODES: [(&str, &str); 2] =
    [("two-layers", "2 layers"), ("eight-layers", "8 layers")];

/// The fields of `GET /api/v2/outputs/{id}/configuration` the desk reads.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeMediaOutputConfiguration {
    id: Uuid,
    name: String,
    personality: String,
    protocol: String,
    universe: u16,
    start_address: u16,
    dmx_pending_restart: bool,
    tempo_source: String,
    #[serde(default)]
    speed_group: Option<u32>,
}

#[derive(Deserialize)]
struct NativeMediaOutputSummary {
    id: String,
}

const OUTDATED_SERVER: &str = "This Media Server needs an update before the desk can patch it. \
    Update ToskLight Media to the current version, then refresh discovery.";

/// Reads one output configuration as the current Media Server contract describes it.
///
/// A server that still reports a channel-layout choice predates the retirement of the legacy
/// layouts, and a configuration that lacks a current field comes from an older server. Both are
/// listed as outdated rather than patched with a guessed personality.
pub(super) fn classify_media_output(
    fallback_id: Option<Uuid>,
    value: serde_json::Value,
) -> Option<DiscoveredMediaOutput> {
    let reported_id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .and_then(|id| id.parse::<Uuid>().ok());
    let id = reported_id.or(fallback_id)?;
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Output")
        .to_owned();
    let retired_layout = value.get("personalityLayout").is_some();
    let parsed = (!retired_layout)
        .then(|| serde_json::from_value::<NativeMediaOutputConfiguration>(value.clone()).ok())
        .flatten();
    let Some(output) = parsed else {
        return Some(outdated_output(id, name, &value));
    };
    let mode = MEDIA_PERSONALITY_MODES
        .iter()
        .find(|(personality, _)| *personality == output.personality)
        .map(|(_, mode)| (*mode).to_owned());
    let issue = mode.is_none().then(|| {
        format!(
            "Output personality '{}' cannot be patched from this desk. Choose 2 layers or \
             8 layers under Settings > Network & DMX on the Media Server, or update ToskLight \
             Media, then refresh discovery.",
            output.personality
        )
    });
    Some(DiscoveredMediaOutput {
        id: output.id,
        name: output.name,
        personality: output.personality,
        protocol: output.protocol,
        universe: output.universe,
        start_address: output.start_address,
        dmx_pending_restart: output.dmx_pending_restart,
        mode,
        tempo_source: Some(output.tempo_source),
        speed_group: output.speed_group,
        issue,
    })
}

fn outdated_output(id: Uuid, name: String, value: &serde_json::Value) -> DiscoveredMediaOutput {
    let text = |field: &str| {
        value
            .get(field)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let number = |field: &str| {
        value
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .and_then(|number| u16::try_from(number).ok())
            .unwrap_or_default()
    };
    DiscoveredMediaOutput {
        id,
        name,
        personality: text("personality"),
        protocol: text("protocol"),
        universe: number("universe"),
        start_address: number("startAddress"),
        dmx_pending_restart: false,
        mode: None,
        tempo_source: None,
        speed_group: None,
        issue: Some(OUTDATED_SERVER.to_owned()),
    }
}

pub(super) async fn discover_native_media_servers(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Json<MediaServerDiscovery>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    let discovered = match discover_servers(Duration::from_millis(750)).await {
        Ok(servers) => servers,
        Err(error) => {
            return Ok(Json(MediaServerDiscovery {
                servers: Vec::new(),
                discovery_error: Some(format!(
                    "Media Server discovery failed: {error}. Manual patching still works; \
                     check the desk's network interface and retry."
                )),
            }));
        }
    };
    let client = native_media_client()?;
    let servers = futures_util::future::join_all(discovered.into_iter().map(|server| {
        let base = format!("http://{}:{TOSKLIGHT_MEDIA_HTTP_PORT}/api/v2", server.host);
        inspect_discovered_native_media_server(client.clone(), server, base)
    }))
    .await;
    Ok(Json(MediaServerDiscovery {
        servers,
        discovery_error: None,
    }))
}

pub(super) async fn inspect_discovered_native_media_server(
    client: reqwest::Client,
    server: light_media::DiscoveredCitpServer,
    base: String,
) -> DiscoveredMediaServer {
    let unavailable = |name: String, status: String, instance, error: &str| DiscoveredMediaServer {
        key: format!("{}:{}", server.host, server.port),
        name,
        host: server.host.clone(),
        citp_port: server.port,
        status,
        instance,
        outputs: Vec::new(),
        error: Some(error.to_owned()),
    };
    let health = native_media_get::<NativeMediaHealth>(&client, &format!("{base}/health")).await;
    let Ok(health) = health else {
        return unavailable(
            server.name.clone(),
            "Unavailable".to_owned(),
            None,
            "The discovered Media Server did not answer its configuration API. Check that it \
             is running and reachable on port 8080, then refresh discovery.",
        );
    };
    let operator_name = format!("ToskLight Pixel Media - {}", health.instance);
    let summaries =
        native_media_get::<Vec<NativeMediaOutputSummary>>(&client, &format!("{base}/outputs"))
            .await;
    let Ok(summaries) = summaries else {
        return unavailable(
            operator_name,
            health.status,
            Some(health.instance),
            "The Media Server output configuration is unavailable. Refresh discovery; if it \
             persists, update ToskLight Media.",
        );
    };
    let outputs = futures_util::future::join_all(summaries.into_iter().map(|summary| {
        let client = client.clone();
        let url = format!("{base}/outputs/{}/configuration", summary.id);
        let fallback = summary.id.parse::<Uuid>().ok();
        async move {
            let value = match native_media_get::<serde_json::Value>(&client, &url).await {
                Ok(value) => value,
                // An outputs list without per-output configuration is an older server.
                Err(_) => serde_json::json!({}),
            };
            classify_media_output(fallback, value)
        }
    }))
    .await
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let error = if outputs.is_empty() {
        Some(
            "The Media Server has no readable output configuration. Refresh discovery; if it \
              persists, update ToskLight Media.",
        )
    } else if outputs.iter().all(|output| output.mode.is_none()) {
        Some(OUTDATED_SERVER)
    } else {
        None
    };
    DiscoveredMediaServer {
        key: format!("{}:{}", server.host, server.port),
        name: operator_name,
        host: server.host,
        citp_port: server.port,
        status: health.status,
        instance: Some(health.instance),
        outputs,
        error: error.map(str::to_owned),
    }
}

pub(super) async fn update_discovered_media_server_address(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<DiscoveredMediaAddressUpdateRequest>,
) -> Result<Json<DiscoveredMediaOutput>, ApiError> {
    let _session = command_http::authenticate_desk_mutation(&state, &headers, &desk)?;
    show.verify(&state)?;
    if input.request_id.trim().is_empty() {
        return Err(ApiError::bad_request(
            "Media Server address request_id is required",
        ));
    }
    let host = input
        .host
        .parse::<Ipv4Addr>()
        .map_err(|_| ApiError::bad_request("Media Server host is invalid"))?;
    if host.is_unspecified() || host.is_multicast() {
        return Err(ApiError::bad_request("Media Server host is not reachable"));
    }
    if input.start_address == 0 || input.start_address > 512 {
        return Err(ApiError::bad_request(
            "Media Server start address must be between 1 and 512",
        ));
    }
    let base = format!("http://{host}:{TOSKLIGHT_MEDIA_HTTP_PORT}/api/v2");
    update_media_output_address(&native_media_client()?, &base, &input)
        .await
        .map(Json)
}

/// Stores a new DMX address on the Media Server and answers with its authoritative output.
pub(super) async fn update_media_output_address(
    client: &reqwest::Client,
    base: &str,
    input: &DiscoveredMediaAddressUpdateRequest,
) -> Result<DiscoveredMediaOutput, ApiError> {
    let url = format!("{base}/outputs/{}/configuration/update", input.output_id);
    let response = client
        .post(url)
        .json(&serde_json::json!({
            "requestId": input.request_id,
            "universe": input.universe,
            "startAddress": input.start_address,
        }))
        .send()
        .await
        .map_err(native_media_unavailable)?;
    let value = native_media_response(response)
        .await?
        .json::<serde_json::Value>()
        .await
        .map_err(|_| ApiError::unavailable("Media Server returned an invalid configuration"))?;
    classify_media_output(None, value)
        .ok_or_else(|| ApiError::unavailable("Media Server returned an invalid configuration"))
}

#[cfg(test)]
#[path = "media_discovery_tests.rs"]
mod tests;
