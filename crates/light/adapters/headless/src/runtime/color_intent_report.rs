//! `GET /api/v2/color-intent/report`: how faithfully each fixture head shows its colour, so an
//! approximate, out-of-gamut, wheel-limited, uncalibrated, or unsupported result is visible.

use super::*;
use light_wire::v2::attribute_configuration as wire;

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v2/color-intent/report", get(report))
}

#[derive(Deserialize)]
struct ReportQuery {
    /// Comma-separated fixture ids; every patched fixture when absent.
    #[serde(default)]
    fixtures: Option<String>,
}

async fn report(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<ReportQuery>,
) -> Result<Json<wire::ColorIntentReport>, ApiError> {
    authenticate(&state, &headers)?;
    context.resolve(&state)?;
    let wanted = query
        .fixtures
        .as_deref()
        .filter(|list| !list.trim().is_empty())
        .map(|list| {
            list.split(',')
                .map(|id| {
                    Uuid::parse_str(id.trim())
                        .map(light_core::FixtureId)
                        .map_err(|_| ApiError::bad_request(format!("invalid fixture id `{id}`")))
                })
                .collect::<Result<std::collections::HashSet<_>, _>>()
        })
        .transpose()?;
    let engine = state.output.engine();
    if state.output.live_family_adapters().engaged(engine) {
        return Ok(Json(accepted_frame_report(&state, wanted.as_ref())));
    }
    let heads = engine
        .color_intent_report(wanted.as_ref())
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let snapshot = state.output.snapshot();
    let fixture = |id: light_core::FixtureId| {
        snapshot
            .fixtures
            .iter()
            .find(|fixture| fixture.fixture_id == id)
    };
    Ok(Json(wire::ColorIntentReport {
        color_model: super::color_model_impact::wire_model(engine.color_model()),
        heads: heads
            .into_iter()
            .map(|head| {
                let patched = fixture(head.fixture_id);
                wire::ColorIntentHeadReport {
                    fixture_id: head.fixture_id.0,
                    fixture_number: patched.and_then(|fixture| fixture.fixture_number),
                    fixture_name: patched
                        .map(|fixture| fixture.name.clone())
                        .unwrap_or_default(),
                    owner_id: head.owner.0,
                    head_name: head.head_name,
                    has_target: head.target.is_some(),
                    quality: wire_quality(head.quality),
                    engine: head.engine.map(wire_engine),
                    delta_uv: head.delta_uv,
                    calibration_revision: head.calibration_revision,
                    uv: None,
                    direct: None,
                    note: None,
                }
            })
            .collect(),
        accepted_frame: None,
    }))
}

/// TL-594: with the family adapters engaged, the report names the Color results of the
/// accepted output frame the publication hub currently holds. Only heads with an active
/// requested colour (a Color sidecar) are reported; nothing is resolved against white. The
/// colour engine and calibration revision are not carried by the sidecar and stay absent.
pub(super) fn accepted_frame_report(
    state: &AppState,
    wanted: Option<&std::collections::HashSet<light_core::FixtureId>>,
) -> wire::ColorIntentReport {
    let engine = state.output.engine();
    let color_model = super::color_model_impact::wire_model(engine.color_model());
    let published = state.output.latest_visualization_frame();
    let accepted = published.as_ref().and_then(|frame| {
        state
            .output
            .live_family_adapters()
            .accepted_color(frame.generation, frame.sampled_at)
    });
    let (generation, heads) = engine.color_report_heads(wanted);
    let (Some(published), Some(accepted)) = (
        published,
        accepted.filter(|accepted| accepted.generation == generation),
    ) else {
        return wire::ColorIntentReport {
            color_model,
            heads: Vec::new(),
            accepted_frame: Some(wire::ColorIntentAcceptedFrame {
                state: wire::ColorIntentFrameState::NotYetAvailable,
                frame: None,
            }),
        };
    };
    let snapshot = &published.source_snapshot;
    // A static requested colour the frame could not show (no colour model) leaves no sidecar
    // and no requirement; the current values name it. Fixed/Dynamic ones are frame-held.
    let current = engine.resolved_values();
    let color = light_core::AttributeKey::color();
    let requested = |fixture: light_core::FixtureId, owner: light_core::FixtureId| {
        accepted.held(fixture, owner)
            || [owner, fixture].into_iter().any(|id| {
                matches!(
                    current.get(&(id, color.clone())),
                    Some(light_core::AttributeValue::ColorProgram(_))
                )
            })
    };
    let fixture = |id: light_core::FixtureId| {
        snapshot
            .fixtures
            .iter()
            .find(|fixture| fixture.fixture_id == id)
    };
    wire::ColorIntentReport {
        color_model,
        heads: heads
            .into_iter()
            .filter_map(|head| {
                let patched = fixture(head.fixture_id);
                let mode = patched.and_then(runtime_mode);
                let row = |quality, note: Option<String>| wire::ColorIntentHeadReport {
                    fixture_id: head.fixture_id.0,
                    fixture_number: patched.and_then(|fixture| fixture.fixture_number),
                    fixture_name: patched
                        .map(|fixture| fixture.name.clone())
                        .unwrap_or_default(),
                    owner_id: head.owner.0,
                    head_name: head.head_name.clone(),
                    has_target: true,
                    quality,
                    engine: None,
                    delta_uv: None,
                    calibration_revision: None,
                    uv: None,
                    direct: None,
                    note,
                };
                let Some(result) = accepted.head(head.fixture_id, head.owner, head.head_id) else {
                    // TL-552: a requested colour this frame held because the head has no
                    // colour model is named with the reason, never silently omitted.
                    let reason = mode?.color_model_exclusion(head.head_id)?;
                    return requested(head.fixture_id, head.owner).then(|| {
                        row(
                            wire::ColorResolutionQuality::Unsupported,
                            Some(format!("No colour model: {reason}")),
                        )
                    });
                };
                let note = mode.and_then(|mode| mode.derived_color_note(head.head_id));
                Some(wire::ColorIntentHeadReport {
                    delta_uv: result.delta_uv,
                    uv: result.uv.map(|uv| wire_uv(uv.status, uv.clipped)),
                    direct: result.direct.as_ref().map(wire_direct),
                    ..row(wire_quality(result.quality), note)
                })
            })
            .collect(),
        accepted_frame: Some(wire::ColorIntentAcceptedFrame {
            state: wire::ColorIntentFrameState::Accepted,
            frame: Some(published.identity()),
        }),
    }
}

