use crate::*;

type GroupAddress = (String, AttributeKey);
type DynamicTrackAddress = (FixtureId, AttributeKey, light_dynamics::DynamicTrackKey);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum DynamicRestoreScope {
    Instance(Uuid),
    Owner(FixtureId, AttributeKey),
    Component(
        FixtureId,
        AttributeKey,
        Option<light_core::programming::ProgrammingComponent>,
    ),
}

pub(super) fn regenerate_automatic_restorations(cue_list: &mut CueList) {
    strip_automatic_restorations(cue_list);
    let mut fixture_state = HashMap::new();
    let mut group_state = HashMap::new();
    let mut fixture_projections = HashMap::new();
    let mut group_projections = HashMap::new();
    let mut dynamic_state = HashMap::new();
    for index in 0..cue_list.cues.len() {
        // A preceding Cue-only event restores before this Cue's explicit events.
        // Its restored baseline is also the baseline for consecutive Cue-only cues.
        for change in cue_list.cues[index]
            .changes
            .iter()
            .filter(|value| value.automatic_restore)
        {
            apply_fixture_changes(&mut fixture_state, std::slice::from_ref(change));
            track_fixture_projection(&mut fixture_projections, change);
        }
        for change in cue_list.cues[index]
            .group_changes
            .iter()
            .filter(|value| value.automatic_restore)
        {
            apply_group_changes(&mut group_state, std::slice::from_ref(change));
            track_group_projection(&mut group_projections, change);
        }
        for change in cue_list.cues[index]
            .dynamic_changes
            .iter()
            .filter(|value| value.automatic_restore)
        {
            apply_dynamic_changes(&mut dynamic_state, std::slice::from_ref(change));
        }
        let restores = restorations_after(
            cue_list,
            index,
            &fixture_state,
            &group_state,
            &dynamic_state,
            &fixture_projections,
            &group_projections,
        );
        for change in &cue_list.cues[index].changes {
            track_fixture_projection(&mut fixture_projections, change);
        }
        for change in &cue_list.cues[index].group_changes {
            track_group_projection(&mut group_projections, change);
        }
        apply_fixture_changes(&mut fixture_state, &cue_list.cues[index].changes);
        apply_group_changes(&mut group_state, &cue_list.cues[index].group_changes);
        apply_dynamic_changes(&mut dynamic_state, &cue_list.cues[index].dynamic_changes);
        append_restorations(cue_list, index + 1, restores);
    }
}

fn track_fixture_projection(
    state: &mut HashMap<AttributeAddress, Option<light_core::ReplacementProgramProjection>>,
    change: &CueChange,
) {
    if change.value.is_some() {
        state.insert(change.address(), change.replacement_projection.clone());
    } else {
        state.remove(&change.address());
    }
}
fn track_group_projection(
    state: &mut HashMap<GroupAddress, light_core::ReplacementProjectionMap>,
    change: &GroupCueChange,
) {
    if change.value.is_some() {
        state.insert(
            group_address(change),
            change.replacement_projections.clone(),
        );
    } else {
        state.remove(&group_address(change));
    }
}

fn strip_automatic_restorations(cue_list: &mut CueList) {
    for cue in &mut cue_list.cues {
        cue.changes.retain(|change| !change.automatic_restore);
        cue.group_changes.retain(|change| !change.automatic_restore);
        cue.dynamic_changes
            .retain(|change| !change.automatic_restore);
    }
}

fn restorations_after(
    cue_list: &CueList,
    index: usize,
    fixture_state: &HashMap<AttributeAddress, AttributeValue>,
    group_state: &HashMap<GroupAddress, AttributeValue>,
    dynamic_state: &HashMap<DynamicTrackAddress, light_dynamics::DynamicSemanticValue>,
    fixture_projections: &HashMap<
        AttributeAddress,
        Option<light_core::ReplacementProgramProjection>,
    >,
    group_projections: &HashMap<GroupAddress, light_core::ReplacementProjectionMap>,
) -> (Vec<CueChange>, Vec<GroupCueChange>, Vec<CueDynamicChange>) {
    let cue = &cue_list.cues[index];
    let Some(next) = cue_list.cues.get(index + 1).filter(|_| cue.cue_only) else {
        return (Vec::new(), Vec::new(), Vec::new());
    };
    (
        fixture_restorations(cue, next, fixture_state, fixture_projections),
        group_restorations(cue, next, group_state, group_projections),
        dynamic_restorations(cue, next, dynamic_state),
    )
}

