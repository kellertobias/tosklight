use crate::{
    ActionError, ActionErrorKind, ProgrammingCueCommit, ProgrammingCueRecordTarget,
    ProgrammingCueResolvedTarget,
};
use light_core::CueListId;
use light_playback::{CueList, PlaybackAddress, PlaybackDefinition, PlaybackPage, PlaybackTarget};
use light_show::{PortableShowDocument, PortableShowObject};

#[derive(Clone)]
pub(super) struct Stored<T> {
    pub object_id: String,
    pub object_revision: u64,
    pub raw_body: serde_json::Value,
    pub typed: T,
}

pub(super) struct ResolvedCueTarget {
    pub cue_list: Option<Stored<CueList>>,
    pub pool_number: Option<u16>,
    pub page: Option<Stored<PlaybackPage>>,
    pub concrete_playback_address: Option<PlaybackAddress>,
}
impl ResolvedCueTarget {
    pub fn creates_topology(&self) -> bool {
        self.cue_list.is_none()
    }
}
pub(super) fn resolve_target(
    document: &PortableShowDocument,
    commit: &ProgrammingCueCommit,
) -> Result<ResolvedCueTarget, ActionError> {
    match commit.environment().target {
        ProgrammingCueResolvedTarget::CuelistPool { number } => {
            let catalog = crate::cuelist_pool_catalog(document)?;
            let cue_list = catalog
                .resolve(number)
                .map(|id| required_cue_list(document, id))
                .transpose()?;
            Ok(ResolvedCueTarget {
                cue_list,
                pool_number: Some(number),
                page: None,
                concrete_playback_address: None,
            })
        }
        ProgrammingCueResolvedTarget::CueList { cue_list_id } => Ok(ResolvedCueTarget {
            cue_list: Some(required_cue_list(document, cue_list_id)?),
            pool_number: None,
            page: None,
            concrete_playback_address: None,
        }),
        ProgrammingCueResolvedTarget::Playback { address } => {
            let page = find_page(document, address.page())?;
            let definition = page
                .as_ref()
                .and_then(|page| page.typed.assignments.get(&address.position()));
            if definition.is_none()
                && matches!(
                    commit.request().target,
                    ProgrammingCueRecordTarget::SelectedPlayback
                )
            {
                return Err(not_found(format!(
                    "Playback {}.{} is not recorded",
                    address.page(),
                    address.position()
                )));
            }
            let cue_list = definition
                .map(|definition| cue_list_for_playback(document, definition, address))
                .transpose()?;
            Ok(ResolvedCueTarget {
                cue_list,
                pool_number: None,
                page,
                concrete_playback_address: Some(address),
            })
        }
    }
}

fn required_cue_list(
    document: &PortableShowDocument,
    cue_list_id: CueListId,
) -> Result<Stored<CueList>, ActionError> {
    find_cue_list(document, cue_list_id)?
        .ok_or_else(|| not_found(format!("Cuelist {} does not exist", cue_list_id.0)))
}

fn cue_list_for_playback(
    document: &PortableShowDocument,
    playback: &PlaybackDefinition,
    address: PlaybackAddress,
) -> Result<Stored<CueList>, ActionError> {
    let PlaybackTarget::CueList { cue_list_id } = playback.target else {
        return Err(invalid(format!(
            "Playback {}.{} is assigned to another target; its assignment is preserved",
            address.page(),
            address.position()
        )));
    };
    required_cue_list(document, cue_list_id)
}

fn find_cue_list(
    document: &PortableShowDocument,
    id: CueListId,
) -> Result<Option<Stored<CueList>>, ActionError> {
    find_unique(document, "cue_list", |object| {
        decode_cue_list(object).map(|value| (value.id == id).then_some(value))
    })
}

fn find_page(
    document: &PortableShowDocument,
    number: u8,
) -> Result<Option<Stored<PlaybackPage>>, ActionError> {
    find_unique(document, "playback_page", |object| {
        decode_page(object).map(|value| (value.number == number).then_some(value))
    })
}

fn find_unique<T>(
    document: &PortableShowDocument,
    kind: &str,
    mut decode_match: impl FnMut(&PortableShowObject) -> Result<Option<T>, ActionError>,
) -> Result<Option<Stored<T>>, ActionError> {
    let mut found = None;
    for object in document.objects_of_kind(kind) {
        let Some(typed) = decode_match(object)? else {
            continue;
        };
        if found.is_some() {
            return Err(invalid(format!(
                "multiple {kind} objects share one identity"
            )));
        }
        found = Some(stored(object, typed));
    }
    Ok(found)
}

fn stored<T>(object: &PortableShowObject, typed: T) -> Stored<T> {
    Stored {
        object_id: object.key().id().to_owned(),
        object_revision: object.revision(),
        raw_body: object.body().clone(),
        typed,
    }
}

fn decode_cue_list(object: &PortableShowObject) -> Result<CueList, ActionError> {
    serde_json::from_value(object.body().clone()).map_err(invalid)
}

fn decode_page(object: &PortableShowObject) -> Result<PlaybackPage, ActionError> {
    serde_json::from_value(object.body().clone()).map_err(invalid)
}

fn invalid(message: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message.to_string())
}

fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}