/// The runtime-projected mode of a patched fixture (with any derived colour model).
fn runtime_mode(fixture: &light_fixture::PatchedFixture) -> Option<&light_fixture::FixtureMode> {
    fixture
        .definition
        .profile_snapshot
        .as_deref()?
        .mode(fixture.definition.mode_id?)
}

fn wire_quality(quality: light_core::ColorResolutionQuality) -> wire::ColorResolutionQuality {
    use light_core::ColorResolutionQuality as Quality;
    match quality {
        Quality::Exact => wire::ColorResolutionQuality::Exact,
        Quality::Approximate => wire::ColorResolutionQuality::Approximate,
        Quality::OutOfGamut => wire::ColorResolutionQuality::OutOfGamut,
        Quality::WheelLimited => wire::ColorResolutionQuality::WheelLimited,
        Quality::Uncalibrated => wire::ColorResolutionQuality::Uncalibrated,
        Quality::Unsupported => wire::ColorResolutionQuality::Unsupported,
    }
}

/// TL-550: the head's UV result beside, never inside, its visible match.
fn wire_uv(
    status: light_fixture::forward::UvFitStatus,
    clipped: bool,
) -> wire::ColorIntentUvReport {
    use light_fixture::forward::UvFitStatus as Status;
    wire::ColorIntentUvReport {
        status: match status {
            Status::NotRequested => wire::ColorIntentUvStatus::NotRequested,
            Status::Applied => wire::ColorIntentUvStatus::Applied,
            Status::Unsupported => wire::ColorIntentUvStatus::Unsupported,
        },
        clipped,
    }
}

