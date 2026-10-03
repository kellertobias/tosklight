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
#[derive(Clone)]
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
        capability_model(&mut self.capabilities, None, source, models)
    }

    pub(super) fn verify(
        &mut self,
        target: FixtureId,
        base: &AttributeValue,
        models: &dyn DynamicNativeModelResolver,
    ) -> Result<(), TransitionError> {
        let Some((program, recipe)) = direct_recipe(base)? else {
            return Ok(());
        };
        let model = self.model(&recipe.source, models)?;
        let key = (target, ProgrammingOwner::Color);
        let (value, model_identity) = (Arc::downgrade(program), Arc::downgrade(&model));
        let entries = self.entries.entry(key).or_default();
        if let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.value.ptr_eq(&value) && entry.model.ptr_eq(&model_identity))
        {
            entry.used = true;
            return entry.result.clone().map_err(Into::into);
        }
        let verified = verify_recipe(recipe, &model, value, model_identity);
        let result = verified.result.clone();
        // One address may read the original Direct family and another its adopted Direct
        // result. Keep both exact proofs; neither may evict the other's warm verification.
        entries.push(verified);
        result.map_err(Into::into)
    }

    /// Apply a parallel worker's verifications (TL-639 round 5).
    pub(super) fn apply(&mut self, fork: NativeCurrentChanges) {
        for (key, index) in fork.used {
            if let Some(entry) = self
                .entries
                .get_mut(&key)
                .and_then(|entries| entries.get_mut(index))
            {
                entry.used = true;
            }
        }
        for (key, verified) in fork.added {
            self.entries.entry(key).or_default().extend(verified);
        }
    }
}

/// The complete Direct recipe a Current read must verify, if the value is one.
fn direct_recipe(
    base: &AttributeValue,
) -> Result<
    Option<(
        &Arc<ColorProgram>,
        &light_core::programming::NativeColorRecipe,
    )>,
    TransitionError,
> {
    let AttributeValue::ColorProgram(program) = base else {
        return Ok(None);
    };
    let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
        return Ok(None);
    };
    // Structural corruption remains invalid even when the original capability is missing.
    program.validate()?;
    if !recipe.spreads.is_empty() {
        return Err(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints,
        ));
    }
    Ok(Some((program, recipe)))
}

/// Prediction is the original adapter's complete-channel/function/range check. Merely matching
/// its source identity does not prove any of those value constraints.
fn verify_recipe(
    recipe: &light_core::programming::NativeColorRecipe,
    model: &Model,
    value: Weak<ColorProgram>,
    model_identity: Weak<dyn NativeColorEditModel + Send + Sync>,
) -> VerifiedCurrent {
    let result = model.predict(recipe).and_then(|prediction| {
        if prediction.model_revision != recipe.source.model_revision {
            return Err(IntentError(
                "Current prediction differs from its original model revision".into(),
            ));
        }
        prediction.validate()
    });
    VerifiedCurrent {
        value,
        model: model_identity,
        result,
        used: true,
    }
}

type Capabilities = Vec<(
    NativeColorIdentity,
    Result<NativeColorModelCapability, IntentError>,
)>;

/// The frame's capability of `source`, resolved once per frame (`frozen`: the frame's own memo
/// when a worker resolves).
fn capability_model(
    capabilities: &mut Capabilities,
    frozen: Option<&Capabilities>,
    source: &NativeColorIdentity,
    models: &dyn DynamicNativeModelResolver,
) -> Result<Model, TransitionError> {
    let known = |list: &Capabilities| list.iter().position(|(known, _)| known == source);
    let capability = match frozen.and_then(|frozen| known(frozen).map(|index| &frozen[index].1)) {
        Some(capability) => capability,
        None => {
            let index = match known(capabilities) {
                Some(index) => index,
                None => {
                    capabilities.push((source.clone(), models.resolve_capability(source)));
                    capabilities.len() - 1
                }
            };
            &capabilities[index].1
        }
    };
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

/// What one worker verified: proofs it reused from the frame's cache and proofs it added.
#[derive(Default)]
pub(super) struct NativeCurrentChanges {
    used: Vec<((FixtureId, ProgrammingOwner), usize)>,
    added: FxHashMap<(FixtureId, ProgrammingOwner), Vec<VerifiedCurrent>>,
}

/// A parallel worker's view of the frame's verification cache: the cache as it stood when the
/// section began, and the worker's own proofs by group (see `memo`).
pub(super) struct NativeCurrentFork<'f> {
    frozen: &'f CurrentNativeVerificationCache,
    capabilities: Capabilities,
    group: NativeCurrentChanges,
    chunk: NativeCurrentChanges,
}

impl<'f> NativeCurrentFork<'f> {
    pub(super) fn new(frozen: &'f CurrentNativeVerificationCache) -> Self {
        Self {
            frozen,
            capabilities: Vec::new(),
            group: NativeCurrentChanges::default(),
            chunk: NativeCurrentChanges::default(),
        }
    }

    pub(super) fn verify(
        &mut self,
        target: FixtureId,
        base: &AttributeValue,
        models: &dyn DynamicNativeModelResolver,
    ) -> Result<(), TransitionError> {
        let Some((program, recipe)) = direct_recipe(base)? else {
            return Ok(());
        };
        let model = capability_model(
            &mut self.capabilities,
            Some(&self.frozen.capabilities),
            &recipe.source,
            models,
        )?;
        let key = (target, ProgrammingOwner::Color);
        let (value, model_identity) = (Arc::downgrade(program), Arc::downgrade(&model));
        let matches = |entry: &VerifiedCurrent| {
            entry.value.ptr_eq(&value) && entry.model.ptr_eq(&model_identity)
        };
        if let Some(index) = self
            .frozen
            .entries
            .get(&key)
            .and_then(|entries| entries.iter().position(matches))
        {
            self.group.used.push((key, index));
            return self.frozen.entries[&key][index]
                .result
                .clone()
                .map_err(Into::into);
        }
        for added in [&self.chunk.added, &self.group.added] {
            if let Some(entry) = added
                .get(&key)
                .and_then(|entries| entries.iter().find(|entry| matches(entry)))
            {
                return entry.result.clone().map_err(Into::into);
            }
        }
        let verified = verify_recipe(recipe, &model, value, model_identity);
        let result = verified.result.clone();
        self.group.added.entry(key).or_default().push(verified);
        result.map_err(Into::into)
    }

    pub(super) fn commit_group(&mut self) {
        self.chunk.used.append(&mut self.group.used);
        for (key, verified) in self.group.added.drain() {
            self.chunk.added.entry(key).or_default().extend(verified);
        }
    }

    pub(super) fn abort_group(&mut self) {
        self.group.used.clear();
        self.group.added.clear();
    }

    pub(super) fn into_changes(mut self) -> NativeCurrentChanges {
        self.commit_group();
        self.chunk
    }
}

#[cfg(test)]
mod tests;
