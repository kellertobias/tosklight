//! Authenticated, revisioned installation API for USB-DMX endpoint identities.

use super::*;
use crate::tolerant_json::TolerantJson;
use axum::extract::rejection::JsonRejection;
use light_output::{UsbEndpointConfiguration, UsbEndpointDiagnostic, UsbEndpointDocument};
use light_usb_dmx_serial::{DiscoveredUsbSerialDevice, discover_usb_serial_devices};

pub(super) const USB_ENDPOINTS_SETTING: &str = "usb_dmx_endpoints_v1";

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/usb-dmx/endpoints", get(snapshot))
        .route("/api/v2/usb-dmx/endpoints/update", post(update))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct UpdateRequest {
    request_id: String,
    expected_revision: u64,
    action: UpdateAction,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum UpdateAction {
    Upsert {
        endpoint: UsbEndpointConfiguration,
    },
    Remove {
        endpoint_id: String,
    },
    /// Explicitly replaces a preserved malformed setting with a fresh revisioned document.
    ResetMalformed,
}

#[derive(Serialize)]
struct EndpointSnapshot {
    document: UsbEndpointDocument,
    diagnostics: Vec<UsbEndpointDiagnostic>,
    discovered_devices: Vec<DiscoveredUsbSerialDevice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discovery_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    configuration_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    replayed: bool,
}

async fn snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<EndpointSnapshot>, ApiError> {
    authenticate(&state, &headers)?;
    let (document, configuration_error) = load(&state)?;
    let (discovered_devices, discovery_error) = discover();
    Ok(Json(EndpointSnapshot {
        document,
        diagnostics: state.output.usb_diagnostics(),
        discovered_devices,
        discovery_error,
        configuration_error,
        request_id: None,
        replayed: false,
    }))
}

async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Result<TolerantJson<UpdateRequest>, JsonRejection>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let TolerantJson(request) =
        request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    show_objects_v2::validate_request_id(&request.request_id)?;
    let key =
        desk_management_v2::ReplayKey::new(session.id, "usb-dmx-endpoints", &request.request_id);
    let fingerprint =
        serde_json::to_value(&request).map_err(|error| ApiError::internal(error.to_string()))?;
    if let Some(mut replay) = state
        .replay
        .lookup_desk_management(&key, &fingerprint)
        .await?
    {
        replay["replayed"] = true.into();
        return Ok(Json(replay));
    }

    let _configuration_guard = state.output.lock_usb_configuration().await;
    if let Some(mut replay) = state
        .replay
        .lookup_desk_management(&key, &fingerprint)
        .await?
    {
        replay["replayed"] = true.into();
        return Ok(Json(replay));
    }

    let (mut document, configuration_error) = load(&state)?;
    if configuration_error.is_none() && matches!(&request.action, UpdateAction::ResetMalformed) {
        return Err(ApiError::bad_request(
            "reset_malformed is only valid when the stored USB endpoint document is malformed",
        ));
    }
    if let Some(error) = configuration_error
        && !matches!(&request.action, UpdateAction::ResetMalformed)
    {
        return Err(ApiError::conflict(format!(
            "stored USB endpoint document is malformed; use reset_malformed to replace it: {error}"
        )));
    }
    if request.expected_revision != document.revision {
        return Err(ApiError::conflict(format!(
            "USB endpoint revision conflict: expected {}, current {}",
            request.expected_revision, document.revision
        )));
    }
    apply_action(&mut document, request.action)?;
    document.revision = document.revision.saturating_add(1);
    document.validate().map_err(ApiError::bad_request)?;
    let serialized =
        serde_json::to_string(&document).map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .installation
        .set_setting(USB_ENDPOINTS_SETTING, &serialized)
        .map_err(ApiError::store)?;
    state
        .output
        .configure_usb_endpoints(&document)
        .map_err(ApiError::internal)?;
    let (discovered_devices, discovery_error) = discover();
    let response = EndpointSnapshot {
        document,
        diagnostics: state.output.usb_diagnostics(),
        discovered_devices,
        discovery_error,
        configuration_error: None,
        request_id: Some(request.request_id),
        replayed: false,
    };
    let value =
        serde_json::to_value(response).map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .replay
        .insert_desk_management(key, fingerprint, value.clone())
        .await;
    Ok(Json(value))
}

