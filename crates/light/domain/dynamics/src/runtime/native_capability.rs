//! Missing original sources are a passive capability state. Exact verified originals are
//! pinned independently of the current show catalogue and never replaced by a destination.
use super::*;
use crate::programming::expression::{ExpressionNode, ExpressionNodeRef};
use crate::{CompiledDynamicValueAddress, DynamicFamilyRepresentation, DynamicValueAddress};
use light_core::{
    NativeColorIdentity,
    programming::{IntentError, NativeColorEditModel},
};
use std::sync::Mutex;

type Model = Arc<dyn NativeColorEditModel + Send + Sync>;
type SampleKey = (Uuid, FixtureId, Uuid);
pub(super) type UnavailableSamples = HashMap<SampleKey, Vec<NativeColorModelUnavailable>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicNativeSourceStatus {
    pub instance_id: Uuid,
    pub lane_id: Uuid,
    /// A retained contribution can outlive its edited or deleted lane.
    pub retained_sample: Option<(Uuid, FixtureId)>,
    pub unavailable: NativeColorModelUnavailable,
}

#[derive(Default)]
struct Pins {
    models: Vec<Model>,
    view: Option<Arc<CapturedModels>>,
}

#[derive(Clone, Default)]
pub(super) struct NativeModelPins(Arc<Mutex<Pins>>);

/// Verified immutable models only: no provider or mutable captured-view cache crosses a
/// definition preparation boundary.
pub(super) struct PreparedNativePins(Vec<Model>);

struct CapturedModels {
    models: Vec<Model>,
    provider: Option<Arc<dyn DynamicNativeModelResolver>>,
}

fn missing(source: &NativeColorIdentity) -> NativeColorModelCapability {
    NativeColorModelCapability::Unavailable(NativeColorModelUnavailable {
        source: source.clone(),
        reason: NativeColorUnavailableReason::MissingResolver,
        detail: "Original native Color model is not available".into(),
    })
}

impl DynamicNativeModelResolver for CapturedModels {
    fn resolve(&self, source: &NativeColorIdentity) -> Result<Model, IntentError> {
        self.resolve_capability(source)?.require_available()
    }
    fn resolve_capability(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<NativeColorModelCapability, IntentError> {
        source.validate()?;
        if let Some(model) = self.models.iter().find(|model| model.source() == source) {
            return Ok(NativeColorModelCapability::Available(Arc::clone(model)));
        }
        self.provider.as_ref().map_or_else(
            || Ok(missing(source)),
            |provider| provider.resolve_capability(source),
        )
    }
}

impl NativeModelPins {
    pub(super) fn detached(&self) -> Self {
        Self(Arc::new(Mutex::new(Pins {
            models: self.0.lock().expect("native model pins").models.clone(),
            view: None,
        })))
    }

    pub(super) fn prepare_verified(&self) -> PreparedNativePins {
        PreparedNativePins(self.0.lock().expect("native model pins").models.clone())
    }

