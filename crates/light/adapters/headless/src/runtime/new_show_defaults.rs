//! A show this desk has just created takes the desk's defaults for new shows. The file is new and
//! not yet the loaded show, so it is written directly, like the packaged default show.

use super::*;

/// Copy the desk's defaults for new shows into a show this desk has just created. Only a new
/// show takes them: uploaded, imported, and existing shows keep exactly what they store, and a
/// later change to the desk default reinterprets nothing.
pub(super) fn apply_new_show_defaults(
    state: &AppState,
    path: &std::path::Path,
) -> Result<(), ApiError> {
    let model = state
        .installation
        .configuration()
        .color_programming_model_default;
    if model == light_core::ColorProgrammingModel::Direct {
        // Direct is what a show without the setting means; writing it would change nothing.
        return Ok(());
    }
    let repository = ActiveShowRepository::open(path).map_err(ApiError::store)?;
    let document = repository.portable_document().map_err(ApiError::store)?;
    let existing = document.object(
        super::attribute_configuration::ATTRIBUTE_CONFIGURATION_KIND,
        super::attribute_configuration::ATTRIBUTE_CONFIGURATION_ID,
    );
    let mut body = match existing {
        Some(object) => object.body().clone(),
        None => serde_json::to_value(light_core::AttributeConfiguration::recommended())
            .map_err(|error| ApiError::internal(error.to_string()))?,
    };
    body["color_model"] =
        serde_json::to_value(model).map_err(|error| ApiError::internal(error.to_string()))?;
    repository
        .put_object(
            super::attribute_configuration::ATTRIBUTE_CONFIGURATION_KIND,
            super::attribute_configuration::ATTRIBUTE_CONFIGURATION_ID,
            &body,
            existing.map_or(0, |object| object.revision()),
        )
        .map_err(ApiError::store)?;
    Ok(())
}
