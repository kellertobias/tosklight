//! Typed, replay-safe v2 fixture-library transport.

use super::fixture_api_replay::ReplayKey;
use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::fixture_library as wire;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/fixture-library", post(fixture_library_action))
        .route(
            "/api/v2/fixture-library/definitions",
            get(fixture_definitions_snapshot),
        )
        .route(
            "/api/v2/fixture-library/profiles",
            get(fixture_profiles_snapshot),
        )
        .route(
            "/api/v2/fixture-library/warnings",
            get(fixture_library_warnings_snapshot),
        )
        .route(
            "/api/v2/fixture-library/source-mappings",
            get(fixture_source_mappings_snapshot),
        )
        .route(
            "/api/v2/fixture-library/profiles/{id}/revisions",
            get(fixture_profile_revisions),
        )
        .route(
            "/api/v2/fixture-library/profiles/{id}/revisions/{revision}/package",
            get(export_fixture_package),
        )
        .route(
            "/api/v2/fixture-library/gel-catalogs",
            get(gel_catalogs_snapshot),
        )
        .route(
            "/api/v2/fixture-library/gel-catalogs/import/preview",
            post(preview_gel_catalog_import),
        )
        .route(
            "/api/v2/fixture-library/gel-catalogs/{catalog_id}/update",
            post(confirm_gel_catalog_import),
        )
}

#[derive(Default, Deserialize)]
struct GelCatalogSearchQuery {
    query: Option<String>,
}

async fn gel_catalogs_snapshot(
    State(state): State<AppState>,
    Query(search): Query<GelCatalogSearchQuery>,
    headers: HeaderMap,
) -> Result<Json<wire::GelCatalogsSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let query = search.query.unwrap_or_default();
    if query.len() > 256 || query.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "gel catalog query must contain at most 256 printable bytes",
        ));
    }
    let needle = query.trim().to_lowercase();
    let catalogs = state
        .installation
        .gel_catalogs()
        .map_err(ApiError::fixture)?
        .into_iter()
        .filter_map(|mut catalog| {
            if !needle.is_empty() && !catalog.name.to_lowercase().contains(&needle) {
                catalog.entries.retain(|entry| {
                    entry.number.to_lowercase().contains(&needle)
                        || entry.name.to_lowercase().contains(&needle)
                        || entry.display_srgb.to_lowercase().contains(&needle)
                });
                if catalog.entries.is_empty() {
                    return None;
                }
            }
            Some(gel_catalog(&catalog))
        })
        .collect();
    Ok(Json(wire::GelCatalogsSnapshot { catalogs }))
}

async fn preview_gel_catalog_import(
    State(state): State<AppState>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::GelCatalogImportPreviewRequest>,
) -> Result<Json<wire::GelCatalogImportPreview>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let csv = decode_archive(&request.csv_base64, "gel catalog CSV")?;
    let preview = state
        .installation
        .preview_gel_catalog_csv_import(
            gel_catalog_import_target(request.target),
            &request.catalog_name,
            &csv,
        )
        .map_err(ApiError::fixture)?;
    Ok(Json(gel_catalog_import_preview(&preview)))
}

async fn confirm_gel_catalog_import(
    State(state): State<AppState>,
    Path(catalog_id): Path<Uuid>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::GelCatalogImportConfirmRequest>,
) -> Result<Json<wire::GelCatalogImportConfirmOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    if catalog_id != wire_gel_catalog_target_id(request.target) {
        return Err(ApiError::bad_request(
            "gel catalog path identity must match the import target",
        ));
    }
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };
    let signature =
        gel_catalog_import_signature(request.target, &request.catalog_name, &request.csv_base64)?;
    if let Some(outcome) = state
        .replay
        .lookup_fixture_library(&key, &signature)
        .await?
    {
        let wire::FixtureLibraryActionResult::GelCatalogImported { catalog } = outcome.result
        else {
            return Err(ApiError::internal(
                "gel catalog replay returned an incompatible fixture-library outcome",
            ));
        };
        return Ok(Json(wire::GelCatalogImportConfirmOutcome {
            request_id: outcome.request_id,
            replayed: true,
            catalog,
        }));
    }
    let catalog = execute_gel_catalog_import(
        &state,
        request.target,
        request.catalog_name,
        request.csv_base64,
    )?;
    let result = wire::FixtureLibraryActionResult::GelCatalogImported {
        catalog: catalog.clone(),
    };
    state
        .replay
        .insert_fixture_library(
            key,
            signature,
            wire::FixtureLibraryActionOutcome {
                request_id: request.request_id.clone(),
                replayed: false,
                result,
            },
        )
        .await;
    Ok(Json(wire::GelCatalogImportConfirmOutcome {
        request_id: request.request_id,
        replayed: false,
        catalog,
    }))
}

