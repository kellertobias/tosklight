//! The v2 PosiStageNet routes: read what is configured and arriving, edit what is configured.
//!
//! The configuration is show data, so an edit is an intent update carrying only the fields that
//! changed (api-rules §3), absorbed by a replay window so a dropped response cannot bind a tracker
//! twice. The status is not stored anywhere — it is what the receiver currently knows — and it
//! rides along with the read so a tab that has just been opened does not need a second request.
//!
//! An accepted edit is installed into the running receiver before it returns. Waiting for the show
//! to be re-read would mean an operator flicking the enable switch and watching nothing happen for
//! a second, which reads as a broken switch rather than a slow one.

use super::show_objects_v2::active_entry;
use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::psn as wire_psn;
use std::collections::VecDeque;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 256;
const PSN_OBJECT_ID: &str = "main";

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/psn", get(read_psn))
        .route("/api/v2/psn/update", post(update_psn))
}

async fn read_psn(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let show_id = context.resolve(&state)?;
    let (revision, configuration) = stored_configuration(&state, show_id)?;
    let status = state.psn.tick(super::psn::listener::now_millis()).status;
    Ok(json_with_etag(
        revision,
        wire_psn::PsnSnapshot {
            revision,
            configuration: wire_configuration(&configuration),
            status: wire_status(&status),
        },
    ))
}

async fn update_psn(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire_psn::PsnUpdateRequest>,
) -> Result<Response, ApiError> {
    let session = authenticate(&state, &headers)?;
    if request.request_id.is_empty() || request.request_id.len() > 128 {
        return Err(ApiError::bad_request(
            "request_id must be between 1 and 128 characters",
        ));
    }
    let show_id = context.resolve(&state)?;
    let key = ReplayKey {
        desk_id: session.desk.id,
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };
    let activation = state.active_show.acquire().await;
    if let Some(replayed) = state.replay.lookup_psn(&key, &request).await? {
        return Ok(json_with_etag(replayed.revision, replayed));
    }
    let (revision, stored) = stored_configuration(&state, show_id)?;
    let updated = apply(stored.clone(), &request)?;
    updated.validate().map_err(ApiError::bad_request)?;
    let outcome = if updated == stored {
        wire_psn::PsnUpdateOutcome {
            request_id: request.request_id.clone(),
            revision,
            configuration: wire_configuration(&updated),
            unchanged: true,
            replayed: false,
        }
    } else {
        let body = serde_json::to_value(&updated).map_err(|error| {
            ApiError::internal(format!(
                "the tracking configuration could not be stored: {error}"
            ))
        })?;
        let action = active_show_object_action(
            operator_action_context(&session, light_application::ActionSource::Http)
                .with_request_id(&request.request_id),
            show_id,
            vec![put_active_show_object(
                light_application::ActiveShowObjectKind::Psn,
                PSN_OBJECT_ID,
                revision,
                body,
            )?],
        );
        let (result, _activation) =
            run_active_show_object_action_async(&state, activation, action).await?;
        let change = result
            .changes
            .first()
            .expect("one PSN mutation returns one change");
        emit(
            &state,
            "show_object_changed",
            serde_json::json!({
                "show_id": show_id,
                "kind": "psn",
                "id": PSN_OBJECT_ID,
                "revision": change.object_revision
            }),
        );
        wire_psn::PsnUpdateOutcome {
            request_id: request.request_id.clone(),
            revision: change.object_revision,
            configuration: wire_configuration(&updated),
            unchanged: false,
            replayed: false,
        }
    };
    // The receiver hears about it now rather than when the show is next read: an enable switch
    // that takes a second to do anything reads as broken.
    state.psn.install(updated);
    state.replay.insert_psn(key, request, outcome.clone()).await;
    Ok(json_with_etag(outcome.revision, outcome))
}

