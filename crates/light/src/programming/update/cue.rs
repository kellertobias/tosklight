mod analysis;

use light_playback::{Cue, CueChange, CueList, GroupCueChange};
use light_programmer::ProgrammerUpdateContent;

use self::analysis::{analyse_cue_list, cue_outcome, cue_source};
use super::error::UpdateError;
use super::incoming::{IncomingValue, incoming_values};
use super::model::{
    CueUpdateMode, UpdateItemOutcome, UpdateMode, UpdatePreview, UpdatePreviewItem,
    UpdateTargetFamily, UpdateTargetIdentity,
};
use super::plan::{AtomicUpdatePlan, PlannedUpdateObject, ensure_revision};
use super::target::ResolvedCueTarget;

pub fn preview_cue_update(
    cue_list: &CueList,
    target: &ResolvedCueTarget,
    mode: CueUpdateMode,
    programmer: &ProgrammerUpdateContent,
) -> Result<UpdatePreview, UpdateError> {
    let current_index = validate_target(cue_list, target, programmer)?;
    let current = &cue_list.cues[current_index];
    let current_source = cue_source(cue_list, current_index);
    let analysis = analyse_cue_list(cue_list, current_index);
    let items = incoming_values(programmer)
        .into_iter()
        .map(|incoming| UpdatePreviewItem {
            address: incoming.address(),
            outcome: cue_outcome(cue_list, &analysis, &current_source, mode, incoming),
        })
        .collect();
    Ok(UpdatePreview {
        target: UpdateTargetIdentity::cue(cue_list, target, current),
        mode: UpdateMode::Cue(mode),
        items,
    })
}

fn validate_target(
    cue_list: &CueList,
    target: &ResolvedCueTarget,
    programmer: &ProgrammerUpdateContent,
) -> Result<usize, UpdateError> {
    if !programmer.has_values() {
        return Err(UpdateError::EmptyProgrammer {
            target_family: UpdateTargetFamily::Cue,
        });
    }
    cue_list
        .validate()
        .map_err(|reason| UpdateError::InvalidTarget { reason })?;
    if cue_list.id != target.cue_list_id {
        return Err(UpdateError::InvalidTarget {
            reason: "resolved Cue target belongs to a different Cuelist".into(),
        });
    }
    cue_list
        .cues
        .iter()
        .position(|cue| cue.id == target.cue_id)
        .ok_or_else(|| UpdateError::MissingTarget {
            target: format!("Cue {}", target.cue_number),
        })
}

fn write_cue_event(
    cue: &mut Cue,
    incoming: IncomingValue<'_>,
    append_if_missing: bool,
) -> Result<(), UpdateError> {
    match incoming {
        IncomingValue::Fixture(value) => {
            let existing = cue.changes.iter_mut().find(|change| {
                change.fixture_id == value.fixture_id && change.attribute == value.attribute
            });
            if let Some(change) = existing {
                change.value = Some(value.value.clone());
                change.automatic_restore = false;
                change.fade_millis = value.fade_millis;
                change.delay_millis = value.delay_millis;
            } else if append_if_missing {
                cue.changes.push(CueChange {
                    fixture_id: value.fixture_id,
                    attribute: value.attribute.clone(),
                    value: Some(value.value.clone()),
                    automatic_restore: false,
                    fade_millis: value.fade_millis,
                    delay_millis: value.delay_millis,
                });
            } else {
                return Err(missing_source("fixture"));
            }
        }
        IncomingValue::Group(value) => {
            let existing = cue.group_changes.iter_mut().find(|change| {
                change.group_id == value.group_id && change.attribute == value.attribute
            });
            if let Some(change) = existing {
                change.value = Some(value.value.clone());
                change.automatic_restore = false;
                change.fade_millis = value.fade_millis;
                change.delay_millis = value.delay_millis;
            } else if append_if_missing {
                cue.group_changes.push(GroupCueChange {
                    group_id: value.group_id.clone(),
                    attribute: value.attribute.clone(),
                    value: Some(value.value.clone()),
                    automatic_restore: false,
                    fade_millis: value.fade_millis,
                    delay_millis: value.delay_millis,
                });
            } else {
                return Err(missing_source("Group"));
            }
        }
    }
    Ok(())
}

fn missing_source(kind: &str) -> UpdateError {
    UpdateError::InvalidTarget {
        reason: format!("authoritative {kind} source event disappeared while planning Update"),
    }
}

pub fn plan_cue_update(
    cue_list: &CueList,
    current_revision: u64,
    expected_revision: u64,
    target: &ResolvedCueTarget,
    mode: CueUpdateMode,
    programmer: &ProgrammerUpdateContent,
) -> Result<AtomicUpdatePlan, UpdateError> {
    ensure_revision(expected_revision, current_revision)?;
    let preview = preview_cue_update(cue_list, target, mode, programmer)?;
    if !preview.has_real_change() {
        return Err(UpdateError::NoOp {
            target: preview.target,
        });
    }
    let current_index = cue_list
        .cues
        .iter()
        .position(|cue| cue.id == target.cue_id)
        .ok_or_else(|| UpdateError::MissingTarget {
            target: format!("Cue {}", target.cue_number),
        })?;
    let mut updated = cue_list.clone();
    apply_preview(&mut updated, current_index, programmer, &preview)?;
    updated
        .validate()
        .map_err(|reason| UpdateError::InvalidTarget { reason })?;
    Ok(AtomicUpdatePlan {
        target: preview.target.clone(),
        expected_revision,
        preview,
        object: PlannedUpdateObject::CueList(updated),
    })
}

fn apply_preview(
    updated: &mut CueList,
    current_index: usize,
    programmer: &ProgrammerUpdateContent,
    preview: &UpdatePreview,
) -> Result<(), UpdateError> {
    for (incoming, item) in incoming_values(programmer).into_iter().zip(&preview.items) {
        match &item.outcome {
            UpdateItemOutcome::ChangeAtSource { source } => {
                write_cue_event(&mut updated.cues[source.cue_index], incoming, false)?;
            }
            UpdateItemOutcome::ChangeInCurrentCue { .. } => {
                write_cue_event(&mut updated.cues[current_index], incoming, false)?;
            }
            UpdateItemOutcome::AddToCurrentCue { .. }
            | UpdateItemOutcome::AddNewToCurrentCue { .. } => {
                write_cue_event(&mut updated.cues[current_index], incoming, true)?;
            }
            UpdateItemOutcome::UpdateExisting
            | UpdateItemOutcome::AddNew
            | UpdateItemOutcome::Unchanged { .. }
            | UpdateItemOutcome::Ignored { .. } => {}
        }
    }
    Ok(())
}
