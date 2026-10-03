//! The staged sampler and composer shared by every Live and Preload hybrid frame: pins one
//! cohort's sources, composes its family groups and projects them into the scalar token.

use super::super::super::family_inputs::CapturedFamilyInput;
use super::super::super::fixed_masks::PreparedFixedMask;
use super::super::static_rows::StaticFamilyRows;
use super::*;

/// Branch constructors provide both scalar Current and static tokens from the same immutable
/// lane. Keeping the staged sampler/composer here prevents Live and Preload semantics drifting.
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_hybrid_frame<T>(
    engine: &Engine,
    capture: &PreparedOutputFrame,
    frame_token: CapturedFrameToken,
    baseline_samples: &[ContributionBatch],
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    inputs: &CapturedDynamicInputs<'_>,
    scratch: &mut HybridFrameScratch,
    resolver: &impl HybridFrameResolver,
    presets: Option<&dyn DynamicValueSourceResolver>,
    observer: &mut impl HybridFrameObserver<T>,
    scalar_sources: &impl DynamicTickSource,
    static_lane: StaticLane<'_>,
    prepare_static: impl Fn(&[ContributionBatch]) -> PreparedStaticFamilyFrame,
) -> Result<PreparedHybridFrame<T>, DynamicRuntimeError> {
    let StaticLane {
        token: static_token,
        with_fixed_bases,
    } = static_lane;
    let baseline_samples = with_fixed_bases.unwrap_or(baseline_samples);
    begin_hybrid_frame(resolver, &frame_token, static_token)?;
    observer.begin_frame(&frame_token).map_err(invalid)?;
    let static_sources = PreparedFamilySources(static_token);
    let models = runtime.captured_native_color_models();
    let HybridFrameScratch {
        sampling,
        preparation,
        fixed,
        scalar,
        families,
        composition,
        position_batch_scratch,
        legacy_owners,
        native_current,
        static_rows,
        parallel,
    } = scratch;
    native_current.borrow_mut().begin_frame();
    let output_pool = engine.output_pool();
    let result = sample_captured_dynamic_inputs_with_context(
        runtime,
        inputs,
        |runtime, now, interval, assignments, controls| {
            let authored_origins = bind_cohort_sources(runtime, origins, inputs, assignments)?;
            let authored = source_bindings::AuthoredDynamicSources(&authored_origins);
            let (fixed, endpoint_controls) = compile_cohort_controls(
                inputs,
                &authored_origins,
                models.as_ref(),
                fixed,
                controls,
            )?;
            runtime.sample_all_programming_staged(
                now,
                interval,
                inputs.speed_transports,
                scalar_sources,
                &authored,
                Some(inputs.addresser),
                sampling,
                |legacy, deferred| {
                    let mut batches = project_hybrid_scalar_samples(
                        inputs,
                        legacy,
                        controls,
                        scalar_sources,
                        fixed,
                        scalar,
                    );
                    collect_legacy_owners(&batches, legacy_owners);
                    batches.extend_from_slice(baseline_samples);
                    let mut token = prepare_static(&batches);
                    let geometry = final_geometry(engine, capture, &mut token, &frame_token)?;
                    let frame = HybridFrameContext {
                        capture,
                        geometry: &geometry,
                        native_models: models.as_ref(),
                        token: &frame_token,
                        scalar: &token,
                    };
                    let adopt =
                        |target, original: &AttributeValue, address: &DynamicValueAddress| {
                            resolver.adopt(frame, target, original, address)
                        };
                    let typed = CapturedProgrammingSources::new(&static_sources, &adopt, presets)
                        .with_source_transaction(origins)
                        .with_native_current_validation(models.as_ref(), native_current);
                    let completed = deferred.complete(&typed)?;
                    super::parallel_preparation::prepare_families(
                        completed.samples(),
                        completed.requirements(),
                        &typed,
                        frame,
                        models.as_ref(),
                        preparation,
                        observer,
                        output_pool.as_deref(),
                    )
                    .map_err(invalid)?;
                    let prepared = preparation.prepared();
                    typed.check().map_err(invalid)?;
                    let control = |rank| endpoint_controls.control_for(rank);
                    let view = CohortView {
                        frame,
                        resolver,
                        typed: &typed,
                        static_sources: &static_sources,
                        static_token,
                        scalar_token: &token,
                        legacy_owners,
                        control: &control,
                    };
                    let (requirements, mut projections) = compose_family_cohort(
                        &view,
                        &prepared,
                        fixed,
                        families,
                        observer,
                        composition,
                        position_batch_scratch,
                        (static_rows, parallel, output_pool.as_deref()),
                    )?;
                    let samples = completed.samples().to_vec();
                    finish_cohort(observer, frame, typed, &mut projections, &requirements)?;
                    let sidecars = project_family_rows(&mut token, projections)?;
                    observer
                        .project_native(capture, &frame_token, &mut token, &sidecars)
                        .map_err(invalid)?;
                    Ok((
                        completed,
                        (samples, (token, geometry, sidecars, requirements)),
                    ))
                },
            )
        },
    );
    native_current.borrow_mut().finish_frame();
    finish_hybrid_frame(result?, resolver, models, origins, frame_token)
}

