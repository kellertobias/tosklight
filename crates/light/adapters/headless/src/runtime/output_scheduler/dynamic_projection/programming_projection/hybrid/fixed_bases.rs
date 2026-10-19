//! TL-552: a participating whole-family FixAT is the operator's complete intent for its owner.
//! Family composition and projection both need a captured static slot of that owner. When no
//! static value sits underneath the mask (FixAT straight after Clear, or over a fixture that was
//! never given that family), the mask's own family is installed as that slot's static baseline,
//! so the FixAT renders through the unchanged family path and its composed sample still wins
//! with its exact rank. Nothing is stored: the slot exists only inside one prepared frame.
//!
//! Only masks that already participate (enabled, past their delay) qualify, so a delayed or
//! disabled FixAT never shows early. A component mask (one Color component, Pan alone) cannot
//! complete a family on its own and keeps requiring a real underlay.
//!
//! The same frame-local slot carries a fixture's declared default Position under a running
//! Dynamic with Position lanes when nothing static sits there (plan §9: the static pre-Dynamic
//! frame includes the declared default where the adapter can map it). Starting an Angle Dynamic
//! then needs no Position edit. The default ranks below every authored row and carries no
//! origin, so it is never Programmer or Cue evidence; an unmappable default stays absent.
use super::super::super::{CapturedDynamicInputs, authored_activation_mix};
use crate::runtime::dynamic_source_origins::captured_programming_fixed_mask;
use chrono::{DateTime, Utc};
use light_core::{
    AttributeKey, AttributeValue, FixtureId, MergeMode, TimedValue, programming::ProgrammingOwner,
};
use light_dynamics::DynamicRuntime;
use light_dynamics::DynamicSemanticValue;
use light_engine::{ContributionBatch, ContributionSample, Engine, PreparedStaticFamilyFrame};
use rustc_hash::FxHashMap;
use std::collections::hash_map::Entry;

struct Base {
    rank: (i16, DateTime<Utc>, u64),
    owner: ProgrammingOwner,
    value: AttributeValue,
}

/// One captured Programmer or Cue row, reduced to what the baseline needs.
struct Row<'a> {
    fixture_id: FixtureId,
    attribute: &'a AttributeKey,
    value: &'a DynamicSemanticValue,
    changed_at_millis: u64,
    rank: RowRank,
    enabled: bool,
}

/// A row's arbitration rank. A Programmer row's edit time is converted only when the row can
/// decide a base: most rows are plain values that never do, every frame.
#[derive(Clone, Copy)]
enum RowRank {
    Programmer { priority: i16, order: u64 },
    Cue((i16, DateTime<Utc>, u64)),
}

impl Row<'_> {
    fn rank(&self, now: DateTime<Utc>) -> (i16, DateTime<Utc>, u64) {
        match self.rank {
            RowRank::Programmer { priority, order } => {
                #[cfg(test)]
                RESOLVED_RANKS.with(|count| count.set(count.get() + 1));
                let changed_at = i64::try_from(self.changed_at_millis).unwrap_or(i64::MAX);
                let changed_at = DateTime::from_timestamp_millis(changed_at).unwrap_or(now);
                (priority, changed_at, order)
            }
            RowRank::Cue(rank) => rank,
        }
    }
}

/// The static baselines missing under participating whole-family FixAT masks and under running
/// Position Dynamics, as one batch to append to the frame's baseline samples, or None when every
/// such owner already has an underlay.
pub(super) fn missing(
    engine: &Engine,
    runtime: &DynamicRuntime,
    inputs: &CapturedDynamicInputs<'_>,
    static_token: &PreparedStaticFamilyFrame,
) -> Option<ContributionBatch> {
    let position = ProgrammingOwner::Position;
    // Active Programmer/Cue Dynamic rows, plus enabled Dynamic Playbacks' running instances. A
    // released source keeps its clock alive but must not output, so it is never a target here.
    let mut targets = Vec::new();
    let mut bases = missing_fixed_mask_bases(inputs, static_token, &mut targets);
    let key = position.key();
    for playback in inputs.dynamic_playbacks.iter().filter(|p| p.enabled) {
        if let Some(definition) = playback.dynamic_id {
            targets.extend(runtime.owner_lane_targets(definition, &key));
        }
    }
    targets.sort_unstable_by_key(|target| target.0);
    targets.dedup();
    for target in targets {
        if static_token.value(target, &position.key()).is_some()
            || bases.contains_key(&(target, position))
        {
            continue;
        }
        let Some(angles) = engine.declared_default_position(inputs.snapshot, target) else {
            continue;
        };
        let value = light_core::programming::PositionIntent::angles(
            angles.pan_degrees,
            angles.tilt_degrees,
        );
        bases.insert(
            (target, position),
            Base {
                rank: (i16::MIN, DateTime::<Utc>::UNIX_EPOCH, 0),
                owner: position,
                value: AttributeValue::Position(std::sync::Arc::new(value)),
            },
        );
    }
    batch(inputs, bases)
}

