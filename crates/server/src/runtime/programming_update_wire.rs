//! Strict v2 transport mapping for the Programming-owned Update workflow.

use light_application::programming_update as application;
use light_core::{CueListId, ShowId};
use light_show::PortableShowRevision;
use light_wire::v2::programming_update as wire;

pub(super) fn preview_request(
    show_id: ShowId,
    request: wire::ProgrammingUpdatePreviewRequest,
) -> Result<(String, application::ProgrammingUpdatePreviewRequest), String> {
    let request_id = request.request_id;
    let command = application::ProgrammingUpdatePreviewRequest {
        show_id,
        target: application_target(request.target)?,
        mode: application_mode(request.mode),
    };
    Ok((request_id, command))
}

pub(super) fn targets_request(
    show_id: ShowId,
    request: wire::ProgrammingUpdateTargetsRequest,
) -> (String, application::ProgrammingUpdateTargetsRequest) {
    (
        request.request_id,
        application::ProgrammingUpdateTargetsRequest {
            show_id,
            filter: match request.filter {
                wire::ProgrammingUpdateTargetFilter::EligibleForUpdateExisting => {
                    application::UpdateTargetFilter::EligibleForUpdateExisting
                }
                wire::ProgrammingUpdateTargetFilter::ShowAllActive => {
                    application::UpdateTargetFilter::ShowAllActive
                }
            },
        },
    )
}

pub(super) fn action_request(
    show_id: ShowId,
    expected_show_revision: u64,
    request: wire::ProgrammingUpdateActionRequest,
) -> Result<(String, application::ProgrammingUpdateCommand), String> {
    let request_id = request.request_id;
    let (target, mode, object_revision, programmer_revision) = match request.action {
        wire::ProgrammingUpdateAction::ConfirmPreview {
            target,
            mode,
            expected_object_revision,
            expected_programmer_revision,
        } => {
            validate_confirmed_target(&target)?;
            validate_programmer_revision(&expected_programmer_revision)?;
            (
                target,
                mode,
                Some(expected_object_revision),
                Some(expected_programmer_revision),
            )
        }
        wire::ProgrammingUpdateAction::ApplyDirect { target, mode } => (target, mode, None, None),
    };
    Ok((
        request_id,
        application::ProgrammingUpdateCommand {
            show_id,
            target: application_target(target)?,
            mode: application_mode(mode),
            expected_object_revision: object_revision,
            expected_programmer_revision: programmer_revision,
            expected_show_revision: Some(PortableShowRevision::from_value(expected_show_revision)),
        },
    ))
}

pub(super) fn wire_settings(
    desk_id: uuid::Uuid,
    settings: &application::UpdateSettings,
) -> wire::ProgrammingUpdateSettingsProjection {
    wire::ProgrammingUpdateSettingsProjection {
        desk_id,
        settings: wire::ProgrammingUpdateSettings {
            cue_mode: wire_cue_mode(settings.cue_mode),
            preset_mode: wire_existing_mode(settings.preset_mode),
            group_mode: wire_existing_mode(settings.group_mode),
            show_update_modal_on_touch: settings.show_update_modal_on_touch,
        },
    }
}

pub(super) fn apply_settings(
    current: &mut application::UpdateSettings,
    settings: wire::ProgrammingUpdateSettings,
) {
    current.cue_mode = application_cue_mode(settings.cue_mode);
    current.preset_mode = application_existing_mode(settings.preset_mode);
    current.group_mode = application_existing_mode(settings.group_mode);
    current.show_update_modal_on_touch = settings.show_update_modal_on_touch;
}

fn application_target(
    target: wire::ProgrammingUpdateTarget,
) -> Result<application::ProgrammingUpdateTargetRequest, String> {
    Ok(match target {
        wire::ProgrammingUpdateTarget::Cue {
            cue_list_id,
            playback_number,
            cue_id,
            cue_number,
            validate_active_context,
        } => {
            validate_non_nil(cue_list_id, "target.cue_list_id")?;
            validate_optional_cue(cue_id, cue_number)?;
            application::ProgrammingUpdateTargetRequest::Cue {
                cue_list_id: CueListId(cue_list_id),
                playback_number,
                cue_id,
                cue_number,
                validate_active_context,
            }
        }
        wire::ProgrammingUpdateTarget::Preset { object_id } => {
            validate_identifier(&object_id, "target.object_id")?;
            application::ProgrammingUpdateTargetRequest::Preset { object_id }
        }
        wire::ProgrammingUpdateTarget::Group { object_id } => {
            validate_identifier(&object_id, "target.object_id")?;
            application::ProgrammingUpdateTargetRequest::Group { object_id }
        }
    })
}

