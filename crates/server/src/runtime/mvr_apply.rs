use super::*;

pub(super) async fn apply_mvr_import(
    State(state): State<AppState>,
    Path(token): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ApplyMvrImport>,
) -> Result<Json<ApplyMvrResult>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let staged = state
        .mvr_imports
        .lock()
        .remove(&token)
        .ok_or_else(|| ApiError::not_found("MVR import preview"))?;
    if staged.created.elapsed() > Duration::from_secs(30 * 60) {
        return Err(ApiError::bad_request("MVR import preview expired"));
    }
    if input.new_show.is_some() == input.existing_show_id.is_some() {
        return Err(ApiError::bad_request(
            "choose exactly one MVR import destination",
        ));
    }
    let (entry, is_new, open_after) = if let Some(new) = input.new_show {
        validate_show_name(&new.name)?;
        let path = state
            .data_dir
            .join("shows")
            .join(format!("{}.show", new.name));
        if path.exists() {
            return Err(ApiError::conflict("a show with that name already exists"));
        }
        initialise_show(&path, &new.name).map_err(ApiError::store)?;
        (
            state
                .desk
                .lock()
                .upsert_show(&new.name, &path.display().to_string(), false)
                .map_err(ApiError::store)?,
            true,
            new.open_after_import,
        )
    } else {
        let id = light_core::ShowId(input.existing_show_id.unwrap());
        (
            state
                .desk
                .lock()
                .show(id)
                .map_err(ApiError::store)?
                .ok_or_else(|| ApiError::not_found("show"))?,
            false,
            false,
        )
    };
    let temporary = state
        .data_dir
        .join("shows")
        .join(format!(".mvr-{}.show", Uuid::new_v4()));
    ShowStore::open(&entry.path)
        .map_err(ApiError::store)?
        .backup_to(&temporary)
        .map_err(ApiError::store)?;
    let (definitions, new_definitions) = mvr_definitions(&state, &staged.document)?;
    let result = (|| {
        let store = ShowStore::open(&temporary).map_err(ApiError::store)?;
        let applied =
            apply_mvr_to_store(&store, &staged.document, &definitions, &input.resolutions)?;
        validate_show_file(&temporary).map_err(ApiError::store)?;
        let probe = ShowEntry {
            path: temporary.display().to_string(),
            ..entry.clone()
        };
        load_engine_snapshot(&probe)
            .map_err(ApiError::bad_request)?
            .validate()
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        Ok::<_, ApiError>(applied)
    })();
    let (imported, unresolved, warnings) = match result {
        Ok(v) => v,
        Err(e) => {
            let _ = std::fs::remove_file(&temporary);
            if is_new {
                let _ = state.desk.lock().remove_show(entry.id);
                let _ = std::fs::remove_file(&entry.path);
            }
            return Err(e);
        }
    };
    if !is_new {
        backup_show(&state, &entry)?;
    }
    std::fs::rename(&temporary, &entry.path).map_err(ApiError::io)?;
    for (definition, source) in new_definitions {
        let json =
            serde_json::to_string(&definition).map_err(|e| ApiError::internal(e.to_string()))?;
        state
            .fixture_library
            .lock()
            .import_json_with_source(&json, Some(&source))
            .map_err(ApiError::fixture)?;
    }
    let should_open = open_after
        || state
            .active_show
            .read()
            .as_ref()
            .is_some_and(|s| s.id == entry.id);
    if should_open {
        let compiled = load_engine_snapshot(&entry).map_err(ApiError::bad_request)?;
        let _lock = state.activation_lock.lock().await;
        activate_snapshot(&state, compiled, &Transition::HoldCurrent, None).await?;
        state
            .desk
            .lock()
            .set_active_show(Some(entry.id))
            .map_err(ApiError::store)?;
        *state.active_show.write() = Some(entry.clone());
    }
    emit(
        &state,
        "mvr_imported",
        serde_json::json!({"show":entry,"fixtures":imported,"unresolved":unresolved,"scenery":0}),
    );
    Ok(Json(ApplyMvrResult {
        show: entry,
        imported_fixtures: imported,
        unresolved_fixtures: unresolved,
        imported_scenery: 0,
        opened: should_open,
        warnings,
    }))
}

