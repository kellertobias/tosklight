//! Typed sampling over the same captured static source lane as scalar Dynamics. It runs in
//! production under the semantic programming contract (contract 1, TL-552).
//! Missing Current is ordinary absence; an unresolved representation is a frame requirement.
#![allow(dead_code)]

use super::*;
use crate::runtime::dynamic_source_origins::DynamicFamilySourceProjection;
use crate::runtime::dynamic_source_origins::{
    DynamicSourceBinding, DynamicSourceOrigins, OriginsStore, bind_static_evidence_in, unbind_in,
};
#[cfg(test)]
use crate::runtime::dynamic_source_origins::{
    DynamicSourceOrigin, DynamicStaticSource, DynamicStaticSourceEntry,
};
use light_core::programming::{
    FamilyEditContext, IntentError, ProgrammingOwner, TransitionError, TransitionRequirement,
    VirtualColorAuthoringV1,
};
use light_dynamics::{
    DynamicPresetSourceBinding, DynamicSourceDependency, DynamicValue, DynamicValueAddress,
    DynamicValueSourceResolver, extract_compatible_dynamic_value,
};
use std::cell::RefCell;

mod current_native;
mod fork;
pub(super) mod hybrid;
mod memo;
mod static_rows;
use fork::{NativeCurrent, SourceTransaction};
use memo::{Log, Memo};
use static_rows::{KeptProjection, StaticFamilyRows};

/// Typed Current reads the immutable static baseline retained for final rendering, as raw
/// parameters: before the output-parameter masters, which the resolution these values feed
/// applies once later.
struct PreparedFamilySources<'a>(&'a light_engine::PreparedStaticFamilyFrame);

impl light_dynamics::ScalarSourceResolver for PreparedFamilySources<'_> {
    fn current(&self, target: FixtureId, attribute: &AttributeKey) -> Option<f32> {
        self.value(target, attribute)
            .and_then(AttributeValue::normalized)
    }

    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}

impl DynamicTickSource for PreparedFamilySources<'_> {
    fn value(&self, target: FixtureId, attribute: &AttributeKey) -> Option<&AttributeValue> {
        self.0.raw_value(target, attribute)
    }

    fn family_evidence(
        &self,
        target: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&Arc<light_engine::ContributionFamilyEvidence>> {
        self.0.contribution_family_evidence(target, attribute)
    }
}

/// A legacy native scalar (percentage or raw of the zoom channel) stored under the Zoom owner.
/// It is not a Zoom family: it is adopted from the measured native output or stays a requirement.
fn legacy_native_zoom(owner: ProgrammingOwner, base: &AttributeValue) -> bool {
    owner == ProgrammingOwner::Zoom
        && matches!(
            base,
            AttributeValue::Normalized(_)
                | AttributeValue::RawDmx(_)
                | AttributeValue::RawDmxExact(_)
        )
}

type CurrentAdoption<'a> = dyn Fn(FixtureId, &AttributeValue, &DynamicValueAddress) -> Result<AttributeValue, TransitionError>
    + 'a;
#[derive(Clone)]
struct ResolvedCurrent {
    value: DynamicValue,
    /// True only when extraction read the original captured representation. An adoption
    /// callback must supply its own transfer before it can claim equivalent source fields.
    compatible: bool,
    dependency: Option<DynamicSourceDependency>,
}
type CurrentResult = Result<Option<ResolvedCurrent>, TransitionError>;
type CurrentCache<'a> = Memo<'a, FixtureId, Vec<(DynamicValueAddress, CurrentResult)>>;
type CapturedFamilyCache<'a> = Memo<'a, (FixtureId, ProgrammingOwner), Option<AttributeValue>>;
type CurrentOccurrenceCache<'a> =
    Memo<'a, (FixtureId, ProgrammingOwner), Option<light_dynamics::DynamicSourceOccurrenceId>>;

/// Expected unresolved geometry/appearance stays attached to its exact target and address.
/// Other targets continue sampling. The frame publisher can expose this as passive quality data.
#[derive(Clone, Debug, PartialEq)]
struct CurrentResolutionRequirement {
    target: FixtureId,
    address: DynamicValueAddress,
    requirement: TransitionRequirement,
}

