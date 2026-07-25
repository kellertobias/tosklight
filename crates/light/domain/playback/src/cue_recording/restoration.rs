use crate::*;

type GroupAddress = (String, AttributeKey);

pub(super) fn regenerate_automatic_restorations(cue_list: &mut CueList) {
    strip_automatic_restorations(cue_list);
    let mut fixture_state = HashMap::new();
    let mut group_state = HashMap::new();
    for index in 0..cue_list.cues.len() {
        let restores = restorations_after(cue_list, index, &fixture_state, &group_state);
        apply_fixture_changes(&mut fixture_state, &cue_list.cues[index].changes);
        apply_group_changes(&mut group_state, &cue_list.cues[index].group_changes);
        append_restorations(cue_list, index + 1, restores);
    }
}

fn strip_automatic_restorations(cue_list: &mut CueList) {
    for cue in &mut cue_list.cues {
        cue.changes.retain(|change| !change.automatic_restore);
        cue.group_changes.retain(|change| !change.automatic_restore);
    }
}

fn restorations_after(
    cue_list: &CueList,
    index: usize,
    fixture_state: &HashMap<AttributeAddress, AttributeValue>,
    group_state: &HashMap<GroupAddress, AttributeValue>,
) -> (Vec<CueChange>, Vec<GroupCueChange>) {
    let cue = &cue_list.cues[index];
    let Some(next) = cue_list.cues.get(index + 1).filter(|_| cue.cue_only) else {
        return (Vec::new(), Vec::new());
    };
    (
        fixture_restorations(cue, next, fixture_state),
        group_restorations(cue, next, group_state),
    )
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
    restores: (Vec<CueChange>, Vec<GroupCueChange>),
) {
    let Some(cue) = cue_list.cues.get_mut(index) else {
        return;
    };
    cue.changes.extend(restores.0);
    cue.group_changes.extend(restores.1);
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
