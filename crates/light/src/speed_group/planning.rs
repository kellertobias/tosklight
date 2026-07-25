use std::collections::BTreeSet;

use crate::{ActionError, ActionErrorKind};

use super::{
    SPEED_GROUP_COUNT, SpeedBpm, SpeedGroupAction, SpeedGroupId, SpeedGroupPortState,
    SpeedGroupProjection, SpeedGroupResolvedAction,
};

pub(super) struct MutationPlan {
    pub resolved: SpeedGroupResolvedAction,
    pub expected: SpeedGroupPortState,
    pub changed: Vec<SpeedGroupId>,
    pub response: Vec<SpeedGroupProjection>,
}

pub(super) fn plan(
    action: SpeedGroupAction,
    before: &SpeedGroupPortState,
    applied_at_millis: u64,
) -> Result<MutationPlan, ActionError> {
    match action {
        SpeedGroupAction::SetBpm { group, bpm } => {
            manual_plan(group, bpm, before, applied_at_millis)
        }
        SpeedGroupAction::AdjustBpm { group, delta } => {
            let current = before.groups[group.index()].manual_bpm;
            let bpm = SpeedBpm::new(current + delta.value()).ok_or_else(|| {
                ActionError::new(
                    ActionErrorKind::Invalid,
                    "relative BPM result must be within 0.1-999",
                )
            })?;
            manual_plan(group, bpm, before, applied_at_millis)
        }
        SpeedGroupAction::Synchronize { source, target } => {
            synchronization_plan(source, target, before, applied_at_millis)
        }
    }
}

fn manual_plan(
    group: SpeedGroupId,
    bpm: SpeedBpm,
    before: &SpeedGroupPortState,
    applied_at_millis: u64,
) -> Result<MutationPlan, ActionError> {
    let mut expected = before.clone();
    let current = expected.groups[group.index()];
    let clean = expected.manual_control_clean.contains(&group);
    let canonical = current.manual_bpm == bpm.value()
        && !current.paused
        && current.speed_master_scale == 1.0
        && current.synchronized_with.is_none()
        && clean;
    if !canonical {
        if let Some(peer) = reciprocal_peer(&expected.groups, group) {
            expected.groups[peer.index()].synchronized_with = None;
        }
        let projection = &mut expected.groups[group.index()];
        projection.manual_bpm = bpm.value();
        projection.paused = false;
        projection.speed_master_scale = 1.0;
        projection.synchronized_with = None;
        projection.phase_origin_millis = applied_at_millis;
        mark_clean(&mut expected.manual_control_clean, group);
    }
    let changed = changed_groups(before, &expected);
    let response = select_groups(&expected.groups, &[group]);
    Ok(MutationPlan {
        resolved: SpeedGroupResolvedAction::SetManualBpm {
            group,
            bpm: bpm.value(),
            applied_at_millis,
        },
        expected,
        changed,
        response,
    })
}

fn synchronization_plan(
    source: SpeedGroupId,
    target: SpeedGroupId,
    before: &SpeedGroupPortState,
    applied_at_millis: u64,
) -> Result<MutationPlan, ActionError> {
    if source == target {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "source and target Speed Groups must be different",
        ));
    }
    let mut expected = before.clone();
    for group in [source, target] {
        if let Some(peer) = reciprocal_peer(&expected.groups, group) {
            expected.groups[peer.index()].synchronized_with = None;
        }
        expected.groups[group.index()].synchronized_with = None;
    }
    let source_projection = before.groups[source.index()];
    for (group, peer) in [(source, target), (target, source)] {
        let projection = &mut expected.groups[group.index()];
        projection.manual_bpm = source_projection.manual_bpm;
        projection.paused = source_projection.paused;
        projection.speed_master_scale = 1.0;
        projection.synchronized_with = Some(peer);
        projection.phase_origin_millis = source_projection.phase_origin_millis;
        mark_clean(&mut expected.manual_control_clean, group);
    }
    let changed = changed_groups(before, &expected);
    let response = select_groups(&expected.groups, &[source, target]);
    Ok(MutationPlan {
        resolved: SpeedGroupResolvedAction::Synchronize {
            source,
            target,
            applied_at_millis,
        },
        expected,
        changed,
        response,
    })
}

pub(super) fn validated_state(
    mut state: SpeedGroupPortState,
) -> Result<SpeedGroupPortState, ActionError> {
    state.groups.sort_by_key(|group| group.group);
    state.manual_control_clean.sort_unstable();
    state.manual_control_clean.dedup();
    let valid = state.groups.len() == SPEED_GROUP_COUNT
        && state
            .groups
            .iter()
            .enumerate()
            .all(|(index, group)| group.group.index() == index)
        && state.groups.iter().all(valid_projection)
        && state
            .manual_control_clean
            .iter()
            .all(|group| group.index() < SPEED_GROUP_COUNT);
    if !valid {
        return Err(ActionError::new(
            ActionErrorKind::Internal,
            "Speed Group adapter returned a malformed authority projection",
        ));
    }
    Ok(state)
}

fn valid_projection(group: &SpeedGroupProjection) -> bool {
    SpeedBpm::new(group.manual_bpm).is_some()
        && group.speed_master_scale.is_finite()
        && (0.0..=4.0).contains(&group.speed_master_scale)
        && group.synchronized_with != Some(group.group)
}

pub(super) fn validate_applied(
    expected: &SpeedGroupPortState,
    applied: &SpeedGroupPortState,
) -> Result<(), ActionError> {
    if applied != expected {
        return Err(ActionError::new(
            ActionErrorKind::Internal,
            "Speed Group adapter returned a non-authoritative projection",
        ));
    }
    Ok(())
}

fn reciprocal_peer(groups: &[SpeedGroupProjection], group: SpeedGroupId) -> Option<SpeedGroupId> {
    let peer = groups[group.index()].synchronized_with?;
    (groups[peer.index()].synchronized_with == Some(group)).then_some(peer)
}

fn mark_clean(clean: &mut Vec<SpeedGroupId>, group: SpeedGroupId) {
    if !clean.contains(&group) {
        clean.push(group);
        clean.sort_unstable();
    }
}

fn changed_groups(before: &SpeedGroupPortState, after: &SpeedGroupPortState) -> Vec<SpeedGroupId> {
    let before_clean = before
        .manual_control_clean
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let after_clean = after
        .manual_control_clean
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    after
        .groups
        .iter()
        .filter(|group| {
            before.groups[group.group.index()] != **group
                || before_clean.contains(&group.group) != after_clean.contains(&group.group)
        })
        .map(|group| group.group)
        .collect()
}

pub(super) fn select_groups(
    groups: &[SpeedGroupProjection],
    identities: &[SpeedGroupId],
) -> Vec<SpeedGroupProjection> {
    let identities = identities.iter().copied().collect::<BTreeSet<_>>();
    groups
        .iter()
        .filter(|group| identities.contains(&group.group))
        .copied()
        .collect()
}
