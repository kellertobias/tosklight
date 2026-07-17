use light_core::{AttributeKey, AttributeValue};
use light_playback::{Cue, CueChange, CueList, GroupCueChange};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CueTransferMode {
    Plain,
    Status,
}

/// Builds the destination Cue without mutating either Cuelist. Plain transfer preserves only the
/// selected Cue delta. Status transfer materializes every direct fixture and live Group address
/// touched at or before the source point, while leaving untouched destination addresses free to
/// track from destination history.
pub fn destination_cue(
    source: &CueList,
    source_index: usize,
    destination_number: f64,
    mode: CueTransferMode,
) -> Result<Cue, String> {
    let mut cue = source
        .cues
        .get(source_index)
        .cloned()
        .ok_or_else(|| "source Cue index is out of range".to_string())?;
    cue.number = destination_number;
    if mode == CueTransferMode::Plain {
        return Ok(cue);
    }

    cue.changes = source
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
        .collect();
    cue.changes.sort_by(|left, right| {
        left.fixture_id
            .0
            .as_bytes()
            .cmp(right.fixture_id.0.as_bytes())
            .then_with(|| left.attribute.0.cmp(&right.attribute.0))
    });

    let mut group_state: HashMap<(String, AttributeKey), AttributeValue> = HashMap::new();
    for tracked in source.cues.iter().take(source_index.saturating_add(1)) {
        for change in &tracked.group_changes {
            let address = (change.group_id.clone(), change.attribute.clone());
            if let Some(value) = &change.value {
                group_state.insert(address, value.clone());
            } else {
                group_state.remove(&address);
            }
        }
    }
    cue.group_changes = group_state
        .into_iter()
        .map(|((group_id, attribute), value)| GroupCueChange {
            group_id,
            attribute,
            value: Some(value),
            automatic_restore: false,
            fade_millis: None,
            delay_millis: None,
        })
        .collect();
    cue.group_changes.sort_by(|left, right| {
        left.group_id
            .cmp(&right.group_id)
            .then_with(|| left.attribute.0.cmp(&right.attribute.0))
    });
    Ok(cue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::{CueListId, FixtureId};
    use light_playback::{CueListMode, CueTrigger, IntensityPriorityMode, RestartMode, WrapMode};

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
            speed_multiplier: 1.0,
            cues,
        }
    }

    #[test]
    fn plain_keeps_the_delta_while_status_materializes_only_touched_addresses() {
        let first = FixtureId::new();
        let second = FixtureId::new();
        let untouched = FixtureId::new();
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
            delay_millis: None,
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

        let plain = destination_cue(&source, 1, 7.0, CueTransferMode::Plain).unwrap();
        assert_eq!(plain.number, 7.0);
        assert_eq!(plain.changes.len(), 1);
        assert_eq!(plain.changes[0].fixture_id, second);

        let status = destination_cue(&source, 1, 7.0, CueTransferMode::Status).unwrap();
        assert_eq!(status.name, "Transfer me");
        assert!(matches!(status.trigger, CueTrigger::Follow { .. }));
        assert_eq!(status.changes.len(), 2);
        assert!(
            status
                .changes
                .iter()
                .any(|change| change.fixture_id == first)
        );
        assert!(
            status
                .changes
                .iter()
                .any(|change| change.fixture_id == second)
        );
        assert!(
            status
                .changes
                .iter()
                .all(|change| change.fixture_id != untouched && change.fade_millis.is_none())
        );
        assert_eq!(status.group_changes.len(), 1);
        assert_eq!(status.group_changes[0].group_id, "1");
        assert_eq!(status.group_changes[0].fade_millis, None);
    }
}
