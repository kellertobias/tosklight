//! Typed v2 show-library snapshot and replay-safe lifecycle intents.

use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::show_library as wire;
use std::collections::VecDeque;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/shows",
            get(show_library_snapshot).post(show_library_action),
        )
        .route("/api/v2/shows/{id}/download", get(download_show))
        .route("/api/v2/mvr/imports/preview", post(preview_mvr_import_v2))
        .route("/api/v2/shows/{id}/mvr/preview", get(preview_mvr_export_v2))
        .route("/api/v2/shows/{id}/mvr", get(export_mvr))
}

async fn show_library_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::ShowLibrarySnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let shows = state.installation.show_library().map_err(ApiError::store)?;
    let mut entries = Vec::with_capacity(shows.len());
    for show in shows {
        let revisions = state
            .installation
            .show_revisions(show.id)
            .map_err(ApiError::store)?
            .into_iter()
            .map(revision)
            .collect();
        entries.push(wire::ShowLibraryEntry {
            show: runtime_wire::show(show),
            revisions,
        });
    }
    Ok(Json(wire::ShowLibrarySnapshot { shows: entries }))
}

async fn show_library_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::ShowLibraryActionRequest>,
) -> Result<Json<wire::ShowLibraryActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };
    let signature = action_signature(&request.action)?;
    if let Some(outcome) = state.replay.lookup_show_library(&key, &signature).await? {
        return Ok(Json(outcome));
    }
    let result = execute_action(&state, &headers, request.action).await?;
    let outcome = wire::ShowLibraryActionOutcome {
        request_id: request.request_id,
        replayed: false,
        result,
    };
    state
        .replay
        .insert_show_library(key, signature, outcome.clone())
        .await;
    Ok(Json(outcome))
}

async fn execute_action(
    state: &AppState,
    headers: &HeaderMap,
    action: wire::ShowLibraryAction,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    use wire::ShowLibraryAction as Action;
    match action {
        Action::Create {
            name,
            data_base64,
            overwrite,
        } => execute_create(state, headers, name, data_base64, overwrite).await,
        Action::Open {
            show_id,
            transition,
            transition_millis,
        } => execute_open(state, headers, show_id, transition, transition_millis).await,
        Action::OpenDefault {
            transition,
            transition_millis,
        } => execute_open_default(state, headers, transition, transition_millis).await,
        Action::Rollback {
            transition,
            transition_millis,
        } => execute_rollback(state, headers, transition, transition_millis).await,
        Action::Rename { show_id, name } => execute_rename(state, headers, show_id, name).await,
        Action::Overwrite {
            source_show_id,
            destination_show_id,
        } => execute_overwrite(state, headers, source_show_id, destination_show_id).await,
        Action::SaveRevision { show_id, name } => {
            execute_save_revision(state, headers, show_id, name).await
        }
        Action::OpenRevision {
            show_id,
            revision,
            transition,
            transition_millis,
        } => {
            execute_open_revision(
                state,
                headers,
                show_id,
                revision,
                transition,
                transition_millis,
            )
            .await
        }
        Action::ApplyMvr {
            token,
            destination,
            resolutions,
        } => execute_mvr_apply(state, headers, token, destination, resolutions).await,
    }
}

async fn execute_create(
    state: &AppState,
    headers: &HeaderMap,
    name: String,
    data_base64: Option<String>,
    overwrite: bool,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let (_, Json(show)) = upload_show(
        State(state.clone()),
        headers.clone(),
        Json(UploadShow {
            name,
            data_base64,
            overwrite,
        }),
    )
    .await?;
    Ok(show_result(show))
}

async fn execute_open(
    state: &AppState,
    headers: &HeaderMap,
    show_id: Uuid,
    transition: wire::ShowOpenTransition,
    transition_millis: Option<u64>,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let Json(show) = open_show(
        State(state.clone()),
        Path(show_id),
        headers.clone(),
        Json(open_input(transition, transition_millis)),
    )
    .await?;
    Ok(show_result(show))
}

