//! Cold compilation of semantic Preset dependencies. The sampler receives immutable values;
//! it never reads show objects, expands Groups or fits a fixture while advancing a clock.
use light_core::{AttributeValue, FixtureId, programming::*};
use light_dynamics::*;
use light_programmer::GroupDefinition;
use std::collections::{HashMap, HashSet};

pub struct CompiledDynamicPresetSources {
    pub values: Vec<DynamicPresetSourceValues>,
    /// Quiet source-quality metadata for deliberate inspection, including held/fallback values.
    /// An expected incompatibility does not reject the complete dependency update.
    pub unavailable: Vec<(uuid::Uuid, FixtureId)>,
    /// Preserve the distinction between expected capability gaps and invalid authored pool
    /// values while retaining last-valid output. This metadata is not an operator error event.
    pub issues: Vec<DynamicPresetSourceIssue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicPresetSourceIssue {
    pub source_id: uuid::Uuid,
    pub target: FixtureId,
    pub reason: DynamicPresetSourceIssueReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DynamicPresetSourceIssueReason {
    NativeUnavailable(NativeColorModelUnavailable),
    IncompatibleSource,
    InvalidValue { fallback: bool, error: IntentError },
}

/// Capture source identities and the immutable verified-original view together. The caller
/// publishes each result using `install_preset_source_values(&expected, values)`; that existing
/// generation check rejects edits made after this capture without resetting any runtime phase.
pub fn compile_runtime_dynamic_preset_sources(
    runtime: &DynamicRuntime,
    groups: &HashMap<String, GroupDefinition>,
    positions: &HashMap<FixtureId, Position3d>,
) -> Result<Vec<(DynamicInstancePresetSources, CompiledDynamicPresetSources)>, IntentError> {
    let models = runtime.captured_native_color_models();
    runtime
        .preset_source_instances()
        .into_iter()
        .map(|instance| {
            let compiled = compile_dynamic_preset_sources(
                &instance,
                groups,
                positions,
                Some(models.as_ref()),
            )?;
            Ok((instance, compiled))
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope<'a> {
    Universal,
    Fixture(FixtureId),
    Group(&'a str),
}
#[derive(Clone, Copy)]
struct Candidate<'a> {
    value: &'a AttributeValue,
    scope: Scope<'a>,
    rank: usize,
    rank_count: usize,
}

pub fn compile_dynamic_preset_sources(
    instance: &DynamicInstancePresetSources,
    groups: &HashMap<String, GroupDefinition>,
    positions: &HashMap<FixtureId, Position3d>,
    native_models: Option<&dyn DynamicNativeModelResolver>,
) -> Result<CompiledDynamicPresetSources, IntentError> {
    let selected = instance
        .ordered_targets
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    if selected.len() != instance.ordered_targets.len() || selected.iter().any(|id| id.0.is_nil()) {
        return Err(IntentError(
            "Dynamic Preset targets require unique stable identities".into(),
        ));
    }
    let mut result = CompiledDynamicPresetSources {
        values: vec![],
        unavailable: vec![],
        issues: vec![],
    };
    let mut occurrences = HashSet::new();
    let mut source_ids = HashSet::new();
    for source in &instance.sources {
        source.address.validate()?;
        let occurrence = source
            .occurrence
            .ok_or_else(|| IntentError("Dynamic Preset source has no stable occurrence".into()))?;
        if !occurrences.insert(occurrence) || source.id.is_nil() || !source_ids.insert(source.id) {
            return Err(IntentError(
                "Dynamic Preset source occurrences and identities must be unique".into(),
            ));
        }
        let records = instance
            .last_valid
            .iter()
            .filter(|record| record.matches(source))
            .collect::<Vec<_>>();
        if records.len() > 1 {
            return Err(IntentError(
                "duplicate last-valid Dynamic Preset occurrence".into(),
            ));
        }
        if let Some(record) = records.first() {
            let mut targets = HashSet::new();
            for fallback in &record.values {
                if fallback.target.0.is_nil() || !targets.insert(fallback.target) {
                    return Err(IntentError(
                        "Dynamic Preset last-valid targets must be unique stable identities".into(),
                    ));
                }
                source.address.validate_value_shape(&fallback.value)?;
            }
        }
        let mut values = instance
            .last_valid
            .iter()
            .find(|record| record.matches(source))
            .map(|record| {
                record
                    .values
                    .iter()
                    .filter(|fallback| selected.contains(&fallback.target))
                    .map(|fallback| (fallback.target, fallback.value.clone()))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        let mut context = OwnedFamilyEditContext::default();
        let native = if let DynamicFamilyRepresentation::DirectColor { source: identity } =
            &source.address.representation
        {
            Some(match native_models {
                Some(models) => models.resolve_capability(identity)?,
                None => NativeColorModelCapability::Unavailable(NativeColorModelUnavailable {
                    source: identity.clone(),
                    reason: NativeColorUnavailableReason::MissingResolver,
                    detail: "Original native Color model is not available".into(),
                }),
            })
        } else {
            None
        };
        let (compiled, unavailable_native) = match native {
            Some(NativeColorModelCapability::Unavailable(reason)) => {
                if !matches!(&source.address.representation, DynamicFamilyRepresentation::DirectColor { source } if source == &reason.source)
                {
                    return Err(IntentError(
                        "native capability refers to a different original source".into(),
                    ));
                }
                (None, Some(reason))
            }
            capability => {
                let model = match capability {
                    Some(NativeColorModelCapability::Available(model)) => Some(model),
                    _ => None,
                };
                context.native_model = model.clone();
                (
                    Some(CompiledDynamicValueAddress::new(
                        source.address.clone(),
                        model,
                    )?),
                    None,
                )
            }
        };
        let latest = source.retained.as_deref();
        if let Some(template) = latest {
            template.validate(source.address.owner())?;
        }
        let authored = latest
            .map(|template| candidates(template, &instance.ordered_targets, groups, positions))
            .unwrap_or_default();
        let fallback = latest
            .and_then(|template| template.fallback.as_deref())
            .map(|template| candidates(template, &instance.ordered_targets, groups, positions))
            .unwrap_or_default();
        if let Some(compiled) = compiled {
            for value in values.values() {
                compiled.validate_source_value(value)?;
            }
            let resolved = materialize(&authored, &source.address, &compiled, &context.borrowed());
            // Match the winning authored scope before considering its fallback. An old fixture
            // exception must never override a newly valid universal or Group value.
            let fallback = fallback
                .into_iter()
                .filter(|(target, value)| {
                    !resolved.values.contains_key(target)
                        && authored
                            .get(target)
                            .is_some_and(|current| current.scope == value.scope)
                })
                .collect();
            let mut fallback =
                materialize(&fallback, &source.address, &compiled, &context.borrowed());
            for target in &instance.ordered_targets {
                if let Some(value) = resolved.values.get(target) {
                    values.insert(*target, value.clone());
                } else {
                    result.unavailable.push((source.id, *target));
                    result.issues.push(DynamicPresetSourceIssue {
                        source_id: source.id,
                        target: *target,
                        reason: if let Some(error) = resolved.invalid.get(target) {
                            DynamicPresetSourceIssueReason::InvalidValue {
                                fallback: false,
                                error: error.clone(),
                            }
                        } else if let Some(error) = fallback.invalid.get(target) {
                            DynamicPresetSourceIssueReason::InvalidValue {
                                fallback: true,
                                error: error.clone(),
                            }
                        } else {
                            DynamicPresetSourceIssueReason::IncompatibleSource
                        },
                    });
                    if let Some(value) = fallback.values.remove(target) {
                        values.insert(*target, value);
                    }
                }
            }
        } else {
            result.unavailable.extend(
                instance
                    .ordered_targets
                    .iter()
                    .map(|target| (source.id, *target)),
            );
            let reason =
                unavailable_native.expect("uncompiled Direct source has a typed capability gap");
            result
                .issues
                .extend(
                    instance
                        .ordered_targets
                        .iter()
                        .map(|target| DynamicPresetSourceIssue {
                            source_id: source.id,
                            target: *target,
                            reason: DynamicPresetSourceIssueReason::NativeUnavailable(
                                reason.clone(),
                            ),
                        }),
                );
        }
        result.values.push(DynamicPresetSourceValues {
            occurrence,
            preset_id: source.preset_id.clone(),
            address: source.address.clone(),
            values: instance
                .ordered_targets
                .iter()
                .filter_map(|target| {
                    values.remove(target).map(|value| DynamicValueFallback {
                        target: *target,
                        value,
                    })
                })
                .collect(),
        });
    }
    Ok(result)
}

fn candidates<'a>(
    template: &'a DynamicPresetTemplate,
    targets: &[FixtureId],
    groups: &HashMap<String, GroupDefinition>,
    positions: &HashMap<FixtureId, Position3d>,
) -> HashMap<FixtureId, Candidate<'a>> {
    let selected = targets.iter().copied().collect::<HashSet<_>>();
    let mut winners = HashMap::new();
    if let Some(value) = &template.universal {
        for (rank, target) in targets.iter().enumerate() {
            winners.insert(
                *target,
                Candidate {
                    value,
                    rank,
                    rank_count: targets.len(),
                    scope: Scope::Universal,
                },
            );
        }
    }
    for fixture in &template.fixtures {
        if selected.contains(&fixture.fixture_id) {
            winners.insert(
                fixture.fixture_id,
                Candidate {
                    value: &fixture.value,
                    rank: 0,
                    rank_count: 1,
                    scope: Scope::Fixture(fixture.fixture_id),
                },
            );
        }
    }
    // Match Preset recall's established precedence: universal -> fixture -> sorted Groups.
    let mut sorted = template.groups.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.group_id.cmp(&b.group_id));
    for group in sorted {
        let Ok(resolved) =
            light_programmer::resolve_group_spatial(&group.group_id, groups, positions)
        else {
            continue;
        };
        let ranking = resolved.ranked_selection;
        for target in ranking
            .ordered_fixture_ids
            .iter()
            .filter(|target| selected.contains(target))
        {
            let value = match &group.value {
                AttributeValue::GroupFamily(group) => group.for_member(*target),
                value => value,
            };
            winners.insert(
                *target,
                Candidate {
                    value,
                    rank: ranking.rank_by_fixture[target],
                    rank_count: ranking.rank_count,
                    scope: Scope::Group(&group.group_id),
                },
            );
        }
    }
    winners
}

#[derive(Default)]
struct Materialized {
    values: HashMap<FixtureId, DynamicValue>,
    invalid: HashMap<FixtureId, IntentError>,
}

fn materialize(
    winners: &HashMap<FixtureId, Candidate<'_>>,
    address: &DynamicValueAddress,
    compiled: &CompiledDynamicValueAddress,
    context: &FamilyEditContext<'_>,
) -> Materialized {
    let mut batches = HashMap::<(usize, usize), (&AttributeValue, Vec<(FixtureId, usize)>)>::new();
    for (target, candidate) in winners {
        if !address.matches_authored_source(candidate.value) {
            continue;
        }
        // Pointer identity groups repeated source references only within this cold call. It is
        // never persisted. Distinct member exceptions still cost only their requested ranks.
        let key = (
            candidate.value as *const AttributeValue as usize,
            candidate.rank_count,
        );
        batches
            .entry(key)
            .or_insert_with(|| (candidate.value, vec![]))
            .1
            .push((*target, candidate.rank));
    }
    let mut result = Materialized::default();
    for ((_, count), (value, members)) in batches {
        if let AttributeValue::ColorProgram(program) = value
            && let ColorProgram::Direct { recipe, .. } = program.as_ref()
        {
            let Some(model) = context.native_model else {
                continue;
            };
            let mut base = recipe.clone();
            base.spreads.clear();
            // Even a one-component source must originate from a complete valid recipe.
            // Otherwise extracting an existing channel could hide a missing sibling control.
            if let Err(error) = model
                .predict(&base)
                .and_then(|estimate| estimate.validate())
            {
                result
                    .invalid
                    .extend(members.iter().map(|(target, _)| (*target, error.clone())));
                continue;
            }
        }
        let ranks = members.iter().map(|(_, rank)| *rank).collect::<Vec<_>>();
        let ranked = match compile_programming_ranks(value, count, &ranks, context) {
            Ok(ranked) => ranked,
            Err(error) => {
                result
                    .invalid
                    .extend(members.iter().map(|(target, _)| (*target, error.clone())));
                continue;
            }
        };
        for (index, (target, _)) in members.iter().enumerate() {
            let Some(value) = ranked.at_rank(index) else {
                continue;
            };
            match extract_compatible_dynamic_value(value, address, context).and_then(|value| {
                if let Some(value) = &value {
                    compiled.validate_source_value(value)?;
                }
                Ok(value)
            }) {
                Ok(Some(value)) => {
                    result.values.insert(*target, value);
                }
                Ok(None) => {}
                Err(error) => {
                    result.invalid.insert(*target, error);
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests;
