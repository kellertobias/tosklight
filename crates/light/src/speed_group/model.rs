use uuid::Uuid;

use crate::{ActionContext, ApplicationCommand, CommandFamily};

pub const SPEED_GROUP_COUNT: usize = 5;
pub const MIN_SPEED_BPM: f64 = 0.1;
pub const MAX_SPEED_BPM: f64 = 999.0;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SpeedGroupId(u8);

impl SpeedGroupId {
    pub const fn new(one_based: u8) -> Option<Self> {
        if one_based >= 1 && one_based <= SPEED_GROUP_COUNT as u8 {
            Some(Self(one_based))
        } else {
            None
        }
    }

    pub const fn one_based(self) -> u8 {
        self.0
    }

    pub const fn index(self) -> usize {
        (self.0 - 1) as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpeedBpm(u64);

impl SpeedBpm {
    pub fn new(value: f64) -> Option<Self> {
        (value.is_finite() && (MIN_SPEED_BPM..=MAX_SPEED_BPM).contains(&value))
            .then(|| Self(value.to_bits()))
    }

    pub fn value(self) -> f64 {
        f64::from_bits(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpeedBpmDelta(u64);

impl SpeedBpmDelta {
    pub fn new(value: f64) -> Option<Self> {
        (value.is_finite() && value != 0.0).then(|| Self(value.to_bits()))
    }

    pub fn value(self) -> f64 {
        f64::from_bits(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeedGroupAction {
    SetBpm {
        group: SpeedGroupId,
        bpm: SpeedBpm,
    },
    AdjustBpm {
        group: SpeedGroupId,
        delta: SpeedBpmDelta,
    },
    Synchronize {
        source: SpeedGroupId,
        target: SpeedGroupId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpeedGroupCommand {
    pub expectation: SpeedGroupExpectation,
    pub action: SpeedGroupAction,
}

impl SpeedGroupCommand {
    pub const fn current(action: SpeedGroupAction) -> Self {
        Self {
            expectation: SpeedGroupExpectation::Current,
            action,
        }
    }

    pub const fn exact(authority_id: Uuid, revision: u64, action: SpeedGroupAction) -> Self {
        Self {
            expectation: SpeedGroupExpectation::Exact {
                authority_id,
                revision,
            },
            action,
        }
    }
}

impl ApplicationCommand for SpeedGroupCommand {
    type Value = SpeedGroupResult;

    const FAMILY: CommandFamily = CommandFamily::Playback;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeedGroupExpectation {
    Current,
    Exact { authority_id: Uuid, revision: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeedGroupProjection {
    pub group: SpeedGroupId,
    pub manual_bpm: f64,
    pub paused: bool,
    pub speed_master_scale: f64,
    pub synchronized_with: Option<SpeedGroupId>,
    pub phase_origin_millis: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpeedGroupAuthorityProjection {
    pub authority_id: Uuid,
    pub revision: u64,
    pub groups: Vec<SpeedGroupProjection>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpeedGroupPortState {
    pub groups: Vec<SpeedGroupProjection>,
    /// Groups whose direct manual ownership is already clean: Sound, Learn, and capture ownership
    /// have all been released. This adapter-only state is never serialized.
    pub manual_control_clean: Vec<SpeedGroupId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpeedGroupResolvedAction {
    SetManualBpm {
        group: SpeedGroupId,
        bpm: f64,
        applied_at_millis: u64,
    },
    Synchronize {
        source: SpeedGroupId,
        target: SpeedGroupId,
        applied_at_millis: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeedGroupOutcome {
    Applied,
    NoChange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeedGroupDurability {
    Durable,
    PersistencePending,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeedGroupApplication {
    pub durability: SpeedGroupDurability,
    pub warning: Option<String>,
}

impl SpeedGroupApplication {
    pub const fn durable() -> Self {
        Self {
            durability: SpeedGroupDurability::Durable,
            warning: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpeedGroupChange {
    pub authority_id: Uuid,
    pub revision: u64,
    pub applied_at_millis: u64,
    pub groups: Vec<SpeedGroupProjection>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpeedGroupResult {
    pub context: ActionContext,
    pub authority_id: Uuid,
    pub revision: u64,
    pub applied_at_millis: u64,
    pub outcome: SpeedGroupOutcome,
    pub durability: SpeedGroupDurability,
    pub warning: Option<String>,
    pub groups: Vec<SpeedGroupProjection>,
    pub event_sequence: Option<u64>,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpeedGroupSnapshot {
    pub event_sequence: u64,
    pub projection: SpeedGroupAuthorityProjection,
}
