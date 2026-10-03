//! Compile captured fixed rows without reading Current, live controls or destination profiles.
//! The result retains every typed mask, including passive native requirements. A later composer
//! must account for a visible unresolved mask; selecting only `Ready` rows is not a valid output
//! policy. The hybrid frame (`programming_projection::hybrid`) is that composer; a whole mask
//! with no static underlay gets its own family as the frame-local baseline (`fixed_bases`).
#![allow(dead_code)]

use super::{CapturedDynamicInputs, authored_activation_mix};
use crate::runtime::dynamic_source_origins::{
    DynamicFixedSource, DynamicFixedStamp, DynamicProgrammerSourceLane, DynamicSourceBinding,
    DynamicSourceOrigin, DynamicSourceOrigins, captured_programming_fixed_mask,
};
use chrono::{DateTime, Utc};
use light_core::{
    FixtureId, NativeColorIdentity,
    programming::{
        FamilyEditContext, IntentError, NativeColorEditModel, ProgrammingComponent,
        ProgrammingOwner, VirtualColorAuthoringV1,
    },
};
use light_dynamics::{
    DynamicAddressValue, DynamicFamilyRepresentation, DynamicNativeModelResolver,
    DynamicSemanticValue, DynamicSourceOccurrenceId, FamilyFixedSampleSource, FamilySample,
    FamilySampleIdentity, FamilySampleRank, FamilyTraceFootprint, FamilyTraceRole,
    FamilyTraceSource, NativeColorModelCapability, NativeColorModelUnavailable,
    NativeColorUnavailableReason, ProgrammingFamilyFixAt,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

type Model = Arc<dyn NativeColorEditModel + Send + Sync>;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum FixedCaptureAddress {
    Programmer {
        source: FamilyFixedSampleSource,
        programmer_id: Uuid,
        lane: DynamicProgrammerSourceLane,
        target: FixtureId,
        owner: ProgrammingOwner,
        component: Option<ProgrammingComponent>,
    },
    Cue(DynamicSourceBinding),
}

/// The original captured arrays are the semantic input. Source evidence is optional and cannot
/// filter the input down to only rows that happen to have a catalogue occurrence.
pub(super) struct CapturedFixedMaskRows<'a> {
    pub now: DateTime<Utc>,
    pub programmer_values: &'a [(Uuid, i16, DynamicAddressValue)],
    pub programmer_rows: Option<&'a [light_engine::CapturedDynamicProgrammerRow]>,
    pub extra_programmer_values: &'a [(Uuid, i16, DynamicAddressValue)],
    pub cue_values: &'a [light_playback::ActiveCueDynamicValue],
}
impl<'a> CapturedFixedMaskRows<'a> {
    pub fn from_inputs(inputs: &'a CapturedDynamicInputs<'_>) -> Self {
        Self {
            now: inputs.now,
            programmer_values: inputs.programmer_values,
            programmer_rows: inputs.programmer_rows,
            extra_programmer_values: inputs.extra_programmer_values,
            cue_values: inputs.cue_values,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum FixedMaskRequirement {
    NativeColorModelUnavailable(NativeColorModelUnavailable),
}

#[derive(Clone)]
pub(super) enum CompiledFixedMaskState {
    Ready(FamilySample),
    Requires(FixedMaskRequirement),
}

pub(super) struct PreparedFixedMask {
    pub target: FixtureId,
    pub owner: ProgrammingOwner,
    pub rank: FamilySampleRank,
    pub stamp: DynamicFixedStamp,
    pub occurrence: Option<DynamicSourceOccurrenceId>,
    pub original: DynamicSemanticValue,
    pub mask: ProgrammingFamilyFixAt,
    pub output_enabled: bool,
    /// Authored delay/fade only. Cue disabled state suppresses sample participation separately.
    /// Sequence masters never become activation; all these owners are non-intensity attributes.
    pub authored_activation_mix: f32,
    pub state: CompiledFixedMaskState,
}
impl PreparedFixedMask {
    pub fn participates(&self) -> bool {
        self.output_enabled && self.authored_activation_mix > 0.0
    }
}

struct CachedFixedMask {
    mask: ProgrammingFamilyFixAt,
    model: Option<Model>,
    state: CompiledFixedMaskState,
}

/// One branch-local reusable scratch. Entries are bounded by currently captured rows. The
/// complete mask and verified original model must match; ordering/evidence/timing are refreshed.
#[derive(Default)]
pub(super) struct FixedMaskCompilationScratch {
    rows: Vec<PreparedFixedMask>,
    cache: HashMap<FamilySampleIdentity, CachedFixedMask>,
    models: Vec<(NativeColorIdentity, NativeColorModelCapability)>,
}
impl FixedMaskCompilationScratch {
    pub fn clear(&mut self) {
        self.rows.clear();
        self.cache.clear();
        self.models.clear();
    }
}

/// Does not mutate the source catalogue or acquire engine/runtime locks. The caller supplies
/// the matching immutable catalogue and native resolver from this frame transaction. A missing
/// occurrence stays unknown; a mismatching supplied occurrence is invalid capture evidence.
pub(super) fn compile_captured_fixed_masks<'a>(
    inputs: &CapturedFixedMaskRows<'_>,
    origins: Option<&DynamicSourceOrigins>,
    native_models: Option<&dyn DynamicNativeModelResolver>,
    scratch: &'a mut FixedMaskCompilationScratch,
) -> Result<&'a [PreparedFixedMask], IntentError> {
    scratch.rows.clear();
    scratch.models.clear();
    let result = compile_rows(inputs, origins, native_models, scratch);
    if let Err(error) = result {
        // An invalid frame exposes no partial prepared result. Valid immutable compiled entries
        // may be reused after retry, but never extend cache lifetime through repeated failures.
        scratch.clear();
        return Err(error);
    }
    let used = scratch
        .rows
        .iter()
        .map(|row| row.rank.identity)
        .collect::<HashSet<_>>();
    scratch.cache.retain(|identity, _| used.contains(identity));
    Ok(&scratch.rows)
}

fn compile_rows(
    inputs: &CapturedFixedMaskRows<'_>,
    origins: Option<&DynamicSourceOrigins>,
    native_models: Option<&dyn DynamicNativeModelResolver>,
    scratch: &mut FixedMaskCompilationScratch,
) -> Result<(), IntentError> {
    let now = u64::try_from(inputs.now.timestamp_millis()).unwrap_or_default();
    let mut captured_addresses = HashSet::new();
    for (source, rows) in [
        (
            FamilyFixedSampleSource::Programmer,
            inputs.programmer_values,
        ),
        (
            FamilyFixedSampleSource::ExtraProgrammer,
            inputs.extra_programmer_values,
        ),
    ] {
        for (row_index, (programmer_id, priority, row)) in rows.iter().enumerate() {
            let Some(mask) = captured_programming_fixed_mask(&row.attribute, &row.value)? else {
                continue;
            };
            if programmer_id.is_nil() {
                return Err(IntentError(
                    "fixed Programmer identity cannot be nil".into(),
                ));
            }
            let stamp = DynamicFixedStamp::Programmer {
                changed_at_millis: row.changed_at_millis,
                programmer_order: row.programmer_order,
            };
            let binding = if source == FamilyFixedSampleSource::Programmer {
                inputs
                    .programmer_rows
                    .and_then(|rows| rows.get(row_index))
                    .map(|sidecar| programmer_binding(*programmer_id, row, &mask, stamp, sidecar))
                    .transpose()?
            } else {
                // Extra rows have no captured lane sidecar. Never guess Live/Preload identity.
                None
            };
            let lane = match binding {
                Some(DynamicSourceBinding::Fixed {
                    source: DynamicFixedSource::Programmer { lane, .. },
                    ..
                }) => Some(lane),
                _ => None,
            };
            // Without the sidecar, matching fields cannot prove one source lane: old
            // captures may contain both Live and Preload. Preserve their actual row order.
            if let Some(lane) = lane {
                if !captured_addresses.insert(FixedCaptureAddress::Programmer {
                    source,
                    programmer_id: *programmer_id,
                    lane,
                    target: row.fixture_id,
                    owner: mask.address.owner(),
                    component: mask.address.component,
                }) {
                    return Err(IntentError(
                        "duplicate captured fixed Programmer address".into(),
                    ));
                }
            }
            let occurrence = matching_occurrence(origins, binding, stamp, *priority, &row.value)?;
            push_row(
                scratch,
                native_models,
                row.fixture_id,
                mask,
                row.value.clone(),
                stamp,
                FamilySampleRank {
                    priority: *priority,
                    changed_at_millis: row.changed_at_millis,
                    changed_at_submillis_nanos: 0,
                    stable_order: u128::from(row.programmer_order),
                    identity: FamilySampleIdentity::Fixed { source, row_index },
                },
                occurrence,
                true,
                now,
            )?;
        }
    }
    compile_cue_rows(
        inputs,
        origins,
        native_models,
        scratch,
        &mut captured_addresses,
        now,
    )
}

/// Compiles each fixed Cue row after checking its capture agrees with its source key and is
/// the only row at its captured address.
fn compile_cue_rows(
    inputs: &CapturedFixedMaskRows<'_>,
    origins: Option<&DynamicSourceOrigins>,
    native_models: Option<&dyn DynamicNativeModelResolver>,
    scratch: &mut FixedMaskCompilationScratch,
    captured_addresses: &mut HashSet<FixedCaptureAddress>,
    now: u64,
) -> Result<(), IntentError> {
    for (row_index, row) in inputs.cue_values.iter().enumerate() {
        let Some(mask) = captured_programming_fixed_mask(&row.attribute, &row.value)? else {
            continue;
        };
        if row.source_key.source() != row.source
            || row.cue_list_id != row.source.cue_list_id
            || row.changed_at_millis
                != u64::try_from(row.changed_at.timestamp_millis()).unwrap_or_default()
        {
            return Err(IntentError(
                "fixed Cue source or timestamp disagrees with its capture".into(),
            ));
        }
        let stamp = DynamicFixedStamp::Cue {
            authored_cue_id: row.authored_cue_id,
            changed_at: row.changed_at,
            transition_ordinal: row.transition_ordinal,
        };
        let binding = DynamicSourceBinding::Fixed {
            source: DynamicFixedSource::Cue {
                source: row.source.into(),
                temporary_kind: match row.source_key {
                    light_playback::CueDynamicSourceKey::Normal { .. } => None,
                    light_playback::CueDynamicSourceKey::Temporary { kind, .. } => {
                        Some(kind.into())
                    }
                },
            },
            target: row.fixture_id,
            owner: mask.address.owner(),
            component: mask.address.component,
        };
        let DynamicSourceBinding::Fixed { source, .. } = binding else {
            unreachable!()
        };
        source.validate()?;
        if row.authored_cue_id.is_nil() {
            return Err(IntentError(
                "fixed authored Cue identity cannot be nil".into(),
            ));
        }
        if !captured_addresses.insert(FixedCaptureAddress::Cue(binding)) {
            return Err(IntentError("duplicate captured fixed Cue address".into()));
        }
        let occurrence =
            matching_occurrence(origins, Some(binding), stamp, row.priority, &row.value)?;
        push_row(
            scratch,
            native_models,
            row.fixture_id,
            mask,
            row.value.clone(),
            stamp,
            FamilySampleRank {
                priority: row.priority,
                changed_at_millis: row.changed_at_millis,
                changed_at_submillis_nanos: row.changed_at.timestamp_subsec_nanos() % 1_000_000,
                stable_order: u128::from(row.transition_ordinal),
                identity: FamilySampleIdentity::Fixed {
                    source: FamilyFixedSampleSource::Cue,
                    row_index,
                },
            },
            occurrence,
            row.output_enabled,
            now,
        )?;
    }
    Ok(())
}

fn programmer_binding(
    programmer_id: Uuid,
    row: &DynamicAddressValue,
    mask: &ProgrammingFamilyFixAt,
    stamp: DynamicFixedStamp,
    sidecar: &light_engine::CapturedDynamicProgrammerRow,
) -> Result<DynamicSourceBinding, IntentError> {
    let (lane, expected_source) = match sidecar.lane {
        light_engine::CapturedDynamicProgrammerLane::Live => (
            DynamicProgrammerSourceLane::Live,
            light_engine::ContributionSourceId::programmer(sidecar.programmer_id),
        ),
        light_engine::CapturedDynamicProgrammerLane::Preload => (
            DynamicProgrammerSourceLane::Preload,
            light_engine::ContributionSourceId::preload(sidecar.programmer_id),
        ),
    };
    if sidecar.programmer_id.0 != programmer_id
        || sidecar.changed_at_millis != row.changed_at_millis
        || sidecar.programmer_order != row.programmer_order
        || sidecar.source != expected_source
        || sidecar
            .stamp
            .map(|stamp| (stamp.changed_at, stamp.programmer_order))
            != stamp
                .exact_changed_at()
                .map(|at| (at, row.programmer_order))
    {
        return Err(IntentError(
            "fixed Programmer sidecar does not match its captured row".into(),
        ));
    }
    Ok(DynamicSourceBinding::Fixed {
        source: DynamicFixedSource::Programmer {
            programmer_id: sidecar.programmer_id,
            lane,
        },
        target: row.fixture_id,
        owner: mask.address.owner(),
        component: mask.address.component,
    })
}

fn matching_occurrence(
    origins: Option<&DynamicSourceOrigins>,
    binding: Option<DynamicSourceBinding>,
    stamp: DynamicFixedStamp,
    priority: i16,
    original: &DynamicSemanticValue,
) -> Result<Option<DynamicSourceOccurrenceId>, IntentError> {
    let (Some(origins), Some(binding)) = (origins, binding) else {
        return Ok(None);
    };
    let Some(occurrence) = origins.binding(&binding) else {
        return Ok(None);
    };
    let expected = DynamicSourceOrigin::Fixed {
        stamp,
        priority,
        value: original.clone(),
    };
    if !origins
        .get(occurrence)
        .is_some_and(|record| record.binding == binding && record.origin == expected)
    {
        return Err(IntentError(
            "fixed source occurrence does not match the original captured row".into(),
        ));
    }
    Ok(Some(occurrence))
}

#[allow(clippy::too_many_arguments)]
fn push_row(
    scratch: &mut FixedMaskCompilationScratch,
    native_models: Option<&dyn DynamicNativeModelResolver>,
    target: FixtureId,
    mask: ProgrammingFamilyFixAt,
    original: DynamicSemanticValue,
    stamp: DynamicFixedStamp,
    rank: FamilySampleRank,
    occurrence: Option<DynamicSourceOccurrenceId>,
    output_enabled: bool,
    now: u64,
) -> Result<(), IntentError> {
    if target.0.is_nil() {
        return Err(IntentError("fixed mask target cannot be nil".into()));
    }
    let timing = match &original {
        DynamicSemanticValue::Static { timing, .. }
        | DynamicSemanticValue::FixAt { timing, .. }
        | DynamicSemanticValue::ProgrammingFixAt { timing, .. } => *timing,
        _ => unreachable!("fixed mask classifier excludes all other rows"),
    };
    let authored_activation_mix = authored_activation_mix(rank.changed_at_millis, timing, now);
    let mix = if output_enabled {
        authored_activation_mix
    } else {
        0.0
    };
    let capability = match &mask.address.representation {
        DynamicFamilyRepresentation::DirectColor { source } => {
            if let Some((_, known)) = scratch
                .models
                .iter()
                .find(|(identity, _)| identity == source)
            {
                Some(known.clone())
            } else {
                let capability = match native_models {
                    Some(models) => models.resolve_capability(source)?,
                    None => NativeColorModelCapability::Unavailable(NativeColorModelUnavailable {
                        source: source.clone(),
                        reason: NativeColorUnavailableReason::MissingResolver,
                        detail: "Original native Color model is not available".into(),
                    }),
                };
                let matches = match &capability {
                    NativeColorModelCapability::Available(model) => model.source() == source,
                    NativeColorModelCapability::Unavailable(reason) => &reason.source == source,
                };
                if !matches {
                    return Err(IntentError(
                        "fixed native resolver returned a different original source".into(),
                    ));
                }
                scratch.models.push((source.clone(), capability.clone()));
                Some(capability)
            }
        }
        _ => None,
    };
    let state = match capability {
        Some(NativeColorModelCapability::Unavailable(reason)) => {
            scratch.cache.remove(&rank.identity);
            CompiledFixedMaskState::Requires(FixedMaskRequirement::NativeColorModelUnavailable(
                reason,
            ))
        }
        available => {
            let model = match available {
                Some(NativeColorModelCapability::Available(model)) => Some(model),
                _ => None,
            };
            let cached = scratch.cache.get(&rank.identity).filter(|cached| {
                cached.mask == mask
                    && match (&cached.model, &model) {
                        (None, None) => true,
                        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                        _ => false,
                    }
            });
            let state = if let Some(cached) = cached {
                cached.state.clone()
            } else {
                let state = compile_verified_mask(&mask, model.clone(), rank, mix)?;
                scratch.cache.insert(
                    rank.identity,
                    CachedFixedMask {
                        mask: mask.clone(),
                        model,
                        state: state.clone(),
                    },
                );
                state
            };
            match state {
                CompiledFixedMaskState::Ready(mut sample) => {
                    sample.rank = rank;
                    sample.activation_mix = mix;
                    let footprint = mask
                        .address
                        .component
                        .map_or(FamilyTraceFootprint::Whole, FamilyTraceFootprint::Component);
                    CompiledFixedMaskState::Ready(sample.with_trace_sources(Arc::from([
                        FamilyTraceSource {
                            rank,
                            footprint,
                            role: FamilyTraceRole::Authored,
                            occurrence,
                        },
                    ])))
                }
                requirement => requirement,
            }
        }
    };
    scratch.rows.push(PreparedFixedMask {
        target,
        owner: mask.address.owner(),
        rank,
        stamp,
        occurrence,
        original,
        mask,
        output_enabled,
        authored_activation_mix,
        state,
    });
    Ok(())
}

fn compile_verified_mask(
    mask: &ProgrammingFamilyFixAt,
    model: Option<Model>,
    rank: FamilySampleRank,
    mix: f32,
) -> Result<CompiledFixedMaskState, IntentError> {
    let context = FamilyEditContext {
        color_model: Some(&VirtualColorAuthoringV1),
        ..Default::default()
    };
    mask.compile(model, &context, rank, mix)
        .map(CompiledFixedMaskState::Ready)
}

#[cfg(test)]
mod tests;
