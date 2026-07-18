use light_core::CueListId;
use serde::Serialize;
use uuid::Uuid;

use super::error::UpdateError;

/// Authoritative concrete playback/Cue context supplied by the playback engine. Keeping the
/// playback number prevents two active instances of one Cuelist from being collapsed together.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ActiveCueContext {
    pub playback_number: u16,
    pub cue_list_id: CueListId,
    pub cue_id: Uuid,
    pub cue_number: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResolvedCueTarget {
    pub cue_list_id: CueListId,
    pub playback_number: Option<u16>,
    pub cue_id: Uuid,
    pub cue_number: f64,
}

impl From<&ActiveCueContext> for ResolvedCueTarget {
    fn from(context: &ActiveCueContext) -> Self {
        Self {
            cue_list_id: context.cue_list_id,
            playback_number: Some(context.playback_number),
            cue_id: context.cue_id,
            cue_number: context.cue_number,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CueTargetRequest {
    /// An explicit Cue is already resolved through the normal command/pool addressing path.
    Explicit(ResolvedCueTarget),
    /// A concrete playback must currently have one authoritative Cue.
    ActivePlayback { playback_number: u16 },
    /// A pool Cuelist without an explicit Cue is valid only with one concrete active context.
    PoolCueList { cue_list_id: CueListId },
}

pub fn resolve_cue_target(
    request: &CueTargetRequest,
    active: &[ActiveCueContext],
) -> Result<ResolvedCueTarget, UpdateError> {
    match request {
        CueTargetRequest::Explicit(target) => Ok(target.clone()),
        CueTargetRequest::ActivePlayback { playback_number } => resolve_unique_context(
            active
                .iter()
                .filter(|context| context.playback_number == *playback_number),
            format!("playback {playback_number}"),
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
