use light_core::CueListId;
use serde::Serialize;
use uuid::Uuid;

use super::error::UpdateError;

/// Authoritative concrete playback/Cue context supplied by the playback engine. Keeping the
/// playback number prevents two active instances of one Cuelist from being collapsed together.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ActiveCueContext {
    pub playback_address: light_playback::PlaybackAddress,
    pub cue_list_id: CueListId,
    pub cue_id: Uuid,
    pub cue_number: light_playback::CueNumber,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResolvedCueTarget {
    pub cue_list_id: CueListId,
    pub playback_address: Option<light_playback::PlaybackAddress>,
    pub cue_id: Uuid,
    pub cue_number: light_playback::CueNumber,
}

impl From<&ActiveCueContext> for ResolvedCueTarget {
    fn from(context: &ActiveCueContext) -> Self {
        Self {
            cue_list_id: context.cue_list_id,
            playback_address: Some(context.playback_address),
            cue_id: context.cue_id,
            cue_number: context.cue_number.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CueTargetRequest {
    /// An explicit Cue is already resolved through the normal command/pool addressing path.
    Explicit(ResolvedCueTarget),
    /// A concrete playback must currently have one authoritative Cue.
    ActivePlayback {
        playback_address: light_playback::PlaybackAddress,
    },
    /// A pool Cuelist without an explicit Cue is valid only with one concrete active context.
    PoolCueList { cue_list_id: CueListId },
}

pub fn resolve_cue_target(
    request: &CueTargetRequest,
    active: &[ActiveCueContext],
) -> Result<ResolvedCueTarget, UpdateError> {
    match request {
        CueTargetRequest::Explicit(target) => Ok(target.clone()),
        CueTargetRequest::ActivePlayback { playback_address } => resolve_unique_context(
            active
                .iter()
                .filter(|context| context.playback_address == *playback_address),
            format!(
                "Playback {}.{}",
                playback_address.page(),
                playback_address.position()
            ),
        ),
        CueTargetRequest::PoolCueList { cue_list_id } => resolve_unique_context(
            active
                .iter()
                .filter(|context| context.cue_list_id == *cue_list_id),
            format!("Cuelist {}", cue_list_id.0),
        ),
    }
}

fn resolve_unique_context<'a>(
    contexts: impl Iterator<Item = &'a ActiveCueContext>,
    target: String,
) -> Result<ResolvedCueTarget, UpdateError> {
    let matches = contexts.collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(UpdateError::MissingCurrentCue { target }),
        [context] => Ok(ResolvedCueTarget::from(*context)),
        contexts => Err(UpdateError::AmbiguousPlaybackContext {
            target,
            contexts: contexts.len(),
        }),
    }
}
