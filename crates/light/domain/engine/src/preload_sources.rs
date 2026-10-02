//! Captured, source-ranked Preload inputs for the two Color Release observations.
//!
//! This prepares source lanes only. Address expansion, release cutoffs, and output continuity
//! belong to the generation-specific resolver that consumes these immutable vectors.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use light_core::{AttributeKey, FixtureId, ProgrammerEditStamp, ProgrammerId, TimedValue};
use light_dynamics::{DynamicAddressValue, DynamicSemanticValue, merge_dynamic_address_values};
use light_programmer::{
    ActiveDynamicSessionSource, GroupProgrammerValue, GroupReleaseProgrammerValue,
    ProgrammerOutputSourceCapture, ProgrammerOutputState,
};

use crate::{ContributionSourceId, Engine};

type GroupValues = HashMap<String, HashMap<AttributeKey, GroupProgrammerValue>>;

/// The authored Programmer source lane. Both lanes use one logical Dynamic controller clock.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CapturedDynamicProgrammerLane {
    Live,
    Preload,
}

/// Provenance of one captured Dynamic Programmer tuple at the same vector index.
///
/// Consumers must zip this with the matching captured value vector, never join it to sampled
/// values by equality or rank. An out-of-range persisted timestamp has no representable
/// `ProgrammerEditStamp`; its exact raw fields remain available instead of a guessed time.
#[derive(Clone, Debug)]
pub struct CapturedDynamicProgrammerRow {
    pub programmer_id: ProgrammerId,
    pub lane: CapturedDynamicProgrammerLane,
    pub source: ContributionSourceId,
    pub stamp: Option<ProgrammerEditStamp>,
    pub changed_at_millis: u64,
    pub programmer_order: u64,
}

type CapturedDynamicValues = Vec<(uuid::Uuid, i16, DynamicAddressValue)>;
type CapturedDynamicRows = Vec<CapturedDynamicProgrammerRow>;

struct CapturedDynamicPair {
    values: CapturedDynamicValues,
    rows: CapturedDynamicRows,
}

impl CapturedDynamicPair {
    fn into_arcs(self) -> (Arc<CapturedDynamicValues>, Arc<CapturedDynamicRows>) {
        (Arc::new(self.values), Arc::new(self.rows))
    }

    fn push(&mut self, id: uuid::Uuid, priority: i16, value: &DynamicAddressValue, preload: bool) {
        let programmer_id = ProgrammerId(id);
        let (lane, source) = if preload {
            (
                CapturedDynamicProgrammerLane::Preload,
                ContributionSourceId::preload(programmer_id),
            )
        } else {
            (
                CapturedDynamicProgrammerLane::Live,
                ContributionSourceId::programmer(programmer_id),
            )
        };
        let stamp = i64::try_from(value.changed_at_millis)
            .ok()
            .and_then(chrono::DateTime::<chrono::Utc>::from_timestamp_millis)
            .map(|changed_at| ProgrammerEditStamp {
                changed_at,
                programmer_order: value.programmer_order,
            });
        self.rows.push(CapturedDynamicProgrammerRow {
            programmer_id,
            lane,
            source,
            stamp,
            changed_at_millis: value.changed_at_millis,
            programmer_order: value.programmer_order,
        });
        self.values.push((id, priority, value.clone()));
    }
}

/// Freeze normal Dynamic Programmer tuples and their source rows from one immutable capture.
/// The legacy static Release cutoff is applied exactly as in the normal output projection.
pub fn capture_dynamic_programmer_rows(
    capture: &ProgrammerOutputSourceCapture,
) -> (
    Arc<Vec<(uuid::Uuid, i16, DynamicAddressValue)>>,
    Arc<Vec<CapturedDynamicProgrammerRow>>,
) {
    capture_dynamic_programmer_rows_from_sources(&capture.normal_dynamics)
}

pub(crate) fn capture_dynamic_programmer_rows_from_sources(
    sources: &[ActiveDynamicSessionSource],
) -> (
    Arc<Vec<(uuid::Uuid, i16, DynamicAddressValue)>>,
    Arc<Vec<CapturedDynamicProgrammerRow>>,
) {
    complete_dynamic_rows_from_sources(sources, None, true).into_arcs()
}

/// The exact captured source rows used by an observer before and after pending Color Release.
/// Static Release masks older unlinked rows only for the owning Programmer in each branch.
/// Linked Dynamic commands, newer edits and other Programmers retain their captured rows.
#[derive(Clone, Debug)]
pub struct PreparedPreloadSources {
    pub before: Vec<ProgrammerOutputState>,
    pub after: Vec<ProgrammerOutputState>,
    /// Complete branch inputs for Dynamic reconciliation. Replace the frame's captured Dynamic
    /// vector with these; appending would duplicate its already-active Preload rows.
    pub dynamic_values_before: Arc<Vec<(uuid::Uuid, i16, DynamicAddressValue)>>,
    pub dynamic_values_after: Arc<Vec<(uuid::Uuid, i16, DynamicAddressValue)>>,
    /// Index-aligned source and exact edit stamp for each complete Dynamic tuple branch.
    pub dynamic_rows_before: Arc<Vec<CapturedDynamicProgrammerRow>>,
    pub dynamic_rows_after: Arc<Vec<CapturedDynamicProgrammerRow>>,
    pub pending_dynamic_before: Arc<Vec<DynamicAddressValue>>,
    pub pending_dynamic_after: Arc<Vec<DynamicAddressValue>>,
    pub touched_fixtures: HashSet<(FixtureId, AttributeKey)>,
    pub touched_groups: HashSet<(String, AttributeKey)>,
    pub has_pending: bool,
}

