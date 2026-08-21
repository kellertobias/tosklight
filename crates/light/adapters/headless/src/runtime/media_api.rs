use super::*;
use light_wire::v2::output_control::{
    DiscoveredMediaAddressUpdateRequest, DiscoveredMediaOutput, DiscoveredMediaServer,
    MediaServerDiscovery,
};

const TOSKLIGHT_MEDIA_SERVER_PROFILE_ID: &str = "0a14fb60-280d-5ef1-aa4a-2ff11bd06943";
const TOSKLIGHT_MEDIA_HTTP_PORT: u16 = 8080;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeMediaHealth {
    status: String,
    instance: String,
    outputs: usize,
    catalog_revision: u64,
    catalog_items: usize,
}

#[derive(Deserialize)]
struct NativeMediaAddress {
    folder: u8,
    file: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeMediaTextResponse {
    address: NativeMediaAddress,
    name: String,
    enabled: bool,
    kind: String,
    text: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeMediaEffectParameterResponse {
    id: String,
    label: String,
    value: f32,
    default_value: f32,
    // A Media Server older than the advertised bounds sends none of these.
    #[serde(default)]
    minimum: Option<f32>,
    #[serde(default)]
    maximum: Option<f32>,
    #[serde(default)]
    step: Option<f32>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeMediaEffectSlotResponse {
    index: usize,
    effect_type: Option<String>,
    label: String,
    enabled: bool,
    mix: f32,
    supported: bool,
    capability_detail: Option<String>,
    parameters: Vec<NativeMediaEffectParameterResponse>,
}

#[derive(Deserialize)]
struct NativeMediaLayerResponse {
    effects: Vec<NativeMediaEffectSlotResponse>,
}

#[derive(Deserialize)]
struct NativeMediaOutputResponse {
    id: String,
    layers: Vec<NativeMediaLayerResponse>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeMediaOutputSummary {
    id: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeMediaOutputConfiguration {
    id: Uuid,
    name: String,
    personality: String,
    protocol: String,
    universe: u16,
    start_address: u16,
    dmx_pending_restart: bool,
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
                discovery_error: Some(format!("Media Server discovery failed: {error}")),
            }));
        }
    };
    let client = native_media_client()?;
    let servers = futures_util::future::join_all(
        discovered
            .into_iter()
            .map(|server| inspect_discovered_native_media_server(client.clone(), server)),
    )
    .await;
    Ok(Json(MediaServerDiscovery {
        servers,
        discovery_error: None,
    }))
}

