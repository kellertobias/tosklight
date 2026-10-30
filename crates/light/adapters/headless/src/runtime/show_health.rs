use super::*;
use light_show::{PortableShowDocument, PortableShowObject};
use light_wire::v2::show_health::{ShowHealthFinding, ShowHealthSeverity, ShowHealthSnapshot};
mod patch;
mod references;

pub(super) async fn read(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Json<ShowHealthSnapshot>, ApiError> {
    authenticate(&state, &headers)?;
    let _activation = state.active_show.acquire().await;
    let show_id = show.resolve(&state)?;
    show.verify(&state)?;
    let (_, store) = active_show_store(&state).map_err(ApiError::unavailable)?;
    let document = store.portable_document().map_err(ApiError::store)?;
    Ok(Json(analyze(
        show_id.0,
        &document,
        &state.output.snapshot(),
    )))
}

pub(super) fn analyze(
    show_id: Uuid,
    document: &PortableShowDocument,
    runtime: &EngineSnapshot,
) -> ShowHealthSnapshot {
    let mut findings = patch::findings(document, runtime);
    findings.extend(references::findings(document, runtime));
    findings.sort_by(|a, b| a.severity.cmp(&b.severity).then_with(|| a.id.cmp(&b.id)));
    findings.dedup_by(|a, b| a.id == b.id);
    ShowHealthSnapshot {
        show_id,
        show_revision: document.revision().value(),
        checked_at: chrono::Utc::now().to_rfc3339(),
        checks: [
            "DMX overlaps and patch routing",
            "Stage position metadata and authored placement",
            "Group, Cue and Preset references",
        ]
        .map(str::to_string)
        .to_vec(),
        findings,
    }
}

fn finding(
    object: &PortableShowObject,
    key: &str,
    severity: ShowHealthSeverity,
    message: String,
    fixture_id: Option<Uuid>,
    fix: bool,
) -> ShowHealthFinding {
    ShowHealthFinding {
        id: format!("{}:{}:{key}", object.key().kind(), object.key().id()),
        severity,
        message,
        object_kind: object.key().kind().into(),
        object_id: object.key().id().into(),
        object_revision: object.revision(),
        fixture_id,
        multipatch_instance_id: None,
        remove_missing_group_fixtures: fix,
    }
}
