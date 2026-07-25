use super::{
    PlaybackTopologyCommand, PlaybackTopologyResolution,
    change::{PreparedTopology, changed_present, no_change},
    stored::{
        Stored, find_page, invalid, not_found, page_object_id, same_typed, stored_projection,
        validate_identity, validate_revision,
    },
    validation::validate_page,
};
use crate::active_show::PreparedActiveShowTransaction;
use crate::{ActionError, ActiveShowObjectKind, lossless_json};
use light_playback::PlaybackPage;
use light_show::PortableShowDocument;
use std::collections::HashMap;

pub(super) fn create_page(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    number: u8,
    expected_revision: u64,
    expected_object_id: Option<&str>,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    validate_page(number)?;
    let stored = validated_page(document, number, expected_revision, expected_object_id)?;
    let resolution = PlaybackTopologyResolution::Page { page: number };
    if let Some(stored) = stored.as_ref() {
        return Ok(no_change(
            document,
            command,
            resolution,
            vec![stored_projection(
                ActiveShowObjectKind::PlaybackPage,
                stored,
            )],
        ));
    }
    let desired = PlaybackPage {
        number,
        name: format!("Page {number}"),
        slots: HashMap::new(),
    };
    desired.validate().map_err(invalid)?;
    let object_id = page_object_id(document, None, number)?;
    changed_present(
        document,
        command,
        resolution,
        vec![(
            ActiveShowObjectKind::PlaybackPage,
            object_id,
            serde_json::to_value(desired).map_err(invalid)?,
        )],
        Vec::new(),
    )
}

pub(super) fn rename_page(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    number: u8,
    name: &str,
    expected_revision: u64,
    expected_object_id: Option<&str>,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    validate_page(number)?;
    validate_name(name)?;
    let stored = validated_page(document, number, expected_revision, expected_object_id)?
        .ok_or_else(|| not_found("Playback Page does not exist"))?;
    let resolution = PlaybackTopologyResolution::Page { page: number };
    let mut desired = stored.typed.clone();
    desired.name = name.to_owned();
    if same_typed(&stored.typed, &desired)? {
        return Ok(no_change(
            document,
            command,
            resolution,
            vec![stored_projection(
                ActiveShowObjectKind::PlaybackPage,
                &stored,
            )],
        ));
    }
    let body =
        lossless_json::merge_typed(&stored.raw_body, &stored.typed, &desired).map_err(invalid)?;
    changed_present(
        document,
        command,
        resolution,
        vec![(ActiveShowObjectKind::PlaybackPage, stored.object_id, body)],
        Vec::new(),
    )
}

fn validated_page(
    document: &PortableShowDocument,
    number: u8,
    expected_revision: u64,
    expected_object_id: Option<&str>,
) -> Result<Option<Stored<PlaybackPage>>, ActionError> {
    let stored = find_page(document, number)?;
    validate_identity(
        stored.as_ref(),
        expected_object_id,
        "Playback Page",
        document.revision().value(),
    )?;
    validate_revision(
        stored.as_ref(),
        expected_revision,
        "Playback Page",
        document.revision().value(),
    )?;
    if let Some(stored) = stored.as_ref() {
        stored.typed.validate().map_err(invalid)?;
    }
    Ok(stored)
}

fn validate_name(name: &str) -> Result<(), ActionError> {
    if !name.is_empty() && name.trim() == name && name.chars().count() <= 80 {
        return Ok(());
    }
    Err(invalid(
        "Playback Page name must be 1-80 characters without surrounding whitespace",
    ))
}