async fn inspect_discovered_native_media_server(
    client: reqwest::Client,
    server: light_media::DiscoveredCitpServer,
) -> DiscoveredMediaServer {
    let key = format!("{}:{}", server.host, server.port);
    let base = format!("http://{}:{TOSKLIGHT_MEDIA_HTTP_PORT}/api/v2", server.host);
    let health = native_media_get::<NativeMediaHealth>(&client, &format!("{base}/health")).await;
    let Ok(health) = health else {
        return DiscoveredMediaServer {
            key,
            name: server.name,
            host: server.host,
            citp_port: server.port,
            status: "Unavailable".to_owned(),
            instance: None,
            outputs: Vec::new(),
            error: Some(
                "The discovered Media Server did not answer its native configuration API"
                    .to_owned(),
            ),
        };
    };
    let operator_name = format!("ToskLight Pixel Media - {}", health.instance);
    let summaries =
        native_media_get::<Vec<NativeMediaOutputSummary>>(&client, &format!("{base}/outputs"))
            .await;
    let Ok(summaries) = summaries else {
        return DiscoveredMediaServer {
            key,
            name: operator_name,
            host: server.host,
            citp_port: server.port,
            status: health.status,
            instance: Some(health.instance),
            outputs: Vec::new(),
            error: Some("The Media Server output configuration is unavailable".to_owned()),
        };
    };
    let outputs = futures_util::future::join_all(summaries.into_iter().map(|output| {
        let client = client.clone();
        let base = base.clone();
        async move {
            native_media_get::<NativeMediaOutputConfiguration>(
                &client,
                &format!("{base}/outputs/{}/configuration", output.id),
            )
            .await
            .ok()
        }
    }))
    .await
    .into_iter()
    .flatten()
    .map(|output| DiscoveredMediaOutput {
        id: output.id,
        name: output.name,
        personality: output.personality,
        protocol: output.protocol,
        universe: output.universe,
        start_address: output.start_address,
        dmx_pending_restart: output.dmx_pending_restart,
    })
    .collect::<Vec<_>>();
    let error = outputs
        .is_empty()
        .then(|| "The Media Server has no readable output configuration".to_owned());
    DiscoveredMediaServer {
        key,
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
    let client = native_media_client()?;
    let url = format!(
        "http://{host}:{TOSKLIGHT_MEDIA_HTTP_PORT}/api/v2/outputs/{}/configuration/update",
        input.output_id
    );
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
    let output = native_media_response(response)
        .await?
        .json::<NativeMediaOutputConfiguration>()
        .await
        .map_err(|_| ApiError::unavailable("Media Server returned an invalid configuration"))?;
    Ok(Json(DiscoveredMediaOutput {
        id: output.id,
        name: output.name,
        personality: output.personality,
        protocol: output.protocol,
        universe: output.universe,
        start_address: output.start_address,
        dmx_pending_restart: output.dmx_pending_restart,
    }))
}

#[derive(Default, Deserialize)]
pub(super) struct VisualizationQuery {
    #[serde(default)]
    pub(super) preload: bool,
    #[serde(default)]
    pub(super) dynamic_stack_only: bool,
    pub(super) fixture_ids: Option<String>,
}
pub(super) async fn media_servers(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    let fixtures = state
        .output
        .snapshot()
        .fixtures
        .iter()
        .filter(|fixture| is_media_server_fixture(fixture) || is_audio_player_fixture(fixture))
        .map(|fixture| {
            let name = format!(
                "{} {}",
                fixture.definition.manufacturer, fixture.definition.model
            );
            if is_audio_player_fixture(fixture) {
                let player = state.internal_audio.player(fixture);
                return serde_json::json!({
                    "fixture_id": fixture.fixture_id,
                    "fixture_number": fixture.fixture_number,
                    "name": name,
                    "kind": "audio_player",
                    "endpoint": serde_json::Value::Null,
                    "native_action": serde_json::Value::Null,
                    "layers": media_layers(fixture),
                    "master_attributes": master_head_attributes(fixture),
                    "status": {
                        "online": player.diagnostic.is_none(),
                        "last_success": serde_json::Value::Null,
                        "last_error": player.diagnostic,
                    },
                    "audio": {
                        "folder": player.folder,
                        "file": player.file,
                        "volume_percent": player.volume_percent,
                        "transport": player.transport,
                        "repeat": player.repeat,
                        "source": player.source,
                    },
                });
            }
            let endpoint = fixture
                .direct_control
                .as_ref()
                .filter(|endpoint| endpoint.protocol == light_fixture::DirectControlProtocol::Citp);
            let status = state.media.status(fixture.fixture_id);
            serde_json::json!({
                "fixture_id": fixture.fixture_id,
                "fixture_number": fixture.fixture_number,
                "name": name,
                "kind": "media_server",
                "endpoint": endpoint,
                "native_action": native_media_action(fixture),
                "layers": media_layers(fixture),
                "master_attributes": master_head_attributes(fixture),
                "status": status,
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({ "fixtures": fixtures })))
}

pub(super) async fn inspect_media_server(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Json<light_media::MediaServerSnapshot>, ApiError> {
    let session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    let address = media_endpoint(&state, fixture_id)?;
    match async {
        let mut client = CitpClient::connect(address, Duration::from_secs(3)).await?;
        client.inspect().await
    }
    .await
    {
        Ok(mut snapshot) => {
            snapshot.capabilities.layers = media_layer_capabilities(&state, fixture_id)?;
            snapshot.capabilities.native_action = native_media_action_for(&state, fixture_id)?;
            state.media.record_inspection(fixture_id, snapshot.clone());
            state.media.record_status(fixture_id, None);
            emit(
                &state,
                "media_inspected",
                serde_json::json!({"session_id":session.id,"fixture_id":fixture_id}),
            );
            Ok(Json(snapshot))
        }
        Err(error) => {
            state.media.clear_inspection(fixture_id);
            state
                .media
                .record_status(fixture_id, Some(error.to_string()));
            Err(ApiError::unavailable(error.to_string()))
        }
    }
}

pub(super) async fn native_media_snapshot(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Json<light_wire::v2::output_control::NativeMediaSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    let endpoint = native_media_endpoint(&state, fixture_id)?;
    let base = format!("http://{endpoint}/api/v2");
    let health_url = format!("{base}/health");
    let outputs_url = format!("{base}/outputs");
    let client = native_media_client()?;
    let (health, outputs) = tokio::try_join!(
        native_media_get::<NativeMediaHealth>(&client, &health_url),
        native_media_get::<Vec<NativeMediaOutputResponse>>(&client, &outputs_url),
    )?;
    let output = outputs.into_iter().next();
    Ok(Json(light_wire::v2::output_control::NativeMediaSnapshot {
        endpoint: format!("http://{endpoint}"),
        status: health.status,
        instance: health.instance,
        outputs: health.outputs,
        catalog_revision: health.catalog_revision,
        catalog_items: health.catalog_items,
        text_slots: Vec::new(),
        effect_controls_available: output.is_some(),
        output_id: output.as_ref().map(|output| output.id.clone()),
        effect_layers: output
            .map(|output| {
                output
                    .layers
                    .into_iter()
                    .map(|layer| layer.effects.into_iter().map(native_effect_slot).collect())
                    .collect()
            })
            .unwrap_or_default(),
    }))
}

pub(super) async fn update_native_media_effect(
    State(state): State<AppState>,
    Path((fixture_id, layer)): Path<(light_core::FixtureId, u8)>,
    show: ShowContext,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<
        light_wire::v2::output_control::NativeMediaEffectUpdateRequest,
    >,
) -> Result<Json<Vec<light_wire::v2::output_control::NativeMediaEffectSlot>>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    if input.request_id.trim().is_empty() {
        return Err(ApiError::bad_request(
            "native Media effect request_id is required",
        ));
    }
    let endpoint = native_media_endpoint(&state, fixture_id)?;
    let base = format!("http://{endpoint}/api/v2");
    let client = native_media_client()?;
    let outputs =
        native_media_get::<Vec<NativeMediaOutputResponse>>(&client, &format!("{base}/outputs"))
            .await?;
    let output = outputs
        .first()
        .ok_or_else(|| ApiError::unavailable("Media Server has no configured output"))?;
    if usize::from(layer) >= output.layers.len() {
        return Err(ApiError::bad_request("Media Server layer is unavailable"));
    }
    let update = native_effect_update(&input, advertised_step(output, layer, &input.control_id))?;
    let response = client
        .post(format!(
            "{base}/outputs/{}/layers/{layer}/native-effects/update",
            output.id
        ))
        .json(&update)
        .send()
        .await
        .map_err(native_media_unavailable)?;
    let response = native_media_response(response).await?;
    let updated = response
        .json::<NativeMediaOutputResponse>()
        .await
        .map_err(|_| ApiError::unavailable("Media Server returned invalid effect state"))?;
    let layer = updated
        .layers
        .get(usize::from(layer))
        .ok_or_else(|| ApiError::unavailable("Media Server returned an invalid layer state"))?;
    Ok(Json(
        layer
            .effects
            .clone()
            .into_iter()
            .map(native_effect_slot)
            .collect(),
    ))
}

fn native_effect_slot(
    slot: NativeMediaEffectSlotResponse,
) -> light_wire::v2::output_control::NativeMediaEffectSlot {
    light_wire::v2::output_control::NativeMediaEffectSlot {
        index: slot.index,
        effect_type: slot.effect_type,
        label: slot.label,
        enabled: slot.enabled,
        mix: slot.mix,
        supported: slot.supported,
        capability_detail: slot.capability_detail,
        parameters: slot
            .parameters
            .into_iter()
            .map(
                |parameter| light_wire::v2::output_control::NativeMediaEffectParameter {
                    id: parameter.id,
                    label: parameter.label,
                    value: parameter.value,
                    default_value: parameter.default_value,
                    minimum: parameter.minimum,
                    maximum: parameter.maximum,
                    step: parameter.step,
                },
            )
            .collect(),
    }
}

/// What the Media Server says this control's parameter accepts, when it advertises it.
///
/// A whole-number parameter lands in an integer field over there, and serde refuses `8.0` for a
/// `u8`. Reading the advertised step keeps that decision with the server that owns the field
/// instead of restating its types here.
fn advertised_step(output: &NativeMediaOutputResponse, layer: u8, control_id: &str) -> Option<f32> {
    let parameter_id = control_id.splitn(3, '-').nth(2)?;
    output
        .layers
        .get(usize::from(layer))?
        .effects
        .iter()
        .flat_map(|effect| effect.parameters.iter())
        .find(|parameter| parameter.id == parameter_id)
        .and_then(|parameter| parameter.step)
}

fn native_effect_update(
    input: &light_wire::v2::output_control::NativeMediaEffectUpdateRequest,
    step: Option<f32>,
) -> Result<serde_json::Value, ApiError> {
    let mut parts = input.control_id.split('-');
    if parts.next() != Some("effect") {
        return Err(ApiError::bad_request("invalid native effect control"));
    }
    let slot = parts
        .next()
        .and_then(|value| value.parse::<u8>().ok())
        .filter(|value| *value < 4)
        .ok_or_else(|| ApiError::bad_request("invalid native effect slot"))?;
    let control = parts.collect::<Vec<_>>().join("-");
    let field = match control.as_str() {
        "type" => "effectType",
        "enabled" => "effectEnabled",
        "tv-curvature" => "tvCurvature",
        "distortion" => "effectDistortion",
        "image-grain" => "imageGrain",
        "compression-damage" => "compressionDamage",
        "block-size" => "blockSize",
        "tile-displacement" => "tileDisplacement",
        "chroma-damage" => "chromaDamage",
        "glitching" => "effectGlitching",
        "cycle-interval" => "cycleInterval",
        "blur-amount" => "blurAmount",
        "feedback-amount" => "feedbackAmount",
        "feedback-motion" => "feedbackMotion",
        "feedback-direction" => "feedbackDirection",
        "beat-move-amount" => "beatMoveAmount",
        "beat-move-direction" => "beatMoveDirection",
        "beat-move-decay" => "beatMoveDecay",
        "kaleidoscope-repetitions" => "kaleidoscopeRepetitions",
        "kaleidoscope-angle" => "kaleidoscopeAngle",
        "rasterize-mode" => "rasterizeMode",
        "rasterize-dot-size" => "rasterizeDotSize",
        "beat-scan-width" => "beatScanWidth",
        "beat-scan-edge" => "beatScanEdge",
        "beat-scan-falloff" => "beatScanFalloff",
        "beat-scan-duration" => "beatScanDuration",
        "beat-scale-amount" => "beatScaleAmount",
        "beat-turn-enabled" => "beatTurnEnabled",
        "beat-turn-rotation" => "beatTurnRotation",
        "beat-scale-decay" => "beatScaleDecay",
        "beat-grid-density" => "beatGridDensity",
        "beat-grid-height" => "beatGridHeight",
        "beat-grid-duration" => "beatGridDuration",
        "beat-grid-origin" => "beatGridOrigin",
        "beat-grid-hue" => "beatGridHue",
        "beat-grid-brightness" => "beatGridBrightness",
        "beat-form-enlargement" => "beatFormEnlargement",
        "beat-form-lifetime" => "beatFormLifetime",
        "beat-form-density" => "beatFormDensity",
        "beat-form-variation" => "beatFormVariation",
        "drawn-strength" => "drawnStrength",
        "drawn-line-detail" => "drawnLineDetail",
        _ => return Err(ApiError::bad_request("unknown native effect control")),
    };
    // A Media Server that advertises nothing still owns two integer fields, and refusing them
    // silently is what made Mirror repetitions unreachable.
    const WHOLE_NUMBER_FIELDS: [&str; 2] = ["kaleidoscopeRepetitions", "beatFormDensity"];
    let whole_number = step.is_some_and(|step| step >= 1.0) || WHOLE_NUMBER_FIELDS.contains(&field);
    let value = match (
        input.number_value,
        input.string_value.as_ref(),
        input.boolean_value,
    ) {
        (Some(value), None, None) if whole_number => serde_json::json!(value.round() as i64),
        (Some(value), None, None) => serde_json::json!(value),
        (None, Some(value), None) => serde_json::json!(value),
        (None, None, Some(value)) => serde_json::json!(value),
        _ => return Err(ApiError::bad_request("native effect value is invalid")),
    };
    Ok(serde_json::json!({"effectSlot": slot, (field): value}))
}

#[cfg(test)]
mod native_effect_update_tests {
    use super::native_effect_update;
    use light_wire::v2::output_control::NativeMediaEffectUpdateRequest;

    fn request(control_id: &str) -> NativeMediaEffectUpdateRequest {
        NativeMediaEffectUpdateRequest {
            request_id: "request-1".into(),
            control_id: control_id.into(),
            number_value: None,
            string_value: None,
            boolean_value: None,
        }
    }

    #[test]
    fn translates_a_typed_effect_parameter_for_the_selected_slot() {
        let mut input = request("effect-2-blur-amount");
        input.number_value = Some(0.8);

        let update = native_effect_update(&input, Some(0.01)).expect("valid native effect update");
        assert_eq!(update["effectSlot"], 2);
        assert!((update["blurAmount"].as_f64().expect("number") - 0.8).abs() < 0.000_001);
    }

    #[test]
    fn translates_boolean_effect_controls_without_treating_them_as_text() {
        let mut input = request("effect-1-enabled");
        input.boolean_value = Some(false);

        assert_eq!(
            native_effect_update(&input, None).expect("valid native effect update"),
            serde_json::json!({"effectSlot": 1, "effectEnabled": false})
        );
    }

    #[test]
    fn rejects_text_source_controls_from_the_effect_path() {
        let mut input = request("clock-format");
        input.string_value = Some("HH:mm".into());

        assert!(native_effect_update(&input, None).is_err());
    }

    /// A whole-number parameter reaches a `u8` field as an integer. Sent as `8.0` the Media
    /// Server answers 400 and the control does nothing at all.
    #[test]
    fn sends_a_whole_number_parameter_as_an_integer() {
        let mut input = request("effect-0-kaleidoscope-repetitions");
        input.number_value = Some(8.0);

        assert_eq!(
            native_effect_update(&input, Some(1.0)).expect("valid native effect update"),
            serde_json::json!({"effectSlot": 0, "kaleidoscopeRepetitions": 8})
        );
        assert_eq!(
            native_effect_update(&input, None).expect("valid native effect update"),
            serde_json::json!({"effectSlot": 0, "kaleidoscopeRepetitions": 8}),
            "a Media Server that advertises no step still owns an integer field"
        );
    }
}

pub(super) async fn update_native_media_text(
    State(state): State<AppState>,
    Path((fixture_id, folder, file)): Path<(light_core::FixtureId, u8, u8)>,
    show: ShowContext,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<light_wire::v2::output_control::NativeMediaTextUpdateRequest>,
) -> Result<Json<light_wire::v2::output_control::NativeMediaTextSlot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    if input.request_id.trim().is_empty() {
        return Err(ApiError::bad_request(
            "native Media text request_id is required",
        ));
    }
    let endpoint = native_media_endpoint(&state, fixture_id)?;
    let url = format!("http://{endpoint}/api/v2/text/{folder}/{file}/update");
    let response = native_media_client()?
        .post(url)
        .json(&serde_json::json!({
            "requestId": input.request_id,
            "text": input.text,
        }))
        .send()
        .await
        .map_err(native_media_unavailable)?;
    let response = native_media_response(response).await?;
    let slot = response
        .json::<NativeMediaTextResponse>()
        .await
        .map_err(|_| ApiError::unavailable("Media Server returned an invalid text response"))?;
    Ok(Json(native_text_slot(slot)))
}

fn native_text_slot(
    slot: NativeMediaTextResponse,
) -> light_wire::v2::output_control::NativeMediaTextSlot {
    light_wire::v2::output_control::NativeMediaTextSlot {
        folder: slot.address.folder,
        file: slot.address.file,
        name: slot.name,
        enabled: slot.enabled,
        kind: slot.kind,
        text: slot.text,
    }
}

fn native_media_client() -> Result<reqwest::Client, ApiError> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(4))
        .build()
        .map_err(|_| ApiError::unavailable("Native Media client is unavailable"))
}

