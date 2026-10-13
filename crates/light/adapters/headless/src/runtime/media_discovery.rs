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
    #[serde(default)]
    name: Option<String>,
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
    let snapshot = state.output.snapshot();
    let discovered = discover_servers(Duration::from_millis(750))
        .await
        .map_err(|error| error.to_string());
    inspect_media_discovery(&snapshot.fixtures, discovered)
        .await
        .map(Json)
}

/// Persisted native physical masters provide authority even when UDP discovery is unavailable.
async fn inspect_media_discovery(
    fixtures: &[light_fixture::PatchedFixture],
    discovered: Result<Vec<light_media::DiscoveredCitpServer>, String>,
) -> Result<MediaServerDiscovery, ApiError> {
    let discovery_error = discovered.as_ref().err().map(|error| {
        format!("Media Server discovery failed: {error}. Configured servers remain available; check the desk's network interface and retry.")
    });
    let mut targets = std::collections::BTreeMap::new();
    for server in discovered.unwrap_or_default() {
        let endpoint = unconfigured_native_endpoint(&server);
        targets.insert((server.host.clone(), server.port), (server, endpoint, None));
    }
    let mut configured = std::collections::BTreeMap::new();
    for fixture in fixtures
        .iter()
        .filter(|fixture| native_media_action(fixture).is_some())
    {
        let Some(citp) = fixture.direct_control.as_ref() else {
            continue;
        };
        let key = (citp.ip_address.to_string(), citp.port);
        let candidate = native_media_fixture_endpoint(fixture).ok();
        let entry =
            configured
                .entry(key.clone())
                .or_insert((fixture.name.clone(), candidate, false));
        if entry.1 != candidate || candidate.is_none() {
            entry.2 = true;
        }
    }
    for ((host, port), (name, endpoint, conflict)) in configured {
        let server = light_media::DiscoveredCitpServer {
            name,
            host: host.clone(),
            port,
        };
        let issue = conflict.then(|| "Configured native Media Server has conflicting or invalid HTTP endpoints. Repair the matching CITP endpoint before retrying.".to_owned());
        targets.insert(
            (host, port),
            (server, endpoint.filter(|_| !conflict), issue),
        );
    }
    let client = native_media_client()?;
    let inspections = targets.into_values().map(|(server, endpoint, issue)| {
        inspect_discovery_target(client.clone(), server, endpoint, issue)
    });
    let servers = futures_util::future::join_all(inspections).await;
    Ok(MediaServerDiscovery {
        servers,
        discovery_error,
    })
}

async fn inspect_discovery_target(
    client: reqwest::Client,
    server: light_media::DiscoveredCitpServer,
    endpoint: Option<SocketAddr>,
    issue: Option<String>,
) -> DiscoveredMediaServer {
    if let Some(endpoint) = endpoint {
        return inspect_discovered_native_media_server(
            client,
            server,
            format!("http://{endpoint}/api/v2"),
        )
        .await;
    }
    unavailable_discovery(
        server,
        issue.unwrap_or_else(|| {
            "This CITP service is not a configured ToskLight native Media Server; \
         its HTTP endpoint will not be guessed."
                .to_owned()
        }),
    )
}

fn unconfigured_native_endpoint(server: &light_media::DiscoveredCitpServer) -> Option<SocketAddr> {
    server
        .name
        .strip_prefix("ToskLight Pixel — ")
        .filter(|instance| !instance.trim().is_empty())?;
    let host = server.host.parse::<Ipv4Addr>().ok()?;
    Some(SocketAddr::new(host.into(), TOSKLIGHT_MEDIA_HTTP_PORT))
}

