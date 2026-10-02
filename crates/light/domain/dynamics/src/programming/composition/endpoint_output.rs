//! Captured output controls apply to an evaluated endpoint, before its activation vote.
use super::*;
use crate::{
    DynamicValueSourceResolver, FamilyExpressionOperation, WholeFamilyExpressionFrameResolver,
};
use light_core::FixtureId;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum FamilyEndpointOutputControl {
    #[default]
    Unchanged,
    Suppressed,
    CrossfadeCurrent {
        mix: f32,
    },
}

#[derive(Clone, Copy)]
pub struct FamilyEndpointOutputContext<'a> {
    pub control: &'a dyn Fn(FamilySampleRank) -> FamilyEndpointOutputControl,
    pub target: FixtureId,
    pub current: &'a dyn DynamicValueSourceResolver,
    /// Original source models from the same immutable capture as Current.
    pub native_models: Option<&'a dyn crate::DynamicNativeModelResolver>,
}

pub(super) fn control(
    rank: FamilySampleRank,
    context: &FamilyCompositionContext<'_>,
) -> FamilyEndpointOutputControl {
    if rank.dynamic_identity().is_none() {
        return FamilyEndpointOutputControl::Unchanged;
    }
    context
        .endpoint_output
        .map_or(FamilyEndpointOutputControl::Unchanged, |context| {
            (context.control)(rank)
        })
}

pub(super) fn validate(control: FamilyEndpointOutputControl) -> Result<(), IntentError> {
    if let FamilyEndpointOutputControl::CrossfadeCurrent { mix } = control {
        ensure(
            mix.is_finite() && (0.0..=1.0).contains(&mix),
            "endpoint Current crossfade must be between zero and one",
        )?;
    }
    Ok(())
}

pub(super) fn component(
    rank: FamilySampleRank,
    address: &CompiledDynamicValueAddress,
    endpoint: DynamicValue,
    endpoint_trace: Option<FamilyTraceNodeId>,
    context: &FamilyCompositionContext<'_>,
    trace: Option<&mut FamilyTraceArena>,
) -> Result<(DynamicValue, Option<FamilyTraceNodeId>), TransitionError> {
    let FamilyEndpointOutputControl::CrossfadeCurrent { mix } = control(rank, context) else {
        return Ok((endpoint, endpoint_trace));
    };
    validate(FamilyEndpointOutputControl::CrossfadeCurrent { mix })?;
    if mix == 1.0 {
        return Ok((endpoint, endpoint_trace));
    }
    let output = context.endpoint_output.expect("captured control context");
    let current = output
        .current
        .try_current(output.target, address.address())?
        .ok_or(TransitionError::Requires(requirement(address.address())))?;
    address.validate_value(&current)?;
    let value = if mix == 0.0 {
        current
    } else {
        address.transition(current, endpoint)?.sample(mix)?
    };
    let node = trace.map(|trace| {
        let current = trace.current_source(
            FamilyTraceSource {
                rank,
                footprint: FamilyTraceFootprint::Component(
                    address.address().component.expect("component envelope"),
                ),
                role: FamilyTraceRole::CalculationDependency,
                occurrence: None,
            },
            output
                .current
                .current_dependency(output.target, address.address()),
        );
        if mix == 0.0 {
            current
        } else {
            trace.blend(current, endpoint_trace.expect("traced endpoint"))
        }
    });
    Ok((value, node))
}

pub(super) fn whole(
    rank: FamilySampleRank,
    owner: ProgrammingOwner,
    endpoint: AttributeValue,
    endpoint_trace: Option<FamilyTraceNodeId>,
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    resolve_model: &dyn Fn(
        &light_core::NativeColorIdentity,
    )
        -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, TransitionError>,
    trace: Option<&mut FamilyTraceArena>,
) -> Result<(AttributeValue, Option<FamilyTraceNodeId>), TransitionError> {
    let FamilyEndpointOutputControl::CrossfadeCurrent { mix } = control(rank, context) else {
        return Ok((endpoint, endpoint_trace));
    };
    validate(FamilyEndpointOutputControl::CrossfadeCurrent { mix })?;
    if mix == 1.0 {
        return Ok((endpoint, endpoint_trace));
    }
    let output = context.endpoint_output.expect("captured control context");
    let address = DynamicValueAddress::whole_family(owner, &endpoint)?;
    let current = output
        .current
        .try_current_family_base(output.target, &address)?
        .ok_or(TransitionError::Requires(requirement(&address)))?;
    ProgrammingFieldScope::for_value(owner, &current)?;
    let (value, transfer) = if mix == 0.0 {
        (current.clone(), None)
    } else {
        transition(
            owner,
            &current,
            &endpoint,
            mix,
            context,
            frame,
            resolve_model,
            trace.is_some(),
        )?
    };
    let node = trace.map(|trace| {
        let current = trace.source(FamilyTraceSource {
            rank,
            footprint: FamilyTraceFootprint::Whole,
            role: FamilyTraceRole::CalculationDependency,
            occurrence: output
                .current
                .current_family_occurrence(output.target, &address),
        });
        if mix == 0.0 {
            current
        } else {
            trace.mapped_blend(current, endpoint_trace.expect("traced endpoint"), transfer)
        }
    });
    Ok((value, node))
}