fn missing_fixed_mask_bases(
    inputs: &CapturedDynamicInputs<'_>,
    static_token: &PreparedStaticFamilyFrame,
    dynamic_position_targets: &mut Vec<FixtureId>,
) -> FxHashMap<(FixtureId, ProgrammingOwner), Base> {
    let position_key = ProgrammingOwner::Position.key();
    let now = u64::try_from(inputs.now.timestamp_millis()).unwrap_or_default();
    let programmer = inputs
        .programmer_values
        .iter()
        .chain(inputs.extra_programmer_values)
        .map(|(_, priority, row)| Row {
            fixture_id: row.fixture_id,
            attribute: &row.attribute,
            value: &row.value,
            changed_at_millis: row.changed_at_millis,
            rank: RowRank::Programmer {
                priority: *priority,
                order: row.programmer_order,
            },
            enabled: true,
        });
    let cues = inputs.cue_values.iter().map(|row| Row {
        fixture_id: row.fixture_id,
        attribute: &row.attribute,
        value: &row.value,
        changed_at_millis: row.changed_at_millis,
        rank: RowRank::Cue((row.priority, row.changed_at, row.transition_ordinal)),
        enabled: row.output_enabled,
    });
    let mut bases = FxHashMap::<(FixtureId, ProgrammingOwner), Base>::default();
    // The newest Position Dynamic/Release row decides: a Release masks the Dynamic's sources.
    let mut dynamics = FxHashMap::<FixtureId, ((i16, DateTime<Utc>, u64), bool)>::default();
    for row in programmer.clone().chain(cues.clone()) {
        let on = match row.value {
            DynamicSemanticValue::DynamicOn { .. } => true,
            DynamicSemanticValue::Release | DynamicSemanticValue::ProgrammingRelease { .. } => {
                false
            }
            _ => continue,
        };
        if row.enabled && *row.attribute == position_key {
            let rank = row.rank(inputs.now);
            let entry = dynamics.entry(row.fixture_id).or_insert((rank, on));
            if rank >= entry.0 {
                *entry = (rank, on);
            }
        }
    }
    dynamic_position_targets.extend(
        dynamics
            .into_iter()
            .filter(|(_, (_, on))| *on)
            .map(|(t, _)| t),
    );
    for row in programmer.chain(cues) {
        let timing = match row.value {
            DynamicSemanticValue::ProgrammingFixAt { timing, .. }
            | DynamicSemanticValue::Static { timing, .. }
            | DynamicSemanticValue::FixAt { timing, .. } => *timing,
            _ => continue,
        };
        if !row.enabled || authored_activation_mix(row.changed_at_millis, timing, now) <= 0.0 {
            continue;
        }
        // Invalid captures are refused by the authoritative Fixed compiler of this frame.
        let Ok(Some(mask)) = captured_programming_fixed_mask(row.attribute, row.value) else {
            continue;
        };
        let owner = mask.address.owner();
        if mask.address.component.is_some()
            || static_token
                .value(row.fixture_id, owner.key_ref())
                .is_some()
        {
            continue;
        }
        let base = Base {
            rank: row.rank(inputs.now),
            owner,
            value: mask.family,
        };
        // The lowest-ranked whole mask is the floor; every mask still composes over it.
        match bases.entry((row.fixture_id, owner)) {
            Entry::Vacant(entry) => {
                entry.insert(base);
            }
            Entry::Occupied(mut entry) if base.rank < entry.get().rank => {
                entry.insert(base);
            }
            Entry::Occupied(_) => {}
        }
    }
    bases
}

fn batch(
    inputs: &CapturedDynamicInputs<'_>,
    bases: FxHashMap<(FixtureId, ProgrammingOwner), Base>,
) -> Option<ContributionBatch> {
    (!bases.is_empty()).then(|| {
        ContributionBatch::new(bases.into_iter().map(|((fixture_id, _), base)| {
            let attribute = base.owner.key();
            let address = inputs.addresser.frame_address(fixture_id, &attribute);
            let (priority, changed_at, programmer_order) = base.rank;
            ContributionSample::independent(TimedValue {
                fixture_id,
                attribute,
                value: base.value,
                priority,
                changed_at,
                programmer_order,
                merge_mode: MergeMode::Ltp,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            })
            .at(address)
        }))
    })
}

// Counts the Programmer row ranks a frame resolved, so a test can prove only rows that can
// decide a base pay for one.
#[cfg(test)]
thread_local! {
    static RESOLVED_RANKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Runs `work` and reports how many Programmer row ranks missing bases resolved during it.
#[cfg(test)]
pub(in crate::runtime::output_scheduler::dynamic_projection::programming_projection) fn count_resolved_ranks<
    T,
>(
    work: impl FnOnce() -> T,
) -> (T, usize) {
    let before = RESOLVED_RANKS.with(std::cell::Cell::get);
    let result = work();
    (result, RESOLVED_RANKS.with(std::cell::Cell::get) - before)
}
