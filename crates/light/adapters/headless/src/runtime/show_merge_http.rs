//! Offline show copies and merging them back into the open show (TL-858).
//!
//! "Take Offline Copy" saves a named revision of the open show, writes a copy of it to the chosen
//! folder and records the copy's objects as its merge base. The copy is then a library show the
//! merge routes name as their operand; the merge target is always the loaded show.

use super::*;
use light_application::show_merge::{
    MergeChoice, MergeConflict, MergeObjects, MergeOutcome, MergeResolutions, OfflineCopy,
};
use light_application::{ActionContext, ActionError, ActionErrorKind, ActionSource};
use light_show::{MergeLineage, PortableShowObjectKey};
use light_wire::v2::show_merge as wire;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/show-merges/{copy_show_id}/preview",
            post(preview_merge),
        )
        .route(
            "/api/v2/show-merges/{copy_show_id}/apply",
            post(apply_merge),
        )
}

pub(super) async fn take_offline_copy(
    state: &AppState,
    headers: &HeaderMap,
    name: String,
    root_id: String,
    path: String,
) -> Result<ShowEntry, ApiError> {
    let session = authenticate(state, headers)?;
    let source = state
        .active_show
        .current()
        .ok_or_else(|| ApiError::conflict("open a show before taking an offline copy"))?;
    let state = state.clone();
    tokio::task::spawn_blocking(move || {
        let context = context(&session);
        let ports = ServerSelectiveImportPorts::new(state.clone());
        state
            .active_show
            .show_merge()
            .prepare_for_offline_copy(&context, source.id, &ports)
            .map_err(action_error)?;
        let copy_name = name.trim().trim_end_matches(".show").to_owned();
        let revision = super::show_library::store_named_revision(
            &state,
            &source,
            &format!("Offline copy: {copy_name}"),
        )?;
        let copy = super::show_save_destination::save_copy(
            &state,
            Some(source.id.0),
            None,
            name,
            root_id,
            path,
            false,
            false,
        )?;
        let lineage = MergeLineage {
            source_show_id: source.id,
            source_show_name: source.name.clone(),
            source_revision: revision.revision,
            fork_id: Uuid::new_v4(),
            taken_at: chrono::Utc::now().to_rfc3339(),
        };
        let recorded = ActiveShowRepository::open(&copy.path)
            .and_then(|store| store.record_merge_base(&lineage));
        if let Err(error) = recorded {
            let _ = state.installation.remove_show(copy.id);
            let _ = std::fs::remove_file(&copy.path);
            return Err(ApiError::store(error));
        }
        Ok(copy)
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

async fn preview_merge(
    State(state): State<AppState>,
    show: ShowContext,
    Path(copy_show_id): Path<Uuid>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::ShowMergePreviewRequest>,
) -> Result<Json<wire::ShowMergePreview>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let target = show.resolve(&state)?;
    let resolutions = resolutions(&request.resolutions);
    tokio::task::spawn_blocking(move || {
        let (copy, lineage) = load_copy(&state, copy_show_id, target)?;
        let ports = ServerSelectiveImportPorts::new(state.clone());
        let preview = state
            .active_show
            .show_merge()
            .preview(&context(&session), target, &copy, &resolutions, &ports)
            .map_err(action_error)?;
        Ok(Json(preview_wire(
            lineage,
            preview.target_revision,
            &preview.outcome,
            &copy.objects,
        )))
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

async fn apply_merge(
    State(state): State<AppState>,
    show: ShowContext,
    Path(copy_show_id): Path<Uuid>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::ShowMergeApplyRequest>,
) -> Result<Json<wire::ShowMergeApplyResult>, ApiError> {
    let session = authenticate(&state, &headers)?;
    super::show_objects_v2::validate_request_id(&request.request_id)?;
    let target = show.resolve_writable(&state)?;
    let resolutions = resolutions(&request.resolutions);
    tokio::task::spawn_blocking(move || {
        let (copy, lineage) = load_copy(&state, copy_show_id, target)?;
        let entry = state
            .active_show
            .current()
            .filter(|entry| entry.id == target)
            .ok_or_else(|| ApiError::conflict("the merge target is no longer the open show"))?;
        let context = context(&session).with_request_id(request.request_id);
        let ports = ServerSelectiveImportPorts::new(state.clone());
        let merge = state.active_show.show_merge();
        // Preview again before the safety revision, so a stale or undecided merge leaves no trace.
        let preview = merge
            .preview(&context, target, &copy, &resolutions, &ports)
            .map_err(action_error)?;
        if preview.target_revision != request.expected_target_revision {
            return Err(ApiError::conflict(
                "the show changed since the merge was previewed; preview it again",
            ));
        }
        if !preview.outcome.conflicts.is_empty() {
            return Err(ApiError::bad_request(format!(
                "{} conflicts still need a decision",
                preview.outcome.conflicts.len()
            )));
        }
        let safety = super::show_library::store_named_revision(
            &state,
            &entry,
            &format!("Before merging {}", lineage.copy_name),
        )?;
        let result = merge
            .apply(
                &context,
                target,
                &copy,
                &resolutions,
                request.expected_target_revision,
                &ports,
            )
            .map_err(action_error)?;
        Ok(Json(wire::ShowMergeApplyResult {
            changed: result.changed,
            show_revision: result.change.show_revision.value(),
            safety_revision: safety.revision,
            written_objects: u32::try_from(result.change.objects.len()).unwrap_or(u32::MAX),
        }))
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

/// Reads an offline copy of the open show. A show without a recorded base is refused: it can
/// only be compared two-way, which Partial Load already does.
fn load_copy(
    state: &AppState,
    copy_show_id: Uuid,
    target: light_core::ShowId,
) -> Result<(OfflineCopy, wire::ShowMergeLineage), ApiError> {
    let entry = state
        .installation
        .show(light_core::ShowId(copy_show_id))
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("offline copy"))?;
    if entry.id == target {
        return Err(ApiError::bad_request("a show cannot be merged into itself"));
    }
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let lineage = store.merge_lineage().map_err(ApiError::store)?.ok_or_else(|| {
        ApiError::bad_request(format!(
            "{} is not an offline copy, so there is nothing to merge it against; use Partial Load to copy objects from it",
            entry.name
        ))
    })?;
    if lineage.source_show_id != target {
        return Err(ApiError::bad_request(format!(
            "{} is an offline copy of {}, not of the open show",
            entry.name, lineage.source_show_name
        )));
    }
    let to_objects = |objects: Vec<(String, String, serde_json::Value)>| -> MergeObjects {
        objects
            .into_iter()
            .map(|(kind, id, body)| (PortableShowObjectKey::new(kind, id), body))
            .collect()
    };
    let base = to_objects(
        store
            .merge_base_objects()
            .map_err(ApiError::store)?
            .into_iter()
            .map(|object| (object.kind, object.id, object.body))
            .collect(),
    );
    let document = store.portable_document().map_err(ApiError::store)?;
    let objects = to_objects(
        document
            .objects()
            .map(|object| {
                (
                    object.key().kind().to_owned(),
                    object.key().id().to_owned(),
                    object.body().clone(),
                )
            })
            .collect(),
    );
    let copy = OfflineCopy {
        base,
        objects,
        profiles: document.fixture_profile_revisions().to_vec(),
    };
    Ok((
        copy,
        wire::ShowMergeLineage {
            copy_show_id,
            copy_name: entry.name,
            source_show_id: lineage.source_show_id.0,
            source_show_name: lineage.source_show_name,
            taken_at: lineage.taken_at,
        },
    ))
}

fn resolutions(resolutions: &[wire::ShowMergeResolution]) -> MergeResolutions {
    resolutions
        .iter()
        .map(|resolution| {
            let choice = match resolution.choice {
                wire::ShowMergeChoice::Mine => MergeChoice::Mine,
                wire::ShowMergeChoice::Theirs => MergeChoice::Theirs,
            };
            (resolution.conflict_id.clone(), choice)
        })
        .collect()
}

fn preview_wire(
    lineage: wire::ShowMergeLineage,
    target_revision: u64,
    outcome: &MergeOutcome,
    copy: &MergeObjects,
) -> wire::ShowMergePreview {
    let count = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
    wire::ShowMergePreview {
        lineage,
        target_revision,
        summary: outcome
            .summary
            .iter()
            .map(|kind| wire::ShowMergeKindSummary {
                kind: kind.kind.clone(),
                added: count(kind.added),
                changed: count(kind.changed),
                deleted: count(kind.deleted),
                conflicts: count(kind.conflicts),
            })
            .collect(),
        auto_merged: count(outcome.auto_merged),
        write_count: count(outcome.writes.len()),
        conflicts: outcome
            .conflicts
            .iter()
            .map(|conflict| conflict_wire(conflict, copy))
            .collect(),
        renumbers: outcome
            .renumbers
            .iter()
            .map(|renumber| wire::ShowMergeRenumber {
                kind: renumber.from.kind().to_owned(),
                from: renumber.from.id().to_owned(),
                to: renumber.to.id().to_owned(),
                name: renumber.name.clone(),
            })
            .collect(),
        macro_warnings: outcome
            .macro_warnings
            .iter()
            .map(|warning| wire::ShowMergeMacroWarning {
                macro_id: warning.key.id().to_owned(),
                name: warning.name.clone(),
                mentions: warning.mentions.clone(),
            })
            .collect(),
    }
}

fn conflict_wire(conflict: &MergeConflict, copy: &MergeObjects) -> wire::ShowMergeConflict {
    let text = |body: Option<&serde_json::Value>, field: &str| {
        body.and_then(|body| body.get(field))
            .and_then(|value| match value {
                serde_json::Value::String(text) if !text.is_empty() => Some(text.clone()),
                serde_json::Value::Number(number) => Some(number.to_string()),
                _ => None,
            })
    };
    let body = copy.get(&conflict.key).or(conflict.mine.as_ref());
    let number = ["pool_number", "fixture_number", "number"]
        .into_iter()
        .find_map(|field| text(body, field));
    wire::ShowMergeConflict {
        id: conflict.id.clone(),
        kind: conflict.key.kind().to_owned(),
        object_id: conflict.key.id().to_owned(),
        object_number: number,
        object_name: text(body, "name"),
        path: conflict
            .path
            .iter()
            .map(|segment| readable_segment(segment, copy))
            .collect(),
        base: conflict.base.clone(),
        mine: conflict.mine.clone(),
        theirs: conflict.theirs.clone(),
    }
}

/// A list element is identified by ids such as `fixture_id|attribute`; the operator reads
/// "Fixture 12 intensity".
fn readable_segment(segment: &str, copy: &MergeObjects) -> String {
    segment
        .split('|')
        .map(|part| {
            let fixture = PortableShowObjectKey::new("patched_fixture", part);
            copy.get(&fixture)
                .and_then(|body| body.get("fixture_number"))
                .and_then(serde_json::Value::as_u64)
                .map_or_else(|| part.to_owned(), |number| format!("Fixture {number}"))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn context(session: &Session) -> ActionContext {
    ActionContext::operator(session.desk.id, session.id.0, ActionSource::Http)
}

fn action_error(error: ActionError) -> ApiError {
    match error.kind {
        ActionErrorKind::Unauthorized => ApiError::unauthorized(error.message),
        ActionErrorKind::Forbidden => ApiError::forbidden(error.message),
        ActionErrorKind::Invalid => ApiError::bad_request(error.message),
        ActionErrorKind::NotFound => ApiError::not_found(error.message),
        ActionErrorKind::Conflict => ApiError::conflict(error.message),
        ActionErrorKind::Busy | ActionErrorKind::Unavailable => {
            ApiError::unavailable(error.message)
        }
        ActionErrorKind::Internal => ApiError::internal(error.message),
    }
}