type StagedHybridOutput<T> = (
    CapturedDynamicSample,
    (
        PreparedStaticFamilyFrame,
        PreparedFrameGeometry,
        Vec<T>,
        Vec<HybridFamilyRequirement>,
    ),
);

/// The resolver holds the frame's requirements before the sampled bundle is completed and the
/// removed controllers retire.
fn finish_hybrid_frame<T>(
    (mut sampled, (token, geometry, family_sidecars, requirements)): StagedHybridOutput<T>,
    resolver: &impl HybridFrameResolver,
    models: Arc<dyn DynamicNativeModelResolver>,
    origins: &mut DynamicSourceOrigins,
    frame_token: CapturedFrameToken,
) -> Result<PreparedHybridFrame<T>, DynamicRuntimeError> {
    resolver
        .hold_frame(&frame_token, &requirements)
        .map_err(invalid)?;
    sampled.native_models = models;
    // The outer staged call has now verified its own single-use proof. Retirement cannot run
    // while a deferred cohort could still add a held source. Publishing/rendering stays outside.
    source_bindings::retire_removed_controllers(
        origins,
        &sampled.before_runtime,
        &sampled.after_runtime,
    );
    Ok(PreparedHybridFrame {
        token,
        frame_token,
        geometry,
        sampled,
        family_sidecars,
        requirements,
    })
}

/// Completes the cohort while its scalar baseline is still unprojected, then closes the typed
/// sources' binding transaction.
fn finish_cohort<T>(
    observer: &mut impl HybridFrameObserver<T>,
    frame: HybridFrameContext<'_>,
    typed: CapturedProgrammingSources<'_, PreparedFamilySources<'_>>,
    projections: &mut Vec<OwnedHybridProjection<T>>,
    requirements: &[HybridFamilyRequirement],
) -> Result<(), DynamicRuntimeError> {
    observer
        .finish(frame, projections, requirements)
        .map_err(invalid)?;
    typed.finish_source_bindings().map_err(invalid)?;
    drop(typed);
    Ok(())
}

/// The resolver's lane rejects a foreign, stale or already accepted token before anything is
/// sampled, and the static lane must be the token's own capture/branch.
fn begin_hybrid_frame(
    resolver: &impl HybridFrameResolver,
    frame_token: &CapturedFrameToken,
    static_token: &PreparedStaticFamilyFrame,
) -> Result<(), DynamicRuntimeError> {
    if !frame_token.matches_static_frame(static_token) {
        return Err(invalid(
            "hybrid frame token does not belong to its static lane",
        ));
    }
    resolver.begin_frame(frame_token).map_err(invalid)
}

/// Final scalar-resolved geometry, checked against the frame token's generation and sample.
fn final_geometry(
    engine: &Engine,
    capture: &PreparedOutputFrame,
    token: &mut PreparedStaticFamilyFrame,
    frame_token: &CapturedFrameToken,
) -> Result<PreparedFrameGeometry, DynamicRuntimeError> {
    let geometry = engine
        .observe_static_family_geometry(capture, token)
        .map_err(invalid)?;
    if !frame_token.matches_geometry(&geometry) {
        return Err(invalid(
            "final geometry does not belong to the hybrid frame token",
        ));
    }
    Ok(geometry)
}

fn collect_legacy_owners(
    batches: &[ContributionBatch],
    legacy_owners: &mut FxHashSet<(FixtureId, ProgrammingOwner)>,
) {
    legacy_owners.clear();
    for sample in batches.iter().flat_map(|batch| batch.samples()) {
        for owner in [
            ProgrammingOwner::Position,
            ProgrammingOwner::Color,
            ProgrammingOwner::Focus,
            ProgrammingOwner::Zoom,
        ] {
            if sample.value().attribute == owner.key()
                || independent_programming_component(&sample.value().attribute, owner)
            {
                legacy_owners.insert((sample.value().fixture_id, owner));
            }
        }
    }
}

