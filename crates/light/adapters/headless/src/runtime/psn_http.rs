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
    // Show recovery: the failed show's PSN configuration and Macros are not served; the answer is
    // the empty show's "off, nothing bound".
    let recovery = state.active_show.in_recovery();
    let (revision, configuration) = if recovery {
        (0, light_application::PsnConfiguration::default())
    } else {
        stored_configuration(&state, show_id)?
    };
    let status = state.psn.status(super::psn::listener::now_millis());
    Ok(json_with_etag(
        revision,
        wire_psn::PsnSnapshot {
            revision,
            configuration: wire_configuration(&configuration),
            status: wire_status(&status),
            points: points(&state),
            macros: if recovery {
                Vec::new()
            } else {
                macros(&state, show_id)?
            },
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
    let show_id = context.resolve_writable(&state)?;
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
    updated
        .validate_update(&stored)
        .map_err(ApiError::bad_request)?;
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
    // The accepted document installed the receiver inside the activation boundary. Reinstalling
    // here after that permit was released could put this show's tracking back after a show switch.
    state.replay.insert_psn(key, request, outcome.clone()).await;
    Ok(json_with_etag(outcome.revision, outcome))
}

/// Every 3D Point in the show, as something an operator can pick.
///
/// The desk decides what counts as a 3D Point — a fixture carrying the point position
/// attributes — because the tab must not have an opinion about how the show is resolved.
fn points(state: &AppState) -> Vec<wire_psn::PsnPointProjection> {
    let snapshot = state.output.snapshot();
    let mut points: Vec<_> = snapshot
        .fixtures
        .iter()
        .filter(|fixture| {
            fixture.definition.heads.iter().any(|head| {
                head.parameters.iter().any(|parameter| {
                    parameter.attribute.0.as_ref() == super::psn::bindings::POINT_AXIS_ATTRIBUTES[0]
                })
            })
        })
        .map(|fixture| wire_psn::PsnPointProjection {
            fixture_id: fixture.fixture_id.0,
            name: fixture.name.clone(),
            fixture_number: fixture.fixture_number,
        })
        .collect();
    points.sort_by(|left, right| {
        (left.fixture_number, &left.name).cmp(&(right.fixture_number, &right.name))
    });
    points
}

/// Every Macro in the show, for a zone's enter and leave.
fn macros(
    state: &AppState,
    show_id: light_core::ShowId,
) -> Result<Vec<wire_psn::PsnMacroProjection>, ApiError> {
    let entry = active_entry(state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let mut macros: Vec<_> = store
        .objects("macro")
        .map_err(ApiError::store)?
        .into_iter()
        .filter_map(|object| {
            let id = Uuid::parse_str(&object.id).ok()?;
            Some(wire_psn::PsnMacroProjection {
                id,
                number: u16::try_from(object.body.get("number")?.as_u64()?).ok()?,
                name: object.body.get("name")?.as_str()?.to_owned(),
            })
        })
        .collect();
    macros.sort_by_key(|entry| entry.number);
    Ok(macros)
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

/// Where a prepared PSN configuration came from. Loading never validates or rejects a stored
/// body: an absent object is the valid "off" configuration, a decodable body is used as stored
/// (the receiver withholds conflicting rows), and an undecodable body passively becomes the
/// default while the stored object is left untouched for repair.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))] // Reported by TL-584's owned activation workflow.
pub(super) enum PsnConfigurationOrigin {
    Absent,
    Stored,
    Undecodable(String),
}

/// PSN configuration derived from one exact portable document without touching live tracking,
/// sockets, events or the show file.
#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))] // Installed by TL-584's owned activation workflow.
pub(super) struct PreparedPsnConfiguration {
    pub(super) show_id: light_core::ShowId,
    pub(super) show_revision: u64,
    pub(super) object_revision: u64,
    pub(super) configuration: light_application::PsnConfiguration,
    pub(super) origin: PsnConfigurationOrigin,
}

impl PreparedPsnConfiguration {
    pub(super) fn for_document(document: &light_show::PortableShowDocument) -> Self {
        let (object_revision, configuration, origin) = match document.object("psn", PSN_OBJECT_ID) {
            None => (
                0,
                light_application::PsnConfiguration::default(),
                PsnConfigurationOrigin::Absent,
            ),
            Some(object) => match serde_json::from_value(object.body().clone()) {
                Ok(configuration) => (
                    object.revision(),
                    configuration,
                    PsnConfigurationOrigin::Stored,
                ),
                Err(error) => (
                    object.revision(),
                    light_application::PsnConfiguration::default(),
                    PsnConfigurationOrigin::Undecodable(error.to_string()),
                ),
            },
        };
        Self {
            show_id: document.id(),
            show_revision: document.revision().value(),
            object_revision,
            configuration,
            origin,
        }
    }
}

/// Install tracking as part of the accepted show transaction, including Undo and imports. Packet
/// handling never reads the portable store; the next listener tick sees this exact owner.
pub(super) fn install_document(state: &AppState, document: &light_show::PortableShowDocument) {
    let configuration = PreparedPsnConfiguration::for_document(document).configuration;
    state
        .psn
        .install_for_show(Some(document.id()), configuration);
    super::psn::output::publish(
        state,
        state
            .psn
            .committed_tracking_frame(super::psn::listener::now_millis()),
    );
}

/// Cold show activation/startup only. Equal configuration in a different show still installs a
/// different tracking owner and cannot reuse the previous show's held marker positions.
pub(super) fn install_current_show(state: &AppState) {
    let show_id = state.active_show.current().map(|show| show.id);
    let configuration = show_id
        .and_then(|show_id| stored_configuration(state, show_id).ok())
        .map(|(_, configuration)| configuration)
        .unwrap_or_default();
    state.psn.reset_for_show(show_id, configuration);
    super::psn::output::publish(
        state,
        state
            .psn
            .committed_tracking_frame(super::psn::listener::now_millis()),
    );
}

/// Memory-only activation installer for a configuration prepared from the compiled document.
/// Like cold activation it always resets tracking ownership, even for an equal configuration or
/// the same show ID, and it never reopens the show file. The receiver's listener observes the
/// new owner on its own schedule; this function does not bind or rebind sockets.
#[cfg_attr(not(test), allow(dead_code))] // Called by TL-584's owned activation workflow.
pub(super) fn install_prepared(state: &AppState, prepared: &PreparedPsnConfiguration) {
    state
        .psn
        .reset_for_show(Some(prepared.show_id), prepared.configuration.clone());
    super::psn::output::publish(
        state,
        state
            .psn
            .committed_tracking_frame(super::psn::listener::now_millis()),
    );
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
                accepted_sample: tracker.accepted_sample.map(wire_accepted_sample),
            })
            .collect(),
        sources: Some(
            status
                .sources
                .iter()
                .map(|source| wire_psn::PsnSourceProjection {
                    source: source.source.to_string(),
                    accepted_sample: source.accepted_sample.map(wire_accepted_sample),
                    diagnostics: wire_psn::PsnIngressDiagnosticsProjection {
                        duplicate_datagrams: source.diagnostics.duplicate_datagrams,
                        rejected_datagrams: source.diagnostics.rejected_datagrams,
                        incomplete_frames: source.diagnostics.incomplete_frames,
                        ambiguous_datagrams: source.diagnostics.ambiguous_datagrams,
                        invalid_positions: source.diagnostics.invalid_positions,
                    },
                })
                .collect(),
        ),
        diagnostics: Some(wire_psn::PsnReceiverDiagnosticsProjection {
            source_count: status.sources.len() as u64,
            source_capacity: status.source_capacity as u64,
            rejected_source_datagrams: status.rejected_source_datagrams,
            invalid_calibrated_positions: status.invalid_calibrated_positions as u64,
            conflicting_binding_rows: status.conflicting_binding_rows as u64,
        }),
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

fn wire_accepted_sample(
    identity: super::psn::service::TrackingSampleIdentity,
) -> wire_psn::PsnAcceptedSampleProjection {
    wire_psn::PsnAcceptedSampleProjection {
        source: identity.source.to_string(),
        source_generation: identity.source_generation,
        source_epoch: identity.sample.id.source_epoch,
        sequence: identity.sample.id.sequence,
        frame_id: identity.sample.frame_id,
        sender_timestamp_micros: identity.sample.sender_timestamp_micros,
        accepted_at_millis: identity.sample.accepted_at_millis,
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

#[cfg(test)]
mod status_tests {
    use super::super::psn::service::{
        PsnStatus, SourceReport, TrackerReport, TrackingSampleIdentity,
    };
    use super::*;
    use light_psn_wire::{PsnAcceptedSample, PsnIngressDiagnostics, PsnSampleId};

    #[test]
    fn passive_diagnostics_preserve_source_and_older_positional_sample_without_errors() {
        let source = "10.0.0.1:56565".parse().unwrap();
        let position = TrackingSampleIdentity {
            source_generation: 7,
            source,
            sample: PsnAcceptedSample {
                id: PsnSampleId {
                    source_epoch: 2,
                    sequence: 4,
                },
                frame_id: 254,
                sender_timestamp_micros: 50_000,
                accepted_at_millis: 100,
            },
        };
        let current = TrackingSampleIdentity {
            sample: PsnAcceptedSample {
                id: PsnSampleId {
                    source_epoch: 2,
                    sequence: 5,
                },
                frame_id: 255,
                sender_timestamp_micros: 60_000,
                accepted_at_millis: 110,
            },
            ..position
        };
        let status = PsnStatus {
            enabled: true,
            trackers: vec![TrackerReport {
                tracker_id: 3,
                name: None,
                position_metres: Some([1.0, 2.0, 3.0]),
                age_millis: 40,
                stale: false,
                source,
                accepted_sample: Some(position),
            }],
            sources: vec![SourceReport {
                source,
                accepted_sample: Some(current),
                diagnostics: PsnIngressDiagnostics {
                    duplicate_datagrams: 1,
                    rejected_datagrams: 2,
                    incomplete_frames: 3,
                    ambiguous_datagrams: 4,
                    invalid_positions: 5,
                },
            }],
            rejected_source_datagrams: 6,
            source_capacity: 64,
            invalid_calibrated_positions: 7,
            conflicting_binding_rows: 2,
            ..Default::default()
        };
        let before = status.clone();
        let wire = wire_status(&status);
        assert_eq!(status, before, "projection must be read-only");
        assert_eq!(
            wire.trackers[0].accepted_sample,
            Some(wire_accepted_sample(position))
        );
        assert_eq!(
            wire.trackers[0].age_millis, 40,
            "do not replace positional age with whole-frame age"
        );
        let sources = wire.sources.as_ref().unwrap();
        assert_eq!(
            sources[0].accepted_sample,
            Some(wire_accepted_sample(current))
        );
        assert_eq!(
            sources[0].diagnostics,
            wire_psn::PsnIngressDiagnosticsProjection {
                duplicate_datagrams: 1,
                rejected_datagrams: 2,
                incomplete_frames: 3,
                ambiguous_datagrams: 4,
                invalid_positions: 5,
            }
        );
        assert_eq!(
            wire.diagnostics.unwrap(),
            wire_psn::PsnReceiverDiagnosticsProjection {
                source_count: 1,
                source_capacity: 64,
                rejected_source_datagrams: 6,
                invalid_calibrated_positions: 7,
                conflicting_binding_rows: 2,
            }
        );
        assert!(wire.error.is_none());
        let encoded = serde_json::to_value(wire).unwrap();
        assert!(encoded.get("error").is_none());
        assert!(encoded.get("output_frame_sequence").is_none());
    }

    #[test]
    fn an_unheard_source_exposes_absent_sample_without_inventing_zero_identity() {
        let status = PsnStatus {
            sources: vec![SourceReport {
                source: "10.0.0.1:56565".parse().unwrap(),
                accepted_sample: None,
                diagnostics: PsnIngressDiagnostics::default(),
            }],
            ..Default::default()
        };
        let wire = wire_status(&status);
        let value = serde_json::to_value(wire).unwrap();
        assert!(value["sources"][0].get("accepted_sample").is_none());
        assert_eq!(value["diagnostics"]["source_count"], 1);
        assert!(value.get("error").is_none());
    }
}
