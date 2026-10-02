use super::*;

pub(super) fn apply_mvr_to_store(
    store: &ActiveShowRepository,
    context: light_application::ActionContext,
    document: &light_mvr::MvrDocument,
    definitions: &HashMap<Uuid, light_fixture::FixtureDefinition>,
    resolutions: &HashMap<Uuid, MvrResolution>,
) -> Result<(usize, usize, Vec<String>), ApiError> {
    let destination = store.portable_document().map_err(ApiError::store)?;
    let planned = light_application::mvr_import::plan_mvr_document_import(
        &destination,
        context,
        document,
        definitions,
        &application_mvr_resolutions(resolutions.clone()),
    )
    .map_err(|error| ApiError::bad_request(error.message))?;
    if !planned.transaction.is_empty() {
        let candidate =
            light_application::prepare_show_candidate(&destination, planned.transaction)
                .map_err(|error| ApiError::bad_request(error.message))?;
        let (transaction, snapshot) = candidate.into_parts();
        snapshot
            .validate()
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        store
            .apply_portable_transaction(transaction)
            .map_err(ApiError::store)?;
    }
    Ok((
        planned.imported_fixtures,
        planned.unresolved_fixtures,
        planned.warnings,
    ))
}