/// What the show holds, and at which object revision. An absent object is the valid "off,
/// nothing bound" configuration, at revision zero.
pub(super) fn stored_configuration(
    state: &AppState,
    show_id: light_core::ShowId,
) -> Result<(u64, light_application::PsnConfiguration), ApiError> {
    let entry = active_entry(state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let object = store
        .object_with_portable_revision("psn", PSN_OBJECT_ID)
        .map_err(ApiError::store)?
        .1;
    match object {
        None => Ok((0, light_application::PsnConfiguration::default())),
        Some(object) => {
            let revision = object.revision;
            // A body the desk cannot read is not a reason to refuse to open the tab: the operator
            // is shown the default and told nothing was bound, which is recoverable by hand.
            let configuration = serde_json::from_value(object.body).unwrap_or_default();
            Ok((revision, configuration))
        }
    }
}

/// Apply an intent update to what is stored.
fn apply(
    mut configuration: light_application::PsnConfiguration,
    request: &wire_psn::PsnUpdateRequest,
) -> Result<light_application::PsnConfiguration, ApiError> {
    if let Some(enabled) = request.enabled {
        configuration.enabled = enabled;
    }
    if let Some(group) = &request.group {
        configuration.group = group
            .parse()
            .map_err(|_| ApiError::bad_request(format!("{group} is not an IPv4 address")))?;
    }
    if let Some(port) = request.port {
        configuration.port = port;
    }
    if let Some(interface) = &request.interface {
        configuration.interface =
            match interface {
                None => None,
                Some(address) => Some(address.parse().map_err(|_| {
                    ApiError::bad_request(format!("{address} is not an IPv4 address"))
                })?),
            };
    }
    if let Some(stale_after_millis) = request.stale_after_millis {
        configuration.stale_after_millis = stale_after_millis;
    }
    if let Some(calibration) = request.calibration {
        configuration.calibration = light_application::PsnCalibration {
            offset_metres: calibration.offset_metres,
            rotation_degrees: calibration.rotation_degrees,
            scale: calibration.scale,
        };
    }
    if let Some(bindings) = &request.bindings {
        configuration.bindings = bindings
            .iter()
            .map(|binding| light_application::PsnBinding {
                id: binding.id,
                tracker_id: binding.tracker_id,
                point_fixture_id: binding.point_fixture_id,
                enabled: binding.enabled,
            })
            .collect();
    }
    if let Some(zones) = &request.zones {
        configuration.zones = zones
            .iter()
            .map(|zone| light_application::PsnZone {
                id: zone.id,
                name: zone.name.clone(),
                min_metres: zone.min_metres,
                max_metres: zone.max_metres,
                tracker_ids: zone.tracker_ids.clone(),
                enter_macro_id: zone.enter_macro_id,
                leave_macro_id: zone.leave_macro_id,
                dwell_millis: zone.dwell_millis,
            })
            .collect();
    }
    Ok(configuration)
}

fn wire_configuration(
    configuration: &light_application::PsnConfiguration,
) -> wire_psn::PsnConfigurationProjection {
    wire_psn::PsnConfigurationProjection {
        enabled: configuration.enabled,
        group: configuration.group.to_string(),
        port: configuration.port,
        interface: configuration.interface.map(|address| address.to_string()),
        stale_after_millis: configuration.stale_after_millis,
        calibration: wire_psn::PsnCalibrationProjection {
            offset_metres: configuration.calibration.offset_metres,
            rotation_degrees: configuration.calibration.rotation_degrees,
            scale: configuration.calibration.scale,
        },
        bindings: configuration
            .bindings
            .iter()
            .map(|binding| wire_psn::PsnBindingProjection {
                id: binding.id,
                tracker_id: binding.tracker_id,
                point_fixture_id: binding.point_fixture_id,
                enabled: binding.enabled,
            })
            .collect(),
        zones: configuration
            .zones
            .iter()
            .map(|zone| wire_psn::PsnZoneProjection {
                id: zone.id,
                name: zone.name.clone(),
                min_metres: zone.min_metres,
                max_metres: zone.max_metres,
                tracker_ids: zone.tracker_ids.clone(),
                enter_macro_id: zone.enter_macro_id,
                leave_macro_id: zone.leave_macro_id,
                dwell_millis: zone.dwell_millis,
            })
            .collect(),
    }
}

pub(super) fn wire_status(
    status: &super::psn::service::PsnStatus,
) -> wire_psn::PsnStatusProjection {
    wire_psn::PsnStatusProjection {
        enabled: status.enabled,
        listening_on: status.listening_on.clone(),
        health: status.health.map(|health| match health {
            super::psn::service::PsnHealth::Silent => wire_psn::PsnHealthProjection::Silent,
            super::psn::service::PsnHealth::Receiving => wire_psn::PsnHealthProjection::Receiving,
            super::psn::service::PsnHealth::Stale { silent_for_millis } => {
                wire_psn::PsnHealthProjection::Stale { silent_for_millis }
            }
        }),
        system_names: status.system_names.clone(),
        trackers: status
            .trackers
            .iter()
            .map(|tracker| wire_psn::PsnTrackerProjection {
                tracker_id: tracker.tracker_id,
                name: tracker.name.clone(),
                position_metres: tracker.position_metres,
                age_millis: tracker.age_millis,
                stale: tracker.stale,
                source: tracker.source.to_string(),
            })
            .collect(),
        placements: status
            .placements
            .iter()
            .map(|placement| wire_psn::PsnPlacementProjection {
                binding_id: placement.binding_id,
                point_fixture_id: placement.point_fixture_id,
                position_metres: placement.position_metres,
                out_of_reach: placement.out_of_reach,
            })
            .collect(),
        occupied_zone_ids: status.occupied_zones.clone(),
        frames: status.frames,
        ignored_datagrams: status.ignored_datagrams,
        error: status.error.clone(),
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    pub(super) desk_id: Uuid,
    pub(super) session_id: Uuid,
    pub(super) request_id: String,
}

struct ReplayEntry {
    request: wire_psn::PsnUpdateRequest,
    outcome: wire_psn::PsnUpdateOutcome,
}

/// The replay window that makes an edit safe to resend (api-rules §3).
#[derive(Default)]
pub(super) struct PsnReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl PsnReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        request: &wire_psn::PsnUpdateRequest,
    ) -> Result<Option<wire_psn::PsnUpdateOutcome>, ApiError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if &entry.request != request {
            return Err(ApiError::conflict(
                "request_id was already used for a different PSN edit",
            ));
        }
        let mut replay = entry.outcome.clone();
        replay.replayed = true;
        Ok(Some(replay))
    }

    pub(super) fn insert(
        &mut self,
        key: ReplayKey,
        request: wire_psn::PsnUpdateRequest,
        outcome: wire_psn::PsnUpdateOutcome,
    ) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, ReplayEntry { request, outcome });
        while self.entries.len() > REQUEST_CACHE_ENTRY_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

fn json_with_etag<T: serde::Serialize>(revision: u64, body: T) -> Response {
    let mut response = Json(body).into_response();
    if let Ok(value) = header::HeaderValue::from_str(&format!("\"{revision}\"")) {
        response.headers_mut().insert(header::ETAG, value);
    }
    response
}
