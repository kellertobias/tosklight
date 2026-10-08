//! Typed v2 show-library snapshot and replay-safe lifecycle intents.

use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::show_library as wire;
use std::collections::VecDeque;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .merge(super::show_network::router())
        .merge(super::show_revision_source::router())
        .route(
            "/api/v2/shows",
            get(show_library_snapshot).post(show_library_action),
        )
        .route("/api/v2/shows/{id}/download", get(download_show))
        .route("/api/v2/mvr/imports/preview", post(preview_mvr_import_v2))
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
        let description = super::show_description::read_description(&show.path)?;
        let revisions = state
            .installation
            .show_revisions(show.id)
            .map_err(ApiError::store)?
            .into_iter()
            .map(revision)
            .collect();
        entries.push(wire::ShowLibraryEntry {
            show: runtime_wire::show(show),
            description,
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
    let may_activate = matches!(
        &request.action,
        wire::ShowLibraryAction::Open { .. }
            | wire::ShowLibraryAction::OpenDefault { .. }
            | wire::ShowLibraryAction::Rollback { .. }
            | wire::ShowLibraryAction::OpenRevision { .. }
            | wire::ShowLibraryAction::ApplyMvr { .. }
            | wire::ShowLibraryAction::ImportFromDesk { open: true, .. }
            | wire::ShowLibraryAction::ImportFromVisualizer { open: true, .. }
    );
    if may_activate {
        return await_owned_activation_action(async move {
            run_show_library_action(state, headers, request, session).await
        })
        .await;
    }
    run_show_library_action(state, headers, request, session).await
}

async fn run_show_library_action(
    state: AppState,
    headers: HeaderMap,
    request: wire::ShowLibraryActionRequest,
    session: Session,
) -> Result<Json<wire::ShowLibraryActionOutcome>, ApiError> {
    let remote_save = matches!(
        &request.action,
        wire::ShowLibraryAction::SaveCopyToPeer { .. }
            | wire::ShowLibraryAction::ExportMvrToPeer { .. }
    );
    // A discovered desk may be this server: remote forwarding must leave its local gate free.
    let _intent = if remote_save {
        None
    } else {
        Some(state.replay.acquire_show_library_action().await)
    };
    let _remote_intent = if remote_save {
        Some(super::show_network::REMOTE_SAVE_LOCK.lock().await)
    } else {
        None
    };
    validate_request_id(&request.request_id)?;
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };
    let signature = action_signature(&request.action)?;
    // Serialize replacement and replay lookup together, including concurrent retries.
    let _document_update = if matches!(
        &request.action,
        wire::ShowLibraryAction::UpdateDocument { .. }
            | wire::ShowLibraryAction::CreateFromBase { .. }
            | wire::ShowLibraryAction::SetBaseShow { .. }
            | wire::ShowLibraryAction::SetDescription { .. }
            | wire::ShowLibraryAction::PrepareRevision { .. }
    ) {
        Some(state.active_show.acquire_show_change().await)
    } else {
        None
    };
    if let Some(outcome) = state.replay.lookup_show_library(&key, &signature).await? {
        return Ok(Json(outcome));
    }
    check_activation_request_cancelled()?;
    let result = execute_action(&state, &headers, &request.request_id, request.action).await?;
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

/// Show recovery preserves the failed show's file: library actions that rewrite it in place are
/// refused for it (see `ensure_not_recovering_show`).
fn ensure_library_action_spares_recovering_show(
    state: &AppState,
    action: &wire::ShowLibraryAction,
) -> Result<(), ApiError> {
    use wire::ShowLibraryAction as Action;
    match action {
        Action::SetDescription { show_id, .. } | Action::Rename { show_id, .. } => {
            ensure_not_recovering_show(state, *show_id)
        }
        Action::Overwrite {
            destination_show_id,
            ..
        }
        | Action::UpdateDocument {
            destination_show_id,
            ..
        } => ensure_not_recovering_show(state, *destination_show_id),
        _ => Ok(()),
    }
}

async fn execute_action(
    state: &AppState,
    headers: &HeaderMap,
    request_id: &str,
    action: wire::ShowLibraryAction,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    use wire::ShowLibraryAction as Action;
    ensure_library_action_spares_recovering_show(state, &action)?;
    match action {
        Action::SetDescription {
            show_id,
            description,
        } => execute_set_description(state, show_id, &description),
        Action::SaveCopy {
            source_show_id,
            data_base64,
            name,
            root_id,
            path,
            is_base_show,
        } => execute_save_copy(
            state,
            source_show_id,
            data_base64,
            name,
            root_id,
            path,
            is_base_show,
        ),
        Action::ExportMvrFile {
            show_id,
            data_base64,
            name,
            root_id,
            path,
        } => execute_export_mvr_file(state, show_id, data_base64, name, root_id, path),
        Action::SaveCopyToPeer {
            instance,
            source_show_id,
            name,
            root_id,
            path,
            is_base_show,
        } => {
            execute_save_copy_to_peer(
                state,
                request_id,
                instance,
                source_show_id,
                name,
                root_id,
                path,
                is_base_show,
            )
            .await
        }
        Action::ExportMvrToPeer {
            instance,
            show_id,
            name,
            root_id,
            path,
        } => {
            execute_export_mvr_to_peer(state, request_id, instance, show_id, name, root_id, path)
                .await
        }
        Action::Create {
            name,
            data_base64,
            overwrite,
        } => execute_create(state, headers, name, data_base64, overwrite).await,
        Action::PrepareRevision { show_id, revision } => Ok(show_result(
            super::show_revision_source::prepare_named_revision_source(state, show_id, revision)?,
        )),
        Action::ImportFromDesk {
            instance,
            show_id,
            revision,
            open,
        } => execute_import_from_desk(state, headers, instance, show_id, revision, open).await,
        Action::SetBaseShow {
            show_id,
            is_base_show,
        } => execute_set_base_show(state, headers, show_id, is_base_show),
        Action::CreateFromBase { show_id, name } => {
            execute_create_from_base(state, headers, show_id, name).await
        }
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
        Action::UpdateDocument {
            destination_show_id,
            expected_revision,
            data_base64,
        } => {
            execute_document_update(
                state,
                headers,
                destination_show_id,
                expected_revision,
                data_base64,
            )
            .await
        }
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
        Action::ImportFromVisualizer { instance, open } => {
            execute_import_from_visualizer(state, headers, instance, open).await
        }
        Action::ApplyMvr {
            token,
            destination,
            resolutions,
        } => execute_mvr_apply(state, headers, token, destination, resolutions).await,
    }
}

fn execute_set_description(
    state: &AppState,
    show_id: Uuid,
    description: &str,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    Ok(show_result(super::show_description::set_description(
        state,
        show_id,
        description,
    )?))
}

async fn execute_import_from_desk(
    state: &AppState,
    headers: &HeaderMap,
    instance: String,
    show_id: Uuid,
    revision: Option<u64>,
    open: bool,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let (source_name, data) =
        super::show_network::fetch_desk_document(state, &instance, show_id, revision).await?;
    let name = unique_import_name(state, &source_name)?;
    let imported = execute_create(state, headers, name, Some(STANDARD.encode(data)), false).await?;
    if !open {
        return Ok(imported);
    }
    let wire::ShowLibraryActionResult::Show { show } = imported else {
        unreachable!()
    };
    execute_open(
        state,
        headers,
        show.id,
        wire::ShowOpenTransition::SafeBlackout,
        None,
    )
    .await
}

fn execute_save_copy(
    state: &AppState,
    source_show_id: Option<Uuid>,
    data_base64: Option<String>,
    name: String,
    root_id: String,
    path: String,
    is_base_show: bool,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let show = super::show_save_destination::save_copy(
        state,
        source_show_id,
        data_base64,
        name,
        root_id,
        path,
        is_base_show,
    )?;
    Ok(show_result(show))
}

fn execute_export_mvr_file(
    state: &AppState,
    show_id: Option<Uuid>,
    data_base64: Option<String>,
    name: String,
    root_id: String,
    path: String,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let (root_id, path, summary) = super::show_save_destination::export_mvr_file(
        state,
        show_id,
        data_base64,
        name,
        root_id,
        path,
    )?;
    // A desk-local export reports what it wrote; archive bytes forwarded by another desk do not
    // carry a summary, so the sending desk reports its own.
    Ok(match summary {
        Some(summary) => wire::ShowLibraryActionResult::MvrExported {
            root_id,
            path,
            summary,
        },
        None => wire::ShowLibraryActionResult::FileSaved { root_id, path },
    })
}

async fn execute_save_copy_to_peer(
    state: &AppState,
    request_id: &str,
    instance: String,
    source_show_id: Uuid,
    name: String,
    root_id: String,
    path: String,
    is_base_show: bool,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let show = super::show_network::save_copy_to_peer(
        state,
        request_id,
        &instance,
        source_show_id,
        &name,
        &root_id,
        &path,
        is_base_show,
    )
    .await?;
    Ok(wire::ShowLibraryActionResult::Show { show })
}

async fn execute_export_mvr_to_peer(
    state: &AppState,
    request_id: &str,
    instance: String,
    show_id: Uuid,
    name: String,
    root_id: String,
    path: String,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let (root_id, path, summary) = super::show_network::export_mvr_to_peer(
        state, request_id, &instance, show_id, &name, &root_id, &path,
    )
    .await?;
    Ok(wire::ShowLibraryActionResult::MvrExported {
        root_id,
        path,
        summary,
    })
}

fn execute_set_base_show(
    state: &AppState,
    headers: &HeaderMap,
    show_id: Uuid,
    is_base_show: bool,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let _session = authenticate(state, headers)?;
    let show = state
        .installation
        .set_show_base(light_core::ShowId(show_id), is_base_show)
        .map_err(ApiError::store)?;
    if state
        .active_show
        .current()
        .is_some_and(|active| active.id == show.id)
    {
        state.active_show.replace_current(Some(show.clone()));
    }
    emit(state, "show_updated", serde_json::json!({"show":show}));
    Ok(show_result(show))
}

async fn execute_create_from_base(
    state: &AppState,
    headers: &HeaderMap,
    show_id: Uuid,
    name: String,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    validate_show_name(&name)?;
    let source = state
        .installation
        .show(light_core::ShowId(show_id))
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("base show"))?;
    if !source.is_base_show {
        return Err(ApiError::conflict("This show is no longer a base show"));
    }
    let export = state
        .installation
        .data_dir()
        .join(format!(".base-{}.show", Uuid::new_v4()));
    ActiveShowRepository::open(&source.path)
        .map_err(ApiError::store)?
        .backup_to(&export)
        .map_err(ApiError::store)?;
    ActiveShowRepository::open(&export)
        .map_err(ApiError::store)?
        .set_identity(source.id, &source.name, None)
        .map_err(ApiError::store)?;
    let bytes = std::fs::read(&export);
    let _ = std::fs::remove_file(&export);
    execute_create(
        state,
        headers,
        name,
        Some(STANDARD.encode(bytes.map_err(ApiError::io)?)),
        false,
    )
    .await
}