async fn native_media_get<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
) -> Result<T, ApiError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(native_media_unavailable)?;
    native_media_response(response)
        .await?
        .json::<T>()
        .await
        .map_err(|_| ApiError::unavailable("Media Server returned an invalid response"))
}

/// The Media Server explains its own refusals; a bare status code leaves the operator guessing
/// which control it disliked, so its message travels with the failure.
async fn native_media_response(response: reqwest::Response) -> Result<reqwest::Response, ApiError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let detail = response
        .json::<NativeMediaErrorResponse>()
        .await
        .ok()
        .map(|error| error.message)
        .filter(|message| !message.trim().is_empty());
    Err(ApiError::unavailable(match detail {
        Some(detail) => format!("Media Server refused this change: {detail}"),
        None => format!("Media Server native API answered {status}"),
    }))
}

#[derive(Deserialize)]
struct NativeMediaErrorResponse {
    message: String,
}

fn native_media_unavailable(_: reqwest::Error) -> ApiError {
    ApiError::unavailable("Media Server native API is unavailable")
}

fn native_media_action_for(
    state: &AppState,
    fixture_id: light_core::FixtureId,
) -> Result<Option<String>, ApiError> {
    let snapshot = state.output.snapshot();
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == fixture_id)
        .ok_or_else(|| ApiError::not_found("fixture"))?;
    Ok(native_media_action(fixture))
}