/// TL-554: the head's passive Direct replay status. `Exact` is native identity replay; the
/// head's `quality` (measured comparison) says whether the visible colour matches.
fn wire_direct(
    status: &super::output_scheduler::physical_adapters::color::DirectColorStatus,
) -> light_wire::v2::native_color::ColorIntentDirectReport {
    use super::output_scheduler::physical_adapters::color::{
        DirectEstimateOrigin, DirectReplayOutcome,
    };
    use light_core::programming::{
        DirectCompatibility, DirectIncompatibility, NativeDriveLimit, UvFallback, VisibleFallback,
    };
    use light_wire::v2::native_color as native;
    let (replay, compatibility, uv) = match &status.replay {
        DirectReplayOutcome::Exact => (native::ColorIntentDirectReplay::Exact, None, None),
        DirectReplayOutcome::Fallback {
            compatibility,
            visible,
            uv,
        } => (
            match visible {
                VisibleFallback::Fit(_) => native::ColorIntentDirectReplay::Fallback,
                VisibleFallback::Hold => native::ColorIntentDirectReplay::NativeOnly,
            },
            Some(match compatibility {
                DirectCompatibility::Compatible => {
                    native::ColorIntentDirectCompatibility::Compatible
                }
                DirectCompatibility::Incompatible(DirectIncompatibility::NoNativeColor) => {
                    native::ColorIntentDirectCompatibility::NoNativeColor
                }
                DirectCompatibility::Incompatible(DirectIncompatibility::DifferentSource) => {
                    native::ColorIntentDirectCompatibility::DifferentSource
                }
                DirectCompatibility::Incompatible(DirectIncompatibility::ChangedLayout) => {
                    native::ColorIntentDirectCompatibility::ChangedLayout
                }
                DirectCompatibility::Unknown(_) => native::ColorIntentDirectCompatibility::Unknown,
            }),
            Some(match uv {
                UvFallback::Apply(_) => native::ColorIntentDirectUv::Apply,
                UvFallback::ParkOff => native::ColorIntentDirectUv::ParkOff,
            }),
        ),
    };
    native::ColorIntentDirectReport {
        replay,
        compatibility,
        uv,
        origin: match status.origin {
            DirectEstimateOrigin::Forward => native::ColorIntentDirectOrigin::Forward,
            DirectEstimateOrigin::Recorded => native::ColorIntentDirectOrigin::Recorded,
        },
        drive_limit: match status.drive_limit {
            NativeDriveLimit::Within => native::ColorIntentDriveLimit::Within,
            NativeDriveLimit::AboveModelMaximum => native::ColorIntentDriveLimit::AboveModelMaximum,
            NativeDriveLimit::Unknown => native::ColorIntentDriveLimit::Unknown,
        },
        limitations: status.limitations.clone(),
    }
}

fn wire_engine(engine: light_fixture::ColorIntentEngine) -> wire::ColorIntentEngine {
    use light_fixture::ColorIntentEngine as Engine;
    match engine {
        Engine::Additive => wire::ColorIntentEngine::Additive,
        Engine::Subtractive => wire::ColorIntentEngine::Subtractive,
        Engine::HueSaturation => wire::ColorIntentEngine::HueSaturation,
        Engine::Wheel => wire::ColorIntentEngine::Wheel,
    }
}

#[cfg(test)]
mod direct_status_tests {
    use super::super::output_scheduler::physical_adapters::color::{
        DirectColorStatus, DirectEstimateOrigin, DirectReplayOutcome,
    };
    use light_core::PhysicalDataQuality;
    use light_core::programming::{
        DirectCompatibility, DirectIncompatibility, NativeDriveLimit, PortableColorEstimate,
        UvFallback, VisibleFallback,
    };
    use light_wire::v2::native_color as native;

    fn status(replay: DirectReplayOutcome) -> DirectColorStatus {
        DirectColorStatus {
            replay,
            origin: DirectEstimateOrigin::Recorded,
            estimate: PortableColorEstimate {
                model_revision: 1,
                visible: None,
                uv: None,
                quality: PhysicalDataQuality::Unknown,
                limitations: vec![],
            },
            drive_limit: NativeDriveLimit::AboveModelMaximum,
            limitations: vec!["recorded".into()],
        }
    }

    /// An unknown appearance on an incompatible head is native-only (visible held), never a
    /// fitted guess; exact replay is native identity and claims no visible match itself.
    #[test]
    fn direct_status_is_reported_passively_and_unknown_appearance_is_native_only() {
        let exact = super::wire_direct(&status(DirectReplayOutcome::Exact));
        assert_eq!(exact.replay, native::ColorIntentDirectReplay::Exact);
        assert_eq!((exact.compatibility, exact.uv), (None, None));
        assert_eq!(
            exact.drive_limit,
            native::ColorIntentDriveLimit::AboveModelMaximum
        );
        let held = super::wire_direct(&status(DirectReplayOutcome::Fallback {
            compatibility: DirectCompatibility::Incompatible(DirectIncompatibility::ChangedLayout),
            visible: VisibleFallback::Hold,
            uv: UvFallback::ParkOff,
        }));
        assert_eq!(held.replay, native::ColorIntentDirectReplay::NativeOnly);
        assert_eq!(
            held.compatibility,
            Some(native::ColorIntentDirectCompatibility::ChangedLayout)
        );
        assert_eq!(held.uv, Some(native::ColorIntentDirectUv::ParkOff));
        assert_eq!(held.origin, native::ColorIntentDirectOrigin::Recorded);
        assert_eq!(held.limitations, ["recorded"]);
    }
}