fn discover() -> (Vec<DiscoveredUsbSerialDevice>, Option<String>) {
    match discover_usb_serial_devices() {
        Ok(devices) => (devices, None),
        Err(error) => (Vec::new(), Some(error)),
    }
}

fn load(state: &AppState) -> Result<(UsbEndpointDocument, Option<String>), ApiError> {
    let serialized = state
        .installation
        .setting(USB_ENDPOINTS_SETTING)
        .map_err(ApiError::store)?;
    Ok(parse_stored(serialized.as_deref()))
}

fn parse_stored(serialized: Option<&str>) -> (UsbEndpointDocument, Option<String>) {
    let Some(serialized) = serialized else {
        return (UsbEndpointDocument::default(), None);
    };
    match serde_json::from_str::<UsbEndpointDocument>(serialized) {
        Ok(document) => match document.validate() {
            Ok(()) => (document, None),
            Err(error) => (UsbEndpointDocument::default(), Some(error)),
        },
        Err(error) => (
            UsbEndpointDocument::default(),
            Some(format!("invalid JSON: {error}")),
        ),
    }
}

fn apply_action(document: &mut UsbEndpointDocument, action: UpdateAction) -> Result<(), ApiError> {
    match action {
        UpdateAction::Upsert { endpoint } => {
            if let Some(existing) = document
                .endpoints
                .iter_mut()
                .find(|existing| existing.endpoint_id == endpoint.endpoint_id)
            {
                *existing = endpoint;
            } else {
                document.endpoints.push(endpoint);
            }
        }
        UpdateAction::Remove { endpoint_id } => {
            let previous_len = document.endpoints.len();
            document
                .endpoints
                .retain(|endpoint| endpoint.endpoint_id != endpoint_id);
            if document.endpoints.len() == previous_len {
                return Err(ApiError::not_found(format!("USB endpoint `{endpoint_id}`")));
            }
        }
        UpdateAction::ResetMalformed => {}
    }
    document
        .endpoints
        .sort_by(|left, right| left.endpoint_id.cmp(&right.endpoint_id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_output::{UsbDeviceIdentity, UsbEndpointDriverKind};

    fn endpoint(id: &str) -> UsbEndpointConfiguration {
        UsbEndpointConfiguration {
            endpoint_id: id.into(),
            driver: UsbEndpointDriverKind::OpenDmx,
            identity: UsbDeviceIdentity {
                vendor_id: 0x0403,
                product_id: 0x6001,
                manufacturer: Some("FTDI".into()),
                product: Some("Open DMX".into()),
                usb_serial: Some(format!("serial-{id}")),
                widget_serial: None,
                port_topology_hint: None,
            },
            enabled: true,
        }
    }

    #[test]
    fn endpoint_actions_are_narrow_and_deterministically_ordered() {
        let mut document = UsbEndpointDocument::default();
        apply_action(
            &mut document,
            UpdateAction::Upsert {
                endpoint: endpoint("z"),
            },
        )
        .unwrap();
        apply_action(
            &mut document,
            UpdateAction::Upsert {
                endpoint: endpoint("a"),
            },
        )
        .unwrap();
        assert_eq!(document.endpoints[0].endpoint_id, "a");
        apply_action(
            &mut document,
            UpdateAction::Remove {
                endpoint_id: "a".into(),
            },
        )
        .unwrap();
        assert_eq!(document.endpoints.len(), 1);
    }

    #[test]
    fn malformed_installation_document_recovers_without_rewriting_the_source() {
        let original = "{not-json";
        let (document, error) = parse_stored(Some(original));
        assert_eq!(document, UsbEndpointDocument::default());
        assert!(error.unwrap().contains("invalid JSON"));
        assert_eq!(original, "{not-json");
    }
}
