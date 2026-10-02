use light_core::programming::IntentError;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

/// Opaque identity of one captured Dynamic source assignment. It is not a controller,
/// lane, Resume boundary, arbitration rank, or a fixture/native channel identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DynamicSourceOccurrenceId(Uuid);

impl DynamicSourceOccurrenceId {
    pub fn new(id: Uuid) -> Result<Self, IntentError> {
        if id.is_nil() {
            return Err(IntentError(
                "Dynamic source occurrence must not be nil".into(),
            ));
        }
        Ok(Self(id))
    }

    pub fn get(self) -> Uuid {
        self.0
    }
}

impl<'de> Deserialize<'de> for DynamicSourceOccurrenceId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(Uuid::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