/// One instance per captured frame/branch. The adoption callback is bound to that same frame's
/// geometry and original source models. Never provide fitted DMX or another Dynamic as Current.
struct CapturedProgrammingSources<'a, S> {
    static_sources: &'a S,
    adopt: &'a CurrentAdoption<'a>,
    presets: Option<&'a dyn DynamicValueSourceResolver>,
    origins: Option<RefCell<SourceTransaction<'a>>>,
    captured_families: RefCell<CapturedFamilyCache<'a>>,
    current_occurrences: RefCell<CurrentOccurrenceCache<'a>>,
    current: RefCell<CurrentCache<'a>>,
    /// The first failure, sticky; at most one entry (a log only so a fork can layer it).
    failure: RefCell<Log<'a, TransitionError>>,
    requirements: RefCell<Log<'a, CurrentResolutionRequirement>>,
    native_current: Option<(
        &'a dyn light_dynamics::DynamicNativeModelResolver,
        NativeCurrent<'a>,
    )>,
}

impl<'a, S: DynamicTickSource> CapturedProgrammingSources<'a, S> {
    fn new(
        static_sources: &'a S,
        adopt: &'a CurrentAdoption<'a>,
        presets: Option<&'a dyn DynamicValueSourceResolver>,
    ) -> Self {
        Self {
            static_sources,
            adopt,
            presets,
            origins: None,
            captured_families: RefCell::default(),
            current_occurrences: RefCell::default(),
            current: RefCell::default(),
            failure: RefCell::default(),
            requirements: RefCell::default(),
            native_current: None,
        }
    }

    /// Read the original logical-owner family once in this captured frame/branch, including
    /// absence. Address-specific adoption and physical-copy fitting stay separate.
    /// Whether [`Self::captured_family_base`] has a value, capturing it exactly as that does,
    /// without copying it out (TL-639 round 4).
    fn has_captured_family_base(&self, target: FixtureId, owner: ProgrammingOwner) -> bool {
        self.captured_families
            .borrow_mut()
            .get_or_insert_with((target, owner), || {
                self.static_sources.value(target, owner.key_ref()).cloned()
            })
            .is_some()
    }

    fn captured_family_base(
        &self,
        target: FixtureId,
        owner: ProgrammingOwner,
    ) -> Option<AttributeValue> {
        self.captured_families
            .borrow_mut()
            .get_or_insert_with((target, owner), || {
                self.static_sources.value(target, owner.key_ref()).cloned()
            })
            .clone()
    }

    /// The gated hybrid path verifies complete Direct recipes when their original model is
    /// available, and requires that proof for native extraction/adopted Direct values. Opaque
    /// whole families may survive unavailable models. The caller brackets this branch's full
    /// frame (including error paths) with begin/finish.
    fn with_native_current_validation(
        mut self,
        models: &'a dyn light_dynamics::DynamicNativeModelResolver,
        cache: &'a RefCell<current_native::CurrentNativeVerificationCache>,
    ) -> Self {
        self.native_current = Some((models, NativeCurrent::Frame(cache)));
        self
    }

    fn verify_native_current(
        &self,
        target: FixtureId,
        base: &AttributeValue,
    ) -> Result<(), TransitionError> {
        if let Some((models, cache)) = &self.native_current {
            cache.verify(target, base, *models)?;
        }
        Ok(())
    }

    /// An opaque whole Direct family or portable adoption can survive an unavailable original
    /// model without claiming a proof. Available models must still reject malformed recipes.
    /// Actual native extraction and newly adopted Direct families use the strict check above.
    fn inspect_native_current(
        &self,
        target: FixtureId,
        base: &AttributeValue,
    ) -> Result<(), TransitionError> {
        match self.verify_native_current(target, base) {
            Err(TransitionError::Requires(TransitionRequirement::NativeColorModel)) => Ok(()),
            result => result,
        }
    }

    /// Use only inside the caller's atomic Dynamic/source frame transaction. This adapter can
    /// bind new captured sources but never publishes them or mutates older retained records.
    fn with_source_transaction(mut self, origins: &'a mut DynamicSourceOrigins) -> Self {
        self.origins = Some(RefCell::new(SourceTransaction::Frame(origins)));
        self
    }

    fn bind_authored_sources(
        &self,
        runtime: &light_dynamics::DynamicRuntime,
        inputs: &CapturedDynamicInputs<'_>,
        assignments: CapturedSourceAssignments<'_>,
    ) -> Result<(), IntentError> {
        if let Some(origins) = &self.origins {
            source_bindings::bind_captured_sources(
                origins.borrow_mut().frame()?,
                runtime,
                inputs,
                assignments,
            )?;
        }
        Ok(())
    }