async fn fixture_definitions_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::FixtureDefinitionsSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(wire::FixtureDefinitionsSnapshot {
        definitions: json_values(
            state
                .installation
                .fixture_definitions()
                .map_err(ApiError::fixture)?,
        )?,
    }))
}

async fn fixture_profiles_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::FixtureProfilesSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(wire::FixtureProfilesSnapshot {
        profiles: json_values(
            state
                .installation
                .fixture_profiles()
                .map_err(ApiError::fixture)?,
        )?,
    }))
}

async fn fixture_library_warnings_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::FixtureLibraryWarningsSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    Ok(Json(wire::FixtureLibraryWarningsSnapshot {
        warnings: state
            .installation
            .fixture_library_warnings()
            .map_err(ApiError::fixture)?,
    }))
}

async fn fixture_source_mappings_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::FixtureSourceMappingsSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let mappings = state
        .installation
        .fixture_source_mapping_preferences()
        .map_err(ApiError::fixture)?
        .into_iter()
        .map(fixture_source_mapping)
        .collect();
    Ok(Json(wire::FixtureSourceMappingsSnapshot { mappings }))
}

async fn fixture_profile_revisions(
    State(state): State<AppState>,
    Path(id): Path<light_core::FixtureId>,
    headers: HeaderMap,
) -> Result<Json<wire::FixtureProfileRevisionsSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let profiles = state
        .installation
        .fixture_profile_revisions(id)
        .map_err(ApiError::fixture)?;
    Ok(Json(wire::FixtureProfileRevisionsSnapshot {
        profiles: json_values(profiles)?,
    }))
}

async fn fixture_library_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::FixtureLibraryActionRequest>,
) -> Result<Json<wire::FixtureLibraryActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };
    let signature = action_signature(&request.action)?;
    if let Some(outcome) = state
        .replay
        .lookup_fixture_library(&key, &signature)
        .await?
    {
        return Ok(Json(outcome));
    }
    let result = execute_action(&state, request.action)?;
    let outcome = wire::FixtureLibraryActionOutcome {
        request_id: request.request_id,
        replayed: false,
        result,
    };
    state
        .replay
        .insert_fixture_library(key, signature, outcome.clone())
        .await;
    Ok(Json(outcome))
}