async fn execute_open_default(
    state: &AppState,
    headers: &HeaderMap,
    transition: wire::ShowOpenTransition,
    transition_millis: Option<u64>,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let Json(show) = open_clean_default_show(
        State(state.clone()),
        headers.clone(),
        Json(open_input(transition, transition_millis)),
    )
    .await?;
    Ok(show_result(show))
}

async fn execute_rollback(
    state: &AppState,
    headers: &HeaderMap,
    transition: wire::ShowOpenTransition,
    transition_millis: Option<u64>,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let Json(show) = rollback_show(
        State(state.clone()),
        headers.clone(),
        Json(open_input(transition, transition_millis)),
    )
    .await?;
    Ok(show_result(show))
}

async fn execute_rename(
    state: &AppState,
    headers: &HeaderMap,
    show_id: Uuid,
    name: String,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let Json(show) = rename_show(
        State(state.clone()),
        Path(show_id),
        headers.clone(),
        Json(RenameShow { name }),
    )
    .await?;
    Ok(show_result(show))
}

async fn execute_overwrite(
    state: &AppState,
    headers: &HeaderMap,
    source_show_id: Uuid,
    destination_show_id: Uuid,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let Json(show) = overwrite_show(
        State(state.clone()),
        Path((source_show_id, destination_show_id)),
        headers.clone(),
    )
    .await?;
    Ok(show_result(show))
}

async fn execute_save_revision(
    state: &AppState,
    headers: &HeaderMap,
    show_id: Uuid,
    name: String,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let (_, Json(saved)) = save_show_revision(
        State(state.clone()),
        Path(show_id),
        headers.clone(),
        Json(SaveShowRevision { name }),
    )
    .await?;
    Ok(wire::ShowLibraryActionResult::Revision {
        revision: revision(saved),
    })
}

async fn execute_open_revision(
    state: &AppState,
    headers: &HeaderMap,
    show_id: Uuid,
    revision: u64,
    transition: wire::ShowOpenTransition,
    transition_millis: Option<u64>,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let Json(show) = open_show_revision(
        State(state.clone()),
        Path((show_id, revision)),
        headers.clone(),
        Json(open_input(transition, transition_millis)),
    )
    .await?;
    Ok(show_result(show))
}

async fn execute_mvr_apply(
    state: &AppState,
    headers: &HeaderMap,
    token: Uuid,
    destination: wire::MvrImportDestination,
    resolutions: Vec<wire::MvrImportResolution>,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let (new_show, existing_show_id) = mvr_destination(destination);
    let resolutions = resolutions
        .into_iter()
        .map(|resolution| (resolution.fixture_id, mvr_resolution(resolution.action)))
        .collect();
    let Json(result) = apply_mvr_import(
        State(state.clone()),
        Path(token),
        headers.clone(),
        Json(ApplyMvrImport {
            new_show,
            existing_show_id,
            resolutions,
        }),
    )
    .await?;
    Ok(wire::ShowLibraryActionResult::MvrApply {
        result: wire::MvrApplyOutcome {
            show: runtime_wire::show(result.show),
            imported_fixtures: result.imported_fixtures,
            unresolved_fixtures: result.unresolved_fixtures,
            imported_scenery: result.imported_scenery,
            opened: result.opened,
            warnings: result.warnings,
        },
    })
}

fn show_result(show: ShowEntry) -> wire::ShowLibraryActionResult {
    wire::ShowLibraryActionResult::Show {
        show: runtime_wire::show(show),
    }
}

fn mvr_destination(destination: wire::MvrImportDestination) -> (Option<NewMvrShow>, Option<Uuid>) {
    match destination {
        wire::MvrImportDestination::NewShow {
            name,
            open_after_import,
        } => (
            Some(NewMvrShow {
                name,
                open_after_import,
            }),
            None,
        ),
        wire::MvrImportDestination::ExistingShow { show_id } => (None, Some(show_id)),
    }
}