    fn bind_current_occurrence(
        &self,
        target: FixtureId,
        owner: ProgrammingOwner,
    ) -> Result<Option<light_dynamics::DynamicSourceOccurrenceId>, IntentError> {
        let Some(origins) = &self.origins else {
            return Ok(None);
        };
        let binding = DynamicSourceBinding::StaticBaseline { target, owner };
        // Borrowed (TL-639 round 7): parallel workers bind here, and cloning the shared
        // canonical key would contend on its count.
        let key = owner.key_ref();
        let evidence = self
            .has_captured_family_base(target, owner)
            .then(|| self.static_sources.family_evidence(target, key))
            .flatten();
        let Some(evidence) = evidence.filter(|evidence| !evidence.entries().is_empty()) else {
            unbind_in(&mut *origins.borrow_mut(), &binding);
            return Ok(None);
        };
        bind_static_evidence_in(&mut *origins.borrow_mut(), binding, evidence).map(Some)
    }

    /// Call after sampling, family preparation and composition have made their final Current
    /// queries. Only this branch's used static bindings survive; held history keeps old records.
    /// Do not finalize at the sampler boundary because deferred Angle partners query later.
    fn finish_source_bindings(&self) -> Result<(), TransitionError> {
        self.check()?;
        if let Some(origins) = &self.origins {
            let used = self.current_occurrences.borrow();
            origins
                .borrow_mut()
                .frame()?
                .retain_bindings_by_key(|binding, occurrence_id| match *binding {
                    DynamicSourceBinding::Authored { .. } | DynamicSourceBinding::Fixed { .. } => {
                        true
                    }
                    DynamicSourceBinding::StaticBaseline { target, owner } => used
                        .get(&(target, owner))
                        .is_some_and(|id| *id == Some(occurrence_id)),
                });
        }
        Ok(())
    }

    fn resolve_current(&self, target: FixtureId, address: &DynamicValueAddress) -> CurrentResult {
        let Some(captured_base) = self.captured_family_base(target, address.owner()) else {
            return Ok(None);
        };
        let base = &captured_base;
        let context = FamilyEditContext {
            color_model: Some(&VirtualColorAuthoringV1),
            ..Default::default()
        };
        let legacy = legacy_native_zoom(address.owner(), base);
        if !legacy {
            DynamicValueAddress::whole_family(address.owner(), base)?;
            self.inspect_native_current(target, base)?;
        }
        if legacy {
            // Never reinterpreted: only the frame adapter's measured native output can adopt it.
        } else if let Some(value) = extract_compatible_dynamic_value(base, address, &context)? {
            if matches!(
                address.component,
                Some(light_core::programming::ProgrammingComponent::NativeColor(
                    _
                ))
            ) {
                self.verify_native_current(target, base)?;
            }
            return Ok(Some(ResolvedCurrent {
                value,
                compatible: true,
                dependency: None,
            }));
        }
        let adopted = (self.adopt)(target, base, address)?;
        self.verify_native_current(target, &adopted)?;
        extract_compatible_dynamic_value(&adopted, address, &context)?
            .map(|value| {
                Some(ResolvedCurrent {
                    value,
                    compatible: false,
                    dependency: None,
                })
            })
            .ok_or_else(|| {
                IntentError(
                    "Current adoption returned a different representation or function".into(),
                )
                .into()
            })
    }