fn execute_action(
    state: &AppState,
    action: wire::FixtureLibraryAction,
) -> Result<wire::FixtureLibraryActionResult, ApiError> {
    use wire::{FixtureLibraryAction as Action, FixtureLibraryActionResult as Result};
    match action {
        Action::SaveProfile {
            profile,
            expected_revision,
        } => save_profile(state, profile, expected_revision),
        Action::DeleteProfileRevision {
            profile_id,
            revision,
        } => {
            let id = light_core::FixtureId(profile_id);
            if !state
                .installation
                .delete_fixture_profile(id, revision)
                .map_err(ApiError::fixture)?
            {
                return Err(ApiError::not_found("fixture profile revision"));
            }
            emit(
                state,
                "fixture_profile_changed",
                serde_json::json!({"id":id,"revision":revision,"deleted":true}),
            );
            Ok(Result::Deleted {
                resource: wire::FixtureLibraryResource::Profile,
                id: profile_id,
                revision,
            })
        }
        Action::ImportPackage {
            package_base64,
            attribute_mappings,
        } => import_package(state, &package_base64, attribute_mappings),
        Action::AttachGdtf {
            profile_id,
            revision,
            source_base64,
        } => {
            let source = decode_archive(&source_base64, "GDTF source archive")?;
            let id = light_core::FixtureId(profile_id);
            if !state
                .installation
                .attach_fixture_profile_gdtf(id, revision, &source)
                .map_err(ApiError::fixture)?
            {
                return Err(ApiError::not_found("fixture profile revision"));
            }
            emit(
                state,
                "fixture_profile_changed",
                serde_json::json!({"id":id,"revision":revision,"source_gdtf":true}),
            );
            Ok(Result::GdtfAttached {
                profile_id,
                revision,
            })
        }
        Action::RememberSourceMapping {
            source_format,
            source_attribute,
            target_attribute,
        } => remember_source_mapping(state, source_format, source_attribute, target_attribute),
        Action::SaveDefinition { definition } => {
            let definition: light_fixture::FixtureDefinition =
                serde_json::from_value(definition)
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
            let stored = state
                .installation
                .import_fixture_definition(&definition)
                .map_err(ApiError::fixture)?;
            emit(
                state,
                "fixture_library_changed",
                serde_json::json!({"id":stored.id,"revision":stored.revision}),
            );
            Ok(Result::Definition {
                definition_id: stored.id.0,
                revision: stored.revision,
            })
        }
        Action::DeleteDefinitionRevision {
            definition_id,
            revision,
        } => {
            let id = light_core::FixtureId(definition_id);
            if !state
                .installation
                .delete_fixture_definition(id, revision)
                .map_err(ApiError::fixture)?
            {
                return Err(ApiError::not_found("fixture definition"));
            }
            emit(
                state,
                "fixture_library_changed",
                serde_json::json!({"id":id,"revision":revision,"deleted":true}),
            );
            Ok(Result::Deleted {
                resource: wire::FixtureLibraryResource::Definition,
                id: definition_id,
                revision,
            })
        }
    }
}

fn remember_source_mapping(
    state: &AppState,
    source_format: String,
    source_attribute: String,
    target_attribute: Option<String>,
) -> Result<wire::FixtureLibraryActionResult, ApiError> {
    let source_format = source_format.trim().to_ascii_lowercase();
    let source_attribute = source_attribute.trim();
    if source_format.is_empty()
        || source_format.len() > 32
        || !source_format
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(ApiError::bad_request(
            "source format must contain 1-32 ASCII letters, numbers, hyphens, or underscores",
        ));
    }
    if source_attribute.is_empty()
        || source_attribute.len() > 256
        || source_attribute.chars().any(char::is_control)
    {
        return Err(ApiError::bad_request(
            "source attribute must contain 1-256 printable bytes",
        ));
    }
    if let Some(target) = target_attribute.as_deref() {
        validate_source_mapping_target(state, target)?;
    }
    let mapping = state
        .installation
        .set_fixture_source_mapping_preference(
            &source_format,
            source_attribute,
            target_attribute.as_deref(),
        )
        .map_err(ApiError::fixture)?
        .map(fixture_source_mapping);
    emit(
        state,
        "fixture_source_mapping_changed",
        serde_json::json!({"source_format":source_format,"source_attribute":source_attribute,"target_attribute":target_attribute}),
    );
    Ok(wire::FixtureLibraryActionResult::SourceMapping { mapping })
}

fn validate_source_mapping_target(state: &AppState, target: &str) -> Result<(), ApiError> {
    if light_core::ATTRIBUTE_REGISTRY
        .iter()
        .any(|descriptor| descriptor.id == target)
    {
        return Ok(());
    }
    let installed = state.attributes.snapshot();
    let custom = installed
        .configuration
        .custom_attributes
        .iter()
        .find(|descriptor| descriptor.id.0 == target)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "source mapping target `{target}` is not configured"
            ))
        })?;
    if custom.lifecycle == light_core::CustomAttributeLifecycle::Retired {
        return Err(ApiError::bad_request(format!(
            "source mapping target `{target}` is retired"
        )));
    }
    Ok(())
}

