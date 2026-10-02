//! Cold, candidate-only refresh of Dynamic Preset dependencies.
//!
//! A reconciled Dynamic candidate still carries Preset tables compiled against the previous show.
//! Target-list and phase changes already advance an instance's dependency generation during
//! reconciliation; Group membership, spatial ranking and stage positions do not, because they can
//! change without touching the ordered targets (a frozen-target instance keeps identical targets
//! and phases while its Group-ranked spread moves). [`materialize_cold_preset_dependencies`]
//! closes that gap before the caller persists and publishes the candidate:
//!
//! 1. every existing Group referenced by a retained or fallback template is resolved strictly
//!    against the destination (the compiler's candidate enumeration skips resolver failures);
//! 2. only instances whose referenced Groups resolve differently between the previous and the
//!    destination show get a new dependency generation; universal-only and unrelated sources
//!    keep theirs;
//! 3. every Preset-source instance is compiled against the exact destination Groups, positions and
//!    the candidate's native models, prepared as one batch and installed once.
//!
//! Clocks, controllers, pause, Random state and held history are never reset. Live is never
//! touched. On any error the candidate may already carry invalidated generations and must be
//! discarded as a whole.

use super::*;
use light_application::{DynamicPresetSourceIssue, compile_dynamic_preset_sources};
use light_dynamics::{
    DynamicInstancePresetSources, DynamicPresetTemplate, DynamicRuntime, DynamicRuntimeError,
    Position3d, PreparedDynamicPresetSources,
};
use light_programmer::GroupDefinition;
use std::collections::BTreeSet;

/// A Preset template refers to a top-level Group absent from the destination show. Passive: the
/// affected targets keep their last valid values and report source quality instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct ColdPresetMissingGroup {
    pub instance_id: Uuid,
    pub group_id: String,
}

/// Passive compiler quality information: native capability gaps, incompatible sources and
/// invalid authored values that fell back to a verified compatible or last-valid value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct ColdPresetSourceQuality {
    pub instance_id: Uuid,
    pub issue: DynamicPresetSourceIssue,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct ColdPresetMaterialization {
    /// Instances whose dependency generation advanced because a referenced Group changed.
    pub invalidated_instances: Vec<Uuid>,
    /// Every instance whose destination Preset table was installed by the single batch.
    pub installed_instances: Vec<Uuid>,
    pub missing_groups: Vec<ColdPresetMissingGroup>,
    pub source_quality: Vec<ColdPresetSourceQuality>,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) enum ColdPresetMaterializationError {
    /// An existing referenced Group failed strict nested or spatial resolution. `resolver` is the
    /// Group resolver's own context, preserved verbatim.
    InvalidGroup { group_id: String, resolver: String },
    /// Genuine Preset compiler failure (malformed template, source identity or native binding).
    Compile(light_core::programming::IntentError),
    /// The compiled batch failed table validation.
    InvalidTable(DynamicRuntimeError),
    /// A batch member's manifest changed between preparation and installation.
    Stale,
}

impl std::fmt::Display for ColdPresetMaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidGroup { group_id, resolver } => {
                write!(formatter, "Group {group_id} cannot be resolved: {resolver}")
            }
            Self::Compile(error) => write!(formatter, "Dynamic Preset compilation failed: {error}"),
            Self::InvalidTable(error) => {
                write!(formatter, "Dynamic Preset table is invalid: {error}")
            }
            Self::Stale => formatter.write_str("Dynamic Preset dependency batch is stale"),
        }
    }
}

impl std::error::Error for ColdPresetMaterializationError {}

/// Refresh every Dynamic Preset table of a detached, already reconciled candidate for the
/// destination show, as one validated all-or-nothing batch.
///
/// `previous` is the Engine snapshot the candidate's tables were compiled for; `destination` is
/// the finalized snapshot being installed. The candidate must be a `fork_for_cold_install` fork
/// whose destination definitions, native models and controllers are already in place. On error
/// discard the whole candidate, including any dependency invalidations made before the error.
pub(in crate::runtime) fn materialize_cold_preset_dependencies(
    previous: &light_engine::EngineSnapshot,
    destination: &light_engine::EngineSnapshot,
    candidate: &mut DynamicRuntime,
) -> Result<ColdPresetMaterialization, ColdPresetMaterializationError> {
    let (prepared, report) = prepare_cold_preset_batch(previous, destination, candidate)?;
    install_cold_preset_batch(candidate, prepared)?;
    Ok(report)
}

