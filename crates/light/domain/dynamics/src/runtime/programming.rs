use super::*;
use crate::{CompiledProgrammingLane, DynamicFamilyRepresentation, DynamicLaneBody};
use light_core::{
    NativeColorIdentity,
    programming::{IntentError, NativeColorEditModel},
};

/// Immutable source profiles, resolved only at definition/restore boundaries.
/// A destination fixture or a newer library revision cannot substitute for this identity.
pub trait DynamicNativeModelResolver: Send + Sync {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError>;

    /// Expected capability absence is distinct from invalid source/value contracts. Existing
    /// verified providers remain strict unless they explicitly declare unavailable originals.
    fn resolve_capability(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<NativeColorModelCapability, IntentError> {
        self.resolve(source)
            .map(NativeColorModelCapability::Available)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeColorUnavailableReason {
    CatalogueNotPrepared,
    MissingRevision,
    UnavailableRevision,
    MissingPath,
    UnverifiedOriginal,
    UnsupportedModel,
    MissingResolver,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeColorModelUnavailable {
    pub source: NativeColorIdentity,
    pub reason: NativeColorUnavailableReason,
    pub detail: String,
}

#[derive(Clone)]
pub enum NativeColorModelCapability {
    Available(Arc<dyn NativeColorEditModel + Send + Sync>),
    Unavailable(NativeColorModelUnavailable),
}

impl NativeColorModelCapability {
    pub fn require_available(
        self,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        match self {
            Self::Available(model) => Ok(model),
            Self::Unavailable(reason) => Err(IntentError(reason.detail)),
        }
    }
}

pub(super) type ProgrammingLanes = HashMap<Uuid, CompiledProgrammingLane>;

#[derive(Clone, Debug)]
pub struct DynamicInstancePresetSources {
    pub instance_id: Uuid,
    pub dependency_generation: Uuid,
    pub ordered_targets: Vec<FixtureId>,
    pub sources: Vec<Arc<crate::DynamicPresetSourceBinding>>,
    pub last_valid: Vec<crate::DynamicPresetSourceValues>,
}

impl DynamicRuntime {
    /// Targets of running instances of `definition` when it has a lane for `owner`, in
    /// instance order (duplicates possible). A frame uses it to find a Dynamic Playback's owners
    /// whose static pre-Dynamic baseline is the fixture's declared default (TL-552).
    pub fn owner_lane_targets<'a>(
        &'a self,
        definition: Uuid,
        owner: &'a light_core::AttributeKey,
    ) -> impl Iterator<Item = FixtureId> + 'a {
        self.instances
            .values()
            .filter(move |i| {
                i.definition.id == definition
                    && i.definition
                        .lanes
                        .iter()
                        .any(|l| l.output_owner() == *owner)
            })
            .flat_map(|i| i.targets.iter().copied())
    }

    /// Cold dependency compiler input, including the original selection order for
    /// universal spreads. Pinned instances expose their retained source generations.
    pub fn preset_source_instances(&self) -> Vec<DynamicInstancePresetSources> {
        self.capture_preset_source_instances(false)
    }

    /// Capture only manifests that have not been materialized for their current dependencies.
    /// A successful empty/unavailable result is still prepared; repeated output ticks must not
    /// retry its Group/native compilation until an actual dependency changes.
    pub fn pending_preset_source_instances(&self) -> Vec<DynamicInstancePresetSources> {
        self.capture_preset_source_instances(true)
    }

    /// Allocation-free cold-work check. No target lists, retained values, Groups or native
    /// profiles are inspected on an unchanged frame.
    pub fn has_pending_preset_sources(&self) -> bool {
        self.instances.values().any(|instance| {
            if instance.preset_values.prepared_generation
                == Some(instance.preset_dependency_generation)
            {
                return false;
            }
            instance.programming_lanes.values().any(|lane| {
                let mut found = false;
                lane.visit_preset_sources(|_| found = true);
                found
            })
        })
    }

    fn capture_preset_source_instances(
        &self,
        pending_only: bool,
    ) -> Vec<DynamicInstancePresetSources> {
        let mut instances = self
            .instances
            .values()
            .filter(|instance| {
                !pending_only
                    || instance.preset_values.prepared_generation
                        != Some(instance.preset_dependency_generation)
            })
            .filter_map(|instance| {
                let mut sources = Vec::new();
                for lane in &instance.definition.lanes {
                    if let Some(compiled) = instance.programming_lanes.get(&lane.id) {
                        compiled.visit_preset_sources(|source| sources.push(Arc::clone(source)));
                    }
                }
                (!sources.is_empty()).then(|| DynamicInstancePresetSources {
                    instance_id: instance.id,
                    dependency_generation: instance.preset_dependency_generation,
                    ordered_targets: instance.targets.clone(),
                    sources,
                    last_valid: instance.preset_values.retained.clone(),
                })
            })
            .collect::<Vec<_>>();
        instances.sort_by_key(|instance| instance.instance_id);
        instances
    }

    pub(super) fn validate_retained_expressions<'a>(
        &self,
        values: impl IntoIterator<Item = &'a DynamicSampleExpression>,
        prepared_tape: Option<&PreparedSampleTape>,
    ) -> Result<(), DynamicRuntimeError> {
        let mut addresses = Vec::<crate::CompiledDynamicValueAddress>::new();
        let mut validate = |address: &crate::DynamicValueAddress, value: &crate::DynamicValue| {
            let index = if let Some(index) = addresses
                .iter()
                .position(|compiled| compiled.address() == address)
            {
                index
            } else {
                addresses.push(self.compile_native_address(address)?);
                addresses.len() - 1
            };
            addresses[index].validate_source_value(value)
        };
        let mut shared_roots = Vec::new();
        let mut standalone = Vec::new();
        for value in values {
            if let (Some(prepared), DynamicSampleExpression::Retained { tape, root }) =
                (prepared_tape, value)
                && Arc::ptr_eq(tape, &prepared.tape)
            {
                shared_roots.push(*root);
            } else {
                standalone.push(value);
            }
        }
        if let Some(prepared) = prepared_tape {
            prepared
                .tape
                .visit_reachable(&shared_roots, |_, node| {
                    match node {
                        crate::RetainedExpressionNode::Programming { address, value, .. } => {
                            validate(address, value)?
                        }
                        crate::RetainedExpressionNode::AngleNumeric { program } => {
                            program.visit_materialized_values(&mut validate)?;
                        }
                        crate::RetainedExpressionNode::Scale { address, base, .. } => {
                            let crate::DynamicValue::Family(family) = base else {
                                return Err(IntentError(
                                    "Dynamic Size requires a whole-family baseline".into(),
                                ));
                            };
                            let source =
                                crate::DynamicValueAddress::whole_family(address.owner(), family)?;
                            validate(&source, base)?;
                        }
                        _ => {}
                    }
                    Ok(())
                })
                .map_err(|error| DynamicRuntimeError::InvalidSnapshot(error.to_string()))?;
        }
        for value in standalone {
            value
                .visit_programming_values(&mut validate)
                .map_err(|error| DynamicRuntimeError::InvalidSnapshot(error.to_string()))?;
        }
        Ok(())
    }

    pub fn with_native_color_models(
        supported_programming_contract: u16,
        models: Arc<dyn DynamicNativeModelResolver>,
    ) -> Self {
        let mut runtime = Self::with_programming_contract_support(supported_programming_contract);
        runtime.native_models = Some(models);
        runtime
    }

    pub(super) fn compile_programming_lanes(
        &self,
        definition: &DynamicDefinition,
    ) -> Result<ProgrammingLanes, DynamicRuntimeError> {
        self.validate_supported_definition(definition)?;
        let compile = || -> Result<ProgrammingLanes, IntentError> {
            let mut lanes = HashMap::new();
            let mut random_domains = HashMap::new();
            for lane in &definition.lanes {
                let DynamicLaneBody::Programming(body) = &lane.body else {
                    continue;
                };
                let capability = if let DynamicFamilyRepresentation::DirectColor { source } =
                    &body.address.representation
                {
                    Some(self.native_capability(source)?)
                } else {
                    None
                };
                let compiled = match capability {
                    Some(NativeColorModelCapability::Unavailable(reason)) => {
                        CompiledProgrammingLane::suspended(lane, &definition.random_groups, reason)?
                    }
                    capability => CompiledProgrammingLane::new(
                        lane,
                        &definition.random_groups,
                        match capability {
                            Some(NativeColorModelCapability::Available(model)) => Some(model),
                            _ => None,
                        },
                    )?,
                };
                if lane.mode() == crate::DynamicLaneMode::Random
                    && compiled.unavailable_native_source().is_none()
                {
                    let group_id = lane.random_group_id.expect("validated Random group");
                    if let Some(previous) = random_domains.get(&group_id) {
                        if !compiled.address().compatible_random_domain(previous) {
                            return Err(IntentError(
                                "shared Random group has incompatible compiled native/unit domains"
                                    .into(),
                            ));
                        }
                    } else {
                        random_domains.insert(group_id, compiled.address().clone());
                    }
                }
                lanes.insert(lane.id, compiled);
            }
            Ok(lanes)
        };
        compile().map_err(|error| DynamicRuntimeError::InvalidDefinition(error.to_string()))
    }
}