/// The LegacyOwnerOverlap and ScalarBaselineChanged guards: keep the scalar-resolved owner
/// until safe family adoption with imported scalar rank/source data exists.
pub(super) fn scalar_owner_guard(
    legacy_owners: &FxHashSet<(FixtureId, ProgrammingOwner)>,
    original: &PreparedStaticFamilyFrame,
    scalar: &PreparedStaticFamilyFrame,
    target: FixtureId,
    owner: ProgrammingOwner,
) -> Option<HybridFamilyRequirementReason> {
    if legacy_owners.contains(&(target, owner)) {
        Some(HybridFamilyRequirementReason::LegacyOwnerOverlap)
    } else if !same_static_baseline(original, scalar, target, owner) {
        Some(HybridFamilyRequirementReason::ScalarBaselineChanged)
    } else {
        None
    }
}

/// Pins the cohort's preset dependencies and binds its captured Dynamic and fixed sources
/// into the frame's source transaction before anything is sampled.
fn bind_cohort_sources(
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    inputs: &CapturedDynamicInputs<'_>,
    assignments: CapturedSourceAssignments<'_>,
) -> Result<DynamicSourceOrigins, DynamicRuntimeError> {
    prepare_captured_preset_dependencies(runtime, inputs)?;
    source_bindings::bind_captured_sources(origins, runtime, inputs, assignments)
        .map_err(invalid)?;
    origins
        .reconcile_captured_programming_fixed_sources(
            inputs.programmer_values,
            inputs.programmer_rows,
            inputs.cue_values,
        )
        .map_err(invalid)?;
    // Authored lookups occur during pinning, before Current bindings can be mutated.
    // This shares immutable catalogue storage; it never clones the runtime/history.
    Ok(origins.clone())
}

/// Compiles the cohort's captured fixed masks against the authored sources and validates its
/// captured endpoint output controls.
fn compile_cohort_controls<'s, 'c>(
    inputs: &CapturedDynamicInputs<'_>,
    authored_origins: &DynamicSourceOrigins,
    models: &dyn DynamicNativeModelResolver,
    fixed: &'s mut FixedMaskCompilationScratch,
    controls: CapturedDynamicOutputControls<'c>,
) -> Result<(&'s [PreparedFixedMask], CapturedFamilyEndpointControls<'c>), DynamicRuntimeError> {
    let fixed = compile_captured_fixed_masks(
        &CapturedFixedMaskRows::from_inputs(inputs),
        Some(authored_origins),
        Some(models),
        fixed,
    )
    .map_err(invalid)?;
    let endpoint_controls = CapturedFamilyEndpointControls::new(controls).map_err(invalid)?;
    Ok((fixed, endpoint_controls))
}

/// The original pre-Freeze static lane of one hybrid frame, and the extended baseline when
/// fixed bases were missing. Typed Current and whole Size always read this source; the later
/// scalar-resolved token is an output underlay and geometry input, never Current feedback.
pub(super) struct StaticLane<'a> {
    pub token: &'a PreparedStaticFamilyFrame,
    pub with_fixed_bases: Option<&'a [ContributionBatch]>,
}

/// Prepares the static lane, then appends the fixed bases it is missing and re-prepares it over
/// them. Returns the extended baseline only when bases were missing.
pub(super) fn prepare_static_with_fixed_bases(
    engine: &Engine,
    runtime: &DynamicRuntime,
    inputs: &CapturedDynamicInputs<'_>,
    baseline_samples: &[ContributionBatch],
    prepare_static: &impl Fn(&[ContributionBatch]) -> PreparedStaticFamilyFrame,
) -> (PreparedStaticFamilyFrame, Option<Vec<ContributionBatch>>) {
    let mut static_token = prepare_static(baseline_samples);
    let with_fixed_bases = match fixed_bases::missing(engine, runtime, inputs, &static_token) {
        Some(bases) => {
            let with_fixed_bases = [baseline_samples, &[bases]].concat();
            static_token = prepare_static(&with_fixed_bases);
            Some(with_fixed_bases)
        }
        None => None,
    };
    (static_token, with_fixed_bases)
}