    pub(super) fn merge_verified(&self, prepared: PreparedNativePins) {
        let mut pins = self.0.lock().expect("native model pins");
        for model in prepared.0 {
            if !pins
                .models
                .iter()
                .any(|live| live.source() == model.source())
            {
                pins.models.push(model);
                pins.view = None;
            }
        }
    }
}

impl DynamicRuntime {
    pub(super) fn native_capability(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<NativeColorModelCapability, IntentError> {
        source.validate()?;
        let mut pins = self.native_model_pins.0.lock().expect("native model pins");
        if let Some(model) = pins.models.iter().find(|model| model.source() == source) {
            return Ok(NativeColorModelCapability::Available(Arc::clone(model)));
        }
        let capability = self.native_models.as_ref().map_or_else(
            || Ok(missing(source)),
            |provider| provider.resolve_capability(source),
        )?;
        match &capability {
            NativeColorModelCapability::Available(model) => {
                if model.source() != source {
                    return Err(IntentError(
                        "native model resolver returned a different original source".into(),
                    ));
                }
                pins.models.push(Arc::clone(model));
                pins.view = None;
            }
            NativeColorModelCapability::Unavailable(reason) if &reason.source != source => {
                return Err(IntentError(
                    "native capability refers to a different original source".into(),
                ));
            }
            _ => {}
        }
        Ok(capability)
    }

    /// Immutable resolver for one captured output generation. Warm reads reuse its Arc.
    /// New verified pins produce another view; an already captured view never changes.
    pub fn captured_native_color_models(&self) -> Arc<dyn DynamicNativeModelResolver> {
        let mut pins = self.native_model_pins.0.lock().expect("native model pins");
        let same_provider =
            pins.view
                .as_ref()
                .is_some_and(|view| match (&view.provider, &self.native_models) {
                    (None, None) => true,
                    (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                    _ => false,
                });
        if !same_provider {
            pins.view = Some(Arc::new(CapturedModels {
                models: pins.models.clone(),
                provider: self.native_models.clone(),
            }));
        }
        pins.view.as_ref().expect("captured resolver").clone()
    }

    pub(super) fn compile_native_address(
        &self,
        address: &DynamicValueAddress,
    ) -> Result<CompiledDynamicValueAddress, IntentError> {
        if let DynamicFamilyRepresentation::DirectColor { source } = &address.representation {
            match self.native_capability(source)? {
                NativeColorModelCapability::Available(model) => {
                    CompiledDynamicValueAddress::new(address.clone(), Some(model))
                }
                NativeColorModelCapability::Unavailable(_) => {
                    CompiledDynamicValueAddress::suspended_native(address.clone())
                }
            }
        } else {
            CompiledDynamicValueAddress::new(address.clone(), None)
        }
    }

    pub(super) fn unavailable_retained_samples<'a>(
        &self,
        samples: impl Iterator<Item = (&'a SampleKey, &'a DynamicSampleExpression)>,
    ) -> Result<UnavailableSamples, DynamicRuntimeError> {
        let mut unavailable = HashMap::<SampleKey, Vec<NativeColorModelUnavailable>>::new();
        for (key, expression) in samples {
            // The held row follows the last-output row and is authoritative for that key.
            unavailable.remove(key);
            let active = || -> Result<Vec<NativeColorModelUnavailable>, IntentError> {
                let mut reasons = Vec::new();
                for node in ExpressionNodeRef::new(expression).postorder(true)? {
                    let address = match node.node()? {
                        ExpressionNode::Programming(address, ..) => address.clone(),
                        ExpressionNode::Scale {
                            address,
                            base: crate::DynamicValue::Family(base),
                            factor,
                            ..
                        } if factor != 1.0 => {
                            DynamicValueAddress::whole_family(address.owner(), base)?
                        }
                        _ => continue,
                    };
                    if let DynamicFamilyRepresentation::DirectColor { source } =
                        &address.representation
                        && let NativeColorModelCapability::Unavailable(reason) =
                            self.native_capability(source)?
                        && !reasons.contains(&reason)
                    {
                        reasons.push(reason);
                    }
                }
                Ok(reasons)
            };
            let reasons = active()
                .map_err(|error| DynamicRuntimeError::InvalidSnapshot(error.to_string()))?;
            if !reasons.is_empty() {
                unavailable.insert(*key, reasons);
            }
        }
        Ok(unavailable)
    }

    pub fn unavailable_native_sources(&self) -> Vec<DynamicNativeSourceStatus> {
        let mut result = Vec::new();
        for instance in self.instances.values() {
            for (lane_id, lane) in &instance.programming_lanes {
                if let Some(unavailable) = lane.unavailable_native_source() {
                    result.push(DynamicNativeSourceStatus {
                        instance_id: instance.id,
                        lane_id: *lane_id,
                        retained_sample: None,
                        unavailable: unavailable.clone(),
                    });
                }
            }
            for ((controller, target, lane_id), reasons) in &instance.unavailable_samples {
                for unavailable in reasons {
                    result.push(DynamicNativeSourceStatus {
                        instance_id: instance.id,
                        lane_id: *lane_id,
                        retained_sample: Some((*controller, *target)),
                        unavailable: unavailable.clone(),
                    });
                }
            }
        }
        result.sort_by_key(|status| {
            (
                status.instance_id,
                status.lane_id,
                status
                    .retained_sample
                    .map(|(controller, target)| (controller, target.0)),
                status.unavailable.source.profile_id,
            )
        });
        result
    }

    /// Cold, transactional source-generation rebind. Clocks, controllers, Random streams and
    /// immutable held/preset payloads survive; invalid newly verifiable values reject the rebind.
    pub fn refresh_native_color_models(
        &mut self,
        models: Arc<dyn DynamicNativeModelResolver>,
    ) -> Result<(), DynamicRuntimeError> {
        assert!(
            self.output_frame_undo.is_none(),
            "native capability refresh is outside an output transaction"
        );
        if self
            .native_models
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, &models))
            && self.compiled_lanes.values().all(|lanes| {
                lanes
                    .values()
                    .all(|lane| lane.unavailable_native_source().is_none())
            })
            && self.instances.values().all(|instance| {
                instance
                    .programming_lanes
                    .values()
                    .all(|lane| lane.unavailable_native_source().is_none())
                    && instance.unavailable_samples.is_empty()
            })
        {
            return Ok(());
        }
        // A prepared definition token can install suspended lanes after this provider was
        // captured. An explicit cold refresh must retry those lanes and retained histories
        // even with the same provider Arc. Fully verified runtimes keep the warm no-op above.
        let mut candidate =
            Self::with_native_color_models(self.supported_programming_contract, models);
        candidate.native_model_pins = self.native_model_pins.detached();
        let recompile = |definition: &DynamicDefinition,
                         old: &ProgrammingLanes|
         -> Result<ProgrammingLanes, DynamicRuntimeError> {
            if old
                .values()
                .all(|lane| lane.unavailable_native_source().is_none())
            {
                return Ok(old.clone());
            }
            let mut next = candidate.compile_programming_lanes(definition)?;
            for (id, lane) in old {
                if lane.unavailable_native_source().is_none() {
                    next.insert(*id, lane.clone());
                }
            }
            Ok(next)
        };
        let mut definitions = HashMap::new();
        for (id, definition) in &self.definitions {
            definitions.insert(*id, recompile(definition, &self.compiled_lanes[id])?);
        }
        let mut instances = Vec::new();
        for instance in self.instances.values() {
            let lanes = recompile(&instance.definition, &instance.programming_lanes)?;
            let values = preset_values::PresetValues::compile(
                instance.preset_values.retained.clone(),
                &lanes,
                &instance.targets,
                true,
            )?;
            candidate.validate_retained_expressions(
                instance
                    .last_sample_values
                    .values()
                    .chain(instance.synchronized_hold_values.values()),
                None,
            )?;
            let unavailable = candidate.unavailable_retained_samples(
                instance
                    .last_sample_values
                    .iter()
                    .chain(instance.synchronized_hold_values.iter()),
            )?;
            let changed = instance
                .programming_lanes
                .values()
                .any(|lane| lane.unavailable_native_source().is_some());
            instances.push((instance.id, lanes, values, unavailable, changed));
        }
        self.native_models = candidate.native_models;
        self.native_model_pins = candidate.native_model_pins;
        self.compiled_lanes = definitions;
        for (id, lanes, values, unavailable, changed) in instances {
            let instance = self.instances.get_mut(&id).expect("staged instance");
            instance.programming_lanes = lanes;
            instance.preset_values = values;
            instance.unavailable_samples = unavailable;
            if changed {
                instance.preset_dependency_generation = Uuid::new_v4();
            }
        }
        Ok(())
    }
}