pub(super) fn unique_import_name(state: &AppState, source_name: &str) -> Result<String, ApiError> {
    let shows = state.installation.show_library().map_err(ApiError::store)?;
    let mut name = source_name.to_owned();
    for suffix in 2.. {
        if !shows
            .iter()
            .any(|show| show.name.eq_ignore_ascii_case(&name))
        {
            return Ok(name);
        }
        name = format!("{source_name} {suffix}");
    }
    unreachable!()
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

/// Load what a Viz editor on the network has open.
///
/// The document arrives as an ordinary show file and is imported as one, so everything that
/// follows — the library entry, the revision, opening it — is the path any other imported show
/// takes. The editor keeps its own document; this desk has a copy of it.
async fn execute_import_from_visualizer(
    state: &AppState,
    headers: &HeaderMap,
    instance: String,
    open: bool,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let (name, data) = discovery_http::fetch_visualizer_document(state, &instance).await?;
    let name = unique_import_name(state, &name)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
    let imported = execute_create(state, headers, name, Some(encoded), false).await?;
    if !open {
        return Ok(imported);
    }
    let wire::ShowLibraryActionResult::Show { show } = &imported else {
        return Ok(imported);
    };
    execute_open(
        state,
        headers,
        show.id,
        wire::ShowOpenTransition::default(),
        None,
    )
    .await
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
        TolerantJson(open_input(transition, transition_millis)),
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
        TolerantJson(open_input(transition, transition_millis)),
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
        TolerantJson(open_input(transition, transition_millis)),
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
        TolerantJson(open_input(transition, transition_millis)),
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

/// Replace precisely the originating entry, refusing a stale desk document before any write.
async fn execute_document_update(
    state: &AppState,
    headers: &HeaderMap,
    destination_id: Uuid,
    expected_revision: u64,
    data_base64: String,
) -> Result<wire::ShowLibraryActionResult, ApiError> {
    let session = authenticate(state, headers)?;
    let _activation = state.active_show.acquire().await;
    let entry = state
        .installation
        .show(light_core::ShowId(destination_id))
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("source desk show"))?;
    let current = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let revision = current
        .portable_revision()
        .map_err(ApiError::store)?
        .value();
    let patch_revision = current
        .portable_patch_revision()
        .map_err(ApiError::store)?
        .value();
    drop(current);
    if revision != expected_revision {
        return Err(ApiError::conflict(
            "The source desk show changed. Reopen it before saving to the desk; your local edits remain available.",
        ));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|_| ApiError::bad_request("data_base64 must contain a portable show"))?;
    let staged = state
        .installation
        .data_dir()
        .join("shows")
        .join(format!(".architect-{}.show", Uuid::new_v4()));
    std::fs::write(&staged, bytes).map_err(ApiError::io)?;
    let result = (|| {
        validate_show_file(&staged).map_err(ApiError::store)?;
        let staged_store = ActiveShowRepository::open(&staged).map_err(ApiError::store)?;
        // The desk's sync request identities outlive a manual save, so an Architect retry that
        // follows it still applies exactly once (docs/engineering/show-sync.md).
        staged_store
            .adopt_sync_applied_requests(
                &ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?,
            )
            .map_err(ApiError::store)?;
        drop(staged_store);
        ActiveShowRepository::open(&staged)
            .map_err(ApiError::store)?
            .set_identity(entry.id, &entry.name, entry.revision_copy.as_ref())
            .map_err(ApiError::store)?;
        ActiveShowRepository::open(&staged)
            .map_err(ApiError::store)?
            .advance_replacement_revisions(revision, patch_revision)
            .map_err(ApiError::store)?;
        let document_revision = ActiveShowRepository::open(&staged)
            .map_err(ApiError::store)?
            .portable_revision()
            .map_err(ApiError::store)?
            .value();
        let mut probe = entry.clone();
        probe.path = staged.display().to_string();
        let prepared = if state
            .active_show
            .current()
            .as_ref()
            .is_some_and(|active| active.id == entry.id)
        {
            super::show_programming_contract::require_for_path(state, &staged)?;
            Some(
                state
                    .output
                    .prepare_snapshot(load_engine_snapshot(&probe).map_err(ApiError::bad_request)?)
                    .map_err(|error| ApiError::internal(error.to_string()))?,
            )
        } else {
            None
        };
        let recovery = backup_show(state, &entry)?;
        ActiveShowRepository::open(&entry.path)
            .map_err(ApiError::store)?
            .checkpoint_for_replacement()
            .map_err(|error| ApiError::conflict(error.to_string()))?;
        std::fs::rename(&staged, &entry.path).map_err(ApiError::io)?;
        let updated = match state.installation.mark_show_updated(entry.id) {
            Ok(updated) => updated,
            Err(error) => {
                std::fs::copy(&recovery, &staged).map_err(ApiError::io)?;
                std::fs::rename(&staged, &entry.path).map_err(ApiError::io)?;
                return Err(ApiError::store(error));
            }
        };
        if let Some(prepared) = prepared {
            let context = operator_action_context(&session, light_application::ActionSource::Http);
            state.programming.run_value_gesture_boundary(&context, || {
                install_prepared_snapshot_with_selection_refresh(
                    state,
                    &context,
                    prepared,
                    None,
                    PlaybackInstallPolicy::Preserve,
                    HighlightInstallPolicy::Reconcile,
                );
            });
            invalidate_active_show_document(state);
            state.active_show.replace_current(Some(updated.clone()));
            state.attributes.install_entry(Some(&updated));
            super::psn_http::install_current_show(state);
            state
                .output
                .engine()
                .set_color_model(state.attributes.color_model());
            state.active_show.set_error(None);
            emit(state, "show_opened", serde_json::json!({"show":updated}));
        }
        emit(
            state,
            "show_overwritten",
            serde_json::json!({"destination_show":updated}),
        );
        Ok(wire::ShowLibraryActionResult::DocumentUpdated {
            show: runtime_wire::show(updated),
            document_revision,
        })
    })();
    let _ = std::fs::remove_file(staged);
    result
}