fn fixture_source_mapping(
    mapping: light_fixture::FixtureSourceMappingPreference,
) -> wire::FixtureSourceMapping {
    wire::FixtureSourceMapping {
        source_format: mapping.source_format,
        source_attribute: mapping.source_attribute,
        target_attribute: mapping.target_attribute,
    }
}

fn execute_gel_catalog_import(
    state: &AppState,
    target: wire::GelCatalogImportTarget,
    catalog_name: String,
    csv_base64: String,
) -> Result<wire::GelCatalog, ApiError> {
    let csv = decode_archive(&csv_base64, "gel catalog CSV")?;
    let preview = state
        .installation
        .preview_gel_catalog_csv_import(gel_catalog_import_target(target), &catalog_name, &csv)
        .map_err(ApiError::fixture)?;
    if !preview.conflicts().is_empty() {
        return Err(ApiError::conflict(
            "gel catalog changed after preview; preview the import again before confirming",
        ));
    }
    if !preview.invalid_rows().is_empty() {
        return Err(ApiError::bad_request(
            "gel catalog import has invalid rows and cannot be confirmed",
        ));
    }
    let catalog = state
        .installation
        .confirm_gel_catalog_csv_import(&preview)
        .map_err(gel_catalog_confirm_error)?;
    emit(
        state,
        "fixture_library_changed",
        serde_json::json!({
            "id": catalog.id,
            "revision": catalog.revision,
            "gel_catalog": true
        }),
    );
    Ok(gel_catalog(&catalog))
}

fn gel_catalog_confirm_error(error: light_fixture::FixtureError) -> ApiError {
    match &error {
        light_fixture::FixtureError::Invalid(message)
            if message.starts_with("stale gel catalog import preview:") =>
        {
            ApiError::conflict(message.clone())
        }
        _ => ApiError::fixture(error),
    }
}

fn gel_catalog_import_target(
    target: wire::GelCatalogImportTarget,
) -> light_fixture::GelCatalogImportTarget {
    match target {
        wire::GelCatalogImportTarget::Create { catalog_id } => {
            light_fixture::GelCatalogImportTarget::Create { catalog_id }
        }
        wire::GelCatalogImportTarget::Update {
            catalog_id,
            expected_revision,
        } => light_fixture::GelCatalogImportTarget::Update {
            catalog_id,
            expected_revision,
        },
    }
}

fn wire_gel_catalog_target_id(target: wire::GelCatalogImportTarget) -> Uuid {
    match target {
        wire::GelCatalogImportTarget::Create { catalog_id }
        | wire::GelCatalogImportTarget::Update { catalog_id, .. } => catalog_id,
    }
}

fn gel_catalog(catalog: &light_fixture::GelCatalog) -> wire::GelCatalog {
    wire::GelCatalog {
        id: catalog.id,
        revision: catalog.revision,
        name: catalog.name.clone(),
        entries: catalog.entries.iter().map(gel_catalog_entry).collect(),
    }
}

fn gel_catalog_entry(entry: &light_fixture::GelCatalogEntry) -> wire::GelCatalogEntry {
    wire::GelCatalogEntry {
        id: entry.id,
        number: entry.number.clone(),
        name: entry.name.clone(),
        display_srgb: entry.display_srgb.clone(),
        visualizer_srgb: entry.visualizer_srgb.clone(),
    }
}

