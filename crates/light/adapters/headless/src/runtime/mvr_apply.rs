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
        .active_show
        .take_mvr_import(token)
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
            .installation
            .data_dir()
            .join("shows")
            .join(format!("{}.show", new.name));
        if path.exists() {
            return Err(ApiError::conflict("a show with that name already exists"));
        }
        initialise_show(&path, &new.name).map_err(ApiError::store)?;
        Ok((
            state
                .installation
                .upsert_show(&new.name, &path.display().to_string(), false)
                .map_err(ApiError::store)?,
            true,
            new.open_after_import,
        ))
    } else {
        let id = light_core::ShowId(existing_show_id.expect("destination was validated"));
        Ok((
            state
                .installation
                .show(id)
                .map_err(ApiError::store)?
                .ok_or_else(|| ApiError::not_found("show"))?,
            false,
            false,
        ))
    }
}

/// Reads retained source GDTF from the desk installation for the shared MVR export builder.
struct InstallationGdtf<'a>(&'a AppState);

impl light_application::mvr_export::GdtfSource for InstallationGdtf<'_> {
    type Error = ApiError;

    fn source_gdtf(
        &self,
        profile: light_core::FixtureId,
        revision: u32,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        self.0
            .installation
            .fixture_source_gdtf(profile, revision)
            .map_err(ApiError::fixture)
    }
}

pub(super) fn build_mvr_export(
    state: &AppState,
    id: Uuid,
) -> Result<(ShowEntry, light_mvr::MvrDocument, MvrExportPreview), ApiError> {
    let entry = state
        .installation
        .show(light_core::ShowId(id))
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let metas: HashMap<String, serde_json::Value> = store
        .objects("mvr_fixture")
        .map_err(ApiError::store)?
        .into_iter()
        .filter_map(|o| {
            let id = o.body.get("fixture_id")?.as_str()?.to_owned();
            Some((id, o.body))
        })
        .collect();
    // A stored patch references an immutable profile revision; the reference must be resolved
    // before the export has a manufacturer, model or mode to write.
    let objects = store
        .objects("patched_fixture")
        .map_err(ApiError::store)?
        .into_iter()
        .map(|o| (o.id, o.body));
    let fixtures = light_application::mvr_export::compile_export_fixtures(objects, |reference| {
        store
            .resolve_fixture_profile_revision(reference.profile_id, reference.profile_revision)
            .ok()
            .flatten()
            .map(|profile| {
                light_fixture::ResolvedFixtureProfileRevision::new(
                    profile.id().profile_id(),
                    profile.id().revision(),
                    profile.digest().as_str(),
                    profile.profile().clone(),
                )
            })
    })
    .map_err(|error| ApiError::internal(error.to_string()))?;
    let (doc, summary) = light_application::mvr_export::build_mvr_document(
        &fixtures,
        &metas,
        &InstallationGdtf(state),
    )?;
    let preview = MvrExportPreview {
        fixtures: summary.fixtures,
        scenery: summary.scenery,
        embedded_profiles: summary.embedded_profiles,
        missing_profiles: summary.missing_profiles,
        omitted: vec!["cues, presets, playbacks, users, and desk layouts".into()],
        warnings: summary.warnings,
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
