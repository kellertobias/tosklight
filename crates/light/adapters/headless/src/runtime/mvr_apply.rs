mod active;
mod legacy;

use super::*;
use active::*;
use legacy::*;

pub(super) async fn apply_mvr_import(
    State(state): State<AppState>,
    Path(token): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ApplyMvrImport>,
) -> Result<Json<ApplyMvrResult>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let staged = state
        .mvr_imports
        .lock()
        .remove(&token)
        .ok_or_else(|| ApiError::not_found("MVR import preview"))?;
    if staged.created.elapsed() > Duration::from_secs(30 * 60) {
        return Err(ApiError::bad_request("MVR import preview expired"));
    }
    let ApplyMvrImport {
        new_show,
        existing_show_id,
        resolutions,
    } = input;
    if new_show.is_some() == existing_show_id.is_some() {
        return Err(ApiError::bad_request(
            "choose exactly one MVR import destination",
        ));
    }
    let (entry, is_new, open_after) = import_destination(&state, new_show, existing_show_id)?;
    let (definitions, new_definitions) = mvr_definitions(&state, &staged.document)?;
    let import = ActiveMvrImport {
        entry,
        document: staged.document,
        definitions,
        new_definitions,
        resolutions,
    };
    if !is_new && active_show_is(&state, import.entry.id) {
        return apply_active_mvr_import(&state, &session, import).await;
    }
    apply_legacy_mvr_import(
        &state,
        session,
        LegacyMvrImport {
            import,
            is_new,
            open_after,
        },
    )
    .await
}

fn import_destination(
    state: &AppState,
    new_show: Option<NewMvrShow>,
    existing_show_id: Option<Uuid>,
) -> Result<(ShowEntry, bool, bool), ApiError> {
    if let Some(new) = new_show {
        validate_show_name(&new.name)?;
        let path = state
            .data_dir
            .join("shows")
            .join(format!("{}.show", new.name));
        if path.exists() {
            return Err(ApiError::conflict("a show with that name already exists"));
        }
        initialise_show(&path, &new.name).map_err(ApiError::store)?;
        Ok((
            state
                .desk
                .lock()
                .upsert_show(&new.name, &path.display().to_string(), false)
                .map_err(ApiError::store)?,
            true,
            new.open_after_import,
        ))
    } else {
        let id = light_core::ShowId(existing_show_id.expect("destination was validated"));
        Ok((
            state
                .desk
                .lock()
                .show(id)
                .map_err(ApiError::store)?
                .ok_or_else(|| ApiError::not_found("show"))?,
            false,
            false,
        ))
    }
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