#[derive(Default)]
pub(crate) struct PreloadSourceMemo {
    // Retain the source Arcs themselves: numeric pointer addresses alone can be reused by an
    // allocator after a former capture is dropped.
    last: Option<(ProgrammerOutputSourceCapture, Arc<PreparedPreloadSources>)>,
}

fn same_sources(a: &ProgrammerOutputSourceCapture, b: &ProgrammerOutputSourceCapture) -> bool {
    if a.identity != b.identity
        || a.normal_values_generation != b.normal_values_generation
        || a.preload_values_generation != b.preload_values_generation
        || a.preload_playback_queue_generation != b.preload_playback_queue_generation
        || !Arc::ptr_eq(&a.preload_playback_actions, &b.preload_playback_actions)
        || a.priority != b.priority
        || a.output_states.len() != b.output_states.len()
        || a.normal_dynamics.len() != b.normal_dynamics.len()
        || !a.normal_dynamics.iter().zip(&b.normal_dynamics).all(
            |((a_id, a_priority, a_normal, a_preload), (b_id, b_priority, b_normal, b_preload))| {
                a_id == b_id
                    && a_priority == b_priority
                    && Arc::ptr_eq(a_normal, b_normal)
                    && Arc::ptr_eq(a_preload, b_preload)
            },
        )
        || !a.output_states.iter().zip(&b.output_states).all(|(a, b)| {
            a.id == b.id
                && a.priority == b.priority
                && Arc::ptr_eq(&a.values, &b.values)
                && Arc::ptr_eq(&a.transient_values, &b.transient_values)
                && Arc::ptr_eq(&a.group_values, &b.group_values)
                && Arc::ptr_eq(&a.preload_active, &b.preload_active)
                && Arc::ptr_eq(&a.preload_group_active, &b.preload_group_active)
                && Arc::ptr_eq(&a.preload_dynamic_active, &b.preload_dynamic_active)
                && Arc::ptr_eq(
                    &a.preload_group_release_active,
                    &b.preload_group_release_active,
                )
        })
    {
        return false;
    }
    match (&a.preload, &b.preload) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            Arc::ptr_eq(&a.pending, &b.pending)
                && Arc::ptr_eq(&a.active_values, &b.active_values)
                && Arc::ptr_eq(&a.active_groups, &b.active_groups)
                && Arc::ptr_eq(&a.active_dynamics, &b.active_dynamics)
                && Arc::ptr_eq(&a.active_group_releases, &b.active_group_releases)
        }
        _ => false,
    }
}

impl PreloadSourceMemo {
    fn prepare(&mut self, capture: &ProgrammerOutputSourceCapture) -> Arc<PreparedPreloadSources> {
        if let Some((old, prepared)) = &self.last {
            if same_sources(old, capture) {
                return Arc::clone(prepared);
            }
        }
        let prepared = Arc::new(prepare_uncached(capture));
        self.last = Some((capture.clone(), Arc::clone(&prepared)));
        prepared
    }
}

impl Engine {
    /// Compile captured Preload sources once per edit boundary. No live Programmer read occurs.
    pub fn prepare_preload_sources(
        &self,
        capture: &ProgrammerOutputSourceCapture,
    ) -> Arc<PreparedPreloadSources> {
        self.preload_sources.lock().prepare(capture)
    }
}

