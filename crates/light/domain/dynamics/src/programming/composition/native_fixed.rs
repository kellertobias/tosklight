//! A native FixAT changes only its captured channel. Discrete values and function switches
//! take effect at the end of activation; their source appearance must not be attributed early.
use super::*;
use light_core::{NativeColorBinding, NativeColorValue};

pub(super) struct NativeFixedStep {
    binding: NativeColorBinding,
    raw: u32,
    model: Arc<dyn NativeColorEditModel + Send + Sync>,
}

pub(super) fn is_native_fixed(sample: &FamilySample) -> bool {
    sample.fix_at
        && matches!(
            sample.address.address().component,
            Some(ProgrammingComponent::NativeColor(_))
        )
}

/// Same-function continuous masks keep the existing numeric interpolation path. Function
/// changes must step even when the new function happens to be declared continuous.
pub(super) fn prepare(
    sample: &FamilySample,
    value: &AttributeValue,
) -> Result<Option<NativeFixedStep>, TransitionError> {
    if !is_native_fixed(sample) {
        return Ok(None);
    }
    let Some(ProgrammingComponent::NativeColor(binding)) = sample.address.address().component
    else {
        unreachable!()
    };
    let Some(DynamicValue::Native(raw)) = sample.materialized_value() else {
        return Err(IntentError("native Fixed mask requires a materialized integer".into()).into());
    };
    let model = sample
        .address
        .native_model()
        .ok_or(TransitionError::Requires(
            TransitionRequirement::NativeColorModel,
        ))?;
    let descriptor = model.descriptor(binding).ok_or_else(|| {
        IntentError("fixed native function is absent from its original model".into())
    })?;
    ensure(
        descriptor.binding == binding,
        "fixed native descriptor differs from its binding",
    )?;
    let changed_function = match value {
        AttributeValue::ColorProgram(program) => match program.as_ref() {
            ColorProgram::Direct { recipe, .. } if &recipe.source == model.source() => {
                let previous = recipe
                    .channels
                    .iter()
                    .find(|channel| channel.channel_id == binding.channel_id)
                    .ok_or_else(|| {
                        IntentError(
                            "fixed native channel is absent from its complete underlay".into(),
                        )
                    })?;
                previous.function_id != binding.function_id
            }
            _ => false,
        },
        _ => false,
    };
    Ok(
        (!descriptor.continuous || changed_function).then_some(NativeFixedStep {
            binding,
            raw: *raw,
            model,
        }),
    )
}

impl NativeFixedStep {
    pub(super) fn apply(
        &self,
        value: AttributeValue,
        sample: &FamilySample,
        context: &FamilyCompositionContext<'_>,
        original_base: &AttributeValue,
        prior: Option<FamilyTraceNodeId>,
        mut trace: Option<&mut FamilyTraceArena>,
    ) -> Result<retained_family::TracedValue, TransitionError> {
        let same_source = matches!(&value, AttributeValue::ColorProgram(program)
            if matches!(program.as_ref(), ColorProgram::Direct { recipe, .. } if &recipe.source == self.model.source()));
        if sample.activation_mix < 1.0 {
            // A foreign representation has no comparable old function/raw. It needs a coherent
            // appearance resolver, not a fabricated recipe from the recorded mask's siblings.
            if !same_source {
                return Err(TransitionError::Requires(
                    TransitionRequirement::ColorAppearance,
                ));
            }
            self.validate_old_channel(&value)?;
            let trace = trace.as_deref_mut().map(|arena| {
                let prior = prior.expect("traced native step underlay");
                // The mask has control influence while appearance still comes entirely from
                // its eligible lower channel. Other channels and their prediction stay exact.
                arena.control(
                    sample.rank,
                    FamilyTraceFootprint::Component(ProgrammingComponent::NativeColor(
                        self.binding,
                    )),
                    prior,
                    Some(prior),
                )
            });
            return Ok(retained_family::TracedValue { value, trace });
        }
        let mut trace_root = prior;
        let mut value = if same_source {
            value
        } else {
            let mut address = sample.address.address().clone();
            address.component = None;
            let adopted = adopt(value, &address, context, original_base)?;
            if let Some(arena) = trace.as_deref_mut() {
                let prior = prior.expect("traced native source adoption");
                trace_root = Some(arena.mapped_blend(prior, prior, None));
            }
            adopted
        };
        self.validate_old_channel(&value)?;
        let AttributeValue::ColorProgram(program) = &value else {
            unreachable!("validated Direct underlay")
        };
        let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
            unreachable!("validated Direct underlay")
        };
        let mut recipe = recipe.clone();
        let channel = recipe
            .channels
            .iter_mut()
            .find(|channel| channel.channel_id == self.binding.channel_id)
            .expect("validated native channel");
        *channel = NativeColorValue {
            channel_id: self.binding.channel_id,
            function_id: self.binding.function_id,
            raw: self.raw,
        };
        recipe.validate()?;
        let portable = self.model.predict(&recipe)?;
        ensure(
            portable.model_revision == recipe.source.model_revision,
            "fixed native prediction differs from its pinned model revision",
        )?;
        portable.validate()?;
        value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct { recipe, portable }));
        value.validate_programming_address(&ProgrammingOwner::Color.key())?;
        if let Some(arena) = trace {
            let source = sample_trace(sample, arena);
            trace_root = Some(arena.write(
                trace_root.expect("traced native fixed base"),
                source,
                FamilyTraceFootprint::Component(ProgrammingComponent::NativeColor(self.binding)),
                false,
            ));
        }
        Ok(retained_family::TracedValue {
            value,
            trace: trace_root,
        })
    }

    fn validate_old_channel(&self, value: &AttributeValue) -> Result<(), TransitionError> {
        let AttributeValue::ColorProgram(program) = value else {
            return Err(TransitionError::Requires(
                TransitionRequirement::ColorAppearance,
            ));
        };
        let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
            return Err(TransitionError::Requires(
                TransitionRequirement::ColorAppearance,
            ));
        };
        ensure(
            &recipe.source == self.model.source(),
            "native Fixed underlay uses a different original model",
        )?;
        let channel = recipe
            .channels
            .iter()
            .find(|channel| channel.channel_id == self.binding.channel_id)
            .ok_or_else(|| {
                IntentError("fixed native channel is absent from its complete underlay".into())
            })?;
        let binding = NativeColorBinding {
            channel_id: channel.channel_id,
            function_id: channel.function_id,
        };
        let descriptor = self.model.descriptor(binding).ok_or_else(|| {
            IntentError("old native function is absent from the pinned source".into())
        })?;
        ensure(
            descriptor.binding == binding
                && (descriptor.raw_from.min(descriptor.raw_to)
                    ..=descriptor.raw_from.max(descriptor.raw_to))
                    .contains(&channel.raw),
            "old native value is outside its pinned function",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
