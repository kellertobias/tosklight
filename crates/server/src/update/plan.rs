use std::collections::HashSet;

use light_playback::CueList;
use light_programmer::{GroupDefinition, Preset};
use serde::Serialize;

use super::error::UpdateError;
use super::model::{CueSource, UpdatePreview, UpdateTargetIdentity};

#[derive(Clone, Debug)]
pub enum PlannedUpdateObject {
    CueList(CueList),
    Preset(Preset),
    Group(GroupDefinition),
}

/// A complete, single-object mutation. Existing Only may alter events in several Cues, but the
/// whole Cuelist remains one revision-checked write and therefore one history/undo action.
#[derive(Clone, Debug)]
pub struct AtomicUpdatePlan {
    pub target: UpdateTargetIdentity,
    pub expected_revision: u64,
    pub preview: UpdatePreview,
    pub object: PlannedUpdateObject,
}

impl AtomicUpdatePlan {
    pub fn object_kind(&self) -> &'static str {
        match self.object {
            PlannedUpdateObject::CueList(_) => "cue_list",
            PlannedUpdateObject::Preset(_) => "preset",
            PlannedUpdateObject::Group(_) => "group",
        }
    }

    pub fn object_id(&self) -> &str {
        &self.target.object_id
    }

    pub fn body(&self) -> Result<serde_json::Value, serde_json::Error> {
        match &self.object {
            PlannedUpdateObject::CueList(value) => serde_json::to_value(value),
            PlannedUpdateObject::Preset(value) => serde_json::to_value(value),
            PlannedUpdateObject::Group(value) => serde_json::to_value(value),
        }
    }

    pub fn complete(self, revision_after: u64) -> UpdateResult {
        let changed_cues = unique_changed_cues(&self.preview);
        UpdateResult {
            target: self.target,
            revision_before: self.expected_revision,
            revision_after,
            eligible_count: self.preview.eligible_count(),
            changed_count: self.preview.changed_count(),
            added_count: self.preview.added_count(),
            ignored_count: self.preview.ignored_count(),
            changed_cues,
            programmer_values_retained: true,
        }
    }
}

fn unique_changed_cues(preview: &UpdatePreview) -> Vec<CueSource> {
    let mut cues = Vec::new();
    let mut seen = HashSet::new();
    for item in &preview.items {
        if let Some(cue) = item.outcome.changed_cue()
            && seen.insert(cue.cue_id)
        {
            cues.push(cue.clone());
        }
    }
    cues
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UpdateResult {
    pub target: UpdateTargetIdentity,
    pub revision_before: u64,
    pub revision_after: u64,
    pub eligible_count: usize,
    pub changed_count: usize,
    pub added_count: usize,
    pub ignored_count: usize,
    pub changed_cues: Vec<CueSource>,
    /// Update follows the chosen desk policy: successful values remain in the programmer.
    pub programmer_values_retained: bool,
}

pub(super) fn ensure_revision(expected: u64, current: u64) -> Result<(), UpdateError> {
    if expected == current {
        Ok(())
    } else {
        Err(UpdateError::StaleRevision { expected, current })
    }
}