fn native_media_action(fixture: &light_fixture::PatchedFixture) -> Option<String> {
    let profile_id = fixture.definition.profile_id?;
    (profile_id.0.to_string() == TOSKLIGHT_MEDIA_SERVER_PROFILE_ID)
        .then(|| "tosklight_media_v2".to_owned())
}

fn native_media_endpoint(
    state: &AppState,
    fixture_id: light_core::FixtureId,
) -> Result<SocketAddr, ApiError> {
    let snapshot = state.output.snapshot();
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == fixture_id)
        .ok_or_else(|| ApiError::not_found("fixture"))?;
    if native_media_action(fixture).is_none() {
        return Err(ApiError::bad_request(
            "fixture profile does not support ToskLight native Media controls",
        ));
    }
    let citp = fixture
        .direct_control
        .as_ref()
        .filter(|endpoint| endpoint.protocol == light_fixture::DirectControlProtocol::Citp)
        .ok_or_else(|| ApiError::bad_request("fixture has no CITP endpoint"))?;
    Ok(SocketAddr::new(citp.ip_address, TOSKLIGHT_MEDIA_HTTP_PORT))
}

pub(super) async fn apply_media_library_selection(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<light_wire::v2::output_control::MediaLibrarySelectionRequest>,
) -> Result<Json<light_wire::v2::output_control::MediaLibrarySelectionOutcome>, ApiError> {
    let session = command_http::authenticate_desk_mutation(&state, &headers, &desk)?;
    show.verify(&state)?;
    if input.request_id.trim().is_empty() {
        return Err(ApiError::bad_request(
            "media selection request_id is required",
        ));
    }
    let inspection = state.media.inspection(fixture_id).ok_or_else(|| {
        ApiError::conflict("Media library must be inspected before selecting a file")
    })?;
    if inspection.library_revision != input.expected_library_revision {
        return Err(ApiError::conflict(format!(
            "Media library revision is stale: expected {}, current {}",
            input.expected_library_revision, inspection.library_revision
        )));
    }
    if !inspection
        .files
        .iter()
        .any(|file| file.folder_id == input.folder && file.id == input.file)
    {
        return Err(ApiError::conflict(
            "The selected media file is absent from the inspected library revision",
        ));
    }
    let capabilities = inspection
        .capabilities
        .layers
        .iter()
        .find(|capability| {
            layer_head_index(
                &state,
                fixture_id,
                light_core::FixtureId(input.layer_fixture_id),
            ) == Some(capability.layer)
        })
        .ok_or_else(|| ApiError::bad_request("selected fixture is not a layer of this server"))?;
    let (folder_attribute, file_attribute, supported) = match input.kind {
        light_wire::v2::output_control::MediaLibraryKind::Content => {
            ("media.folder", "media.file", capabilities.content_library)
        }
        light_wire::v2::output_control::MediaLibraryKind::Mask => (
            "media.mask.folder",
            "media.mask.file",
            capabilities.mask_library,
        ),
    };
    if !supported {
        return Err(ApiError::bad_request(
            "The selected layer does not advertise this library capability",
        ));
    }

    let context = operator_action_context(&session, light_application::ActionSource::UserInterface);
    let ports = command_http::ServerProgrammingPorts::new(&state, &session, "media", true);
    let values = state
        .programming
        .values_snapshot(&context, &ports)
        .map_err(|error| ApiError::conflict(error.message))?;
    let capture = state
        .programming
        .capture_mode_snapshot(&context, &ports)
        .map_err(|error| ApiError::conflict(error.message))?;
    let context = context
        .with_request_id(input.request_id.clone())
        .with_expected_revision(values.projection.revision);
    let timing = light_application::ProgrammingValueTiming::default();
    let fixture_id = light_core::FixtureId(input.layer_fixture_id);
    let normalized = |value: u8| light_core::AttributeValue::Normalized(f32::from(value) / 255.0);
    let command = light_application::ProgrammingValuesRequest {
        expected_capture_mode_revision: capture.projection.revision,
        command: light_application::ProgrammingValuesCommand::Batch {
            mutations: vec![
                light_application::ProgrammingValueMutation::SetFixture {
                    fixture_id,
                    attribute: light_core::AttributeKey(folder_attribute.into()),
                    value: normalized(input.folder),
                    timing,
                },
                light_application::ProgrammingValueMutation::SetFixture {
                    fixture_id,
                    attribute: light_core::AttributeKey(file_attribute.into()),
                    value: normalized(input.file),
                    timing,
                },
            ],
        },
    };
    let _activation = state.active_show.acquire().await;
    let result = state
        .programming
        .handle_values(
            light_application::ActionEnvelope { context, command },
            &ports,
        )
        .map_err(|error| ApiError::conflict(error.message))?;
    Ok(Json(
        light_wire::v2::output_control::MediaLibrarySelectionOutcome {
            request_id: input.request_id,
            library_revision: inspection.library_revision,
            programmer_revision: result.outcome.revision(),
        },
    ))
}