    /// Preserve the distinction between an absent static value and a conversion requirement.
    /// Numeric typed sampling must observe this result before retaining a calculated value.
    fn checked_current(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<DynamicValue>, TransitionError> {
        let mut cache = self.current.borrow_mut();
        let known = cache
            .get(&target)
            .and_then(|entries| entries.iter().position(|(known, _)| known == address));
        let index = known.unwrap_or_else(|| {
            let result = self.resolve_current(target, address);
            let entries = cache.entry_or_default(target);
            entries.push((address.clone(), result));
            entries.len() - 1
        });
        let entries = cache.get(&target).expect("cached Current of this target");
        match &entries[index].1 {
            Ok(value) => Ok(value.as_ref().map(|resolved| resolved.value.clone())),
            Err(error) => {
                self.remember_current_error(target, address, error);
                Err(error.clone())
            }
        }
    }

    fn checked_current_family_base(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        let Some(captured_base) = self.captured_family_base(target, address.owner()) else {
            return Ok(None);
        };
        let base = &captured_base;
        if legacy_native_zoom(address.owner(), base) {
            // No Zoom family exists before adoption; keep the requirement scoped to this address.
            let error = TransitionError::Requires(TransitionRequirement::ZoomConvention);
            self.remember_current_error(target, address, &error);
            return Err(error);
        }
        if let Err(error) = DynamicValueAddress::whole_family(address.owner(), base) {
            let error = TransitionError::from(error);
            self.remember_failure(error.clone());
            return Err(error);
        }
        if let Err(error) = self.inspect_native_current(target, base) {
            self.remember_current_error(target, address, &error);
            return Err(error);
        }
        // Size keeps the original Angle/Target or Semantic/Direct family, before adoption.
        Ok(Some(base.clone()))
    }

    fn remember_current_error(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
        error: &TransitionError,
    ) {
        if let TransitionError::Requires(requirement) = error {
            let mut requirements = self.requirements.borrow_mut();
            if !requirements
                .iter()
                .any(|known| known.target == target && known.address == *address)
            {
                requirements.push(CurrentResolutionRequirement {
                    target,
                    address: address.clone(),
                    requirement: *requirement,
                });
            }
        } else {
            self.remember_failure(error.clone());
        }
    }

    /// The source trait expresses unavailable data with Option. Invalid values still reject
    /// the evaluation; expected frame requirements stay scoped to the affected address.
    fn check(&self) -> Result<(), TransitionError> {
        self.failure
            .borrow()
            .iter()
            .next()
            .cloned()
            .map_or(Ok(()), Err)
    }

    fn remember_failure(&self, failure: TransitionError) {
        let mut known = self.failure.borrow_mut();
        if known.iter().next().is_none() {
            known.push(failure);
        }
    }

    fn requirements(&self) -> Vec<CurrentResolutionRequirement> {
        self.requirements.borrow().iter().cloned().collect()
    }

    /// Compose once and consume both its value and source graph before reusing the workspace.
    /// The caller binds `context`/`frame` to this capture and keeps this whole operation inside
    /// the existing runtime/catalogue transaction. Expected frame requirements remain scoped
    /// to this family; final physical solving belongs to its resolver, never to an observer.
    fn compose_family<T>(
        &self,
        group: &light_dynamics::DynamicFamilySampleGroup,
        context: &light_dynamics::FamilyCompositionContext<'_>,
        frame: &dyn light_dynamics::WholeFamilyExpressionFrameResolver,
        scratch: &mut light_dynamics::RetainedFamilyCompositionScratch,
        observe: impl FnOnce(CapturedFamilyObservation<'_, '_, S>) -> Result<T, TransitionError>,
    ) -> Result<T, TransitionError> {
        let base = self
            .static_sources
            .value(group.target, group.owner.key_ref())
            .ok_or(TransitionError::Requires(
                TransitionRequirement::MaterializedEndpoints,
            ))?;
        let value = light_dynamics::compose_retained_dynamic_family_traced(
            group.owner,
            base,
            &group.samples,
            context,
            frame,
            scratch,
        )?;
        self.check()?;
        let output = observe(CapturedFamilyObservation {
            target: group.target,
            owner: group.owner,
            value: &value,
            trace: scratch.family_trace(),
            sources: self,
            kept: None,
            kept_observation: None,
        })?;
        self.check()?;
        Ok(output)
    }

    /// [`Self::compose_family`] for a static-only row (no samples): the composition and the
    /// source projection are kept while their inputs are equal (see `static_rows`).
    fn compose_static_family<T>(
        &self,
        group: &light_dynamics::DynamicFamilySampleGroup,
        context: &light_dynamics::FamilyCompositionContext<'_>,
        frame: &dyn light_dynamics::WholeFamilyExpressionFrameResolver,
        scratch: &mut light_dynamics::RetainedFamilyCompositionScratch,
        rows: &mut StaticFamilyRows,
        observe: impl FnOnce(CapturedFamilyObservation<'_, '_, S>) -> Result<T, TransitionError>,
    ) -> Result<T, TransitionError> {
        debug_assert!(group.samples.is_empty());
        let base = self
            .static_sources
            .value(group.target, group.owner.key_ref())
            .ok_or(TransitionError::Requires(
                TransitionRequirement::MaterializedEndpoints,
            ))?;
        let row = rows.row((group.target, group.owner), base, scratch, |scratch| {
            light_dynamics::compose_retained_dynamic_family_traced(
                group.owner,
                base,
                &group.samples,
                context,
                frame,
                scratch,
            )
        })?;
        self.check()?;
        let output = observe(CapturedFamilyObservation {
            target: group.target,
            owner: group.owner,
            value: &row.value,
            trace: &row.trace,
            sources: self,
            kept: Some(&row.projection),
            kept_observation: Some(&row.observed),
        })?;
        self.check()?;
        Ok(output)
    }
}

/// A loan into one completed family composition. Queries can retain immutable source records,
/// but cannot retain this graph past the callback or trigger another Current/adoption solve.
struct CapturedFamilyObservation<'a, 'sources, S> {
    target: FixtureId,
    owner: ProgrammingOwner,
    value: &'a AttributeValue,
    trace: &'a light_dynamics::FamilyTraceArena,
    sources: &'a CapturedProgrammingSources<'sources, S>,
    /// The kept projection of a static-only row, answered again while its inputs are equal.
    kept: Option<&'a RefCell<Option<KeptProjection>>>,
    /// The kept row's observer derivations (`static_rows::KeptObservation`).
    kept_observation: Option<&'a RefCell<static_rows::KeptObservation>>,
}

impl<S: DynamicTickSource> CapturedFamilyObservation<'_, '_, S> {
    fn value(&self) -> &AttributeValue {
        self.value
    }