/// The endpoint output control of a sample rank; shared by a cohort's parallel workers.
pub(super) type EndpointControl<'a> = dyn Fn(light_dynamics::FamilySampleRank) -> light_dynamics::FamilyEndpointOutputControl
    + Sync
    + 'a;

/// The pinned, immutable view one family cohort composes against.
pub(super) struct CohortView<'a, 't, S, R> {
    pub frame: HybridFrameContext<'a>,
    pub resolver: &'a R,
    pub typed: &'a CapturedProgrammingSources<'t, S>,
    pub static_sources: &'a PreparedFamilySources<'a>,
    pub static_token: &'a PreparedStaticFamilyFrame,
    pub scalar_token: &'a PreparedStaticFamilyFrame,
    pub legacy_owners: &'a FxHashSet<(FixtureId, ProgrammingOwner)>,
    pub control: &'a EndpointControl<'a>,
}

/// Composes every family group of one pinned cohort: the Position batch first, then each
/// remaining owner. Returns the requirements that held owners back and the composed rows.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn compose_family_cohort<T, R: HybridFrameResolver>(
    view: &CohortView<'_, '_, PreparedFamilySources<'_>, R>,
    prepared: &light_dynamics::PreparedDynamicFamilySamples<'_>,
    fixed: &[PreparedFixedMask],
    families: &mut CapturedFamilyInputScratch,
    observer: &mut impl HybridFrameObserver<T>,
    composition: &mut RetainedFamilyCompositionScratch,
    position_batch_scratch: &mut Vec<RetainedFamilyCompositionScratch>,
    (static_rows, parallel, pool): (
        &mut StaticFamilyRows,
        &mut Vec<RetainedFamilyCompositionScratch>,
        Option<&light_engine::parallel::OutputPool>,
    ),
) -> Result<(Vec<HybridFamilyRequirement>, Vec<OwnedHybridProjection<T>>), DynamicRuntimeError> {
    let static_targets = observer
        .static_program_targets(view.frame, view.static_token)
        .map_err(invalid)?;
    assemble_captured_family_inputs(prepared, fixed, families, pool);
    let (groups, static_only) = families.with_static_targets(&static_targets);
    // TL-596: membership is asked once per group and batch row; a slice scan made the cohort
    // quadratic in its static-only targets (thousands at full-rig size).
    let static_only = static_only.into_iter().collect::<StaticOnlyTargets>();
    let static_only = &static_only;
    // Sized once (TL-639 round 4): a cohort projects at most one row per group, and growing a
    // list of thousands of rows by doubling allocated and copied it about a dozen times.
    let (mut requirements, mut projections) = (Vec::new(), Vec::with_capacity(groups.len()));
    let protected_current = protected_current_targets(view, groups);
    observer
        .prepare_current(view.frame, view.static_token, &protected_current)
        .map_err(invalid)?;
    let programs = groups
        .iter()
        .filter_map(|entry| {
            let group = &entry.group;
            view.static_sources
                .value(group.target, group.owner.key_ref())
                .map(|base| HybridFamilyProgram {
                    target: group.target,
                    owner: group.owner,
                    base,
                    samples: &group.samples,
                    has_requirements: !entry.requirements.is_empty(),
                    frame: view.frame,
                })
        })
        .collect::<Vec<_>>();
    observer
        .prepare_programs(view.frame, &programs)
        .map_err(invalid)?;
    let eligible_position_groups = eligible_position_groups(view, groups, static_only);
    let handled_position = compose_position_batch(
        view,
        &eligible_position_groups,
        static_only,
        observer,
        composition,
        position_batch_scratch,
        &mut projections,
        &mut requirements,
    )?;
    super::parallel_groups::compose_owner_groups(
        view,
        groups,
        static_only,
        &handled_position,
        observer,
        (composition, static_rows, parallel),
        &mut projections,
        &mut requirements,
        pool,
    )?;
    static_rows.finish_cohort();
    requirements.extend(view.typed.requirements().into_iter().map(|required| {
        HybridFamilyRequirement {
            target: required.target,
            owner: required.address.owner(),
            reason: HybridFamilyRequirementReason::Current {
                address: required.address,
                requirement: required.requirement,
            },
        }
    }));
    Ok((requirements, projections))
}

/// Static-only `(target, owner)` rows of one cohort.
pub(super) type StaticOnlyTargets = FxHashSet<(FixtureId, ProgrammingOwner)>;