fn layer_head_index(
    state: &AppState,
    master_id: light_core::FixtureId,
    layer_id: light_core::FixtureId,
) -> Option<u16> {
    state
        .output
        .snapshot()
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == master_id)?
        .logical_heads
        .iter()
        .find(|head| head.fixture_id == layer_id)
        .map(|head| head.head_index)
}

fn media_layer_capabilities(
    state: &AppState,
    fixture_id: light_core::FixtureId,
) -> Result<Vec<light_media::MediaLayerCapabilities>, ApiError> {
    let snapshot = state.output.snapshot();
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == fixture_id)
        .ok_or_else(|| ApiError::not_found("fixture"))?;
    Ok(fixture
        .logical_heads
        .iter()
        .filter_map(|patched| {
            let head = fixture
                .definition
                .heads
                .iter()
                .find(|head| head.index == patched.head_index)?;
            let attributes = head
                .parameters
                .iter()
                .map(|parameter| parameter.attribute.0.as_str())
                .collect::<std::collections::BTreeSet<_>>();
            let content_library =
                attributes.contains("media.folder") && attributes.contains("media.file");
            let mask_library =
                attributes.contains("media.mask.folder") && attributes.contains("media.mask.file");
            let secondary_controls = head
                .parameters
                .iter()
                .map(|parameter| parameter.attribute.0.as_str())
                .filter(|attribute| attribute.starts_with("media."))
                .filter(|attribute| {
                    !matches!(
                        *attribute,
                        "media.folder" | "media.file" | "media.mask.folder" | "media.mask.file"
                    )
                })
                .map(|attribute| light_media::MediaControlCapability {
                    attribute: attribute.to_owned(),
                })
                .collect();
            Some(light_media::MediaLayerCapabilities {
                layer: patched.head_index,
                content_library,
                mask_library,
                secondary_controls,
            })
        })
        .collect())
}