fn gel_catalog_import_preview(
    preview: &light_fixture::GelCatalogImportPreview,
) -> wire::GelCatalogImportPreview {
    wire::GelCatalogImportPreview {
        catalog_id: preview.catalog_id(),
        catalog_name: preview.catalog_name().to_owned(),
        catalog_name_changed: preview.catalog_name_changed(),
        additions: preview
            .additions()
            .iter()
            .map(|addition| wire::GelCatalogImportAddition {
                entry: gel_catalog_entry(&addition.entry),
            })
            .collect(),
        replacements: preview
            .replacements()
            .iter()
            .map(|replacement| wire::GelCatalogImportReplacement {
                previous: gel_catalog_entry(&replacement.previous),
                replacement: gel_catalog_entry(&replacement.replacement),
            })
            .collect(),
        unchanged: preview
            .unchanged()
            .iter()
            .map(|unchanged| gel_catalog_entry(&unchanged.entry))
            .collect(),
        conflicts: preview
            .conflicts()
            .iter()
            .map(|conflict| match conflict {
                light_fixture::GelCatalogImportConflict::CatalogIdentityAlreadyExists {
                    catalog_id,
                } => wire::GelCatalogImportConflict::CatalogIdentityAlreadyExists {
                    catalog_id: *catalog_id,
                },
                light_fixture::GelCatalogImportConflict::CatalogMissing { catalog_id } => {
                    wire::GelCatalogImportConflict::CatalogMissing {
                        catalog_id: *catalog_id,
                    }
                }
                light_fixture::GelCatalogImportConflict::RevisionMismatch {
                    catalog_id,
                    expected,
                    current,
                } => wire::GelCatalogImportConflict::RevisionMismatch {
                    catalog_id: *catalog_id,
                    expected: *expected,
                    current: *current,
                },
            })
            .collect(),
        invalid_rows: preview
            .invalid_rows()
            .iter()
            .map(|error| wire::GelCatalogCsvError {
                row: error.row,
                message: error.message.clone(),
            })
            .collect(),
        confirmable: preview.is_confirmable(),
    }
}

fn save_profile(
    state: &AppState,
    profile: serde_json::Value,
    expected_revision: u64,
) -> Result<wire::FixtureLibraryActionResult, ApiError> {
    let profile = serde_json::from_value(profile)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let expected = u32::try_from(expected_revision)
        .map_err(|_| ApiError::bad_request("fixture profile revision exceeds u32"))?;
    if expected == 0 {
        require_known_canonical_attributes(state, &profile)?;
    }
    let stored = state
        .installation
        .save_fixture_profile(profile, expected)
        .map_err(ApiError::fixture)?;
    emit(
        state,
        "fixture_profile_changed",
        serde_json::json!({"id":stored.id,"revision":stored.revision}),
    );
    Ok(wire::FixtureLibraryActionResult::Profile {
        profile_id: stored.id.0,
        revision: stored.revision,
    })
}

fn import_package(
    state: &AppState,
    package_base64: &str,
    attribute_mappings: Vec<wire::FixtureAttributeMapping>,
) -> Result<wire::FixtureLibraryActionResult, ApiError> {
    use wire::FixtureLibraryActionResult as Result;
    let package = decode_archive(package_base64, "fixture package")?;
    let mut profile = light_fixture::read_fixture_package(&package)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let unknown = unknown_canonical_attributes(state, &profile);
    if !unknown.is_empty() && attribute_mappings.is_empty() {
        return Ok(Result::ImportRequired {
            unknown_attributes: unknown
                .into_iter()
                .map(|(attribute, value_type)| wire::FixtureImportRequirement {
                    attribute,
                    value_type: fixture_import_value_type(value_type),
                })
                .collect(),
        });
    }
    apply_fixture_attribute_mappings(state, &mut profile, &unknown, attribute_mappings)?;
    let package = light_fixture::write_fixture_package(&profile)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let stored = state
        .installation
        .import_fixture_package(&package)
        .map_err(ApiError::fixture)?;
    emit(
        state,
        "fixture_profile_changed",
        serde_json::json!({"id":stored.id,"revision":stored.revision,"imported_package":true}),
    );
    Ok(Result::Profile {
        profile_id: stored.id.0,
        revision: stored.revision,
    })
}

fn require_known_canonical_attributes(
    state: &AppState,
    profile: &light_fixture::FixtureProfile,
) -> Result<(), ApiError> {
    let unknown = unknown_canonical_attributes(state, profile)
        .into_iter()
        .map(|(attribute, _)| attribute)
        .collect::<Vec<_>>();
    if unknown.is_empty() {
        return Ok(());
    }
    Err(ApiError::bad_request(format!(
        "Fixture import paused: map or create canonical descriptors for {} in Show > Desk Setup > Programmer > Attributes, then retry the import",
        unknown.join(", ")
    )))
}

