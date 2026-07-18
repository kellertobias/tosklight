use std::fmt;

use serde::Serialize;

use super::model::{UpdateTargetFamily, UpdateTargetIdentity};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum UpdateError {
    EmptyProgrammer { target_family: UpdateTargetFamily },
    MissingTarget { target: String },
    MissingCurrentCue { target: String },
    AmbiguousPlaybackContext { target: String, contexts: usize },
    StaleRevision { expected: u64, current: u64 },
    NoOp { target: UpdateTargetIdentity },
    InvalidTarget { reason: String },
}

impl fmt::Display for UpdateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProgrammer { target_family } => write!(
                formatter,
                "the programmer has no content for {target_family:?} Update"
            ),
            Self::MissingTarget { target } => write!(formatter, "{target} does not exist"),
            Self::MissingCurrentCue { target } => write!(
                formatter,
                "{target} has no current Cue; identify an explicit Cue"
            ),
            Self::AmbiguousPlaybackContext { target, contexts } => write!(
                formatter,
                "{target} has {contexts} active playback/Cue contexts; identify a concrete playback or Cue"
            ),
            Self::StaleRevision { expected, current } => write!(
                formatter,
                "Update target is stale: expected revision {expected}, current revision is {current}"
            ),
            Self::NoOp { target } => write!(
                formatter,
                "Update would not change {} {}",
                target.name, target.object_id
            ),
            Self::InvalidTarget { reason } => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for UpdateError {}
