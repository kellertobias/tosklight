use super::{super::*, active::*};

pub(super) struct LegacyMvrImport {
    pub import: ActiveMvrImport,
    pub is_new: bool,
    pub open_after: bool,
}

pub(super) async fn apply_legacy_mvr_import(
    state: &AppState,
    session: Session,
    legacy: LegacyMvrImport,
) -> Result<Json<ApplyMvrResult>, ApiError> {
    let LegacyMvrImport {
        import,
        is_new,
        open_after,
    } = legacy;
    let ActiveMvrImport {
        entry,
        document,
        definitions,
        new_profiles,
        warnings: source_warnings,
        resolutions,
    } = import;
    let mut entry = entry;
    let temporary = state
        .installation
        .data_dir()
        .join("shows")
        .join(format!(".mvr-{}.show", Uuid::new_v4()));
    let source_store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let source_revision = source_store.portable_revision().map_err(ApiError::store)?;
    source_store
        .backup_to(&temporary)
        .map_err(ApiError::store)?;
    // Closing an old WAL connection after renaming a replacement can checkpoint old pages over
    // the imported database. Release it before preparing and replacing the destination file.
    drop(source_store);
    let context = operator_action_context(&session, light_application::ActionSource::Http);
    let result =
        apply_to_temporary_show(&temporary, context, &document, &definitions, &resolutions);
    let (imported, unresolved, mut warnings) = match result {
        Ok(result) => result,
        Err(error) => {
            clean_failed_import(state, &entry, &temporary, is_new);
            return Err(error);
        }
    };
    warnings.splice(0..0, source_warnings.clone());
    let show_change = state.active_show.acquire_show_change().await;
    let activation = state.active_show.acquire().await;
    if active_show_is(state, entry.id) {
        let _ = std::fs::remove_file(&temporary);
        drop(activation);
        return apply_active_mvr_import(
            state,
            &session,
            ActiveMvrImport {
                entry,
                document,
                definitions,
                new_profiles,
                warnings: source_warnings,
                resolutions,
            },
        )
        .await;
    }
    if let Err(error) = ensure_source_revision(&entry, source_revision) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if !is_new {
        backup_show(state, &entry)?;
    }
    {
        let destination = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
        destination
            .checkpoint_for_replacement()
            .map_err(ApiError::store)?;
    }
    std::fs::rename(&temporary, &entry.path).map_err(ApiError::io)?;
    drop(activation);
    publish_mvr_profiles(state, new_profiles, &mut warnings);
    if open_after {
        let output_runtime = load_output_runtime_for_show(state, entry.id)?;
        let prepared = prepare_show_activation_for_runtime(state, &entry)?;
        let context = operator_action_context(&session, light_application::ActionSource::Http);
        entry = activate_prepared_show(
            state,
            prepared,
            &context,
            &Transition::HoldCurrent,
            None,
            entry.clone(),
            output_runtime,
            ActivationCompletion::Mvr {
                imported,
                unresolved,
            },
            show_change,
        )
        .await?;
    } else {
        emit(
            state,
            "mvr_imported",
            serde_json::json!({"show":entry,"fixtures":imported,"unresolved":unresolved,"scenery":0}),
        );
        drop(show_change);
    }
    Ok(Json(ApplyMvrResult {
        show: entry,
        imported_fixtures: imported,
        unresolved_fixtures: unresolved,
        imported_scenery: 0,
        opened: open_after,
        warnings,
    }))
}

fn ensure_source_revision(
    entry: &ShowEntry,
    expected: light_show::PortableShowRevision,
) -> Result<(), ApiError> {
    let current = ActiveShowRepository::open(&entry.path)
        .map_err(ApiError::store)?
        .portable_revision()
        .map_err(ApiError::store)?;
    if current == expected {
        Ok(())
    } else {
        Err(ApiError::conflict(
            "show changed while the MVR import was being prepared",
        ))
    }
}

fn apply_to_temporary_show(
    temporary: &FsPath,
    context: light_application::ActionContext,
    document: &light_mvr::MvrDocument,
    definitions: &HashMap<Uuid, light_fixture::FixtureDefinition>,
    resolutions: &HashMap<Uuid, MvrResolution>,
) -> Result<(usize, usize, Vec<String>), ApiError> {
    let store = ActiveShowRepository::open(temporary).map_err(ApiError::store)?;
    let applied = apply_mvr_to_store(&store, context, document, definitions, resolutions)?;
    store
        .checkpoint_for_replacement()
        .map_err(ApiError::store)?;
    drop(store);
    validate_show_file(temporary).map_err(ApiError::store)?;
    Ok(applied)
}

fn clean_failed_import(state: &AppState, entry: &ShowEntry, temporary: &FsPath, is_new: bool) {
    let _ = std::fs::remove_file(temporary);
    if is_new {
        let _ = state.installation.remove_show(entry.id);
        let _ = std::fs::remove_file(&entry.path);
    }
}
