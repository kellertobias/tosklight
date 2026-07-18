use std::collections::HashMap;

use light_core::{AttributeKey, AttributeValue, CueListId, FixtureId};
use light_playback::{
    Cue, CueChange, CueList, CueListMode, GroupCueChange, IntensityPriorityMode, RestartMode,
    WrapMode,
};
use light_programmer::{
    GroupDefinition, Preset, ProgrammerFixtureUpdate, ProgrammerGroupUpdate,
    ProgrammerUpdateContent,
};
use uuid::Uuid;

use super::*;

mod cue_cases;
mod object_cases;
mod workflow_cases;

pub(super) fn fixture(id: u128) -> FixtureId {
    FixtureId(Uuid::from_u128(id))
}

pub(super) fn attribute(name: &str) -> AttributeKey {
    AttributeKey(name.into())
}

pub(super) fn normalized(value: f32) -> AttributeValue {
    AttributeValue::Normalized(value)
}

pub(super) fn fixture_update(
    fixture_id: FixtureId,
    name: &str,
    value: f32,
    programmer_order: u64,
) -> ProgrammerFixtureUpdate {
    ProgrammerFixtureUpdate {
        fixture_id,
        attribute: attribute(name),
        value: normalized(value),
        programmer_order,
        fade_millis: None,
        delay_millis: None,
    }
}

pub(super) fn content(values: Vec<ProgrammerFixtureUpdate>) -> ProgrammerUpdateContent {
    ProgrammerUpdateContent {
        fixture_values: values,
        ..Default::default()
    }
}

pub(super) fn cue(number: f64, changes: Vec<CueChange>) -> Cue {
    let mut cue = Cue::new(number);
    cue.changes = changes;
    cue
}

pub(super) fn change(fixture_id: FixtureId, name: &str, value: f32) -> CueChange {
    CueChange::set(fixture_id, attribute(name), normalized(value))
}

pub(super) fn cue_list(cues: Vec<Cue>) -> CueList {
    CueList {
        id: CueListId(Uuid::from_u128(900)),
        name: "Cuelist 1".into(),
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

pub(super) fn target(
    list: &CueList,
    index: usize,
    playback_number: Option<u16>,
) -> ResolvedCueTarget {
    ResolvedCueTarget {
        cue_list_id: list.id,
        playback_number,
        cue_id: list.cues[index].id,
        cue_number: list.cues[index].number,
    }
}

pub(super) fn planned_cue_list(plan: AtomicUpdatePlan) -> CueList {
    match plan.object {
        PlannedUpdateObject::CueList(list) => list,
        _ => panic!("expected Cuelist plan"),
    }
}

pub(super) fn planned_preset(plan: AtomicUpdatePlan) -> Preset {
    match plan.object {
        PlannedUpdateObject::Preset(preset) => preset,
        _ => panic!("expected Preset plan"),
    }
}

pub(super) fn planned_group(plan: AtomicUpdatePlan) -> GroupDefinition {
    match plan.object {
        PlannedUpdateObject::Group(group) => group,
        _ => panic!("expected Group plan"),
    }
}

pub(super) fn stored_value(cue: &Cue, fixture_id: FixtureId, name: &str) -> Option<f32> {
    cue.changes
        .iter()
        .find(|change| change.fixture_id == fixture_id && change.attribute == attribute(name))
        .and_then(|change| change.value.as_ref())
        .and_then(AttributeValue::normalized)
}
