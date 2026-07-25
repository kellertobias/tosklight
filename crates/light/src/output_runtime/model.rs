use uuid::Uuid;

use crate::{ActionContext, ApplicationCommand, CommandFamily};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OutputRuntimeIdentity {
    GlobalMaster,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputLevel(u32);

impl OutputLevel {
    pub fn new(value: f32) -> Option<Self> {
        (value.is_finite() && (0.0..=1.0).contains(&value)).then(|| Self(value.to_bits()))
    }

    pub fn value(self) -> f32 {
        f32::from_bits(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputRuntimeCommand {
    pub expectation: OutputRuntimeExpectation,
    pub grand_master: Option<OutputLevel>,
    pub blackout: Option<bool>,
}

impl OutputRuntimeCommand {
    pub const fn new(grand_master: Option<OutputLevel>, blackout: Option<bool>) -> Self {
        Self {
            expectation: OutputRuntimeExpectation::Current,
            grand_master,
            blackout,
        }
    }

    pub const fn exact(
        show_id: Uuid,
        revision: u64,
        grand_master: Option<OutputLevel>,
        blackout: Option<bool>,
    ) -> Self {
        Self {
            expectation: OutputRuntimeExpectation::Exact { show_id, revision },
            grand_master,
            blackout,
        }
    }

    pub fn desired(self, mut current: OutputRuntimeProjection) -> OutputRuntimeProjection {
        if let Some(level) = self.grand_master {
            current.grand_master = level.value();
        }
        if let Some(blackout) = self.blackout {
            current.blackout = blackout;
        }
        current
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputRuntimeExpectation {
    Current,
    Exact { show_id: Uuid, revision: u64 },
}

impl ApplicationCommand for OutputRuntimeCommand {
    type Value = OutputRuntimeResult;

    const FAMILY: CommandFamily = CommandFamily::Output;
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct OutputRuntimeScope {
    pub show_id: Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputRuntimeProjection {
    pub scope: OutputRuntimeScope,
    pub identity: OutputRuntimeIdentity,
    pub revision: u64,
    pub grand_master: f32,
    pub blackout: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputRuntimeChange {
    pub projection: OutputRuntimeProjection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputRuntimeOutcome {
    Applied,
    NoChange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputRuntimeDurability {
    Durable,
    PersistencePending,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputRuntimeApplication {
    pub durability: OutputRuntimeDurability,
    pub warning: Option<String>,
}

impl OutputRuntimeApplication {
    pub const fn durable() -> Self {
        Self {
            durability: OutputRuntimeDurability::Durable,
            warning: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OutputRuntimeResult {
    pub context: ActionContext,
    pub outcome: OutputRuntimeOutcome,
    pub durability: OutputRuntimeDurability,
    pub warning: Option<String>,
    pub projection: OutputRuntimeProjection,
    pub event_sequence: Option<u64>,
    pub replayed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputRuntimeSnapshot {
    pub event_sequence: u64,
    pub projection: OutputRuntimeProjection,
}