fn unknown_canonical_attributes(
    state: &AppState,
    profile: &light_fixture::FixtureProfile,
) -> Vec<(String, light_core::AttributeValueType)> {
    let installed = state.attributes.snapshot();
    let custom = installed
        .configuration
        .custom_attributes
        .iter()
        .map(|descriptor| descriptor.id.0.as_str())
        .collect::<HashSet<_>>();
    let is_unknown = |attribute: &str| {
        !light_core::ATTRIBUTE_REGISTRY
            .iter()
            .any(|descriptor| descriptor.id == attribute)
            && !custom.contains(attribute)
    };
    let mut unknown = HashMap::<String, light_core::AttributeValueType>::new();
    for channel in profile.modes.iter().flat_map(|mode| &mode.channels) {
        if is_unknown(&channel.attribute.0) {
            unknown
                .entry(channel.attribute.0.clone())
                .or_insert(light_core::AttributeValueType::Continuous);
        }
        for function in &channel.functions {
            if !is_unknown(&function.attribute.0) {
                continue;
            }
            let value_type = match function.behavior {
                light_fixture::ChannelFunctionBehavior::Control { .. } => {
                    light_core::AttributeValueType::Control
                }
                light_fixture::ChannelFunctionBehavior::Fixed { .. }
                | light_fixture::ChannelFunctionBehavior::Indexed { .. } => {
                    light_core::AttributeValueType::Indexed
                }
                light_fixture::ChannelFunctionBehavior::Continuous { .. } => {
                    light_core::AttributeValueType::Continuous
                }
            };
            unknown
                .entry(function.attribute.0.clone())
                .and_modify(|current| {
                    if import_value_type_rank(value_type) > import_value_type_rank(*current) {
                        *current = value_type;
                    }
                })
                .or_insert(value_type);
        }
    }
    let mut unknown = unknown.into_iter().collect::<Vec<_>>();
    unknown.sort_by(|left, right| left.0.cmp(&right.0));
    unknown
}

fn import_value_type_rank(value_type: light_core::AttributeValueType) -> u8 {
    match value_type {
        light_core::AttributeValueType::Continuous => 0,
        light_core::AttributeValueType::Color => 1,
        light_core::AttributeValueType::Indexed => 2,
        light_core::AttributeValueType::Control => 3,
    }
}

fn fixture_import_value_type(
    value_type: light_core::AttributeValueType,
) -> wire::AttributeValueType {
    match value_type {
        light_core::AttributeValueType::Continuous => wire::AttributeValueType::Continuous,
        light_core::AttributeValueType::Color => wire::AttributeValueType::Color,
        light_core::AttributeValueType::Indexed => wire::AttributeValueType::Indexed,
        light_core::AttributeValueType::Control => wire::AttributeValueType::Control,
    }
}

