use crate::*;
use std::error::Error;
use std::fmt::{Display, Formatter};

mod restoration;
mod topology;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CueRecordingTiming {
    pub fade_millis: Option<u64>,
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CueRecordingContent {
    pub changes: Vec<CueChange>,
    pub group_changes: Vec<GroupCueChange>,
    pub dynamic_changes: Vec<CueDynamicChange>,
    pub timing: CueRecordingTiming,
    pub cue_only: bool,
    /// Names apply only when creating a Cue; recording never renames an existing Cue.
    pub name: Option<String>,
}

impl CueRecordingContent {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty() && self.group_changes.is_empty() && self.dynamic_changes.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CueRecordOperation {
    Append,
    Overwrite { cue_number: CueNumber },
    Merge { cue_number: CueNumber },
    Subtract { cue_number: CueNumber },
    MergeActive { active_cue_id: Option<Uuid> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CueListRecordingPlan {
    pub cue_list: CueList,
    pub changed: bool,
    pub cue_id: Uuid,
    pub cue_number: CueNumber,
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CueRecordingPlanError {
    EmptySource,
    InvalidCueNumber,
    CueDoesNotExist { cue_number: CueNumber },
    ActiveCueDoesNotExist { cue_id: Uuid },
    CannotDeleteOnlyCue,
    SourceContainsAutomaticRestore,
    DuplicateFixtureAddress,
    DuplicateGroupAddress,
}

impl Display for CueRecordingPlanError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySource => formatter.write_str("the programmer has no values to record"),
            Self::InvalidCueNumber => formatter.write_str("Cue number must be a canonical path"),
            Self::CueDoesNotExist { cue_number } => {
                write!(formatter, "Cue {cue_number} does not exist")
            }
            Self::ActiveCueDoesNotExist { cue_id } => {
                write!(formatter, "active Cue {cue_id} does not exist")
            }
            Self::CannotDeleteOnlyCue => formatter.write_str(
                "cannot delete the only Cue; delete the Cuelist from its configuration instead",
            ),
            Self::SourceContainsAutomaticRestore => {
                formatter.write_str("recording source cannot contain automatic restorations")
            }
            Self::DuplicateFixtureAddress => {
                formatter.write_str("recording source contains a duplicate fixture attribute")
            }
            Self::DuplicateGroupAddress => {
                formatter.write_str("recording source contains a duplicate Group attribute")
            }
        }
    }
}

impl Error for CueRecordingPlanError {}

impl CueList {
    pub fn plan_recording(
        &self,
        content: CueRecordingContent,
        operation: CueRecordOperation,
    ) -> Result<CueListRecordingPlan, CueRecordingPlanError> {
        validate_content(&content)?;
        let original = self;
        let mut cue_list = self.clone();
        let target = apply_operation(&mut cue_list, content, operation)?;
        if !target.appended && cue_list == *original {
            return Ok(CueListRecordingPlan {
                cue_list,
                changed: false,
                cue_id: target.cue_id,
                cue_number: target.cue_number,
                deleted: target.deleted,
            });
        }
        cue_list
            .cues
            .sort_by(|left, right| left.number.cmp(&right.number));
        restoration::regenerate_automatic_restorations(&mut cue_list);
        Ok(CueListRecordingPlan {
            changed: target.appended || cue_list != *original,
            cue_list,
            cue_id: target.cue_id,
            cue_number: target.cue_number,
            deleted: target.deleted,
        })
    }
}

#[derive(Clone)]
struct AppliedTarget {
    cue_id: Uuid,
    cue_number: CueNumber,
    deleted: bool,
    appended: bool,
}

fn apply_operation(
    cue_list: &mut CueList,
    content: CueRecordingContent,
    operation: CueRecordOperation,
) -> Result<AppliedTarget, CueRecordingPlanError> {
    match operation {
        CueRecordOperation::Append => append(cue_list, content, next_whole_number(cue_list)?),
        CueRecordOperation::Overwrite { cue_number } => overwrite(cue_list, content, cue_number),
        CueRecordOperation::Merge { cue_number } => merge_numbered(cue_list, content, cue_number),
        CueRecordOperation::Subtract { cue_number } => subtract(cue_list, content, cue_number),
        CueRecordOperation::MergeActive { active_cue_id } => {
            merge_active(cue_list, content, active_cue_id)
        }
    }
}

fn append(
    cue_list: &mut CueList,
    content: CueRecordingContent,
    cue_number: CueNumber,
) -> Result<AppliedTarget, CueRecordingPlanError> {
    require_values(&content)?;
    let cue = new_cue(content, cue_number);
    let target = stored_target(&cue, true);
    cue_list.cues.push(cue);
    Ok(target)
}

fn overwrite(
    cue_list: &mut CueList,
    content: CueRecordingContent,
    cue_number: CueNumber,
) -> Result<AppliedTarget, CueRecordingPlanError> {
    require_values(&content)?;
    let Some(index) = cue_index(cue_list, &cue_number) else {
        return append(cue_list, content, cue_number);
    };
    let cue = &mut cue_list.cues[index];
    cue.changes = content.changes;
    cue.group_changes = content.group_changes;
    cue.dynamic_changes = content.dynamic_changes;
    cue.fade_millis = content.timing.fade_millis.unwrap_or(0);
    cue.delay_millis = 0;
    cue.out_fade_millis = None;
    cue.out_fade_link = None;
    cue.out_delay_millis = None;
    cue.out_delay_link = None;
    cue.trigger = trigger(content.timing.delay_millis);
    cue.cue_only = content.cue_only;
    Ok(stored_target(cue, false))
}

fn merge_numbered(
    cue_list: &mut CueList,
    content: CueRecordingContent,
    cue_number: CueNumber,
) -> Result<AppliedTarget, CueRecordingPlanError> {
    require_values(&content)?;
    let index =
        cue_index(cue_list, &cue_number).ok_or_else(|| CueRecordingPlanError::CueDoesNotExist {
            cue_number: cue_number.clone(),
        })?;
    Ok(merge_at(&mut cue_list.cues[index], content))
}

fn merge_active(
    cue_list: &mut CueList,
    content: CueRecordingContent,
    cue_id: Option<Uuid>,
) -> Result<AppliedTarget, CueRecordingPlanError> {
    require_values(&content)?;
    let Some(cue_id) = cue_id else {
        return append(cue_list, content, next_whole_number(cue_list)?);
    };
    let cue = cue_list
        .cues
        .iter_mut()
        .find(|cue| cue.id == cue_id)
        .ok_or(CueRecordingPlanError::ActiveCueDoesNotExist { cue_id })?;
    Ok(merge_at(cue, content))
}

fn merge_at(cue: &mut Cue, content: CueRecordingContent) -> AppliedTarget {
    merge_fixture_changes(&mut cue.changes, content.changes);
    merge_group_changes(&mut cue.group_changes, content.group_changes);
    merge_dynamic_changes(&mut cue.dynamic_changes, content.dynamic_changes);
    stored_target(cue, false)
}

fn subtract(
    cue_list: &mut CueList,
    content: CueRecordingContent,
    cue_number: CueNumber,
) -> Result<AppliedTarget, CueRecordingPlanError> {
    let index =
        cue_index(cue_list, &cue_number).ok_or_else(|| CueRecordingPlanError::CueDoesNotExist {
            cue_number: cue_number.clone(),
        })?;
    if content.is_empty() {
        return delete_at(cue_list, index);
    }
    let cue = &mut cue_list.cues[index];
    subtract_fixture_changes(&mut cue.changes, &content.changes);
    subtract_group_changes(&mut cue.group_changes, &content.group_changes);
    subtract_dynamic_changes(&mut cue.dynamic_changes, &content.dynamic_changes);
    Ok(stored_target(cue, false))
}

fn delete_at(cue_list: &mut CueList, index: usize) -> Result<AppliedTarget, CueRecordingPlanError> {
    if cue_list.cues.len() == 1 {
        return Err(CueRecordingPlanError::CannotDeleteOnlyCue);
    }
    let cue = cue_list.cues.remove(index);
    Ok(AppliedTarget {
        cue_id: cue.id,
        cue_number: cue.number,
        deleted: true,
        appended: false,
    })
}

fn new_cue(content: CueRecordingContent, cue_number: CueNumber) -> Cue {
    let mut cue = Cue::new(cue_number);
    cue.name = content.name.unwrap_or_default();
    cue.changes = content.changes;
    cue.group_changes = content.group_changes;
    cue.dynamic_changes = content.dynamic_changes;
    cue.fade_millis = content.timing.fade_millis.unwrap_or(0);
    cue.trigger = trigger(content.timing.delay_millis);
    cue.cue_only = content.cue_only;
    cue
}

fn trigger(delay_millis: Option<u64>) -> CueTrigger {
    match delay_millis {
        Some(0) => CueTrigger::Follow { delay_millis: 0 },
        Some(delay_millis) => CueTrigger::Wait { delay_millis },
        None => CueTrigger::Manual,
    }
}

fn stored_target(cue: &Cue, appended: bool) -> AppliedTarget {
    AppliedTarget {
        cue_id: cue.id,
        cue_number: cue.number.clone(),
        deleted: false,
        appended,
    }
}

fn cue_index(cue_list: &CueList, cue_number: &CueNumber) -> Option<usize> {
    cue_list
        .cues
        .iter()
        .position(|cue| cue.number == *cue_number)
}

fn next_whole_number(cue_list: &CueList) -> Result<CueNumber, CueRecordingPlanError> {
    Ok(cue_list
        .cues
        .iter()
        .map(|cue| &cue.number)
        .max()
        .map_or_else(|| CueNumber::from(1_u8), CueNumber::next_whole))
}

fn require_values(content: &CueRecordingContent) -> Result<(), CueRecordingPlanError> {
    if content.is_empty() {
        return Err(CueRecordingPlanError::EmptySource);
    }
    Ok(())
}

fn validate_content(content: &CueRecordingContent) -> Result<(), CueRecordingPlanError> {
    validate_fixture_changes(&content.changes)?;
    validate_group_changes(&content.group_changes)?;
    validate_dynamic_changes(&content.dynamic_changes)
}

fn validate_dynamic_changes(changes: &[CueDynamicChange]) -> Result<(), CueRecordingPlanError> {
    let mut addresses = HashSet::new();
    for change in changes {
        if change.automatic_restore {
            return Err(CueRecordingPlanError::SourceContainsAutomaticRestore);
        }
        if !addresses.insert(dynamic_change_key(change)) {
            return Err(CueRecordingPlanError::DuplicateFixtureAddress);
        }
    }
    Ok(())
}

fn validate_fixture_changes(changes: &[CueChange]) -> Result<(), CueRecordingPlanError> {
    let mut addresses = HashSet::new();
    for change in changes {
        validate_source_change(change.value.is_some(), change.automatic_restore)?;
        if !addresses.insert((change.fixture_id, change.attribute.clone())) {
            return Err(CueRecordingPlanError::DuplicateFixtureAddress);
        }
    }
    Ok(())
}

fn validate_group_changes(changes: &[GroupCueChange]) -> Result<(), CueRecordingPlanError> {
    let mut addresses = HashSet::new();
    for change in changes {
        validate_source_change(change.value.is_some(), change.automatic_restore)?;
        if !addresses.insert((change.group_id.clone(), change.attribute.clone())) {
            return Err(CueRecordingPlanError::DuplicateGroupAddress);
        }
    }
    Ok(())
}

fn validate_source_change(
    _has_value: bool,
    automatic_restore: bool,
) -> Result<(), CueRecordingPlanError> {
    if automatic_restore {
        return Err(CueRecordingPlanError::SourceContainsAutomaticRestore);
    }
    Ok(())
}

fn merge_fixture_changes(stored: &mut Vec<CueChange>, incoming: Vec<CueChange>) {
    let addresses = incoming
        .iter()
        .map(|change| (change.fixture_id, change.attribute.clone()))
        .collect::<HashSet<_>>();
    stored.retain(|change| !addresses.contains(&(change.fixture_id, change.attribute.clone())));
    stored.extend(incoming);
}

fn merge_group_changes(stored: &mut Vec<GroupCueChange>, incoming: Vec<GroupCueChange>) {
    let addresses = incoming
        .iter()
        .map(|change| (change.group_id.clone(), change.attribute.clone()))
        .collect::<HashSet<_>>();
    stored
        .retain(|change| !addresses.contains(&(change.group_id.clone(), change.attribute.clone())));
    stored.extend(incoming);
}

fn merge_dynamic_changes(stored: &mut Vec<CueDynamicChange>, incoming: Vec<CueDynamicChange>) {
    let addresses = incoming
        .iter()
        .map(dynamic_change_key)
        .collect::<HashSet<_>>();
    stored.retain(|change| !addresses.contains(&dynamic_change_key(change)));
    stored.extend(incoming);
}

fn subtract_fixture_changes(stored: &mut Vec<CueChange>, incoming: &[CueChange]) {
    let addresses = incoming
        .iter()
        .map(|change| (change.fixture_id, change.attribute.clone()))
        .collect::<HashSet<_>>();
    stored.retain(|change| !addresses.contains(&(change.fixture_id, change.attribute.clone())));
}

fn subtract_group_changes(stored: &mut Vec<GroupCueChange>, incoming: &[GroupCueChange]) {
    let addresses = incoming
        .iter()
        .map(|change| (change.group_id.clone(), change.attribute.clone()))
        .collect::<HashSet<_>>();
    stored
        .retain(|change| !addresses.contains(&(change.group_id.clone(), change.attribute.clone())));
}

fn subtract_dynamic_changes(stored: &mut Vec<CueDynamicChange>, incoming: &[CueDynamicChange]) {
    let addresses = incoming
        .iter()
        .map(dynamic_change_key)
        .collect::<HashSet<_>>();
    stored.retain(|change| !addresses.contains(&dynamic_change_key(change)));
}

fn dynamic_change_key(change: &CueDynamicChange) -> (FixtureId, AttributeKey, Option<Uuid>) {
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
