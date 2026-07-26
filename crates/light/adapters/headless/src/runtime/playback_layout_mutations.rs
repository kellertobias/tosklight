use super::*;

pub(super) fn put_page(
    page: light_playback::PlaybackPage,
    expected: u64,
) -> Result<light_application::ActiveShowObjectMutation, ApiError> {
    page.validate().map_err(ApiError::bad_request)?;
    put_active_show_object(
        light_application::ActiveShowObjectKind::PlaybackPage,
        page.number.to_string(),
        expected,
        serde_json::to_value(page).map_err(|error| ApiError::internal(error.to_string()))?,
    )
}
