use std::collections::{HashMap, HashSet};

use light_playback::{CueAction, CueTimecodeStart, TimecodeDefinition, TimecodeLaneContent};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Node {
    CueList(light_core::CueListId),
    Timecode(light_playback::TimecodeId),
}

/// Validates the portable Cuelist/Timecode dependency graph before either object family mutates.
pub fn validate_cue_timecode_graph(
    cue_lists: &[light_playback::CueList],
    timecodes: &[TimecodeDefinition],
) -> Result<(), String> {
    let cue_ids = cue_lists
        .iter()
        .map(|value| value.id)
        .collect::<HashSet<_>>();
    let timecode_ids = timecodes
        .iter()
        .map(|value| value.id)
        .collect::<HashSet<_>>();
    let markers = timecodes
        .iter()
        .map(|value| {
            (
                value.id,
                value
                    .markers
                    .iter()
                    .map(|marker| marker.id)
                    .collect::<HashSet<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut edges = HashMap::<Node, Vec<Node>>::new();
    for cue_list in cue_lists {
        for cue in &cue_list.cues {
            for action in &cue.actions {
                let (timecode_id, start) = match action {
                    CueAction::Jump { .. } => continue,
                    CueAction::TimecodeStart {
                        timecode_id, start, ..
                    } => (*timecode_id, Some(*start)),
                    CueAction::TimecodeStop { timecode_id } => (*timecode_id, None),
                };
                if !timecode_ids.contains(&timecode_id) {
                    return Err(format!(
                        "Cuelist {} references a missing Timecode",
                        cue_list.id.0
                    ));
                }
                if let Some(CueTimecodeStart::Marker { marker_id }) = start
                    && !markers
                        .get(&timecode_id)
                        .is_some_and(|ids| ids.contains(&marker_id))
                {
                    return Err(format!(
                        "Cuelist {} references a missing Timecode marker",
                        cue_list.id.0
                    ));
                }
                edges
                    .entry(Node::CueList(cue_list.id))
                    .or_default()
                    .push(Node::Timecode(timecode_id));
            }
        }
    }
    for timecode in timecodes {
        for lane in &timecode.lanes {
            if let TimecodeLaneContent::CueList { cue_list_id, .. } = lane.content {
                if !cue_ids.contains(&cue_list_id) {
                    return Err(format!(
                        "Timecode {} references a missing Cuelist",
                        timecode.id.0
                    ));
                }
                edges
                    .entry(Node::Timecode(timecode.id))
                    .or_default()
                    .push(Node::CueList(cue_list_id));
            }
        }
    }
    fn visit(
        node: Node,
        edges: &HashMap<Node, Vec<Node>>,
        visiting: &mut HashSet<Node>,
        visited: &mut HashSet<Node>,
    ) -> Result<(), String> {
        if visiting.contains(&node) {
            return Err("Cuelist and Timecode actions form a recursion cycle".into());
        }
        if !visited.insert(node) {
            return Ok(());
        }
        visiting.insert(node);
        for next in edges.get(&node).into_iter().flatten().copied() {
            visit(next, edges, visiting, visited)?;
        }
        visiting.remove(&node);
        Ok(())
    }
    let mut visited = HashSet::new();
    for node in edges.keys().copied() {
        visit(node, &edges, &mut HashSet::new(), &mut visited)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::CueListId;
    use light_playback::*;
    use uuid::Uuid;

    fn cue_list(id: u128, action: Option<CueAction>) -> CueList {
        let mut cue = Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap());
        cue.actions.extend(action);
        CueList {
            id: CueListId(Uuid::from_u128(id)),
            name: "Graph".into(),
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
            auto_off_at_zero: false,
            auto_off_flash_release: false,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_multiplier: 1.0,
            cues: vec![cue],
        }
    }

    fn timecode(id: u128, cue_list_id: Option<CueListId>) -> TimecodeDefinition {
        TimecodeDefinition {
            id: TimecodeId(Uuid::from_u128(id)),
            number: id as u32,
            name: "Graph".into(),
            duration: Some(TimecodeFrame(100)),
            transport_offset: TimecodeFrame::ZERO,
            auto_start: false,
            audio: None,
            markers: vec![],
            lanes: cue_list_id
                .map(|cue_list_id| TimecodeLane {
                    id: TimecodeLaneId(Uuid::new_v4()),
                    name: "Cuelist".into(),
                    content: TimecodeLaneContent::CueList {
                        cue_list_id,
                        clips: vec![],
                    },
                })
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn rejects_direct_and_indirect_recursion() {
        let tc70 = TimecodeId(Uuid::from_u128(70));
        let start70 = CueAction::TimecodeStart {
            timecode_id: tc70,
            start: CueTimecodeStart::Frame {
                frame: TimecodeFrame::ZERO,
            },
        };
        let direct_list = cue_list(1, Some(start70.clone()));
        assert!(
            validate_cue_timecode_graph(
                &[direct_list.clone()],
                &[timecode(70, Some(direct_list.id))]
            )
            .unwrap_err()
            .contains("recursion")
        );

        let second = cue_list(2, Some(start70));
        let first = cue_list(
            1,
            Some(CueAction::TimecodeStart {
                timecode_id: TimecodeId(Uuid::from_u128(71)),
                start: CueTimecodeStart::Frame {
                    frame: TimecodeFrame::ZERO,
                },
            }),
        );
        assert!(
            validate_cue_timecode_graph(
                &[first.clone(), second.clone()],
                &[timecode(70, Some(first.id)), timecode(71, Some(second.id))]
            )
            .unwrap_err()
            .contains("recursion")
        );
    }
}
