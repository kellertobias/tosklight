use super::{
    CompiledDynamicValueAddress, DynamicPresetTemplate, DynamicValue, DynamicValueAddress,
    address::ensure,
};
use light_core::{FixtureId, programming::IntentError};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicValueFallback {
    pub target: FixtureId,
    pub value: DynamicValue,
}

/// One cold-compiled source occurrence. Instance clones retain its identity; a
/// changed source gets a new generation independently of the authored pool revision.
#[derive(Clone, Debug)]
pub struct DynamicPresetSourceBinding {
    pub id: uuid::Uuid,
    pub preset_id: String,
    pub address: DynamicValueAddress,
    pub retained: Option<std::sync::Arc<DynamicPresetTemplate>>,
    pub occurrence: Option<DynamicPresetSourceOccurrence>,
}

/// Stable within the retained definition. The opaque binding UUID still identifies a
/// particular compilation; this address carries last-valid values across save and reload.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicPresetSourceOccurrence {
    pub lane_id: uuid::Uuid,
    pub slot: DynamicPresetSourceSlot,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DynamicPresetSourceSlot {
    Keyframe { index: u32, position_bits: u32 },
    Minimum,
    Maximum,
    Middle,
    RandomLow,
    RandomHigh,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicPresetSourceValues {
    pub occurrence: DynamicPresetSourceOccurrence,
    pub preset_id: String,
    pub address: DynamicValueAddress,
    pub values: Vec<DynamicValueFallback>,
}

impl DynamicPresetSourceValues {
    pub fn matches(&self, source: &DynamicPresetSourceBinding) -> bool {
        source.occurrence == Some(self.occurrence)
            && self.preset_id == source.preset_id
            && self.address == source.address
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DynamicValueSource {
    Current,
    Value {
        value: DynamicValue,
    },
    Preset {
        preset_id: String,
        address: DynamicValueAddress,
        #[serde(default)]
        last_valid_by_target: Vec<DynamicValueFallback>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retained: Option<std::sync::Arc<DynamicPresetTemplate>>,
    },
}

/// Implemented by one immutable pre-Dynamic frame and its compiled live Preset dependencies.
/// Missing data is None; it is never normalized zero or the previous modulated frame.
/// Current is prepared in the lane's representation, including explicit adoption:
/// an Angle takeover over a Target uses its solved pose at this frame's timestamp.
/// Incompatible data that still needs adoption must not be reported as merely absent.
pub trait DynamicValueSourceResolver {
    /// Opt in with an authoritative original Position Current family for deferred destination
    /// adoption. The default must not call scalar Current or the Size baseline: legacy readers
    /// may perform adoption, carry a different owner, or be counted once per address.
    fn try_position_current_family(
        &self,
        _target: FixtureId,
        _address: &DynamicValueAddress,
    ) -> Result<Option<light_core::AttributeValue>, light_core::programming::TransitionError> {
        Ok(None)
    }

    fn current(&self, target: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue>;
    /// A captured typed frame distinguishes missing data from required representation
    /// adoption. Legacy implementations keep their existing optional-value behavior.
    fn try_current(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<DynamicValue>, light_core::programming::TransitionError> {
        Ok(self.current(target, address))
    }
    /// Exact captured assignment for a fresh authored lane. Historical held leaves keep
    /// their own occurrence rather than consulting this binding after an edit.
    fn authored_occurrence(
        &self,
        _instance_id: uuid::Uuid,
        _controller_id: uuid::Uuid,
        _target: FixtureId,
        _lane_id: uuid::Uuid,
    ) -> Option<super::DynamicSourceOccurrenceId> {
        None
    }
    /// Exact static source dependency for this captured pre-Dynamic family.
    fn current_family_occurrence(
        &self,
        _target: FixtureId,
        _address: &DynamicValueAddress,
    ) -> Option<super::DynamicSourceOccurrenceId> {
        None
    }
    /// Evidence for a used, compatible or adopted Current read. A default resolver has no
    /// transfer proof; preserve that uncertainty even if the source identity is available.
    /// Whole-family Size uses current_family_occurrence for its unchanged original baseline.
    fn current_dependency(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> super::DynamicSourceDependency {
        super::DynamicSourceDependency::unknown(self.current_family_occurrence(target, address))
    }
    /// Whole-family Size uses the actual pre-Dynamic owner, which can have a different
    /// representation. Keep this separate from a compatible/adopted lane Current.
    fn current_family_base(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Option<light_core::AttributeValue> {
        match self.current(target, address) {
            Some(DynamicValue::Family(value)) => Some(value),
            _ => None,
        }
    }
    /// Fallible access to the original whole-family Size baseline, before adoption.
    fn try_current_family_base(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<light_core::AttributeValue>, light_core::programming::TransitionError> {
        Ok(self.current_family_base(target, address))
    }
    fn preset(
        &self,
        source: &DynamicPresetSourceBinding,
        instance_id: uuid::Uuid,
        target: FixtureId,
    ) -> Option<DynamicValue>;
}

pub(crate) struct UnavailableProgrammingSources;

/// Bridge existing arithmetic callbacks to fallible captured Current without using a
/// missing-value fallback as a successful answer. Check `take_error` before accepting output.
pub(crate) struct CheckedCurrentSources<'a> {
    pub(crate) sources: &'a dyn DynamicValueSourceResolver,
    error: std::cell::RefCell<Option<light_core::programming::TransitionError>>,
}

impl<'a> CheckedCurrentSources<'a> {
    pub(crate) fn new(sources: &'a dyn DynamicValueSourceResolver) -> Self {
        Self {
            sources,
            error: Default::default(),
        }
    }
    pub(crate) fn take_error(&self) -> Option<light_core::programming::TransitionError> {
        self.error.borrow_mut().take()
    }
    fn record<T>(
        &self,
        result: Result<Option<T>, light_core::programming::TransitionError>,
    ) -> Option<T> {
        match result {
            Ok(value) => value,
            Err(error) => {
                let mut previous = self.error.borrow_mut();
                if previous.is_none()
                    || matches!(error, light_core::programming::TransitionError::Invalid(_))
                {
                    *previous = Some(error);
                }
                None
            }
        }
    }
}

impl DynamicValueSourceResolver for CheckedCurrentSources<'_> {
    fn try_position_current_family(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<light_core::AttributeValue>, light_core::programming::TransitionError> {
        self.sources.try_position_current_family(target, address)
    }
    fn current(&self, target: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.record(self.sources.try_current(target, address).and_then(|value| {
            if let Some(value) = &value {
                address.validate_value_shape(value)?;
            }
            Ok(value)
        }))
    }
    fn current_family_base(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Option<light_core::AttributeValue> {
        self.record(self.sources.try_current_family_base(target, address))
    }
    fn current_dependency(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> super::DynamicSourceDependency {
        self.sources.current_dependency(target, address)
    }
    fn current_family_occurrence(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Option<super::DynamicSourceOccurrenceId> {
        self.sources.current_family_occurrence(target, address)
    }
    fn authored_occurrence(
        &self,
        instance: uuid::Uuid,
        controller: uuid::Uuid,
        target: FixtureId,
        lane: uuid::Uuid,
    ) -> Option<super::DynamicSourceOccurrenceId> {
        self.sources
            .authored_occurrence(instance, controller, target, lane)
    }
    fn preset(
        &self,
        source: &DynamicPresetSourceBinding,
        instance: uuid::Uuid,
        target: FixtureId,
    ) -> Option<DynamicValue> {
        self.record((|| {
            let value = self.sources.preset(source, instance, target);
            if let Some(value) = &value {
                source.address.validate_value_shape(value)?;
            }
            Ok(value)
        })())
    }
}

impl DynamicValueSourceResolver for UnavailableProgrammingSources {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        None
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: uuid::Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
}

impl DynamicValueSource {
    pub fn validate_shape(&self, lane: &DynamicValueAddress) -> Result<(), IntentError> {
        match self {
            Self::Current => lane.validate(),
            Self::Value { value } => lane.validate_value_shape(value),
            Self::Preset {
                preset_id,
                address,
                last_valid_by_target,
                retained,
            } => {
                ensure(
                    !preset_id.trim().is_empty()
                        && preset_id.len() <= 256
                        && !preset_id.chars().any(char::is_control),
                    "Dynamic Preset source needs a valid identity",
                )?;
                ensure(
                    address == lane,
                    "Dynamic Preset source and lane use different component frames or native identities",
                )?;
                lane.validate()?;
                if let Some(template) = retained {
                    template.validate(lane.owner())?;
                }
                let mut targets = HashSet::new();
                for fallback in last_valid_by_target {
                    ensure(
                        !fallback.target.0.is_nil() && targets.insert(fallback.target),
                        "Dynamic fallback targets must be unique stable UUIDs",
                    )?;
                    lane.validate_value_shape(&fallback.value)?;
                }
                Ok(())
            }
        }
    }

    pub fn validate(&self, lane: &CompiledDynamicValueAddress) -> Result<(), IntentError> {
        match self {
            Self::Current => Ok(()),
            Self::Value { value } => lane.validate_source_value(value),
            Self::Preset {
                preset_id,
                address,
                last_valid_by_target,
                retained,
            } => {
                ensure(
                    !preset_id.trim().is_empty()
                        && preset_id.len() <= 256
                        && !preset_id.chars().any(char::is_control),
                    "Dynamic Preset source needs a valid identity",
                )?;
                ensure(
                    address == lane.address(),
                    "Dynamic Preset source and lane use different component frames or native identities",
                )?;
                let mut targets = HashSet::new();
                if let Some(template) = retained {
                    template.validate(lane.address().owner())?;
                }
                for fallback in last_valid_by_target {
                    ensure(
                        !fallback.target.0.is_nil() && targets.insert(fallback.target),
                        "Dynamic fallback targets must be unique stable UUIDs",
                    )?;
                    lane.validate_source_value(&fallback.value)?;
                }
                Ok(())
            }
        }
    }

    /// Validation/compilation runs when a source changes. The hot sampler resolves the already
    /// compiled source without rescanning fallbacks or revalidating a complete native recipe.
    pub fn compile(
        &self,
        lane: &CompiledDynamicValueAddress,
    ) -> Result<CompiledDynamicValueSource, IntentError> {
        self.validate(lane)?;
        let source = match self {
            Self::Current => CompiledValueSource::Current,
            Self::Value { value } => CompiledValueSource::Value(value.clone()),
            Self::Preset {
                preset_id,
                last_valid_by_target,
                retained,
                ..
            } => CompiledValueSource::Preset {
                binding: std::sync::Arc::new(DynamicPresetSourceBinding {
                    id: uuid::Uuid::new_v4(),
                    preset_id: preset_id.clone(),
                    address: lane.address().clone(),
                    retained: retained.clone(),
                    occurrence: None,
                }),
                fallbacks: last_valid_by_target
                    .iter()
                    .map(|value| (value.target, value.value.clone()))
                    .collect(),
            },
        };
        Ok(CompiledDynamicValueSource {
            address: lane.address().clone(),
            source,
        })
    }
}

#[derive(Clone)]
pub struct CompiledDynamicValueSource {
    address: DynamicValueAddress,
    source: CompiledValueSource,
}

#[derive(Clone)]
enum CompiledValueSource {
    Current,
    Value(DynamicValue),
    Preset {
        binding: std::sync::Arc<DynamicPresetSourceBinding>,
        fallbacks: std::collections::HashMap<FixtureId, DynamicValue>,
    },
}

impl CompiledDynamicValueSource {
    pub(crate) fn is_current(&self) -> bool {
        matches!(&self.source, CompiledValueSource::Current)
    }
    pub(super) fn with_occurrence(mut self, occurrence: DynamicPresetSourceOccurrence) -> Self {
        if let CompiledValueSource::Preset { binding, .. } = &mut self.source {
            std::sync::Arc::make_mut(binding).occurrence = Some(occurrence);
        }
        self
    }
    pub fn preset_binding(&self) -> Option<&std::sync::Arc<DynamicPresetSourceBinding>> {
        match &self.source {
            CompiledValueSource::Preset { binding, .. } => Some(binding),
            _ => None,
        }
    }
    /// Resolver output is a validated member of the compiled address's domain. The frame and
    /// dependency compiler establish that contract before publication, outside the tick loop.
    pub fn resolve(
        &self,
        instance_id: uuid::Uuid,
        target: FixtureId,
        sources: &dyn DynamicValueSourceResolver,
    ) -> Option<DynamicValue> {
        match &self.source {
            CompiledValueSource::Current => sources.current(target, &self.address),
            CompiledValueSource::Value(value) => Some(value.clone()),
            CompiledValueSource::Preset { binding, fallbacks } => sources
                .preset(binding, instance_id, target)
                .or_else(|| fallbacks.get(&target).cloned()),
        }
    }
}