type Model = Arc<dyn NativeColorEditModel + Send + Sync>;

pub(super) fn resolve_native_model(
    source: &light_core::NativeColorIdentity,
    context: &FamilyCompositionContext<'_>,
    retained: &dyn Fn(&light_core::NativeColorIdentity) -> Result<Model, TransitionError>,
) -> Result<Model, TransitionError> {
    let model = match retained(source) {
        Ok(model) => model,
        Err(TransitionError::Requires(TransitionRequirement::NativeColorModel)) => {
            let models = context
                .endpoint_output
                .and_then(|output| output.native_models)
                .ok_or(TransitionError::Requires(
                    TransitionRequirement::NativeColorModel,
                ))?;
            match models.resolve_capability(source)? {
                crate::NativeColorModelCapability::Available(model) => model,
                crate::NativeColorModelCapability::Unavailable(reason) => {
                    ensure(
                        &reason.source == source,
                        "captured native capability differs from original source",
                    )?;
                    return Err(TransitionError::Requires(
                        TransitionRequirement::NativeColorModel,
                    ));
                }
            }
        }
        Err(error) => return Err(error),
    };
    ensure(
        model.source() == source,
        "captured native model differs from original source",
    )?;
    Ok(model)
}

fn direct_source(value: &AttributeValue) -> Option<&light_core::NativeColorIdentity> {
    match value {
        AttributeValue::ColorProgram(program) => match program.as_ref() {
            ColorProgram::Direct { recipe, .. } => Some(&recipe.source),
            _ => None,
        },
        _ => None,
    }
}

/// The endpoint envelope can introduce a captured Direct source absent from retained leaves.
/// Resolve only models actually needed for native interpolation or a surviving Direct result.
/// Portable Direct inputs can still become Semantic through the pinned frame appearance solver.
#[allow(clippy::too_many_arguments)]
pub(super) fn transition(
    owner: ProgrammingOwner,
    from: &AttributeValue,
    to: &AttributeValue,
    mix: f32,
    context: &FamilyCompositionContext<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
    retained: &dyn Fn(&light_core::NativeColorIdentity) -> Result<Model, TransitionError>,
    traced: bool,
) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
    let model = match (direct_source(from), direct_source(to)) {
        (Some(a), Some(b)) if a == b && mix > 0.0 && mix < 1.0 => {
            Some(resolve_native_model(a, context, retained)?)
        }
        _ => None,
    };
    let compiled = CompiledProgrammingTransition::new(from.clone(), to.clone(), model.clone())?;
    let sampled = if traced {
        compiled.sample_with_trace(owner, mix)
    } else {
        compiled.sample(mix).map(|value| (value, None))
    };
    let (result, from_frame) = match sampled {
        Ok(result) => (result, false),
        Err(TransitionError::Requires(
            requirement @ (TransitionRequirement::LiveTargetPoints
            | TransitionRequirement::LiveJointAngles
            | TransitionRequirement::ColorAppearance
            | TransitionRequirement::ZoomConvention),
        )) => {
            let operation = FamilyExpressionOperation::Transition { progress: mix };
            let result = if traced {
                frame.resolve_with_trace(requirement, from, to, operation)?
            } else {
                (frame.resolve(requirement, from, to, operation)?, None)
            };
            ProgrammingFieldScope::for_value(owner, &result.0)?;
            (result, true)
        }
        Err(error) => return Err(error),
    };
    if let Some(source) = direct_source(&result.0) {
        let model = match model.filter(|model| model.source() == source) {
            Some(model) => model,
            None => resolve_native_model(source, context, retained)?,
        };
        if from_frame {
            let address = CompiledDynamicValueAddress::new(
                DynamicValueAddress::whole_family(owner, &result.0)?,
                Some(model),
            )?;
            address.validate_source_value(&DynamicValue::Family(result.0.clone()))?;
        }
    }
    Ok(result)
}

pub(super) fn control_trace(
    rank: FamilySampleRank,
    footprint: FamilyTraceFootprint,
    appearance: FamilyTraceNodeId,
    prior: Option<FamilyTraceNodeId>,
    _context: &FamilyCompositionContext<'_>,
    trace: &mut FamilyTraceArena,
) -> FamilyTraceNodeId {
    trace.control(rank, footprint, appearance, prior)
}

// Nested endpoint reconstruction must not apply the same output control at its leaves.
pub(super) fn with_control<'a>(
    context: &FamilyCompositionContext<'a>,
    endpoint_output: Option<FamilyEndpointOutputContext<'a>>,
) -> FamilyCompositionContext<'a> {
    FamilyCompositionContext {
        edit: FamilyEditContext {
            solved_angles: context.edit.solved_angles,
            semantic_color_adoption: context.edit.semantic_color_adoption,
            color_model: context.edit.color_model,
            native_model: context.edit.native_model,
        },
        adopted_base: context.adopted_base,
        resolve_adoption: context.resolve_adoption,
        endpoint_output,
    }
}