fn apply_fixture_attribute_mappings(
    state: &AppState,
    profile: &mut light_fixture::FixtureProfile,
    unknown: &[(String, light_core::AttributeValueType)],
    mappings: Vec<wire::FixtureAttributeMapping>,
) -> Result<(), ApiError> {
    if unknown.is_empty() {
        if mappings.is_empty() {
            return Ok(());
        }
        return Err(ApiError::bad_request(
            "attribute mappings were supplied but the fixture package has no unknown attributes",
        ));
    }
    let expected = unknown
        .iter()
        .map(|(attribute, value_type)| (attribute.as_str(), *value_type))
        .collect::<HashMap<_, _>>();
    let installed = state.attributes.snapshot();
    let mut targets = light_core::ATTRIBUTE_REGISTRY
        .iter()
        .map(|descriptor| (descriptor.id, (descriptor.value_type, false)))
        .collect::<HashMap<_, _>>();
    targets.extend(
        installed
            .configuration
            .custom_attributes
            .iter()
            .map(|descriptor| {
                (
                    descriptor.id.0.as_str(),
                    (
                        descriptor.value_type,
                        descriptor.lifecycle == light_core::CustomAttributeLifecycle::Retired,
                    ),
                )
            }),
    );
    let mut resolved = HashMap::<String, String>::new();
    for mapping in mappings {
        let source_type = expected
            .get(mapping.source_attribute.as_str())
            .copied()
            .ok_or_else(|| {
                ApiError::bad_request(format!(
                    "attribute mapping source `{}` is not an unknown attribute in this package",
                    mapping.source_attribute
                ))
            })?;
        let (target_type, retired) = targets
            .get(mapping.target_attribute.as_str())
            .copied()
            .ok_or_else(|| {
                ApiError::bad_request(format!(
                    "attribute mapping target `{}` is not configured",
                    mapping.target_attribute
                ))
            })?;
        if retired {
            return Err(ApiError::bad_request(format!(
                "attribute mapping target `{}` is retired",
                mapping.target_attribute
            )));
        }
        if source_type != target_type {
            return Err(ApiError::bad_request(format!(
                "attribute mapping `{}` to `{}` is incompatible: expected {:?}, received {:?}",
                mapping.source_attribute, mapping.target_attribute, source_type, target_type
            )));
        }
        if resolved
            .insert(mapping.source_attribute.clone(), mapping.target_attribute)
            .is_some()
        {
            return Err(ApiError::bad_request(format!(
                "attribute mapping source `{}` appears more than once",
                mapping.source_attribute
            )));
        }
    }
    let missing = expected
        .keys()
        .filter(|source| !resolved.contains_key(**source))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(ApiError::bad_request(format!(
            "Fixture import paused: choose a compatible mapping for {}",
            missing.join(", ")
        )));
    }
    drop(installed);
    for channel in profile.modes.iter_mut().flat_map(|mode| &mut mode.channels) {
        if let Some(target) = resolved.get(&channel.attribute.0) {
            channel.attribute = light_core::AttributeKey(target.clone());
        }
        for function in &mut channel.functions {
            if let Some(target) = resolved.get(&function.attribute.0) {
                function.attribute = light_core::AttributeKey(target.clone());
            }
        }
    }
    require_known_canonical_attributes(state, profile)
}

async fn export_fixture_package(
    State(state): State<AppState>,
    Path((id, revision)): Path<(light_core::FixtureId, u32)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let (profile, bytes) = state
        .installation
        .export_fixture_package(id, revision)
        .map_err(ApiError::fixture)?
        .ok_or_else(|| ApiError::not_found("fixture profile revision"))?;
    let filename = format!(
        "{}-{}.toskfixture",
        profile
            .manufacturer
            .chars()
            .chain(std::iter::once('-'))
            .chain(profile.name.chars())
            .map(|character| if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            })
            .collect::<String>()
            .trim_matches('-'),
        revision
    );
    Ok((
        [
            (
                header::CONTENT_TYPE,
                light_fixture::FIXTURE_PACKAGE_MIME_TYPE,
            ),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{filename}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}

fn decode_archive(encoded: &str, label: &str) -> Result<Vec<u8>, ApiError> {
    if encoded.is_empty() {
        return Err(ApiError::bad_request(format!("{label} is empty")));
    }
    STANDARD
        .decode(encoded)
        .map_err(|error| ApiError::bad_request(format!("invalid {label} base64: {error}")))
}

fn json_values<T: Serialize>(values: Vec<T>) -> Result<Vec<serde_json::Value>, ApiError> {
    values
        .into_iter()
        .map(|value| encode_value(value, "fixture library projection"))
        .collect()
}

fn action_signature(action: &wire::FixtureLibraryAction) -> Result<[u8; 32], ApiError> {
    let bytes = serde_json::to_vec(action).map_err(|error| {
        ApiError::internal(format!("fixture-library action encoding failed: {error}"))
    })?;
    Ok(Sha256::digest(bytes).into())
}

fn gel_catalog_import_signature(
    target: wire::GelCatalogImportTarget,
    catalog_name: &str,
    csv_base64: &str,
) -> Result<[u8; 32], ApiError> {
    let bytes = serde_json::to_vec(&(target, catalog_name, csv_base64)).map_err(|error| {
        ApiError::internal(format!("gel catalog import encoding failed: {error}"))
    })?;
    Ok(Sha256::digest(bytes).into())
}

fn encode_value<T: Serialize>(value: T, label: &str) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(value)
        .map_err(|error| ApiError::internal(format!("{label} encoding failed: {error}")))
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