pub(super) fn build_mvr_export(
    state: &AppState,
    id: Uuid,
) -> Result<(ShowEntry, light_mvr::MvrDocument, MvrExportPreview), ApiError> {
    let entry = state
        .desk
        .lock()
        .show(light_core::ShowId(id))
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
    let metas: HashMap<String, serde_json::Value> = store
        .objects("mvr_fixture")
        .map_err(ApiError::store)?
        .into_iter()
        .filter_map(|o| {
            let id = o.body.get("fixture_id")?.as_str()?.to_owned();
            Some((id, o.body))
        })
        .collect();
    let fixtures = store
        .objects("patched_fixture")
        .map_err(ApiError::store)?
        .into_iter()
        .filter_map(|o| {
            serde_json::from_value::<light_fixture::PatchedFixture>(o.body)
                .ok()
                .map(|f| (o.id, f))
        })
        .collect::<Vec<_>>();
    let mut doc = light_mvr::MvrDocument::default();
    let mut missing = Vec::new();
    let mut embedded = 0;
    for (id, f) in &fixtures {
        let meta = metas.get(id);
        let gdtf = meta
            .and_then(|m| m.get("gdtf_spec"))
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!("{}@{}.gdtf", f.definition.manufacturer, f.definition.model)
            });
        if let Some(source) = state
            .fixture_library
            .lock()
            .source_gdtf(f.definition.id, f.definition.revision)
            .map_err(ApiError::fixture)?
        {
            doc.files.entry(gdtf.to_ascii_lowercase()).or_insert(source);
            embedded += 1;
        } else {
            missing.push(format!(
                "{} · {}",
                f.definition.manufacturer, f.definition.model
            ));
        }
        let uuid = metas
            .iter()
            .find(|(_, m)| m.get("fixture_id").and_then(|v| v.as_str()) == Some(id))
            .and_then(|(uuid, _)| Uuid::parse_str(uuid).ok())
            .unwrap_or(f.fixture_id.0);
        let rx = f64::from(f.rotation.x).to_radians();
        let ry = f64::from(f.rotation.y).to_radians();
        let rz = f64::from(f.rotation.z).to_radians();
        let (sx, cx) = rx.sin_cos();
        let (sy, cy) = ry.sin_cos();
        let (sz, cz) = rz.sin_cos();
        doc.fixtures.push(light_mvr::MvrFixture {
            uuid,
            name: if f.name.is_empty() {
                f.definition.name.clone()
            } else {
                f.name.clone()
            },
            fixture_id: Some(id.clone()),
            gdtf_spec: gdtf,
            gdtf_mode: f.definition.mode.clone(),
            universe: f.universe,
            address: f.address,
            matrix: [
                cy * cz,
                cz * sx * sy - cx * sz,
                sx * sz + cx * cz * sy,
                cy * sz,
                cx * cz + sx * sy * sz,
                cx * sy * sz - cz * sx,
                -sy,
                cy * sx,
                cx * cy,
                f64::from(f.location.x),
                f64::from(f.location.y),
                f64::from(f.location.z),
            ],
            layer: Some(f.layer_id.clone()),
            class: None,
        });
    }
    let warnings = if missing.is_empty() {
        vec![]
    } else {
        vec!["Some fixture profiles have no retained source GDTF and are referenced but not embedded".into()]
    };
    let preview = MvrExportPreview {
        fixtures: doc.fixtures.len(),
        scenery: doc.geometry.len(),
        embedded_profiles: embedded,
        missing_profiles: missing,
        omitted: vec!["cues, presets, playbacks, users, and desk layouts".into()],
        warnings,
    };
    Ok((entry, doc, preview))
}
pub(super) async fn preview_mvr_export(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<MvrExportPreview>, ApiError> {
    let _ = authenticate(&state, &headers)?;
    Ok(Json(build_mvr_export(&state, id)?.2))
}
pub(super) async fn export_mvr(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _ = authenticate(&state, &headers)?;
    let (entry, doc, _) = build_mvr_export(&state, id)?;
    let data = light_mvr::write(&doc).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}.mvr\"", entry.name),
            ),
        ],
        data,
    )
        .into_response())
}