fn prepare_uncached(capture: &ProgrammerOutputSourceCapture) -> PreparedPreloadSources {
    let Some(preload) = capture.preload.as_ref() else {
        let (dynamic_values, dynamic_rows) = complete_dynamic_rows(capture, None, true).into_arcs();
        return PreparedPreloadSources {
            before: capture.output_states.clone(),
            after: capture.output_states.clone(),
            dynamic_values_before: Arc::clone(&dynamic_values),
            dynamic_values_after: dynamic_values,
            dynamic_rows_before: Arc::clone(&dynamic_rows),
            dynamic_rows_after: dynamic_rows,
            pending_dynamic_before: Arc::default(),
            pending_dynamic_after: Arc::default(),
            touched_fixtures: HashSet::new(),
            touched_groups: HashSet::new(),
            has_pending: !capture.preload_playback_actions.is_empty(),
        };
    };
    let pending = &preload.pending;
    let mut touched_fixtures = HashSet::new();
    let mut touched_groups = HashSet::new();
    for value in pending.fixture_values.iter() {
        touched_fixtures.insert((value.fixture_id, value.attribute.clone()));
    }
    for value in pending.dynamic_values.iter() {
        touched_fixtures.insert((value.fixture_id, value.attribute.clone()));
    }
    for (group, values) in pending.group_values.iter() {
        for attribute in values.keys() {
            touched_groups.insert((group.clone(), attribute.clone()));
        }
    }
    for release in pending.group_release_values.iter() {
        touched_groups.insert((release.group_id.clone(), release.attribute.clone()));
    }
    let has_pending = !touched_fixtures.is_empty()
        || !touched_groups.is_empty()
        || !pending.released_colors.is_empty()
        || !capture.preload_playback_actions.is_empty();

    let mut before_values = pending.fixture_values.as_ref().clone();
    let mut before_groups = pending.group_values.as_ref().clone();
    let mut before_dynamics: Vec<_> = pending
        .dynamic_values
        .iter()
        .filter(|value| !is_release(&value.value))
        .cloned()
        .collect();
    for candidate in pending.released_colors.fixtures.values() {
        if let Some(value) = &candidate.value {
            before_values.push(value.clone());
        }
        if let Some(value) = &candidate.fixed {
            before_dynamics.push(value.clone());
        }
        before_dynamics.extend(candidate.fixed_components.iter().cloned());
    }
    for (group, value) in &pending.released_colors.groups {
        before_groups
            .entry(group.clone())
            .or_default()
            .insert(AttributeKey("color".into()), value.clone());
    }
    let effective_before = merge_dynamic(&preload.active_dynamics, &before_dynamics);
    let effective_after = merge_dynamic(&preload.active_dynamics, &pending.dynamic_values);
    let (dynamic_values_before, dynamic_rows_before) =
        complete_dynamic_rows(capture, Some(&effective_before), true).into_arcs();
    let (dynamic_values_after, dynamic_rows_after) =
        complete_dynamic_rows(capture, Some(&effective_after), true).into_arcs();

    let mut before = capture.output_states.clone();
    let mut after = capture.output_states.clone();
    for (before_state, after_state) in before.iter_mut().zip(after.iter_mut()) {
        if Some(before_state.id) != capture.identity {
            continue;
        }
        before_state.preload_active =
            Arc::new(merge_fixture_values(&preload.active_values, &before_values));
        before_state.preload_group_active =
            Arc::new(merge_group_values(&preload.active_groups, &before_groups));
        before_state.preload_dynamic_active = Arc::new(effective_before.clone());
        before_state.preload_group_release_active = Arc::clone(&preload.active_group_releases);
        after_state.preload_active = Arc::new(merge_fixture_values(
            &preload.active_values,
            &pending.fixture_values,
        ));
        after_state.preload_group_active = Arc::new(merge_group_values(
            &preload.active_groups,
            &pending.group_values,
        ));
        after_state.preload_dynamic_active = Arc::new(effective_after.clone());
        after_state.preload_group_release_active = Arc::new(merge_group_releases(
            &preload.active_group_releases,
            &pending.group_release_values,
        ));
    }
    PreparedPreloadSources {
        before,
        after,
        dynamic_values_before,
        dynamic_values_after,
        dynamic_rows_before,
        dynamic_rows_after,
        pending_dynamic_before: Arc::new(before_dynamics),
        pending_dynamic_after: Arc::clone(&pending.dynamic_values),
        touched_fixtures,
        touched_groups,
        has_pending,
    }
}

fn is_release(value: &DynamicSemanticValue) -> bool {
    matches!(
        value,
        DynamicSemanticValue::Release | DynamicSemanticValue::ProgrammingRelease { .. }
    )
}

fn later(
    new_order: u64,
    new_time: chrono::DateTime<chrono::Utc>,
    old_order: u64,
    old_time: chrono::DateTime<chrono::Utc>,
) -> bool {
    ProgrammerEditStamp {
        changed_at: new_time,
        programmer_order: new_order,
    }
    .supersedes(old_time, old_order)
}

fn immediate(mut value: TimedValue) -> TimedValue {
    value.fade = false;
    value.fade_millis = None;
    value.delay_millis = None;
    value
}

fn merge_fixture_values(active: &[TimedValue], pending: &[TimedValue]) -> Vec<TimedValue> {
    let mut result = Vec::with_capacity(active.len() + pending.len());
    let mut positions = HashMap::new();
    for (value, is_pending) in active
        .iter()
        .map(|v| (v, false))
        .chain(pending.iter().map(|v| (v, true)))
    {
        let key = (value.fixture_id, value.attribute.clone());
        let candidate = if is_pending {
            immediate(value.clone())
        } else {
            value.clone()
        };
        if let Some(&index) = positions.get(&key) {
            let old: &TimedValue = &result[index];
            if later(
                candidate.programmer_order,
                candidate.changed_at,
                old.programmer_order,
                old.changed_at,
            ) {
                result[index] = candidate;
            }
        } else {
            positions.insert(key, result.len());
            result.push(candidate);
        }
    }
    result
}

fn merge_group_values(active: &GroupValues, pending: &GroupValues) -> GroupValues {
    let mut result = active.clone();
    for (group, values) in pending {
        let destination = result.entry(group.clone()).or_default();
        for (attribute, value) in values {
            let replace = destination.get(attribute).is_none_or(|old| {
                later(
                    value.programmer_order,
                    value.changed_at,
                    old.programmer_order,
                    old.changed_at,
                )
            });
            if replace {
                let mut value = value.clone();
                value.fade = false;
                value.fade_millis = None;
                value.delay_millis = None;
                destination.insert(attribute.clone(), value);
            }
        }
    }
    result
}

fn merge_dynamic(
    active: &[DynamicAddressValue],
    pending: &[DynamicAddressValue],
) -> Vec<DynamicAddressValue> {
    merge_dynamic_address_values(active.iter().chain(pending))
        .into_iter()
        .cloned()
        .collect()
}

fn complete_dynamic_rows(
    capture: &ProgrammerOutputSourceCapture,
    effective_preload: Option<&[DynamicAddressValue]>,
    normal_release_cutoff: bool,
) -> CapturedDynamicPair {
    complete_dynamic_rows_from_sources(
        &capture.normal_dynamics,
        capture
            .identity
            .and_then(|owner| effective_preload.map(|rows| (owner.0, rows))),
        normal_release_cutoff,
    )
}