fn dynamic_restorations(
    cue: &Cue,
    _next: &Cue,
    state: &HashMap<DynamicTrackAddress, light_dynamics::DynamicSemanticValue>,
) -> Vec<CueDynamicChange> {
    use light_dynamics::{DynamicSemanticValue, DynamicValueTiming};
    let mut restored = Vec::new();
    let mut visited = HashSet::new();
    for change in cue
        .dynamic_changes
        .iter()
        .filter(|change| !change.automatic_restore)
    {
        let key = change.value.track_key();
        // Off addresses the whole instance, including other fixtures and owners.
        let owner_release = cue.dynamic_changes.iter().any(|candidate| {
            !candidate.automatic_restore
                && candidate.fixture_id == change.fixture_id
                && candidate.attribute == change.attribute
                && matches!(candidate.value, DynamicSemanticValue::Release)
        });
        let scope = match key.instance_link {
            Some(instance) => DynamicRestoreScope::Instance(instance),
            None if owner_release => {
                DynamicRestoreScope::Owner(change.fixture_id, change.attribute.clone())
            }
            None => DynamicRestoreScope::Component(
                change.fixture_id,
                change.attribute.clone(),
                key.component,
            ),
        };
        if !visited.insert(scope.clone()) {
            continue;
        }
        let same_scope = |address: &DynamicTrackAddress| match &scope {
            DynamicRestoreScope::Instance(instance) => address.2.instance_link == Some(*instance),
            DynamicRestoreScope::Owner(..) | DynamicRestoreScope::Component(..) => {
                address.2.instance_link.is_none()
                    && address.0 == change.fixture_id
                    && address.1 == change.attribute
                    && (matches!(scope, DynamicRestoreScope::Owner(..))
                        || address.2.component == key.component)
            }
        };
        let mut previous = state
            .iter()
            .filter(|(address, _)| same_scope(address))
            .collect::<Vec<_>>();
        previous.sort_by(|(a, _), (b, _)| {
            a.0.0
                .cmp(&b.0.0)
                .then(a.1.0.cmp(&b.1.0))
                .then(a.2.lane_id.cmp(&b.2.lane_id))
                .then(a.2.component.cmp(&b.2.component))
        });
        if let Some(instance_link) = key.instance_link {
            // A newly introduced lane requires a reset, then replay of the prior
            // complete instance. Restorations precede the next explicit events.
            let introduced_lane = cue.dynamic_changes.iter().any(|candidate| {
                !candidate.automatic_restore
                    && candidate.value.track_key().lane_id.is_some()
                    && same_scope(&dynamic_address(candidate))
                    && !state.contains_key(&dynamic_address(candidate))
            });
            let prior_off = previous
                .iter()
                .any(|(_, value)| matches!(value, DynamicSemanticValue::DynamicOff { .. }));
            if introduced_lane && !prior_off {
                restored.push(CueDynamicChange {
                    fixture_id: change.fixture_id,
                    attribute: change.attribute.clone(),
                    value: DynamicSemanticValue::DynamicOff {
                        instance_link,
                        timing: DynamicValueTiming::default(),
                    },
                    automatic_restore: true,
                });
            }
        } else {
            let mut introduced = HashSet::new();
            for candidate in cue.dynamic_changes.iter().filter(|candidate| {
                !candidate.automatic_restore
                    && !candidate.value.is_programming_release()
                    && same_scope(&dynamic_address(candidate))
                    && !state.contains_key(&dynamic_address(candidate))
            }) {
                let component = candidate.value.track_key().component;
                if !introduced.insert(component) {
                    continue;
                }
                let typed = matches!(
                    candidate.value,
                    DynamicSemanticValue::ProgrammingFixAt { .. }
                );
                restored.push(CueDynamicChange {
                    fixture_id: candidate.fixture_id,
                    attribute: candidate.attribute.clone(),
                    value: if typed {
                        DynamicSemanticValue::ProgrammingRelease { component }
                    } else {
                        DynamicSemanticValue::Release
                    },
                    automatic_restore: true,
                });
            }
        }
        restored.extend(previous.into_iter().map(|((fixture, owner, _), value)| {
            CueDynamicChange {
                fixture_id: *fixture,
                attribute: owner.clone(),
                value: value.clone(),
                automatic_restore: true,
            }
        }));
    }
    restored
}