    fn project_fields(
        &self,
        fields: &light_core::programming::ProgrammingFieldScope,
        projection: &mut DynamicFamilySourceProjection,
    ) -> Result<(), TransitionError> {
        if self.project_kept_fields(fields, projection)? {
            return Ok(());
        }
        fields.validate(self.owner)?;
        let query = self
            .trace
            .root()
            .and_then(|root| self.trace.query_fields_with_base(root, fields));
        let address = if query.as_ref().is_some_and(|query| {
            !query.base_fields.is_empty() || !query.base_dependency_fields.is_empty()
        }) {
            Some(DynamicValueAddress::whole_family(self.owner, self.value)?)
        } else {
            None
        };
        let baseline = address
            .as_ref()
            .and_then(|address| self.sources.current_family_occurrence(self.target, address));
        self.sources.check()?;
        if let Some(origins) = &self.sources.origins {
            projection.project(
                &*origins.borrow(),
                self.target,
                self.owner,
                query.as_ref(),
                baseline,
            )?;
        } else {
            projection.project(
                &DynamicSourceOrigins::default(),
                self.target,
                self.owner,
                query.as_ref(),
                baseline,
            )?;
        }
        // Only the baseline record (immutable per occurrence) is read when the query names no
        // other source; a named source's record could be retired between frames.
        if let Some(kept) = self.kept
            && query.as_ref().is_some_and(|query| query.sources.is_empty())
        {
            *kept.borrow_mut() = Some(KeptProjection {
                fields: fields.clone(),
                address,
                baseline,
                records: self
                    .sources
                    .origins
                    .as_ref()
                    .map(|origins| origins.borrow().records_identity()),
                result: projection.clone(),
            });
        }
        Ok(())
    }

    /// Answers from the kept projection when the fields and the freshly bound occurrence are
    /// those it was made from. The binding and the failure check run as on the full path.
    fn project_kept_fields(
        &self,
        fields: &light_core::programming::ProgrammingFieldScope,
        projection: &mut DynamicFamilySourceProjection,
    ) -> Result<bool, TransitionError> {
        let Some(kept) = self.kept else {
            return Ok(false);
        };
        let kept = kept.borrow();
        let Some(kept) = kept.as_ref().filter(|kept| kept.fields == *fields) else {
            return Ok(false);
        };
        let baseline = kept
            .address
            .as_ref()
            .and_then(|address| self.sources.current_family_occurrence(self.target, address));
        self.sources.check()?;
        let same_records = match (&self.sources.origins, &kept.records) {
            (Some(origins), Some(records)) => origins.borrow().has_records_identity(records),
            (None, None) => true,
            _ => false,
        };
        if baseline != kept.baseline || !same_records {
            return Ok(false);
        }
        projection.clone_from(&kept.result);
        Ok(true)
    }
}

impl<S: DynamicTickSource> DynamicValueSourceResolver for CapturedProgrammingSources<'_, S> {
    fn try_position_current_family(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        // This is an authoritative original-family read, never scalar adoption. Opt-in keeps
        // scalar-only source resolvers from performing an extra read or losing validation.
        self.checked_current_family_base(target, address)
    }

