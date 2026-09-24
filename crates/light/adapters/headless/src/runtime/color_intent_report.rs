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
                }
            })
            .collect(),
    }))
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

fn wire_engine(engine: light_fixture::ColorIntentEngine) -> wire::ColorIntentEngine {
    use light_fixture::ColorIntentEngine as Engine;
    match engine {
        Engine::Additive => wire::ColorIntentEngine::Additive,
        Engine::Subtractive => wire::ColorIntentEngine::Subtractive,
        Engine::HueSaturation => wire::ColorIntentEngine::HueSaturation,
        Engine::Wheel => wire::ColorIntentEngine::Wheel,
    }
}