fn fixture_restorations(
    cue: &Cue,
    next: &Cue,
    state: &HashMap<AttributeAddress, AttributeValue>,
    projections: &HashMap<AttributeAddress, Option<light_core::ReplacementProgramProjection>>,
) -> Vec<CueChange> {
    let explicit = next
        .changes
        .iter()
        .filter(|change| !change.automatic_restore)
        .map(CueChange::address)
        .collect::<HashSet<_>>();
    cue.changes
        .iter()
        .filter(|change| !change.automatic_restore && !explicit.contains(&change.address()))
        .map(|change| CueChange {
            replacement_projection: if state.contains_key(&change.address()) {
                projections.get(&change.address()).cloned().flatten()
            } else {
                change.replacement_projection.clone()
            },
            preset_reference: None,
            fixture_id: change.fixture_id,
            attribute: change.attribute.clone(),
            value: state.get(&change.address()).cloned(),
            automatic_restore: true,
            fade_millis: None,
            delay_millis: None,
        })
        .collect()
}

fn group_restorations(
    cue: &Cue,
    next: &Cue,
    state: &HashMap<GroupAddress, AttributeValue>,
    projections: &HashMap<GroupAddress, light_core::ReplacementProjectionMap>,
) -> Vec<GroupCueChange> {
    let explicit = next
        .group_changes
        .iter()
        .filter(|change| !change.automatic_restore)
        .map(group_address)
        .collect::<HashSet<_>>();
    cue.group_changes
        .iter()
        .filter(|change| !change.automatic_restore && !explicit.contains(&group_address(change)))
        .map(|change| GroupCueChange {
            replacement_projections: if state.contains_key(&group_address(change)) {
                projections
                    .get(&group_address(change))
                    .cloned()
                    .unwrap_or_default()
            } else {
                change.replacement_projections.clone()
            },
            preset_reference: None,
            group_id: change.group_id.clone(),
            attribute: change.attribute.clone(),
            value: state.get(&group_address(change)).cloned(),
            automatic_restore: true,
            fade_millis: None,
            delay_millis: None,
        })
        .collect()
}

fn append_restorations(
    cue_list: &mut CueList,
    index: usize,
    restores: (Vec<CueChange>, Vec<GroupCueChange>, Vec<CueDynamicChange>),
) {
    let Some(cue) = cue_list.cues.get_mut(index) else {
        return;
    };
    cue.changes.extend(restores.0);
    cue.group_changes.extend(restores.1);
    // Restorations establish the previous set; the next Cue's explicit events win.
    let explicit = std::mem::take(&mut cue.dynamic_changes);
    cue.dynamic_changes = restores.2;
    cue.dynamic_changes.extend(explicit);
}

fn apply_fixture_changes(
    state: &mut HashMap<AttributeAddress, AttributeValue>,
    changes: &[CueChange],
) {
    for change in changes {
        apply_value(state, change.address(), &change.value);
    }
}

fn apply_group_changes(
    state: &mut HashMap<GroupAddress, AttributeValue>,
    changes: &[GroupCueChange],
) {
    for change in changes {
        apply_value(state, group_address(change), &change.value);
    }
}

fn apply_dynamic_changes(
    state: &mut HashMap<DynamicTrackAddress, light_dynamics::DynamicSemanticValue>,
    changes: &[CueDynamicChange],
) {
    for change in changes {
        let address = dynamic_address(change);
        state.retain(|existing, _| {
            !change.value.replaces_address(
                address.0,
                &address.1,
                existing.2,
                existing.0,
                &existing.1,
            )
        });
        match &change.value {
            value if value.is_programming_release() => {
                state.remove(&address);
            }
            value => {
                state.insert(address, value.clone());
            }
        }
    }
}

fn dynamic_address(change: &CueDynamicChange) -> DynamicTrackAddress {
    (
        change.fixture_id,
        change.attribute.clone(),
        change.value.track_key(),
    )
}

fn apply_value<K: Eq + std::hash::Hash>(
    state: &mut HashMap<K, AttributeValue>,
    address: K,
    value: &Option<AttributeValue>,
) {
    if let Some(value) = value {
        state.insert(address, value.clone());
    } else {
        state.remove(&address);
    }
}

fn group_address(change: &GroupCueChange) -> GroupAddress {
    (change.group_id.clone(), change.attribute.clone())
}