pub(super) async fn refresh_media_thumbnails(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    show: ShowContext,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<light_wire::v2::output_control::MediaThumbnailRefreshRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    if !(1..=2).contains(&input.library_type) || input.library_level > 3 {
        return Err(ApiError::bad_request("invalid CITP library type or level"));
    }
    let library = library_id(&input);
    let address = media_endpoint(&state, fixture_id)?;
    let result = async {
        let mut client = CitpClient::connect(address, Duration::from_secs(3)).await?;
        client
            .request_thumbnail(
                input.library_type,
                library,
                &input.elements,
                input.width,
                input.height,
            )
            .await
    }
    .await;
    match result {
        Ok(images) => {
            let count = images.len();
            state
                .media
                .put_thumbnails(images.into_iter().map(|(element, image)| {
                    (
                        ThumbnailKey {
                            fixture: fixture_id.0.to_string(),
                            library_type: input.library_type,
                            library,
                            element,
                        },
                        image,
                    )
                }))
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
            state.media.record_status(fixture_id, None);
            emit(
                &state,
                "media_thumbnails_refreshed",
                serde_json::json!({"session_id":session.id,"fixture_id":fixture_id,"count":count}),
            );
            Ok(Json(
                serde_json::json!({"fixture_id":fixture_id,"count":count}),
            ))
        }
        Err(error) => {
            state
                .media
                .record_status(fixture_id, Some(error.to_string()));
            emit(
                &state,
                "media_server_offline",
                serde_json::json!({"fixture_id":fixture_id,"error":error.to_string()}),
            );
            Err(ApiError::unavailable(error.to_string()))
        }
    }
}