/// Targets whose Position Current must stay protected: scalar Position owners, and Position
/// groups held by a scalar guard, a Fixed mask, or a missing static base.
fn protected_current_targets<S, R>(
    view: &CohortView<'_, '_, S, R>,
    groups: &[CapturedFamilyInput],
) -> Vec<FixtureId> {
    let mut protected_current = view
        .legacy_owners
        .iter()
        .filter_map(|(target, owner)| (*owner == ProgrammingOwner::Position).then_some(*target))
        .collect::<Vec<_>>();
    for entry in groups.iter() {
        let group = &entry.group;
        if group.owner == ProgrammingOwner::Position
            && (scalar_owner_guard(
                view.legacy_owners,
                view.static_token,
                view.scalar_token,
                group.target,
                group.owner,
            )
            .is_some()
                || entry.requirements.iter().any(|requirement| {
                    matches!(requirement, CapturedFamilyRequirement::Fixed { .. })
                })
                || view
                    .static_sources
                    .value(group.target, group.owner.key_ref())
                    .is_none())
        {
            protected_current.push(group.target);
        }
    }
    protected_current
}

/// Position groups the batch composer may own: sampled or static-only, with a static base, no
/// scalar guard and no Fixed mask.
fn eligible_position_groups<'g, S, R>(
    view: &CohortView<'_, '_, S, R>,
    groups: &'g [CapturedFamilyInput],
    static_only: &StaticOnlyTargets,
) -> Vec<&'g light_dynamics::DynamicFamilySampleGroup> {
    groups
        .iter()
        .filter_map(|entry| {
            let group = &entry.group;
            (group.owner == ProgrammingOwner::Position
                && (!group.samples.is_empty()
                    || static_only.contains(&(group.target, group.owner)))
                && view
                    .static_sources
                    .value(group.target, group.owner.key_ref())
                    .is_some()
                && scalar_owner_guard(
                    view.legacy_owners,
                    view.static_token,
                    view.scalar_token,
                    group.target,
                    group.owner,
                )
                .is_none()
                && !entry.requirements.iter().any(|requirement| {
                    matches!(requirement, CapturedFamilyRequirement::Fixed { .. })
                }))
            .then_some(group)
        })
        .collect::<Vec<_>>()
}

/// Lets the observer compose the eligible Position groups as one batch. Returns the targets it
/// handled, which the ordinary per-owner loop then skips.
#[allow(clippy::too_many_arguments)]
fn compose_position_batch<T, S: DynamicTickSource, R>(
    view: &CohortView<'_, '_, S, R>,
    eligible_position_groups: &[&light_dynamics::DynamicFamilySampleGroup],
    static_only: &StaticOnlyTargets,
    observer: &mut impl HybridFrameObserver<T>,
    composition: &mut RetainedFamilyCompositionScratch,
    position_batch_scratch: &mut Vec<RetainedFamilyCompositionScratch>,
    projections: &mut Vec<OwnedHybridProjection<T>>,
    requirements: &mut Vec<HybridFamilyRequirement>,
) -> Result<FxHashSet<FixtureId>, DynamicRuntimeError> {
    let mut batch_composer = position_batch::CapturedHybridPositionBatchComposer::new_with_pool(
        view.typed,
        eligible_position_groups,
        view.frame,
        view.static_token,
        view.control,
        std::mem::take(composition),
        std::mem::take(position_batch_scratch),
    );
    let batch = observer
        .compose_position_batch(view.frame, &mut batch_composer)
        .map_err(invalid)?;
    let handled_position = match batch {
        Some(batch) => {
            position_batch::validate_result(&batch, eligible_position_groups).map_err(invalid)?;
            for mut row in batch.projections {
                if static_only.contains(&(row.target, row.owner)) {
                    row.metadata = FamilyProjectionMetadata {
                        changed_at: view
                            .static_token
                            .changed_at(row.target, row.owner.key_ref()),
                        evidence: light_engine::FamilyProjectionEvidence::PreserveBaseline,
                        master: light_engine::FamilyProjectionMaster::PreserveBaseline,
                    };
                }
                projections.push(row);
            }
            requirements.extend(batch.requirements);
            batch.handled.into_iter().collect::<FxHashSet<_>>()
        }
        None => FxHashSet::default(),
    };
    let (local, pool) = batch_composer.into_workspaces();
    *composition = local;
    *position_batch_scratch = pool;
    Ok(handled_position)
}

