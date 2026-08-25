#![forbid(unsafe_code)]
//! Shared, transport-neutral lighting domain primitives.

mod attributes;
mod clock;
mod surface;

pub use attributes::{
    ATTRIBUTE_CONFIGURATION_VERSION, ATTRIBUTE_REGISTRY, AttributeActivationGroup, AttributeBounds,
    AttributeClass, AttributeConfiguration, AttributeConfigurationError, AttributeDescriptor,
    AttributeEntry, AttributeId, AttributeKey, AttributePlacement, AttributeTable, AttributeValue,
    AttributeValueType, CanonicalAttributeTransform, CustomAttributeDescriptor,
    CustomAttributeLifecycle, ENCODER_SLOTS_PER_PAGE, EncoderGroup, EncoderPlacement, MergeMode,
    PROJECTION_ONLY_BUILT_IN_ATTRIBUTES, PickerColor, RETIRED_BUILT_IN_ATTRIBUTES,
    ResolvedAttributeDescriptor, SPECIAL_DIALOG_ONLY_BUILT_IN_ATTRIBUTES, TimedValue, Xyz,
    attribute_descriptor, built_in_attribute_is_projection_only, built_in_attribute_is_retired,
    built_in_attribute_is_special_dialog_only, canonical_attribute_migration,
    canonical_attribute_migration_id, color_range_color, hsv_to_rgb, spread_position,
    transform_canonical_normalized, transform_canonical_value,
};
pub use clock::{ApplicationClock, EngineClock, ManualClock, SharedClock, SystemClock};
pub use surface::SurfaceCapability;

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
