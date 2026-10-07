use super::*;
use light_dynamics::{DynamicDefinition, DynamicLane, DynamicLaneSelection, DynamicSemanticValue};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime::output_scheduler) struct ReconciledSourceAssignment {
    pub instance_id: Uuid,
    pub controller_id: Uuid,
    pub target: FixtureId,
    pub lane_id: Uuid,
    pub captured_index: usize,
}

/// Resolve source anchors while expanding the same recorded lane selection used by runtime.
/// Uniform Live Groups use one newest enabling row per lane for all current/future members.
/// Per-target selections never borrow another target's activation. Exact lane rows take
/// precedence over an activation which merely enables it through Angle closure or legacy All.
/// Ties retain the earliest captured input index, matching the shared authored-row fold.
/// Mixed legacy stamps use that fold's ordered comparison; this does not invent timestamps.
pub(super) fn planned_source_rows(
    desired: &DesiredProgrammerController<'_>,
    definition: &DynamicDefinition,
    selection: &DynamicLaneSelection,
    targets: &[FixtureId],
) -> Vec<(FixtureId, Uuid, usize)> {
    let source_rows = desired
        .source_rows
        .iter()
        .map(|source| {
            let DynamicSemanticValue::DynamicOn { lane_id, .. } = source.row.value else {
                unreachable!("the shared controller plan contains only surviving On rows")
            };
            LaneSource {
                captured_index: source.captured_index,
                target: source.row.fixture_id,
                lane_id,
                row: source.row,
            }
        })
        .collect();
    planned_rows(
        desired.reference,
        definition,
        selection,
        targets,
        source_rows,
        light_dynamics::dynamic_address_edit_is_later,
    )
}

pub(super) fn planned_cue_source_rows(
    desired: &DesiredCueController<'_>,
    definition: &DynamicDefinition,
    selection: &DynamicLaneSelection,
    targets: &[FixtureId],
) -> Vec<(FixtureId, Uuid, usize)> {
    let source_rows = desired
        .source_rows
        .iter()
        .map(|(captured_index, row)| {
            let DynamicSemanticValue::DynamicOn { lane_id, .. } = row.value else {
                unreachable!("the shared Cue controller plan contains only On rows")
            };
            LaneSource {
                captured_index: *captured_index,
                target: row.fixture_id,
                lane_id,
                row: *row,
            }
        })
        .collect();
    planned_rows(
        desired.reference,
        definition,
        selection,
        targets,
        source_rows,
        |new, old| {
            (new.changed_at, new.transition_ordinal) > (old.changed_at, old.transition_ordinal)
        },
    )
}

struct LaneSource<'a, T> {
    captured_index: usize,
    target: FixtureId,
    lane_id: Uuid,
    row: &'a T,
}

fn planned_rows<T>(
    reference: &light_dynamics::DynamicReference,
    definition: &DynamicDefinition,
    selection: &DynamicLaneSelection,
    targets: &[FixtureId],
    mut source_rows: Vec<LaneSource<'_, T>>,
    is_later: impl Fn(&T, &T) -> bool,
) -> Vec<(FixtureId, Uuid, usize)> {
    source_rows.sort_by_key(|source| source.captured_index);
    // TL-641: a single row's selection depends on its target only as the one `PerTarget` key,
    // so it is resolved once per lane and re-keyed for every other target of a large start.
    let mut by_lane = rustc_hash::FxHashMap::<Uuid, DynamicLaneSelection>::default();
    let sources = source_rows
        .into_iter()
        .map(|source| {
            let lane_id = source.lane_id;
            let target = source.target;
            let selection = by_lane
                .entry(lane_id)
                .or_insert_with(|| {
                    DynamicLaneSelection::for_recorded_values(
                        reference,
                        definition,
                        &[(target, lane_id)],
                    )
                })
                .clone();
            (source, lane_id, retarget(selection, target))
        })
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    match selection {
        DynamicLaneSelection::All | DynamicLaneSelection::Uniform { .. } => {
            let Some(&target) = targets.first() else {
                return output;
            };
            for lane in &definition.lanes {
                if lane.is_angle_current_passthrough() {
                    continue;
                }
                if let Some(index) = newest_source(
                    &sources,
                    0..sources.len(),
                    definition,
                    target,
                    lane,
                    &is_later,
                ) {
                    output.extend(targets.iter().map(|target| (*target, lane.id, index)));
                }
            }
        }
        DynamicLaneSelection::PerTarget { .. } => {
            let mut by_target = HashMap::<FixtureId, Vec<_>>::new();
            for (index, source) in sources.iter().enumerate() {
                by_target.entry(source.0.target).or_default().push(index);
            }
            for &target in targets {
                let Some(candidates) = by_target.get(&target) else {
                    continue;
                };
                for lane in &definition.lanes {
                    if lane.is_angle_current_passthrough() {
                        continue;
                    }
                    if let Some(index) = newest_source(
                        &sources,
                        candidates.iter().copied(),
                        definition,
                        target,
                        lane,
                        &is_later,
                    ) {
                        output.push((target, lane.id, index));
                    }
                }
            }
        }
    }
    output
}

/// The one-row selection of another target with the same lane.
fn retarget(mut selection: DynamicLaneSelection, target: FixtureId) -> DynamicLaneSelection {
    if let DynamicLaneSelection::PerTarget { targets } = &mut selection {
        for selected in targets {
            selected.target = target;
        }
    }
    selection
}

type SourceSelection<'a, T> = (LaneSource<'a, T>, Uuid, DynamicLaneSelection);

fn newest_source<T>(
    sources: &[SourceSelection<'_, T>],
    indices: impl IntoIterator<Item = usize>,
    definition: &DynamicDefinition,
    target: FixtureId,
    lane: &DynamicLane,
    is_later: &impl Fn(&T, &T) -> bool,
) -> Option<usize> {
    let mut selected: Option<(&LaneSource<'_, T>, bool)> = None;
    for index in indices {
        let (source, declared_lane, selection) = &sources[index];
        if !selection_allows(selection, definition, target, lane) {
            continue;
        }
        let exact = *declared_lane == lane.id;
        if selected.is_none_or(|(previous, previous_exact)| {
            exact && !previous_exact
                || exact == previous_exact && is_later(source.row, previous.row)
        }) {
            selected = Some((source, exact));
        }
    }
    selected.map(|(source, _)| source.captured_index)
}

fn selection_allows(
    selection: &DynamicLaneSelection,
    definition: &DynamicDefinition,
    target: FixtureId,
    lane: &DynamicLane,
) -> bool {
    let selected = match selection {
        DynamicLaneSelection::All => return true,
        DynamicLaneSelection::Uniform { lanes } => lanes,
        DynamicLaneSelection::PerTarget { targets } => {
            let Some(target) = targets.iter().find(|candidate| candidate.target == target) else {
                return false;
            };
            &target.lanes
        }
    };
    selected.contains(&lane.id)
        || lane.is_programming_angles()
            && definition.lanes.iter().any(|candidate| {
                selected.contains(&candidate.id) && candidate.is_programming_angles()
            })
}

#[cfg(test)]
mod tests;