async fn preview_mvr_import_v2(
    State(state): State<AppState>,
    query: Query<MvrPreviewQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<wire::MvrImportPreview>, ApiError> {
    let Json(preview) = preview_mvr_import(State(state), query, headers, body).await?;
    Ok(Json(wire::MvrImportPreview {
        token: preview.token,
        fixtures: preview
            .fixtures
            .into_iter()
            .map(|fixture| wire::MvrPreviewFixture {
                uuid: fixture.uuid,
                name: fixture.name,
                gdtf_spec: fixture.gdtf_spec,
                gdtf_mode: fixture.gdtf_mode,
                universe: fixture.universe,
                address: fixture.address,
                matched: fixture.matched,
            })
            .collect(),
        scenery: preview.scenery,
        missing_profiles: preview.missing_profiles,
        warnings: preview.warnings,
        address_conflicts: preview.address_conflicts,
    }))
}

async fn preview_mvr_export_v2(
    State(state): State<AppState>,
    path: Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<wire::MvrExportPreview>, ApiError> {
    let Json(preview) = preview_mvr_export(State(state), path, headers).await?;
    Ok(Json(wire::MvrExportPreview {
        fixtures: preview.fixtures,
        scenery: preview.scenery,
        embedded_profiles: preview.embedded_profiles,
        missing_profiles: preview.missing_profiles,
        omitted: preview.omitted,
        warnings: preview.warnings,
    }))
}

fn open_input(transition: wire::ShowOpenTransition, transition_millis: Option<u64>) -> OpenShow {
    OpenShow {
        transition: Some(match transition {
            wire::ShowOpenTransition::HoldCurrent => Transition::HoldCurrent,
            wire::ShowOpenTransition::TimedFade => Transition::TimedFade,
            wire::ShowOpenTransition::SafeBlackout => Transition::SafeBlackout,
        }),
        transition_millis,
    }
}

fn mvr_resolution(action: wire::MvrImportResolutionAction) -> MvrResolution {
    match action {
        wire::MvrImportResolutionAction::Import => MvrResolution::Import,
        wire::MvrImportResolutionAction::Skip => MvrResolution::Skip,
        wire::MvrImportResolutionAction::ImportUnpatched => MvrResolution::ImportUnpatched,
        wire::MvrImportResolutionAction::Replace => MvrResolution::Replace,
        wire::MvrImportResolutionAction::Address { universe, address } => {
            MvrResolution::Address { universe, address }
        }
    }
}

fn revision(saved: ShowRevision) -> wire::ShowLibraryRevision {
    wire::ShowLibraryRevision {
        show_id: saved.show_id.0,
        revision: saved.revision,
        name: saved.name,
        created_at: saved.created_at,
    }
}

fn action_signature(action: &wire::ShowLibraryAction) -> Result<[u8; 32], ApiError> {
    let bytes = serde_json::to_vec(action)
        .map_err(|error| ApiError::internal(format!("show action encoding failed: {error}")))?;
    Ok(Sha256::digest(bytes).into())
}

fn validate_request_id(request_id: &str) -> Result<(), ApiError> {
    if request_id.trim().is_empty()
        || request_id.len() > 128
        || request_id.chars().any(char::is_control)
    {
        return Err(ApiError::bad_request(
            "request_id must contain 1-128 printable bytes",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    session_id: Uuid,
    request_id: String,
}

struct ReplayEntry {
    signature: [u8; 32],
    outcome: wire::ShowLibraryActionOutcome,
}

#[derive(Default)]
pub(super) struct ShowLibraryReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl ShowLibraryReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        signature: &[u8; 32],
    ) -> Result<Option<wire::ShowLibraryActionOutcome>, ApiError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if &entry.signature != signature {
            return Err(ApiError::conflict(
                "request_id was already used for a different show-library action",
            ));
        }
        let mut outcome = entry.outcome.clone();
        outcome.replayed = true;
        Ok(Some(outcome))
    }

    pub(super) fn insert(
        &mut self,
        key: ReplayKey,
        signature: [u8; 32],
        outcome: wire::ShowLibraryActionOutcome,
    ) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, ReplayEntry { signature, outcome });
        while self.entries.len() > REQUEST_CACHE_ENTRY_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}
