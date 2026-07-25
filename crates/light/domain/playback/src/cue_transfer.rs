use crate::{Cue, CueChange, CueList, GroupCueChange};
use light_core::{AttributeKey, AttributeValue};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CueTransferMode {
    Plain,
    Status,
}

/// Builds one destination Cue without mutating either Cuelist.
///
/// Plain retains the stored source delta. Status materializes only direct fixture and live Group
/// addresses touched through the source Cue, while leaving untouched destination addresses free
/// to track from their own history.
pub fn transferred_cue(
    source: &CueList,
    source_index: usize,
    destination_number: f64,
    mode: CueTransferMode,
) -> Result<Cue, String> {
    let mut cue = source
        .cues
        .get(source_index)
        .cloned()
        .ok_or_else(|| "source Cue index is out of range".to_owned())?;
    cue.number = destination_number;
    if mode == CueTransferMode::Plain {
        return Ok(cue);
    }
    cue.changes = tracked_fixture_changes(source, source_index);
    cue.group_changes = tracked_group_changes(source, source_index);
    Ok(cue)
}

fn tracked_fixture_changes(source: &CueList, source_index: usize) -> Vec<CueChange> {
    let mut changes = source
        .state_at_index(source_index)
        .into_iter()
        .map(|((fixture_id, attribute), value)| CueChange {
            fixture_id,
            attribute,
            value: Some(value),
            automatic_restore: false,
            fade_millis: None,
            delay_millis: None,
        })
        .collect::<Vec<_>>();
    changes.sort_by(|left, right| {
        left.fixture_id
            .0
            .as_bytes()
            .cmp(right.fixture_id.0.as_bytes())
            .then_with(|| left.attribute.0.cmp(&right.attribute.0))
    });
    changes
}

fn tracked_group_changes(source: &CueList, source_index: usize) -> Vec<GroupCueChange> {
    let mut state: HashMap<(String, AttributeKey), AttributeValue> = HashMap::new();
    for cue in source.cues.iter().take(source_index.saturating_add(1)) {
        apply_group_changes(&mut state, &cue.group_changes);
    }
    let mut changes = state
        .into_iter()
        .map(|((group_id, attribute), value)| GroupCueChange {
            group_id,
            attribute,
            value: Some(value),
            automatic_restore: false,
            fade_millis: None,
            delay_millis: None,
        })
        .collect::<Vec<_>>();
    changes.sort_by(|left, right| {
        left.group_id
            .cmp(&right.group_id)
            .then_with(|| left.attribute.0.cmp(&right.attribute.0))
    });
    changes
}

