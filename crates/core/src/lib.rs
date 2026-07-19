#![forbid(unsafe_code)]
//! Shared, transport-neutral lighting domain primitives.

mod attributes;
mod clock;

pub use attributes::{
    ATTRIBUTE_REGISTRY, AttributeClass, AttributeDescriptor, AttributeKey, AttributeValue,
    AttributeValueType, MergeMode, TimedValue, Xyz, attribute_descriptor,
};
pub use clock::{ApplicationClock, EngineClock, ManualClock, SharedClock, SystemClock};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub type Revision = u64;
pub type Universe = u16;
pub type DmxAddress = u16;

macro_rules! id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

id!(UserId);
id!(SessionId);
id!(ShowId);
id!(FixtureId);
id!(CueListId);
id!(ProgrammerId);

#[derive(Debug, Error)]
pub enum LightError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("revision conflict: expected {expected}, current {current}")]
    RevisionConflict {
        expected: Revision,
        current: Revision,
    },
    #[error("not found: {0}")]
    NotFound(String),
}
