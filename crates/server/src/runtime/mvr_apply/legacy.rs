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
        new_definitions,
        resolutions,
    } = import;
    let temporary = state
        .data_dir
        .join("shows")
        .join(format!(".mvr-{}.show", Uuid::new_v4()));
    let source_store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
    let source_revision = source_store.portable_revision().map_err(ApiError::store)?;
    source_store
        .backup_to(&temporary)
        .map_err(ApiError::store)?;
    let result = apply_to_temporary_show(&temporary, &entry, &document, &definitions, &resolutions);
    let (imported, unresolved, mut warnings) = match result {
        Ok(result) => result,
        Err(error) => {
            clean_failed_import(state, &entry, &temporary, is_new);
            return Err(error);
        }
    };
    let activation = state.activation_lock.clone().lock_owned().await;
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
                new_definitions,
                resolutions,
            },
        )
        .await;
    }
    let _activation = activation;
    if let Err(error) = ensure_source_revision(&entry, source_revision) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if !is_new {
        backup_show(state, &entry)?;
    }
    std::fs::rename(&temporary, &entry.path).map_err(ApiError::io)?;
    publish_mvr_definitions(state, new_definitions, &mut warnings);
    if open_after {
        let compiled = load_engine_snapshot(&entry).map_err(ApiError::bad_request)?;
        activate_snapshot(state, compiled, &Transition::HoldCurrent, None).await?;
        state
            .desk
            .lock()
            .set_active_show(Some(entry.id))
            .map_err(ApiError::store)?;
        *state.active_show.write() = Some(entry.clone());
    }
    emit(
        state,
        "mvr_imported",
        serde_json::json!({"show":entry,"fixtures":imported,"unresolved":unresolved,"scenery":0}),
    );
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
    let current = ShowStore::open(&entry.path)
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
    entry: &ShowEntry,
    document: &light_mvr::MvrDocument,
    definitions: &[light_fixture::FixtureDefinition],
    resolutions: &HashMap<Uuid, MvrResolution>,
) -> Result<(usize, usize, Vec<String>), ApiError> {
    let store = ShowStore::open(temporary).map_err(ApiError::store)?;
    let applied = apply_mvr_to_store(&store, document, definitions, resolutions)?;
    validate_show_file(temporary).map_err(ApiError::store)?;
    let probe = ShowEntry {
        path: temporary.display().to_string(),
        ..entry.clone()
    };
    load_engine_snapshot(&probe)
        .map_err(ApiError::bad_request)?
        .validate()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(applied)
}

fn clean_failed_import(state: &AppState, entry: &ShowEntry, temporary: &FsPath, is_new: bool) {
    let _ = std::fs::remove_file(temporary);
    if is_new {
        let _ = state.desk.lock().remove_show(entry.id);
        let _ = std::fs::remove_file(&entry.path);
    }
}