pub(super) async fn refresh_media_preview(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    show: ShowContext,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<light_wire::v2::output_control::MediaPreviewRefreshRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    let address = media_endpoint(&state, fixture_id)?;
    let result = async {
        let mut client = CitpClient::connect(address, Duration::from_secs(3)).await?;
        client
            .request_preview(input.source, input.width, input.height)
            .await
    }
    .await;
    match result {
        Ok(image) => {
            let format = image.format;
            let width = image.width;
            let height = image.height;
            state
                .media
                .put_preview(
                    PreviewKey {
                        fixture: fixture_id.0.to_string(),
                        source: input.source,
                    },
                    image,
                )
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
            state.media.record_status(fixture_id, None);
            emit(
                &state,
                "media_preview_refreshed",
                serde_json::json!({"session_id":session.id,"fixture_id":fixture_id,"source":input.source}),
            );
            Ok(Json(
                serde_json::json!({"fixture_id":fixture_id,"source":input.source,"format":format,"width":width,"height":height}),
            ))
        }
        Err(error) => {
            state
                .media
                .record_status(fixture_id, Some(error.to_string()));
            emit(
                &state,
                "media_server_offline",
                serde_json::json!({"fixture_id":fixture_id,"error":error.to_string()}),
            );
            Err(ApiError::unavailable(error.to_string()))
        }
    }
}

