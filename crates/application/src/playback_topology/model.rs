use crate::{ActionContext, ActiveShowObjectKind, ApplicationCommand, CommandFamily};
use light_core::{CueListId, Revision, ShowId};
use light_playback::{CueList, PlaybackDefinition};
use light_show::PortableShowRevision;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

/// One typed portable Cuelist, Playback, or Page topology action.
#[derive(Clone, Debug)]
pub struct PlaybackTopologyCommand {
    pub show_id: ShowId,
    pub action: PlaybackTopologyAction,
}

impl ApplicationCommand for PlaybackTopologyCommand {
    type Value = PlaybackTopologyResult;

    const FAMILY: CommandFamily = CommandFamily::Show;
}

#[derive(Clone, Debug)]
pub enum PlaybackTopologyAction {
    SaveCueList {
        cue_list_id: CueListId,
        expected_revision: Revision,
        expected_object_id: Option<String>,
        cue_list: CueList,
        /// Exact request body retained so supplied extension fields survive a real typed change.
        raw_body: Arc<Value>,
    },
    ConfigureSlot {
        page: u8,
        slot: u8,
        expected_page_revision: Revision,
        expected_page_object_id: Option<String>,
        expected_playback_revision: Revision,
        expected_playback_object_id: Option<String>,
        playback: PlaybackDefinition,
    },
    MapExistingPlayback {
        page: u8,
        slot: u8,
        playback_number: u16,
        expected_page_revision: Revision,
        expected_page_object_id: Option<String>,
        expected_playback_revision: Revision,
        expected_playback_object_id: Option<String>,
    },
    ClearMappedPlayback {
        page: u8,
        slot: u8,
        expected_page_revision: Revision,
        expected_page_object_id: Option<String>,
        expected_playback_revision: Revision,
        expected_playback_object_id: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackTopologyResolution {
    CueList {
        cue_list_id: CueListId,
    },
    PageSlot {
        page: u8,
        slot: u8,
        playback_number: Option<u16>,
    },
}

/// Exact authoritative state of one object involved in the action.
#[derive(Clone, Debug, PartialEq)]
pub enum PlaybackTopologyObjectProjection {
    Present {
        kind: ActiveShowObjectKind,
        object_id: String,
        object_revision: Revision,
        raw_body: Arc<Value>,
    },
    Deleted {
        kind: ActiveShowObjectKind,
        object_id: String,
        object_revision: Revision,
    },
}

impl PlaybackTopologyObjectProjection {
    pub const fn kind(&self) -> ActiveShowObjectKind {
        match self {
            Self::Present { kind, .. } | Self::Deleted { kind, .. } => *kind,
        }
    }

    pub fn object_id(&self) -> &str {
        match self {
            Self::Present { object_id, .. } | Self::Deleted { object_id, .. } => object_id,
        }
    }

    pub const fn object_revision(&self) -> Revision {
        match self {
            Self::Present {
                object_revision, ..
            }
            | Self::Deleted {
                object_revision, ..
            } => *object_revision,
        }
    }

    pub fn raw_body(&self) -> Option<&Arc<Value>> {
        match self {
            Self::Present { raw_body, .. } => Some(raw_body),
            Self::Deleted { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlaybackTopologyOutcome {
    Changed {
        show_revision: PortableShowRevision,
        resolution: PlaybackTopologyResolution,
        objects: Arc<[PlaybackTopologyObjectProjection]>,
        event_sequence: u64,
    },
    NoChange {
        show_revision: PortableShowRevision,
        resolution: PlaybackTopologyResolution,
        objects: Arc<[PlaybackTopologyObjectProjection]>,
    },
}

impl PlaybackTopologyOutcome {
    pub const fn show_revision(&self) -> PortableShowRevision {
        match self {
            Self::Changed { show_revision, .. } | Self::NoChange { show_revision, .. } => {
                *show_revision
            }
        }
    }

    pub const fn resolution(&self) -> PlaybackTopologyResolution {
        match self {
            Self::Changed { resolution, .. } | Self::NoChange { resolution, .. } => *resolution,
        }
    }

    pub fn objects(&self) -> &[PlaybackTopologyObjectProjection] {
        match self {
            Self::Changed { objects, .. } | Self::NoChange { objects, .. } => objects,
        }
    }

    pub const fn event_sequence(&self) -> Option<u64> {
        match self {
            Self::Changed { event_sequence, .. } => Some(*event_sequence),
            Self::NoChange { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackTopologyResult {
    pub context: ActionContext,
    pub request_id: String,
    pub correlation_id: Uuid,
    pub replayed: bool,
    pub outcome: PlaybackTopologyOutcome,
}