fn apply_group_changes(
    state: &mut HashMap<(String, AttributeKey), AttributeValue>,
    changes: &[GroupCueChange],
) {
    for change in changes {
        let address = (change.group_id.clone(), change.attribute.clone());
        match &change.value {
            Some(value) => {
                state.insert(address, value.clone());
            }
            None => {
                state.remove(&address);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CueListMode, CueTrigger, IntensityPriorityMode, RestartMode, WrapMode};
    use light_core::{CueListId, FixtureId};

    #[test]
    fn plain_retains_delta_and_status_materializes_touched_addresses() {
        let first = FixtureId::new();
        let second = FixtureId::new();
        let mut cue_one = Cue::new(1.0);
        cue_one.changes.push(CueChange::set(
            first,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        ));
        cue_one.group_changes.push(GroupCueChange {
            group_id: "1".into(),
            attribute: AttributeKey("pan".into()),
            value: Some(AttributeValue::Normalized(0.25)),
            automatic_restore: false,
            fade_millis: Some(500),
            delay_millis: Some(20),
        });
        let mut cue_two = Cue::new(2.0);
        cue_two.name = "Transfer me".into();
        cue_two.trigger = CueTrigger::Follow { delay_millis: 700 };
        cue_two.changes.push(CueChange::set(
            second,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        ));
        let source = list(vec![cue_one, cue_two]);

        let plain = transferred_cue(&source, 1, 7.0, CueTransferMode::Plain).unwrap();
        assert_eq!(plain.changes.len(), 1);
        assert_eq!(plain.changes[0].fixture_id, second);

        let status = transferred_cue(&source, 1, 7.0, CueTransferMode::Status).unwrap();
        assert_eq!(status.name, "Transfer me");
        assert!(matches!(status.trigger, CueTrigger::Follow { .. }));
        assert_eq!(status.changes.len(), 2);
        assert!(
            status
                .changes
                .iter()
                .all(|change| { change.fade_millis.is_none() && change.delay_millis.is_none() })
        );
        assert_eq!(status.group_changes.len(), 1);
        assert_eq!(status.group_changes[0].group_id, "1");
        assert_eq!(status.group_changes[0].fade_millis, None);
        assert_eq!(status.group_changes[0].delay_millis, None);
    }

    #[test]
    fn plain_changes_only_the_number_and_keeps_the_source_cue_identity() {
        let fixture = FixtureId::new();
        let mut source_cue = Cue::new(2.0);
        source_cue.name = "Keep every stored field".into();
        source_cue.fade_millis = 900;
        source_cue.delay_millis = 40;
        source_cue.trigger = CueTrigger::Wait { delay_millis: 50 };
        source_cue.cue_only = true;
        source_cue.changes.push(CueChange::set(
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.7),
        ));
        let source_id = source_cue.id;
        let source = list(vec![source_cue.clone()]);

        let transferred = transferred_cue(&source, 0, 8.5, CueTransferMode::Plain).unwrap();

        assert_eq!(transferred.id, source_id);
        assert_eq!(transferred.number, 8.5);
        let mut expected = source_cue;
        expected.number = 8.5;
        assert_eq!(transferred, expected);
    }

    #[test]
    fn status_omits_released_addresses_and_resets_per_address_timing() {
        let released = FixtureId::new();
        let retained = FixtureId::new();
        let mut first = Cue::new(1.0);
        first.changes.extend([
            timed_fixture_set(released, 0.2),
            timed_fixture_set(retained, 0.4),
        ]);
        first.group_changes.extend([
            timed_group_set("released", 0.3),
            timed_group_set("retained", 0.5),
        ]);
        let mut second = Cue::new(2.0);
        second.changes.push(CueChange {
            fixture_id: released,
            attribute: AttributeKey::intensity(),
            value: None,
            automatic_restore: false,
            fade_millis: Some(600),
            delay_millis: Some(30),
        });
        second.group_changes.push(GroupCueChange {
            value: None,
            ..timed_group_set("released", 0.0)
        });
        let source = list(vec![first, second]);

        let status = transferred_cue(&source, 1, 3.0, CueTransferMode::Status).unwrap();

        assert_eq!(status.changes.len(), 1);
        assert_eq!(status.changes[0].fixture_id, retained);
        assert_eq!(status.changes[0].fade_millis, None);
        assert_eq!(status.changes[0].delay_millis, None);
        assert!(!status.changes[0].automatic_restore);
        assert_eq!(status.group_changes.len(), 1);
        assert_eq!(status.group_changes[0].group_id, "retained");
        assert_eq!(status.group_changes[0].fade_millis, None);
        assert_eq!(status.group_changes[0].delay_millis, None);
        assert!(!status.group_changes[0].automatic_restore);
    }

    #[test]
    fn missing_source_index_is_rejected() {
        let error = transferred_cue(&list(Vec::new()), 0, 2.0, CueTransferMode::Plain).unwrap_err();
        assert_eq!(error, "source Cue index is out of range");
    }

    fn timed_fixture_set(fixture_id: FixtureId, value: f32) -> CueChange {
        CueChange {
            fixture_id,
            attribute: AttributeKey::intensity(),
            value: Some(AttributeValue::Normalized(value)),
            automatic_restore: true,
            fade_millis: Some(500),
            delay_millis: Some(20),
        }
    }

    fn timed_group_set(group_id: &str, value: f32) -> GroupCueChange {
        GroupCueChange {
            group_id: group_id.into(),
            attribute: AttributeKey::intensity(),
            value: Some(AttributeValue::Normalized(value)),
            automatic_restore: true,
            fade_millis: Some(500),
            delay_millis: Some(20),
        }
    }

    fn list(cues: Vec<Cue>) -> CueList {
        CueList {
            id: CueListId::new(),
            name: "Source".into(),
            priority: 0,
            mode: CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            intensity_priority_mode: IntensityPriorityMode::Htp,
            wrap_mode: Some(WrapMode::Off),
            restart_mode: RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_multiplier: 1.0,
            cues,
        }
    }
}
