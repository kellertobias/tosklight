//! A Current read must not certify a structurally valid but incomplete Direct recipe. Keep the
//! original model's complete verification while that exact immutable value/model pair survives.
use super::*;
use light_core::{
    NativeColorIdentity,
    programming::{ColorProgram, NativeColorEditModel},
};
use light_dynamics::{DynamicNativeModelResolver, NativeColorModelCapability};
use std::sync::Weak;

type Model = Arc<dyn NativeColorEditModel + Send + Sync>;
struct VerifiedCurrent {
    value: Weak<ColorProgram>,
    model: Weak<dyn NativeColorEditModel + Send + Sync>,
    result: Result<(), IntentError>,
    used: bool,
}

/// Runtime scratch belongs to one Live/Preload branch. Neither values nor original models are
/// kept alive by historical proofs. begin/finish bracket the entire frame, including failures.
#[derive(Default)]
pub(super) struct CurrentNativeVerificationCache {
    entries: FxHashMap<(FixtureId, ProgrammingOwner), Vec<VerifiedCurrent>>,
    capabilities: Vec<(
        NativeColorIdentity,
        Result<NativeColorModelCapability, IntentError>,
    )>,
}

impl CurrentNativeVerificationCache {
    pub(super) fn begin_frame(&mut self) {
        for entries in self.entries.values_mut() {
            for entry in entries {
                entry.used = false;
            }
        }
        self.capabilities.clear();
    }

    pub(super) fn finish_frame(&mut self) {
        self.entries.retain(|_, entries| {
            entries.retain(|entry| entry.used);
            !entries.is_empty()
        });
        self.capabilities.clear();
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.capabilities.clear();
    }

    fn model(
        &mut self,
        source: &NativeColorIdentity,
        models: &dyn DynamicNativeModelResolver,
    ) -> Result<Model, TransitionError> {
        let index = match self
            .capabilities
            .iter()
            .position(|(known, _)| known == source)
        {
            Some(index) => index,
            None => {
                self.capabilities
                    .push((source.clone(), models.resolve_capability(source)));
                self.capabilities.len() - 1
            }
        };
        let capability = &self.capabilities[index].1;
        match capability {
            Ok(NativeColorModelCapability::Available(model)) => {
                if model.source() != source {
                    return Err(IntentError(
                        "captured Current model differs from its original source".into(),
                    )
                    .into());
                }
                Ok(model.clone())
            }
            Ok(NativeColorModelCapability::Unavailable(reason)) => {
                if &reason.source != source {
                    return Err(IntentError(
                        "captured Current capability differs from its original source".into(),
                    )
                    .into());
                }
                Err(TransitionError::Requires(
                    TransitionRequirement::NativeColorModel,
                ))
            }
            Err(error) => Err(error.clone().into()),
        }
    }

    pub(super) fn verify(
        &mut self,
        target: FixtureId,
        base: &AttributeValue,
        models: &dyn DynamicNativeModelResolver,
    ) -> Result<(), TransitionError> {
        let key = (target, ProgrammingOwner::Color);
        let AttributeValue::ColorProgram(program) = base else {
            return Ok(());
        };
        let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
            return Ok(());
        };
        // Structural corruption remains invalid even when the original capability is missing.
        program.validate()?;
        if !recipe.spreads.is_empty() {
            return Err(TransitionError::Requires(
                TransitionRequirement::MaterializedEndpoints,
            ));
        }
        let model = self.model(&recipe.source, models)?;
        let value_identity = Arc::downgrade(program);
        let model_identity = Arc::downgrade(&model);
        let entries = self.entries.entry(key).or_default();
        if let Some(entry) = entries.iter_mut().find(|entry| {
            entry.value.ptr_eq(&value_identity) && entry.model.ptr_eq(&model_identity)
        }) {
            entry.used = true;
            return entry.result.clone().map_err(Into::into);
        }
        // Prediction is the original adapter's complete-channel/function/range check. Merely
        // matching its source identity does not prove any of those value constraints.
        let result = model.predict(recipe).and_then(|prediction| {
            if prediction.model_revision != recipe.source.model_revision {
                return Err(IntentError(
                    "Current prediction differs from its original model revision".into(),
                ));
            }
            prediction.validate()
        });
        // One address may read the original Direct family and another its adopted Direct
        // result. Keep both exact proofs; neither may evict the other's warm verification.
        entries.push(VerifiedCurrent {
            value: value_identity,
            model: model_identity,
            result: result.clone(),
            used: true,
        });
        result.map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