fn complete_dynamic_rows_from_sources(
    sources: &[ActiveDynamicSessionSource],
    effective_preload: Option<(uuid::Uuid, &[DynamicAddressValue])>,
    normal_release_cutoff: bool,
) -> CapturedDynamicPair {
    let mut result = CapturedDynamicPair {
        values: Vec::new(),
        rows: Vec::new(),
    };
    for (id, priority, normal, active) in sources {
        // Pending rows belong to one Programmer. Never substitute them into another owner's
        // rows: that would duplicate Fixed sources and apply its Release cutoff across desks.
        let preload = effective_preload
            .filter(|(owner, _)| owner == id)
            .map_or(active.as_slice(), |(_, rows)| rows);
        // Typed Fixed values are composed after static resolution, so they need the same
        // source-local Release cutoff here rather than relying on the static resolver alone.
        let releases = normal_release_cutoff
            .then(|| {
                preload
                    .iter()
                    .filter(|value| matches!(value.value, DynamicSemanticValue::Release))
                    .filter_map(|value| {
                        crate::programmer_release::release_cutoff(
                            value.changed_at_millis,
                            value.programmer_order,
                        )
                        .map(|cutoff| ((value.fixture_id, value.attribute.clone()), cutoff))
                    })
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        for (value, is_preload) in normal
            .iter()
            .map(|value| (value, false))
            .chain(preload.iter().map(|value| (value, true)))
        {
            if value.value.track_key().instance_link.is_none()
                && releases
                    .get(&(value.fixture_id, value.attribute.clone()))
                    .is_some_and(|cutoff| {
                        crate::programmer_release::release_cutoff(
                            value.changed_at_millis,
                            value.programmer_order,
                        )
                        .is_some_and(|authored| {
                            cutoff.supersedes(authored.changed_at, authored.programmer_order)
                        })
                    })
            {
                continue;
            }
            result.push(*id, *priority, value, is_preload);
        }
    }
    result
}

fn merge_group_releases(
    active: &[GroupReleaseProgrammerValue],
    pending: &[GroupReleaseProgrammerValue],
) -> Vec<GroupReleaseProgrammerValue> {
    let mut result = Vec::with_capacity(active.len() + pending.len());
    result.extend_from_slice(active);
    result.extend_from_slice(pending);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::{AttributeValue, SessionId};
    use light_dynamics::{
        ActivationBoundary, ActivationPolicy, DynamicDefinition, DynamicDefinitionSnapshot,
        DynamicInstanceOverrides, DynamicPhaseSpreadMode, DynamicReference, DynamicRunMode,
        DynamicSpeed, DynamicTargetBinding, DynamicValueTiming, PhaseDistribution, PhaseOrdering,
        Rational,
    };
    use light_programmer::{
        PreloadProgrammerValueMutation, ProgrammerRegistry, ReleasedPreloadFixtureColor,
    };

    fn captured_pending() -> (ProgrammerOutputSourceCapture, FixtureId) {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let fixture = FixtureId::new();
        registry.start(session);
        registry.arm_preload(session, true);
        assert!(registry.apply_preload_values(
            session,
            &[PreloadProgrammerValueMutation::SetFixture {
                fixture_id: fixture,
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Normalized(0.4),
                timing: Default::default(),
            }],
        ));
        (registry.capture_output_sources(), fixture)
    }

    fn dynamic_row(
        fixture: FixtureId,
        attribute: AttributeKey,
        instance: uuid::Uuid,
        lane: uuid::Uuid,
        order: u64,
    ) -> DynamicAddressValue {
        let definition = DynamicDefinition {
            id: uuid::Uuid::from_u128(0xd1),
            pool_number: 1,
            revision: 1,
            name: "source preparation".into(),
            color: None,
            icon: None,
            target_binding: DynamicTargetBinding::Targetless,
            lanes: vec![],
            random_groups: vec![],
            phase_spread_mode: DynamicPhaseSpreadMode::Uniform,
            spatial_mapping: Default::default(),
            phase: PhaseDistribution {
                ordering: PhaseOrdering::Selection,
                offset_degrees: 0.0,
                span_degrees: 360.0,
                block_size: 1,
                repeats: 1,
                wings: false,
                anchors_degrees: vec![],
            },
            speed: DynamicSpeed::Fixed {
                duration_millis: 1_000,
            },
            overall_speed_multiplier: Rational::ONE,
            run_mode: DynamicRunMode::Loop,
            default_activation: ActivationPolicy::StartNow,
            activation_boundary: ActivationBoundary::Beat,
        };
        DynamicAddressValue {
            fixture_id: fixture,
            attribute,
            value: DynamicSemanticValue::DynamicOn {
                instance_link: instance,
                dynamic: DynamicReference {
                    dynamic_id: Some(definition.id),
                    last_known_pool_number: 1,
                    embedded_fallback: DynamicDefinitionSnapshot {
                        definition: Arc::new(definition),
                    },
                },
                lane_id: lane,
                overrides: DynamicInstanceOverrides {
                    size: 1.0,
                    speed_multiplier: Rational::ONE,
                    phase_offset_degrees: 0.0,
                },
                timing: DynamicValueTiming::default(),
            },
            programmer_order: order,
            changed_at_millis: order,
        }
    }

    #[test]
    fn dynamic_merge_replaces_exact_track_but_preserves_sibling_angle_lane() {
        let fixture = FixtureId::new();
        let instance = uuid::Uuid::new_v4();
        let pan = uuid::Uuid::new_v4();
        let tilt = uuid::Uuid::new_v4();
        let owner = AttributeKey("position".into());
        let active = vec![
            dynamic_row(fixture, owner.clone(), instance, pan, 1),
            dynamic_row(fixture, owner.clone(), instance, tilt, 2),
        ];
        let pending = vec![dynamic_row(fixture, owner, instance, pan, 3)];
        let merged = merge_dynamic(&active, &pending);
        assert_eq!(merged.len(), 2);
        assert!(merged.iter().any(|row| row.programmer_order == 3
            && matches!(row.value, DynamicSemanticValue::DynamicOn { lane_id, .. } if lane_id == pan)));
        assert!(merged.iter().any(|row| row.programmer_order == 2
            && matches!(row.value, DynamicSemanticValue::DynamicOn { lane_id, .. } if lane_id == tilt)));
    }

    #[test]
    fn dynamic_off_sweeps_instance_and_later_on_clears_old_off() {
        let fixture = FixtureId::new();
        let instance = uuid::Uuid::new_v4();
        let active = vec![
            dynamic_row(
                fixture,
                AttributeKey("position".into()),
                instance,
                uuid::Uuid::new_v4(),
                1,
            ),
            dynamic_row(
                fixture,
                AttributeKey("color".into()),
                instance,
                uuid::Uuid::new_v4(),
                2,
            ),
        ];
        let off = DynamicAddressValue {
            fixture_id: FixtureId::new(),
            attribute: AttributeKey("position".into()),
            value: DynamicSemanticValue::DynamicOff {
                instance_link: instance,
                timing: DynamicValueTiming::default(),
            },
            programmer_order: 3,
            changed_at_millis: 3,
        };
        assert_eq!(merge_dynamic(&active, &[off.clone()]), vec![off.clone()]);
        let newer = dynamic_row(
            fixture,
            AttributeKey("position".into()),
            instance,
            uuid::Uuid::new_v4(),
            4,
        );
        assert_eq!(merge_dynamic(&[off], &[newer.clone()]), vec![newer]);

        // A pending Off whose edit stamp falls between two active rows still removes the older
        // track. The later sibling must survive without reviving that older track.
        let older = dynamic_row(
            fixture,
            AttributeKey("position".into()),
            instance,
            uuid::Uuid::new_v4(),
            1,
        );
        let later_sibling = dynamic_row(
            fixture,
            AttributeKey("position".into()),
            instance,
            uuid::Uuid::new_v4(),
            3,
        );
        let middle_off = DynamicAddressValue {
            fixture_id: fixture,
            attribute: AttributeKey("position".into()),
            value: DynamicSemanticValue::DynamicOff {
                instance_link: instance,
                timing: DynamicValueTiming::default(),
            },
            programmer_order: 2,
            changed_at_millis: 2,
        };
        assert_eq!(
            merge_dynamic(&[older, later_sibling.clone()], &[middle_off]),
            vec![later_sibling]
        );
    }

    #[test]
    fn branch_dynamic_tuples_keep_normal_once_and_replace_active_preload_once() {
        let (mut capture, fixture) = captured_pending();
        let owner = AttributeKey("position".into());
        let normal = DynamicAddressValue {
            fixture_id: fixture,
            attribute: AttributeKey("focus".into()),
            value: DynamicSemanticValue::FixAt {
                value: 0.3,
                timing: DynamicValueTiming::default(),
            },
            programmer_order: 1,
            changed_at_millis: 1,
        };
        let active = dynamic_row(
            fixture,
            owner.clone(),
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            2,
        );
        let mut pending = active.clone();
        pending.programmer_order = 3;
        pending.changed_at_millis = 3;
        capture.normal_dynamics[0].2 = Arc::new(vec![normal.clone()]);
        capture.normal_dynamics[0].3 = Arc::new(vec![active.clone()]);
        let preload = capture.preload.as_mut().unwrap();
        preload.active_dynamics = Arc::new(vec![active]);
        Arc::make_mut(&mut preload.pending).dynamic_values = Arc::new(vec![pending.clone()]);
        let prepared = prepare_uncached(&capture);
        assert_eq!(prepared.dynamic_values_before.len(), 2);
        assert_eq!(prepared.dynamic_values_after.len(), 2);
        assert_eq!(prepared.dynamic_values_after[0].2, normal);
        assert_eq!(prepared.dynamic_values_after[1].2, pending);
        let programmer_id = capture.identity.unwrap();
        assert_eq!(
            prepared.dynamic_rows_before.len(),
            prepared.dynamic_values_before.len()
        );
        assert_eq!(
            prepared.dynamic_rows_after.len(),
            prepared.dynamic_values_after.len()
        );
        assert_eq!(
            prepared.dynamic_rows_after[0].source,
            ContributionSourceId::programmer(programmer_id)
        );
        assert_eq!(prepared.dynamic_rows_after[0].programmer_id, programmer_id);
        assert_eq!(
            prepared.dynamic_rows_after[0].lane,
            CapturedDynamicProgrammerLane::Live
        );
        assert_eq!(
            prepared.dynamic_rows_after[1].source,
            ContributionSourceId::preload(programmer_id)
        );
        assert_eq!(prepared.dynamic_rows_after[1].programmer_id, programmer_id);
        assert_eq!(
            prepared.dynamic_rows_after[1].lane,
            CapturedDynamicProgrammerLane::Preload
        );
        assert_eq!(prepared.dynamic_rows_after[1].programmer_order, 3);
        assert_eq!(prepared.dynamic_rows_after[1].changed_at_millis, 3);
        assert_eq!(
            prepared.dynamic_rows_after[1]
                .stamp
                .unwrap()
                .programmer_order,
            3
        );
        assert!(
            prepared
                .dynamic_values_after
                .iter()
                .all(|(id, priority, _)| *id == capture.identity.unwrap().0
                    && Some(*priority) == capture.priority)
        );
    }

    #[test]
    fn captured_normal_rows_keep_release_cutoff_and_exact_source_stamps() {
        let (mut capture, fixture) = captured_pending();
        let attribute = AttributeKey("focus".into());
        let fixed = DynamicAddressValue {
            fixture_id: fixture,
            attribute: attribute.clone(),
            value: DynamicSemanticValue::FixAt {
                value: 0.3,
                timing: DynamicValueTiming::default(),
            },
            programmer_order: 1,
            changed_at_millis: 1,
        };
        let release = DynamicAddressValue {
            fixture_id: fixture,
            attribute,
            value: DynamicSemanticValue::Release,
            programmer_order: 2,
            changed_at_millis: 2,
        };
        capture.normal_dynamics[0].2 = Arc::new(vec![fixed]);
        capture.normal_dynamics[0].3 = Arc::new(vec![release.clone()]);
        let (values, rows) = capture_dynamic_programmer_rows(&capture);
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].2, release);
        assert_eq!(rows.len(), values.len());
        assert_eq!(
            rows[0].source,
            ContributionSourceId::preload(capture.identity.unwrap())
        );
        assert_eq!(rows[0].stamp.unwrap().programmer_order, 2);
        assert_eq!(rows[0].stamp.unwrap().changed_at.timestamp_millis(), 2);

        let mut unrepresentable = values[0].2.clone();
        unrepresentable.changed_at_millis = u64::MAX;
        capture.normal_dynamics[0].3 = Arc::new(vec![unrepresentable]);
        let (values, rows) = capture_dynamic_programmer_rows(&capture);
        assert_eq!(values.len(), 2);
        assert!(rows[1].stamp.is_none());
        assert_eq!(rows[1].changed_at_millis, u64::MAX);
        assert_eq!(rows[1].programmer_order, 2);
    }

    #[test]
    fn color_release_branches_keep_the_winning_preload_row_identity() {
        let (mut capture, fixture) = captured_pending();
        let color = AttributeKey("color".into());
        let fixed = DynamicAddressValue {
            fixture_id: fixture,
            attribute: color.clone(),
            value: DynamicSemanticValue::FixAt {
                value: 0.4,
                timing: DynamicValueTiming::default(),
            },
            programmer_order: 3,
            changed_at_millis: 3,
        };
        let release = DynamicAddressValue {
            fixture_id: fixture,
            attribute: color,
            value: DynamicSemanticValue::Release,
            programmer_order: 4,
            changed_at_millis: 4,
        };
        let mut active = fixed.clone();
        active.value = DynamicSemanticValue::FixAt {
            value: 0.2,
            timing: DynamicValueTiming::default(),
        };
        active.programmer_order = 1;
        active.changed_at_millis = 1;
        let preload = capture.preload.as_mut().unwrap();
        preload.active_dynamics = Arc::new(vec![active]);
        let pending = Arc::make_mut(&mut preload.pending);
        pending.dynamic_values = Arc::new(vec![release.clone()]);
        Arc::make_mut(&mut pending.released_colors).fixtures.insert(
            fixture,
            ReleasedPreloadFixtureColor {
                fixed: Some(fixed.clone()),
                ..Default::default()
            },
        );
        let prepared = prepare_uncached(&capture);
        let programmer_id = capture.identity.unwrap();
        assert_eq!(prepared.dynamic_values_before.len(), 1);
        assert_eq!(prepared.dynamic_values_before[0].2, fixed);
        assert_eq!(prepared.dynamic_rows_before.len(), 1);
        assert_eq!(
            prepared.dynamic_rows_before[0].source,
            ContributionSourceId::preload(programmer_id)
        );
        assert_eq!(
            prepared.dynamic_rows_before[0]
                .stamp
                .unwrap()
                .programmer_order,
            3
        );
        assert_eq!(prepared.dynamic_values_after.len(), 1);
        assert_eq!(prepared.dynamic_values_after[0].2, release);
        assert_eq!(prepared.dynamic_rows_after.len(), 1);
        assert_eq!(
            prepared.dynamic_rows_after[0].source,
            ContributionSourceId::preload(programmer_id)
        );
        assert_eq!(
            prepared.dynamic_rows_after[0]
                .stamp
                .unwrap()
                .programmer_order,
            4
        );
    }

    #[test]
    fn pending_release_cuts_only_older_unlinked_rows_of_its_programmer() {
        let (mut capture, fixture) = captured_pending();
        let owner = capture.identity.unwrap().0;
        let color = AttributeKey("color".into());
        let fixed = DynamicAddressValue {
            fixture_id: fixture,
            attribute: color.clone(),
            value: DynamicSemanticValue::FixAt {
                value: 0.3,
                timing: Default::default(),
            },
            programmer_order: 1,
            changed_at_millis: 100,
        };
        let release = DynamicAddressValue {
            value: DynamicSemanticValue::Release,
            programmer_order: 2,
            ..fixed.clone()
        };
        let newer = DynamicAddressValue {
            programmer_order: 3,
            ..fixed.clone()
        };
        let other = DynamicAddressValue {
            fixture_id: FixtureId::new(),
            ..fixed.clone()
        };
        let linked = dynamic_row(
            fixture,
            color,
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            1,
        );
        capture.normal_dynamics[0].2 = Arc::new(vec![
            fixed.clone(),
            newer.clone(),
            other.clone(),
            linked.clone(),
        ]);
        let foreign = uuid::Uuid::new_v4();
        capture
            .normal_dynamics
            .push((foreign, 1, Arc::new(vec![fixed.clone()]), Arc::new(vec![])));
        Arc::make_mut(&mut capture.preload.as_mut().unwrap().pending).dynamic_values =
            Arc::new(vec![release.clone()]);
        let prepared = prepare_uncached(&capture);
        let owned = |rows: &CapturedDynamicValues| {
            rows.iter()
                .filter(|(id, _, _)| *id == owner)
                .map(|(_, _, row)| row.clone())
                .collect::<Vec<_>>()
        };
        assert!(owned(&prepared.dynamic_values_before).contains(&fixed));
        let after = owned(&prepared.dynamic_values_after);
        assert!(!after.contains(&fixed));
        for retained in [newer, other, linked, release] {
            assert!(after.contains(&retained));
        }
        for (values, rows) in [
            (
                &prepared.dynamic_values_before,
                &prepared.dynamic_rows_before,
            ),
            (&prepared.dynamic_values_after, &prepared.dynamic_rows_after),
        ] {
            assert_eq!(values.len(), rows.len());
            for ((id, _, value), row) in values.iter().zip(rows.iter()) {
                assert_eq!(*id, row.programmer_id.0);
                assert_eq!(value.programmer_order, row.programmer_order);
                assert_eq!(value.changed_at_millis, row.changed_at_millis);
            }
            let foreign_rows = values
                .iter()
                .filter(|(id, _, _)| *id == foreign)
                .collect::<Vec<_>>();
            assert_eq!(foreign_rows.len(), 1);
            assert_eq!(foreign_rows[0].2, fixed);
        }
    }

    #[test]
    fn committed_release_remains_effective_in_both_pending_branches_and_without_pending() {
        let (mut capture, fixture) = captured_pending();
        let fixed = DynamicAddressValue {
            fixture_id: fixture,
            attribute: AttributeKey("color".into()),
            value: DynamicSemanticValue::FixAt {
                value: 0.4,
                timing: Default::default(),
            },
            programmer_order: 1,
            changed_at_millis: 1,
        };
        let release = DynamicAddressValue {
            value: DynamicSemanticValue::Release,
            programmer_order: 2,
            ..fixed.clone()
        };
        capture.normal_dynamics[0].2 = Arc::new(vec![fixed]);
        capture.normal_dynamics[0].3 = Arc::new(vec![release.clone()]);
        capture.preload.as_mut().unwrap().active_dynamics = Arc::new(vec![release.clone()]);
        for pending in [true, false] {
            if !pending {
                capture.preload = None;
            }
            let prepared = prepare_uncached(&capture);
            for rows in [
                &prepared.dynamic_values_before,
                &prepared.dynamic_values_after,
            ] {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].2, release);
            }
        }
    }

    #[test]
    fn queue_only_edits_invalidate_pending_presence_without_fixture_edits() {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        registry.start(session);
        let mut memo = PreloadSourceMemo::default();
        let empty = memo.prepare(&registry.capture_output_sources());
        assert!(!empty.has_pending);
        registry.queue_preload_playback_action(
            session,
            7,
            Some(3),
            light_programmer::PreloadPlaybackQueueAction::Go,
            light_programmer::PreloadPlaybackQueueSurface::Virtual,
        );
        let queued = memo.prepare(&registry.capture_output_sources());
        assert!(queued.has_pending);
        assert!(!Arc::ptr_eq(&empty, &queued));
        assert!(queued.touched_fixtures.is_empty());
        registry.take_preload_playback_actions(session);
        assert!(!memo.prepare(&registry.capture_output_sources()).has_pending);
        assert!(queued.has_pending);
    }

    #[test]
    fn repeated_capture_reuses_merged_arcs_and_old_capture_is_immutable() {
        let (mut capture, fixture) = captured_pending();
        let mut memo = PreloadSourceMemo::default();
        let first = memo.prepare(&capture);
        let repeated = memo.prepare(&capture);
        assert!(Arc::ptr_eq(&first, &repeated));
        assert_eq!(
            first.after[0].preload_active[0].value.normalized(),
            Some(0.4)
        );

        let preload = capture.preload.as_mut().unwrap();
        let pending = Arc::make_mut(&mut preload.pending);
        let mut edited = pending.fixture_values[0].clone();
        edited.value = AttributeValue::Normalized(0.8);
        edited.programmer_order += 1;
        pending.fixture_values = Arc::new(vec![edited]);
        capture.preload_values_generation += 1;
        let next = memo.prepare(&capture);
        assert!(!Arc::ptr_eq(
            &first.after[0].preload_active,
            &next.after[0].preload_active
        ));
        assert_eq!(
            first.after[0].preload_active[0].value.normalized(),
            Some(0.4)
        );
        assert_eq!(
            next.after[0].preload_active[0].value.normalized(),
            Some(0.8)
        );
        assert!(
            next.touched_fixtures
                .contains(&(fixture, AttributeKey::intensity()))
        );
    }

    #[test]
    fn changed_captured_dynamic_arc_invalidates_branch_sidecar_cache() {
        let (mut capture, fixture) = captured_pending();
        let mut memo = PreloadSourceMemo::default();
        let first = memo.prepare(&capture);
        let value = DynamicAddressValue {
            fixture_id: fixture,
            attribute: AttributeKey("focus".into()),
            value: DynamicSemanticValue::FixAt {
                value: 0.5,
                timing: DynamicValueTiming::default(),
            },
            programmer_order: 11,
            changed_at_millis: 12,
        };
        capture.normal_dynamics[0].2 = Arc::new(vec![value.clone()]);
        let next = memo.prepare(&capture);
        assert!(!Arc::ptr_eq(&first, &next));
        assert!(first.dynamic_values_after.is_empty());
        assert_eq!(next.dynamic_values_after[0].2, value);
        assert_eq!(next.dynamic_rows_after[0].programmer_order, 11);
        assert_eq!(
            next.dynamic_rows_after[0].source,
            ContributionSourceId::programmer(capture.identity.unwrap())
        );
    }

    #[test]
    fn active_fade_survives_older_pending_and_newer_live_remains_separate() {
        let (mut capture, _) = captured_pending();
        let pending = capture.preload.as_ref().unwrap().pending.fixture_values[0].clone();
        let mut active = pending.clone();
        active.value = AttributeValue::Normalized(0.2);
        active.programmer_order = pending.programmer_order + 1;
        active.fade = true;
        active.fade_millis = Some(800);
        active.delay_millis = Some(100);
        let preload = capture.preload.as_mut().unwrap();
        preload.active_values = Arc::new(vec![active.clone()]);
        let mut live = active.clone();
        live.value = AttributeValue::Normalized(0.9);
        live.programmer_order += 1;
        capture.output_states[0].values = Arc::new(vec![live.clone()]);
        let result = prepare_uncached(&capture);
        assert_eq!(result.after[0].preload_active.len(), 1);
        assert_eq!(
            result.after[0].preload_active[0].value.normalized(),
            Some(0.2)
        );
        assert!(result.after[0].preload_active[0].fade);
        assert_eq!(result.after[0].preload_active[0].fade_millis, Some(800));
        assert_eq!(result.after[0].preload_active[0].delay_millis, Some(100));
        assert_eq!(result.after[0].values[0].value.normalized(), Some(0.9));
        assert_eq!(result.before[0].values[0].value.normalized(), Some(0.9));

        // A later pending target takes this Preload key immediately, while the independently
        // newer Live source remains available to win the final priority/LTP arbitration.
        let pending_source = Arc::make_mut(&mut capture.preload.as_mut().unwrap().pending);
        let mut newer_pending = pending.clone();
        newer_pending.value = AttributeValue::Normalized(0.7);
        newer_pending.programmer_order = active.programmer_order + 1;
        newer_pending.fade = true;
        newer_pending.fade_millis = Some(900);
        newer_pending.delay_millis = Some(200);
        pending_source.fixture_values = Arc::new(vec![newer_pending]);
        let result = prepare_uncached(&capture);
        assert_eq!(result.after[0].preload_active.len(), 1);
        assert_eq!(
            result.after[0].preload_active[0].value.normalized(),
            Some(0.7)
        );
        assert!(!result.after[0].preload_active[0].fade);
        assert_eq!(result.after[0].preload_active[0].fade_millis, None);
        assert_eq!(result.after[0].preload_active[0].delay_millis, None);
        assert_eq!(result.after[0].values[0].value.normalized(), Some(0.9));
    }

    #[test]
    fn color_release_before_retains_only_removed_pending_candidate() {
        let (mut capture, fixture) = captured_pending();
        let color = AttributeKey("color".into());
        let pending = Arc::make_mut(&mut capture.preload.as_mut().unwrap().pending);
        let mut candidate = pending.fixture_values[0].clone();
        candidate.attribute = color.clone();
        candidate.fade = true;
        candidate.fade_millis = Some(400);
        pending.fixture_values = Arc::default();
        Arc::make_mut(&mut pending.released_colors).fixtures.insert(
            fixture,
            ReleasedPreloadFixtureColor {
                value: Some(candidate.clone()),
                ..Default::default()
            },
        );
        pending.dynamic_values = Arc::new(vec![DynamicAddressValue {
            fixture_id: fixture,
            attribute: color.clone(),
            value: DynamicSemanticValue::Release,
            programmer_order: candidate.programmer_order + 1,
            changed_at_millis: candidate.changed_at.timestamp_millis() as u64,
        }]);
        let result = prepare_uncached(&capture);
        assert!(result.has_pending);
        assert!(result.before[0].preload_dynamic_active.is_empty());
        assert_eq!(result.after[0].preload_dynamic_active.len(), 1);
        assert_eq!(result.before[0].preload_active.len(), 1);
        assert_eq!(
            result.before[0].preload_active[0].programmer_order,
            candidate.programmer_order
        );
        assert!(!result.before[0].preload_active[0].fade);
        assert!(result.after[0].preload_active.is_empty());
    }
}
