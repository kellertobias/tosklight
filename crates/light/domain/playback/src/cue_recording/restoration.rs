use crate::*;

type GroupAddress = (String, AttributeKey);
type DynamicTrackAddress = (FixtureId, AttributeKey, Option<Uuid>);

pub(super) fn regenerate_automatic_restorations(cue_list: &mut CueList) {
    strip_automatic_restorations(cue_list);
    let mut fixture_state = HashMap::new();
    let mut group_state = HashMap::new();
    let mut dynamic_state = HashMap::new();
    for index in 0..cue_list.cues.len() {
        let restores = restorations_after(
            cue_list,
            index,
            &fixture_state,
            &group_state,
            &dynamic_state,
        );
        apply_fixture_changes(&mut fixture_state, &cue_list.cues[index].changes);
        apply_group_changes(&mut group_state, &cue_list.cues[index].group_changes);
        apply_dynamic_changes(&mut dynamic_state, &cue_list.cues[index].dynamic_changes);
        append_restorations(cue_list, index + 1, restores);
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
) -> (Vec<CueChange>, Vec<GroupCueChange>, Vec<CueDynamicChange>) {
    let cue = &cue_list.cues[index];
    let Some(next) = cue_list.cues.get(index + 1).filter(|_| cue.cue_only) else {
        return (Vec::new(), Vec::new(), Vec::new());
    };
    (
        fixture_restorations(cue, next, fixture_state),
        group_restorations(cue, next, group_state),
        dynamic_restorations(cue, next, dynamic_state),
    )
}

fn dynamic_restorations(
    cue: &Cue,
    next: &Cue,
    state: &HashMap<DynamicTrackAddress, light_dynamics::DynamicSemanticValue>,
) -> Vec<CueDynamicChange> {
    let explicit = next
        .dynamic_changes
        .iter()
        .filter(|change| !change.automatic_restore)
        .map(dynamic_address)
        .collect::<HashSet<_>>();
    cue.dynamic_changes
        .iter()
        .filter(|change| !change.automatic_restore && !explicit.contains(&dynamic_address(change)))
        .filter_map(|change| {
            let address = dynamic_address(change);
            let value = state
                .get(&address)
                .cloned()
                .or_else(|| absent_dynamic_restoration(&change.value))?;
            Some(CueDynamicChange {
                fixture_id: change.fixture_id,
                attribute: change.attribute.clone(),
                value,
                automatic_restore: true,
            })
        })
        .collect()
}

fn absent_dynamic_restoration(
    value: &light_dynamics::DynamicSemanticValue,
) -> Option<light_dynamics::DynamicSemanticValue> {
    use light_dynamics::{DynamicSemanticValue, DynamicValueTiming};
    match value {
        DynamicSemanticValue::DynamicOn { instance_link, .. } => {
            Some(DynamicSemanticValue::DynamicOff {
                instance_link: *instance_link,
                timing: DynamicValueTiming::default(),
            })
        }
        DynamicSemanticValue::FixAt { .. } | DynamicSemanticValue::Static { .. } => {
            Some(DynamicSemanticValue::Release)
        }
        DynamicSemanticValue::DynamicOff { .. } | DynamicSemanticValue::Release => None,
    }
}

fn fixture_restorations(
    cue: &Cue,
    next: &Cue,
    state: &HashMap<AttributeAddress, AttributeValue>,
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
    cue.dynamic_changes.extend(restores.2);
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
        match &change.value {
            light_dynamics::DynamicSemanticValue::Release => {
                state.remove(&address);
            }
            value => {
                state.insert(address, value.clone());
            }
        }
    }
}

fn dynamic_address(change: &CueDynamicChange) -> DynamicTrackAddress {
    let instance_link = match &change.value {
        light_dynamics::DynamicSemanticValue::DynamicOn { instance_link, .. }
        | light_dynamics::DynamicSemanticValue::DynamicOff { instance_link, .. } => {
            Some(*instance_link)
        }
        light_dynamics::DynamicSemanticValue::Static { .. }
        | light_dynamics::DynamicSemanticValue::FixAt { .. }
        | light_dynamics::DynamicSemanticValue::Release => None,
    };
    (change.fixture_id, change.attribute.clone(), instance_link)
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
