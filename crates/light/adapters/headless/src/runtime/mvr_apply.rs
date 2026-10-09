mod active;
mod legacy;

use super::*;
use active::*;
use legacy::*;

pub(super) async fn apply_mvr_import(
    State(state): State<AppState>,
    Path(token): Path<Uuid>,
    headers: HeaderMap,
    crate::tolerant_json::TolerantJson(input): crate::tolerant_json::TolerantJson<ApplyMvrImport>,
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
        copy_conflicting_profiles,
    } = input;
    if new_show.is_some() == existing_show_id.is_some() {
        return Err(ApiError::bad_request(
            "choose exactly one MVR import destination",
        ));
    }
    if staged.destination_id != existing_show_id {
        return Err(ApiError::conflict(
            "MVR destination differs from its preview. Inspect the archive again for this destination.",
        ));
    }
    let destination_revision = staged.destination_revision;
    if let Some(id) = existing_show_id {
        let entry = state
            .installation
            .show(light_core::ShowId(id))
            .map_err(ApiError::store)?
            .ok_or_else(|| ApiError::not_found("show"))?;
        let current = ActiveShowRepository::open(&entry.path)
            .map_err(ApiError::store)?
            .portable_document()
            .map_err(ApiError::store)?;
        if destination_revision != Some((current.revision(), current.patch_revision())) {
            return Err(ApiError::conflict(
                "MVR destination changed after preview. Inspect the archive again before importing.",
            ));
        }
    }
    let worker_state = state.clone();
    let worker_resolutions = resolutions.clone();
    let bindings = tokio::task::spawn_blocking(move || {
        let mut bindings = staged.definitions;
        prepare_mvr_profiles(
            &worker_state,
            &mut bindings,
            existing_show_id,
            &worker_resolutions,
            copy_conflicting_profiles,
            &staged.profile_slots,
        )?;
        Ok::<_, ApiError>(bindings)
    })
    .await
    .map_err(|error| ApiError::internal(format!("MVR profile planning failed: {error}")))??;
    let (entry, is_new, open_after) = import_destination(&state, new_show, existing_show_id)?;
    let import = ActiveMvrImport {
        entry,
        destination_revision,
        document: staged.document,
        definitions: bindings.definitions,
        new_profiles: bindings.new_profiles,
        warnings: bindings.warnings,
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
        super::new_show_defaults::apply_new_show_defaults(state, &path)?;
        let entry = state
            .installation
            .upsert_show(&new.name, &path.display().to_string(), false)
            .map_err(ApiError::store)?;
        // Installation entries and portable show documents must share one show identity.
        ActiveShowRepository::open(&path)
            .and_then(|store| {
                store.set_identity(entry.id, &entry.name, entry.revision_copy.as_ref())
            })
            .map_err(ApiError::store)?;
        Ok((entry, true, new.open_after_import))
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

    fn source_gdtf_with_evidence(
        &self,
        profile: light_core::FixtureId,
        revision: u32,
    ) -> Result<Option<light_fixture::FixtureGdtfSource>, Self::Error> {
        self.0
            .installation
            .fixture_source_gdtf_with_evidence(profile, revision)
            .map_err(ApiError::fixture)
    }
}

pub(super) fn build_mvr_export(
    state: &AppState,
    id: Uuid,
) -> Result<
    (
        ShowEntry,
        light_mvr::MvrDocument,
        light_wire::v2::show_library::MvrExportSummary,
    ),
    ApiError,
> {
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
    let layers = light_application::mvr_export::mvr_layers(
        store
            .objects("patch_layer")
            .map_err(ApiError::store)?
            .into_iter()
            .map(|o| (o.id, o.body)),
    );
    let (doc, summary) = light_application::mvr_export::build_mvr_document(
        &fixtures,
        &metas,
        layers,
        &InstallationGdtf(state),
        // The hinge the Visualizer and the CAD turn the lamp's body about.
        viz_project::patched_bracket_hinge_millimetres,
    )?;
    let summary = light_wire::v2::show_library::MvrExportSummary {
        fixtures: summary.fixtures,
        scenery: summary.scenery,
        // A generated GDTF is embedded too; the warnings say which kind the archive carries.
        embedded_profiles: summary.embedded_profiles + summary.generated_profiles,
        missing_profiles: summary.missing_profiles,
        omitted: vec!["cues, presets, playbacks, users, and desk layouts".into()],
        warnings: summary.warnings,
    };
    Ok((entry, doc, summary))
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

pub(super) fn application_mvr_resolutions(
    resolutions: HashMap<Uuid, MvrResolution>,
) -> HashMap<Uuid, light_application::MvrImportResolution> {
    resolutions
        .into_iter()
        .map(|(id, resolution)| {
            let resolution = match resolution {
                MvrResolution::Import => light_application::MvrImportResolution::Import,
                MvrResolution::Skip => light_application::MvrImportResolution::Skip,
                MvrResolution::ImportUnpatched => {
                    light_application::MvrImportResolution::ImportUnpatched
                }
                MvrResolution::Replace => light_application::MvrImportResolution::Replace,
                MvrResolution::Address { universe, address } => {
                    light_application::MvrImportResolution::Address { universe, address }
                }
            };
            (id, resolution)
        })
        .collect()
}