/// Materialize newly started, restored or retargeted instances before their first sample.
/// The caller holds the runtime lease and an output transaction (or a detached candidate).
/// The steady path does no Group/native lookup or manifest allocation. Show dependency edits
/// still use the full cold helper above, which supplies external Group invalidation.
pub(in crate::runtime) fn materialize_pending_preset_dependencies(
    snapshot: &light_engine::EngineSnapshot,
    candidate: &mut DynamicRuntime,
) -> Result<ColdPresetMaterialization, ColdPresetMaterializationError> {
    if !candidate.has_pending_preset_sources() {
        return Ok(ColdPresetMaterialization::default());
    }
    let manifests = candidate.pending_preset_source_instances();
    let show = ShowGroups::new(snapshot);
    let mut report = ColdPresetMaterialization::default();
    for manifest in &manifests {
        for group_id in referenced_groups(manifest) {
            match show.signature(group_id) {
                GroupSignature::Invalid(resolver) => {
                    return Err(ColdPresetMaterializationError::InvalidGroup {
                        group_id: group_id.to_owned(),
                        resolver,
                    });
                }
                GroupSignature::Absent => report.missing_groups.push(ColdPresetMissingGroup {
                    instance_id: manifest.instance_id,
                    group_id: group_id.to_owned(),
                }),
                GroupSignature::Resolved { .. } => {}
            }
        }
    }
    let prepared = compile_preset_batch(candidate, manifests, &show, &mut report)?;
    install_cold_preset_batch(candidate, prepared)?;
    Ok(report)
}

/// Preflight, invalidate and compile. Nothing but dependency generations changes on the
/// candidate; the returned token holds the complete batch.
fn prepare_cold_preset_batch(
    previous: &light_engine::EngineSnapshot,
    destination: &light_engine::EngineSnapshot,
    candidate: &mut DynamicRuntime,
) -> Result<(PreparedDynamicPresetSources, ColdPresetMaterialization), ColdPresetMaterializationError>
{
    let before = ShowGroups::new(previous);
    let after = ShowGroups::new(destination);
    let mut report = ColdPresetMaterialization::default();
    let manifests = candidate.preset_source_instances();
    let references = manifests
        .iter()
        .map(|manifest| (manifest.instance_id, referenced_groups(manifest)))
        .collect::<Vec<_>>();
    // Strict preflight of every referenced Group before any generation changes.
    let mut changed = HashMap::<&str, bool>::new();
    for group_id in references.iter().flat_map(|(_, ids)| ids.iter().copied()) {
        if changed.contains_key(group_id) {
            continue;
        }
        let signature = after.signature(group_id);
        if let GroupSignature::Invalid(resolver) = signature {
            return Err(ColdPresetMaterializationError::InvalidGroup {
                group_id: group_id.to_owned(),
                resolver,
            });
        }
        changed.insert(group_id, before.signature(group_id) != signature);
    }
    for (instance_id, group_ids) in &references {
        for group_id in group_ids {
            if !after.groups.contains_key(*group_id) {
                report.missing_groups.push(ColdPresetMissingGroup {
                    instance_id: *instance_id,
                    group_id: (*group_id).to_owned(),
                });
            }
        }
        if group_ids.iter().any(|group_id| changed[group_id])
            && candidate.invalidate_preset_source_dependencies(*instance_id)
        {
            report.invalidated_instances.push(*instance_id);
        }
    }

    let prepared = compile_preset_batch(
        candidate,
        candidate.preset_source_instances(),
        &after,
        &mut report,
    )?;
    Ok((prepared, report))
}