fn unavailable_discovery(
    server: light_media::DiscoveredCitpServer,
    error: String,
) -> DiscoveredMediaServer {
    DiscoveredMediaServer {
        key: format!("{}:{}", server.host, server.port),
        name: server.name,
        host: server.host,
        citp_port: server.port,
        status: "Unavailable".to_owned(),
        instance: None,
        outputs: Vec::new(),
        error: Some(error),
    }
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
             is running and reachable at its selected HTTP endpoint, then refresh discovery.",
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
    let inspections = futures_util::future::join_all(summaries.into_iter().map(|summary| {
        let client = client.clone();
        let url = format!("{base}/outputs/{}/configuration", summary.id);
        let fallback = summary.id.parse::<Uuid>().ok();
        async move {
            match native_media_get::<serde_json::Value>(&client, &url).await {
                Ok(value) => Ok(classify_media_output(fallback, value)),
                Err(error) => Err(format!(
                    "{} ({}) configuration unavailable: {}. Refresh discovery to retry; \
                     patching this output requires a received compatible configuration.",
                    summary.name.as_deref().unwrap_or("Output"),
                    summary.id,
                    error.message
                )),
            }
        }
    }))
    .await;
    let mut outputs = Vec::new();
    let mut failures = Vec::new();
    for inspection in inspections {
        match inspection {
            Ok(Some(output)) => outputs.push(output),
            Ok(None) => failures.push(
                "An output returned configuration without a usable identity. Refresh discovery."
                    .to_owned(),
            ),
            Err(error) => failures.push(error),
        }
    }
    let error = if !failures.is_empty() {
        Some(failures.join("\n"))
    } else if outputs.is_empty() {
        Some("The Media Server has no readable output configuration. Refresh discovery.".to_owned())
    } else if outputs.iter().all(|output| output.mode.is_none()) {
        Some(OUTDATED_SERVER.to_owned())
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
        error,
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
    validate_media_input_universe(input.protocol.as_deref(), input.universe)?;
    let snapshot = state.output.snapshot();
    let endpoint = discovered_address_endpoint(&snapshot.fixtures, host, input.output_id)?;
    let base = format!("http://{endpoint}/api/v2");
    update_media_output_address(&native_media_client()?, &base, &input)
        .await
        .map(Json)
}

/// A configured host may only write the exact committed output binding, never a guessed API.
fn discovered_address_endpoint(
    fixtures: &[light_fixture::PatchedFixture],
    host: Ipv4Addr,
    output: Uuid,
) -> Result<SocketAddr, ApiError> {
    let native = fixtures
        .iter()
        .filter(|fixture| {
            native_media_action(fixture).is_some()
                && fixture
                    .direct_control
                    .as_ref()
                    .is_some_and(|endpoint| endpoint.ip_address == std::net::IpAddr::V4(host))
        })
        .collect::<Vec<_>>();
    if native.is_empty() {
        return Ok(SocketAddr::new(host.into(), TOSKLIGHT_MEDIA_HTTP_PORT));
    }
    let mut endpoints = std::collections::BTreeSet::new();
    for fixture in native.into_iter().filter(|fixture| {
        fixture
            .internal_bindings
            .output
            .as_deref()
            .and_then(|id| id.trim().parse::<Uuid>().ok())
            == Some(output)
    }) {
        endpoints.insert(native_media_fixture_endpoint(fixture)?);
    }
    match endpoints.len() {
        1 => Ok(*endpoints.first().expect("one committed endpoint")),
        0 => Err(ApiError::bad_request(
            "Media Server output has no committed native binding at this host; commit the exact output before changing its address",
        )),
        _ => Err(ApiError::bad_request(
            "Media Server output binding is ambiguous across HTTP endpoints; repair the copied output binding before retrying",
        )),
    }
}

/// The universe ranges each Media Server DMX input protocol accepts. Art-Net numbers its 15-bit
/// Port-Address from 0; sACN numbers universes from 1.
pub(super) fn validate_media_input_universe(
    protocol: Option<&str>,
    universe: u16,
) -> Result<(), ApiError> {
    let (label, minimum, maximum) = match protocol {
        None => return Ok(()),
        Some("art-net") => ("Art-Net", 0, 32_767),
        Some("sacn") => ("sACN", 1, 63_999),
        Some(other) => {
            return Err(ApiError::bad_request(format!(
                "Media Server DMX protocol '{other}' is not supported; use art-net or sacn"
            )));
        }
    };
    if universe < minimum || universe > maximum {
        return Err(ApiError::bad_request(format!(
            "{label} universe must be from {minimum} to {maximum}"
        )));
    }
    Ok(())
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
        .json(&media_address_update_body(input))
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

/// The Media Server configuration edit: the address, and the input protocol only when the desk
/// named one, so an omitted protocol never resets the server's own choice.
pub(super) fn media_address_update_body(
    input: &DiscoveredMediaAddressUpdateRequest,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "requestId": input.request_id,
        "universe": input.universe,
        "startAddress": input.start_address,
    });
    if let Some(protocol) = &input.protocol {
        body["protocol"] = serde_json::json!(protocol);
    }
    body
}

#[cfg(test)]
#[path = "media_discovery_tests.rs"]
mod tests;
