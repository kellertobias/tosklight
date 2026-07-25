//! Transport-neutral output seam for future bidirectional fixture adapters.

use async_trait::async_trait;
use light_core::{AttributeKey, AttributeValue, FixtureId};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, sync::Arc};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExternalAdapterId(pub String);

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExternalBindingId(pub String);

/// Desired semantic state produced by the desk for one external fixture binding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalDeviceIntent {
    pub adapter_id: ExternalAdapterId,
    pub binding_id: ExternalBindingId,
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
}

/// Immutable desired-state batch from one compiled show generation.
#[derive(Clone, Debug)]
pub struct ExternalIntentBatch {
    adapter_id: ExternalAdapterId,
    revision: u64,
    intents: Arc<[ExternalDeviceIntent]>,
}

impl ExternalIntentBatch {
    pub fn new(
        adapter_id: ExternalAdapterId,
        revision: u64,
        intents: impl IntoIterator<Item = ExternalDeviceIntent>,
    ) -> Result<Self, ExternalAdapterError> {
        let intents: Arc<[ExternalDeviceIntent]> =
            Arc::from(intents.into_iter().collect::<Vec<_>>());
        if intents.iter().any(|intent| intent.adapter_id != adapter_id) {
            return Err(ExternalAdapterError::new(
                "an external intent batch may target only one adapter",
            ));
        }
        Ok(Self {
            adapter_id,
            revision,
            intents,
        })
    }

    pub fn adapter_id(&self) -> &ExternalAdapterId {
        &self.adapter_id
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn intents(&self) -> &[ExternalDeviceIntent] {
        &self.intents
    }
}

/// Device-observed state. It is deliberately not merged back into desired desk state here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalDeviceObservation {
    pub binding_id: ExternalBindingId,
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
}

#[derive(Clone, Debug, Default)]
pub struct ExternalObservationBatch {
    source_revision: u64,
    observations: Arc<[ExternalDeviceObservation]>,
}

impl ExternalObservationBatch {
    pub fn new(
        source_revision: u64,
        observations: impl IntoIterator<Item = ExternalDeviceObservation>,
    ) -> Self {
        Self {
            source_revision,
            observations: Arc::from(observations.into_iter().collect::<Vec<_>>()),
        }
    }

    pub const fn source_revision(&self) -> u64 {
        self.source_revision
    }

    pub fn observations(&self) -> &[ExternalDeviceObservation] {
        &self.observations
    }
}

/// Port implemented by future REST, ATEM, mixer, or other external-device integrations.
///
/// Connection management, authentication, retry, health, and feedback remain adapter-owned.
/// Callers schedule this work outside the timing-critical DMX render and delivery path.
#[async_trait]
pub trait ExternalDeviceAdapter: Send + Sync {
    fn id(&self) -> &ExternalAdapterId;

    async fn apply(
        &self,
        batch: ExternalIntentBatch,
    ) -> Result<ExternalObservationBatch, ExternalAdapterError>;

    async fn shutdown(&self) -> Result<(), ExternalAdapterError> {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalAdapterError {
    message: String,
}

impl ExternalAdapterError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ExternalAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ExternalAdapterError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artdmx_packet;
    use std::sync::Mutex;

    struct FakeBidirectionalAdapter {
        id: ExternalAdapterId,
        desired: Mutex<Vec<ExternalDeviceIntent>>,
    }

    #[async_trait]
    impl ExternalDeviceAdapter for FakeBidirectionalAdapter {
        fn id(&self) -> &ExternalAdapterId {
            &self.id
        }

        async fn apply(
            &self,
            batch: ExternalIntentBatch,
        ) -> Result<ExternalObservationBatch, ExternalAdapterError> {
            if batch.adapter_id() != self.id() {
                return Err(ExternalAdapterError::new(
                    "batch was routed to the wrong external adapter",
                ));
            }
            self.desired
                .lock()
                .expect("fake desired-state mutex poisoned")
                .extend_from_slice(batch.intents());
            Ok(ExternalObservationBatch::new(
                batch.revision(),
                batch
                    .intents()
                    .iter()
                    .map(|intent| ExternalDeviceObservation {
                        binding_id: intent.binding_id.clone(),
                        fixture_id: intent.fixture_id,
                        attribute: intent.attribute.clone(),
                        value: AttributeValue::Normalized(0.4),
                    }),
            ))
        }
    }

    #[tokio::test]
    async fn bidirectional_adapter_keeps_desired_observed_and_dmx_paths_separate() {
        let adapter = FakeBidirectionalAdapter {
            id: ExternalAdapterId("mock-rest".into()),
            desired: Mutex::new(Vec::new()),
        };
        let fixture_id = FixtureId::new();
        let intent = ExternalDeviceIntent {
            adapter_id: adapter.id().clone(),
            binding_id: ExternalBindingId("device-7".into()),
            fixture_id,
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(0.8),
        };
        let mut frame = [0_u8; crate::DMX_SLOTS];
        frame[0] = 127;
        let dmx_before = artdmx_packet(1, 9, &frame);

        let observed = adapter
            .apply(ExternalIntentBatch::new(adapter.id().clone(), 42, [intent.clone()]).unwrap())
            .await
            .unwrap();

        assert_eq!(observed.source_revision(), 42);
        assert_eq!(
            observed.observations()[0].value,
            AttributeValue::Normalized(0.4)
        );
        assert_eq!(
            adapter.desired.lock().unwrap().as_slice(),
            std::slice::from_ref(&intent)
        );
        assert_eq!(artdmx_packet(1, 9, &frame), dmx_before);
    }

    #[test]
    fn intent_batch_rejects_mixed_adapter_targets() {
        let fixture_id = FixtureId::new();
        let intent = |adapter: &str| ExternalDeviceIntent {
            adapter_id: ExternalAdapterId(adapter.into()),
            binding_id: ExternalBindingId("device-7".into()),
            fixture_id,
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(0.8),
        };

        let error = ExternalIntentBatch::new(
            ExternalAdapterId("rest".into()),
            1,
            [intent("rest"), intent("atem")],
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "an external intent batch may target only one adapter"
        );
    }
}