fn compile_preset_batch(
    candidate: &DynamicRuntime,
    manifests: Vec<DynamicInstancePresetSources>,
    show: &ShowGroups,
    report: &mut ColdPresetMaterialization,
) -> Result<PreparedDynamicPresetSources, ColdPresetMaterializationError> {
    let models = candidate.captured_native_color_models();
    let mut batch = Vec::with_capacity(manifests.len());
    for manifest in manifests {
        let sources = compile_dynamic_preset_sources(
            &manifest,
            &show.groups,
            &show.positions,
            Some(models.as_ref()),
        )
        .map_err(ColdPresetMaterializationError::Compile)?;
        report.installed_instances.push(manifest.instance_id);
        report
            .source_quality
            .extend(
                sources
                    .issues
                    .into_iter()
                    .map(|issue| ColdPresetSourceQuality {
                        instance_id: manifest.instance_id,
                        issue,
                    }),
            );
        batch.push((manifest, sources.values));
    }
    candidate
        .prepare_preset_source_values(batch)
        .map_err(ColdPresetMaterializationError::InvalidTable)?
        .ok_or(ColdPresetMaterializationError::Stale)
}

fn install_cold_preset_batch(
    candidate: &mut DynamicRuntime,
    prepared: PreparedDynamicPresetSources,
) -> Result<(), ColdPresetMaterializationError> {
    candidate
        .install_prepared_preset_source_values(prepared)
        .then_some(())
        .ok_or(ColdPresetMaterializationError::Stale)
}

/// Group ids referenced by the latest retained templates and their bounded fallbacks.
fn referenced_groups(manifest: &DynamicInstancePresetSources) -> BTreeSet<&str> {
    fn visit<'a>(template: &'a DynamicPresetTemplate, ids: &mut BTreeSet<&'a str>) {
        ids.extend(template.groups.iter().map(|group| group.group_id.as_str()));
        if let Some(fallback) = &template.fallback {
            visit(fallback, ids);
        }
    }
    let mut ids = BTreeSet::new();
    for source in &manifest.sources {
        if let Some(template) = &source.retained {
            visit(template, &mut ids);
        }
    }
    ids
}

struct ShowGroups {
    groups: HashMap<String, GroupDefinition>,
    positions: HashMap<FixtureId, Position3d>,
}

/// Everything a Group-scoped Preset value depends on: ordered membership, ranks and the stage
/// positions of the members. Stage-position edits of members invalidate conservatively even
/// when the discrete ranking happens to stay identical.
#[derive(Debug, PartialEq)]
enum GroupSignature {
    Absent,
    Invalid(String),
    Resolved {
        order: Vec<FixtureId>,
        ranks: Vec<usize>,
        rank_count: usize,
        positions: Vec<Option<Position3d>>,
    },
}

impl ShowGroups {
    fn new(snapshot: &light_engine::EngineSnapshot) -> Self {
        Self {
            groups: snapshot
                .groups
                .iter()
                .map(|group| (group.id.clone(), group.clone()))
                .collect(),
            positions: snapshot
                .dynamic_stage_positions
                .iter()
                .map(|(fixture_id, position)| {
                    (
                        *fixture_id,
                        Position3d {
                            x: f64::from(position.x),
                            y: f64::from(position.y),
                            z: f64::from(position.z),
                        },
                    )
                })
                .collect(),
        }
    }

    fn signature(&self, group_id: &str) -> GroupSignature {
        if !self.groups.contains_key(group_id) {
            return GroupSignature::Absent;
        }
        match light_programmer::resolve_group_spatial(group_id, &self.groups, &self.positions) {
            Ok(resolved) => {
                let ranking = resolved.ranked_selection;
                GroupSignature::Resolved {
                    ranks: ranking
                        .ordered_fixture_ids
                        .iter()
                        .map(|fixture| ranking.rank_by_fixture[fixture])
                        .collect(),
                    positions: ranking
                        .ordered_fixture_ids
                        .iter()
                        .map(|fixture| self.positions.get(fixture).copied())
                        .collect(),
                    order: ranking.ordered_fixture_ids,
                    rank_count: ranking.rank_count,
                }
            }
            Err(resolver) => GroupSignature::Invalid(resolver),
        }
    }
}

#[cfg(test)]
mod tests;