    fn try_current(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<DynamicValue>, TransitionError> {
        self.checked_current(target, address)
    }

    fn try_current_family_base(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.checked_current_family_base(target, address)
    }

    fn authored_occurrence(
        &self,
        instance_id: Uuid,
        controller_id: Uuid,
        target: FixtureId,
        lane_id: Uuid,
    ) -> Option<light_dynamics::DynamicSourceOccurrenceId> {
        OriginsStore::binding(
            &*self.origins.as_ref()?.borrow(),
            &DynamicSourceBinding::Authored {
                instance_id,
                controller_id,
                target,
                lane_id,
            },
        )
    }

    fn current_family_occurrence(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Option<light_dynamics::DynamicSourceOccurrenceId> {
        let key = (target, address.owner());
        let mut cache = self.current_occurrences.borrow_mut();
        *cache.get_or_insert_with(key, || {
            match self.bind_current_occurrence(target, address.owner()) {
                Ok(id) => id,
                Err(error) => {
                    self.remember_failure(error.into());
                    None
                }
            }
        })
    }

    fn current(&self, target: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.checked_current(target, address).ok().flatten()
    }

    fn current_dependency(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> DynamicSourceDependency {
        // Populate/read the same cached sample. Never run adoption a second time merely to
        // explain it, and never use an adopted component as the whole family's Size baseline.
        let _ = self.current(target, address);
        let occurrence = self.current_family_occurrence(target, address);
        let mut current = self.current.borrow_mut();
        // Read before writing: a fork copies a frame entry up only to change it.
        if let Some(dependency) = current
            .get(&target)
            .and_then(|entries| entries.iter().find(|(known, _)| known == address))
            .and_then(|(_, result)| result.as_ref().ok())
            .and_then(Option::as_ref)
            .and_then(|resolved| resolved.dependency.as_ref())
        {
            return dependency.clone();
        }
        let resolved = current
            .get_mut(&target)
            .and_then(|entries| entries.iter_mut().find(|(known, _)| known == address))
            .and_then(|(_, result)| result.as_mut().ok())
            .and_then(Option::as_mut);
        let Some(resolved) = resolved else {
            return DynamicSourceDependency::unknown(occurrence);
        };
        if let Some(dependency) = &resolved.dependency {
            return dependency.clone();
        }
        let dependency = if resolved.compatible {
            DynamicSourceDependency::compatible(occurrence, address)
        } else {
            DynamicSourceDependency::unknown(occurrence)
        };
        resolved.dependency = Some(dependency.clone());
        dependency
    }

    fn current_family_base(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Option<AttributeValue> {
        self.checked_current_family_base(target, address)
            .ok()
            .flatten()
    }

    fn preset(
        &self,
        source: &DynamicPresetSourceBinding,
        instance: Uuid,
        target: FixtureId,
    ) -> Option<DynamicValue> {
        self.presets
            .and_then(|presets| presets.preset(source, instance, target))
    }
}

fn sample_captured_programming_inputs(
    dynamics: &mut light_dynamics::DynamicRuntime,
    inputs: &CapturedDynamicInputs<'_>,
    programming_sources: &CapturedProgrammingSources<'_, impl DynamicTickSource>,
) -> Result<CapturedDynamicSample, TransitionError> {
    let result = sample_captured_dynamic_inputs_with(
        dynamics,
        inputs,
        |runtime, now, interval, assignments, _| {
            prepare_captured_preset_dependencies(runtime, inputs)?;
            programming_sources
                .bind_authored_sources(runtime, inputs, assignments)
                .map_err(|error| {
                    light_dynamics::DynamicRuntimeError::InvalidSample(error.to_string())
                })?;
            runtime.sample_all_programming_addressed(
                now,
                interval,
                inputs.speed_transports,
                programming_sources.static_sources,
                programming_sources,
                Some(inputs.addresser),
            )
        },
    );
    programming_sources.check()?;
    result.map_err(|error| IntentError(error.to_string()).into())
}

fn prepare_captured_programming_samples<'a>(
    sampled: &CapturedDynamicSample,
    sources: &CapturedProgrammingSources<'_, impl DynamicTickSource>,
    scratch: &'a mut light_dynamics::DynamicFamilyPreparationScratch,
) -> Result<light_dynamics::PreparedDynamicFamilySamples<'a>, TransitionError> {
    let result = light_dynamics::prepare_dynamic_family_samples(
        &sampled.samples,
        sources,
        Some(sampled.native_models.as_ref()),
        scratch,
    );
    // Held Current tokens can first request adoption during pair assembly. Validate after
    // both stages; target-scoped requirements remain available even when a pair is absent.
    sources.check()?;
    result
}

#[cfg(test)]
mod tests;