fn application_mode(mode: wire::ProgrammingUpdateMode) -> application::UpdateMode {
    match mode {
        wire::ProgrammingUpdateMode::Cue(mode) => {
            application::UpdateMode::Cue(application_cue_mode(mode))
        }
        wire::ProgrammingUpdateMode::ExistingContent(mode) => {
            application::UpdateMode::ExistingContent(application_existing_mode(mode))
        }
    }
}

fn application_cue_mode(mode: wire::ProgrammingUpdateCueMode) -> application::CueUpdateMode {
    use application::CueUpdateMode as Output;
    use wire::ProgrammingUpdateCueMode as Input;
    match mode {
        Input::ExistingOnly => Output::ExistingOnly,
        Input::ExistingInCurrentCue => Output::ExistingInCurrentCue,
        Input::AddToCurrentCue => Output::AddToCurrentCue,
        Input::AddNew => Output::AddNew,
    }
}

fn application_existing_mode(
    mode: wire::ProgrammingUpdateExistingContentMode,
) -> application::ExistingContentMode {
    match mode {
        wire::ProgrammingUpdateExistingContentMode::UpdateExisting => {
            application::ExistingContentMode::UpdateExisting
        }
        wire::ProgrammingUpdateExistingContentMode::AddNew => {
            application::ExistingContentMode::AddNew
        }
    }
}

pub(super) fn wire_cue_mode(mode: application::CueUpdateMode) -> wire::ProgrammingUpdateCueMode {
    use application::CueUpdateMode as Input;
    use wire::ProgrammingUpdateCueMode as Output;
    match mode {
        Input::ExistingOnly => Output::ExistingOnly,
        Input::ExistingInCurrentCue => Output::ExistingInCurrentCue,
        Input::AddToCurrentCue => Output::AddToCurrentCue,
        Input::AddNew => Output::AddNew,
    }
}

pub(super) fn wire_existing_mode(
    mode: application::ExistingContentMode,
) -> wire::ProgrammingUpdateExistingContentMode {
    match mode {
        application::ExistingContentMode::UpdateExisting => {
            wire::ProgrammingUpdateExistingContentMode::UpdateExisting
        }
        application::ExistingContentMode::AddNew => {
            wire::ProgrammingUpdateExistingContentMode::AddNew
        }
    }
}

fn validate_confirmed_target(target: &wire::ProgrammingUpdateTarget) -> Result<(), String> {
    if let wire::ProgrammingUpdateTarget::Cue {
        cue_id, cue_number, ..
    } = target
        && (cue_id.is_none() || cue_number.is_none())
    {
        return Err("ConfirmPreview requires the exact previewed Cue identity".into());
    }
    Ok(())
}

fn validate_optional_cue(
    cue_id: Option<uuid::Uuid>,
    cue_number: Option<f64>,
) -> Result<(), String> {
    if cue_id.is_some() != cue_number.is_some() {
        return Err("target.cue_id and target.cue_number must be supplied together".into());
    }
    if let Some(cue_id) = cue_id {
        validate_non_nil(cue_id, "target.cue_id")?;
    }
    if cue_number.is_some_and(|number| !number.is_finite()) {
        return Err("target.cue_number must be finite".into());
    }
    Ok(())
}

fn validate_programmer_revision(value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("expected_programmer_revision must be a lowercase SHA-256 digest".into());
    }
    Ok(())
}

fn validate_identifier(value: &str, name: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(format!("{name} must contain 1-256 printable bytes"));
    }
    Ok(())
}

fn validate_non_nil(value: uuid::Uuid, name: &str) -> Result<(), String> {
    if value.is_nil() {
        Err(format!("{name} must not be nil"))
    } else {
        Ok(())
    }
}