pub(super) async fn media_preview(
    State(state): State<AppState>,
    Path((fixture_id, source)): Path<(light_core::FixtureId, u16)>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    cached_image_response(
        state.media.preview(&PreviewKey {
            fixture: fixture_id.0.to_string(),
            source,
        }),
        "preview",
    )
}

pub(super) async fn media_thumbnail(
    State(state): State<AppState>,
    Path((fixture_id, folder, element)): Path<(light_core::FixtureId, u8, u8)>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    cached_image_response(
        state.media.thumbnail(&ThumbnailKey {
            fixture: fixture_id.0.to_string(),
            library_type: 1,
            library: LibraryId {
                level: 1,
                ids: [folder, 0, 0],
            },
            element,
        }),
        "thumbnail",
    )
}

pub(super) fn media_endpoint(
    state: &AppState,
    fixture_id: light_core::FixtureId,
) -> Result<SocketAddr, ApiError> {
    let snapshot = state.output.snapshot();
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == fixture_id)
        .ok_or_else(|| ApiError::not_found("fixture"))?;
    let endpoint = fixture
        .direct_control
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("fixture has no direct-control endpoint"))?;
    if endpoint.protocol != light_fixture::DirectControlProtocol::Citp {
        return Err(ApiError::bad_request(
            "fixture direct-control endpoint is not CITP",
        ));
    }
    Ok(SocketAddr::new(endpoint.ip_address, endpoint.port))
}
pub(super) fn library_id(
    input: &light_wire::v2::output_control::MediaThumbnailRefreshRequest,
) -> LibraryId {
    LibraryId {
        level: input.library_level,
        ids: [input.library_1, input.library_2, input.library_3],
    }
}
pub(super) fn cached_image_response(
    image: Option<light_media::CachedImage>,
    kind: &str,
) -> Result<Response, ApiError> {
    let image = image.ok_or_else(|| ApiError::not_found(format!("cached media {kind}")))?;
    let mut response = image.image.bytes.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(image.image.format.mime()),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("private, max-age=5"),
    );
    if let Ok(received_at) = image.received_at.duration_since(std::time::UNIX_EPOCH) {
        response.headers_mut().insert(
            header::HeaderName::from_static("x-light-received-at-millis"),
            header::HeaderValue::from_str(&received_at.as_millis().to_string())
                .expect("valid received-at header"),
        );
    }
    response.headers_mut().insert(
        header::HeaderName::from_static("x-light-image-width"),
        header::HeaderValue::from_str(&image.image.width.to_string()).expect("valid width header"),
    );
    response.headers_mut().insert(
        header::HeaderName::from_static("x-light-image-height"),
        header::HeaderValue::from_str(&image.image.height.to_string())
            .expect("valid height header"),
    );
    Ok(response)
}