/// Composes one ordinary owner group, or records the requirement that holds it back.
#[allow(clippy::too_many_arguments)]
pub(super) fn compose_owner_group<T, S: DynamicTickSource, R: HybridFrameResolver>(
    view: &CohortView<'_, '_, S, R>,
    entry: &CapturedFamilyInput,
    static_only: &StaticOnlyTargets,
    handled_position: &FxHashSet<FixtureId>,
    observer: &mut impl HybridFrameObserver<T>,
    (composition, static_rows): (&mut RetainedFamilyCompositionScratch, &mut StaticFamilyRows),
    projections: &mut Vec<OwnedHybridProjection<T>>,
    requirements: &mut Vec<HybridFamilyRequirement>,
) -> Result<(), DynamicRuntimeError> {
    let group = &entry.group;
    requirements.extend(entry.requirements.iter().cloned().map(|requirement| {
        HybridFamilyRequirement {
            target: group.target,
            owner: group.owner,
            reason: HybridFamilyRequirementReason::Input(requirement),
        }
    }));
    if let Some(reason) = scalar_owner_guard(
        view.legacy_owners,
        view.static_token,
        view.scalar_token,
        group.target,
        group.owner,
    ) {
        requirements.push(HybridFamilyRequirement {
            target: group.target,
            owner: group.owner,
            reason,
        });
        return Ok(());
    }
    if group.owner == ProgrammingOwner::Position && handled_position.contains(&group.target) {
        return Ok(());
    }
    let static_only = static_only.contains(&(group.target, group.owner));
    if (group.samples.is_empty() && !static_only)
        || entry
            .requirements
            .iter()
            .any(|requirement| matches!(requirement, CapturedFamilyRequirement::Fixed { .. }))
    {
        return Ok(());
    }
    let adoption = |original: &AttributeValue, address: &DynamicValueAddress| {
        view.resolver
            .adopt(view.frame, group.target, original, address)
    };
    let frame_resolver = BoundFrameResolver {
        resolver: view.resolver,
        frame: view.frame,
        target: group.target,
    };
    // A static-only row keeps its composition while its base is equal (`static_rows`). Position
    // is excluded: its base composition can adopt through the frame's geometry.
    let keeps_row =
        static_only && group.samples.is_empty() && group.owner != ProgrammingOwner::Position;
    let mut composer = CapturedHybridProgramComposer {
        typed: view.typed,
        group,
        frame: view.frame,
        baseline: view.static_token,
        control: view.control,
        scratch: composition,
        static_rows: keeps_row.then_some(static_rows),
    };
    let base = view
        .static_sources
        .value(group.target, group.owner.key_ref());
    let deferred = match base {
        Some(base) => observer.compose_program(
            HybridFamilyProgram {
                target: group.target,
                owner: group.owner,
                base,
                samples: &group.samples,
                has_requirements: !entry.requirements.is_empty(),
                frame: view.frame,
            },
            &mut composer,
        ),
        None => Err(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints,
        )),
    };
    let output = match deferred {
        Ok(Some(row)) => Ok(row),
        Ok(None) => composer.compose(&frame_resolver, &adoption, &mut |observation| {
            observer.observe(observation)
        }),
        Err(error) => Err(error),
    };
    match output {
        Ok(mut row) => {
            if static_only {
                let metadata = FamilyProjectionMetadata {
                    changed_at: view
                        .static_token
                        .changed_at(group.target, group.owner.key_ref()),
                    evidence: light_engine::FamilyProjectionEvidence::PreserveBaseline,
                    master: light_engine::FamilyProjectionMaster::PreserveBaseline,
                };
                row.metadata = metadata;
            }
            projections.push(row);
        }
        Err(TransitionError::Requires(requirement)) => requirements.push(HybridFamilyRequirement {
            target: group.target,
            owner: group.owner,
            reason: HybridFamilyRequirementReason::Composition(requirement),
        }),
        Err(error) => return Err(invalid(error)),
    }
    Ok(())
}

/// Projects each composed row into the scalar-resolved token, keeping its sidecar in order.
fn project_family_rows<T>(
    token: &mut PreparedStaticFamilyFrame,
    projections: Vec<OwnedHybridProjection<T>>,
) -> Result<Vec<T>, DynamicRuntimeError> {
    let mut sidecars = Vec::with_capacity(projections.len());
    token.reserve_family_projections(projections.len());
    for OwnedHybridProjection {
        target,
        owner,
        value,
        metadata,
        sidecar,
    } in projections
    {
        token
            .project_family(target, owner, value, metadata)
            .map_err(invalid)?;
        sidecars.push(sidecar);
    }
    Ok(sidecars)
}
