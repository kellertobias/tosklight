use std::collections::HashMap;

use light_core::{AttributeKey, FixtureId};
use light_playback::{Cue, CueList};
use light_programmer::{GroupDefinition, Preset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::target::ResolvedCueTarget;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CueUpdateMode {
    ExistingOnly,
    ExistingInCurrentCue,
    #[default]
    AddToCurrentCue,
    AddNew,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExistingContentMode {
    #[default]
    UpdateExisting,
    AddNew,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "target_type", content = "mode", rename_all = "snake_case")]
pub enum UpdateMode {
    Cue(CueUpdateMode),
    ExistingContent(ExistingContentMode),
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdateTargetFamily {
    Cue,
    Preset,
    Group,
    Other { kind: String },
}

/// Desk/operator workflow preferences. This is not show programming data and should be persisted
/// in the established desk settings scope. `serde(default)` gives old settings deterministic
/// migration values.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct UpdateSettings {
    pub cue_mode: CueUpdateMode,
    pub preset_mode: ExistingContentMode,
    pub group_mode: ExistingContentMode,
    pub other_target_modes: HashMap<String, ExistingContentMode>,
    pub show_update_modal_on_touch: bool,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            cue_mode: CueUpdateMode::AddToCurrentCue,
            preset_mode: ExistingContentMode::UpdateExisting,
            group_mode: ExistingContentMode::UpdateExisting,
            other_target_modes: HashMap::new(),
            show_update_modal_on_touch: true,
        }
    }
}

impl UpdateSettings {
    pub fn configured_mode(&self, family: &UpdateTargetFamily) -> UpdateMode {
        match family {
            UpdateTargetFamily::Cue => UpdateMode::Cue(self.cue_mode),
            UpdateTargetFamily::Preset => UpdateMode::ExistingContent(self.preset_mode),
            UpdateTargetFamily::Group => UpdateMode::ExistingContent(self.group_mode),
            UpdateTargetFamily::Other { kind } => UpdateMode::ExistingContent(
                self.other_target_modes
                    .get(kind)
                    .copied()
                    .unwrap_or_default(),
            ),
        }
    }

    pub fn confirmation_behavior(
        &self,
        family: &UpdateTargetFamily,
        path: UpdateConfirmationPath,
    ) -> UpdateConfirmationBehavior {
        if path == UpdateConfirmationPath::Touch && self.show_update_modal_on_touch {
            UpdateConfirmationBehavior::OpenModal
        } else {
            UpdateConfirmationBehavior::ApplyDefault(self.configured_mode(family))
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateConfirmationPath {
    Enter,
    Touch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "action", content = "mode", rename_all = "snake_case")]
pub enum UpdateConfirmationBehavior {
    OpenModal,
    ApplyDefault(UpdateMode),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CueIdentity {
    pub id: Uuid,
    pub number: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UpdateTargetIdentity {
    pub family: UpdateTargetFamily,
    pub object_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback_number: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cue: Option<CueIdentity>,
}

impl UpdateTargetIdentity {
    pub(super) fn cue(cue_list: &CueList, target: &ResolvedCueTarget, current_cue: &Cue) -> Self {
        Self {
            family: UpdateTargetFamily::Cue,
            object_id: cue_list.id.0.to_string(),
            name: cue_list.name.clone(),
            playback_number: target.playback_number,
            cue: Some(CueIdentity {
                id: current_cue.id,
                number: current_cue.number,
            }),
        }
    }

    pub(super) fn preset(id: &str, preset: &Preset) -> Self {
        Self {
            family: UpdateTargetFamily::Preset,
            object_id: id.to_owned(),
            name: preset.name.clone(),
            playback_number: None,
            cue: None,
        }
    }

    pub(super) fn group(group: &GroupDefinition) -> Self {
        Self {
            family: UpdateTargetFamily::Group,
            object_id: group.id.clone(),
            name: group.name.clone(),
            playback_number: None,
            cue: None,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdateAddress {
    FixtureAttribute {
        fixture_id: FixtureId,
        attribute: AttributeKey,
    },
    GroupAttribute {
        group_id: String,
        attribute: AttributeKey,
    },
    GroupMembership {
        fixture_id: FixtureId,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CueSource {
    pub cue_id: Uuid,
    pub cue_number: f64,
    pub cue_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateIgnoreReason {
    NewAddress,
    NotInCurrentCue,
    NotInActiveTrackedState,
    NewGroupMember,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum UpdateItemOutcome {
    ChangeAtSource {
        source: CueSource,
    },
    ChangeInCurrentCue {
        cue: CueSource,
    },
    AddToCurrentCue {
        cue: CueSource,
    },
    AddNewToCurrentCue {
        cue: CueSource,
    },
    UpdateExisting,
    AddNew,
    Unchanged {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<CueSource>,
    },
    Ignored {
        reason: UpdateIgnoreReason,
    },
}

impl UpdateItemOutcome {
    pub fn is_eligible(&self) -> bool {
        !matches!(self, Self::Ignored { .. })
    }

    pub fn changes_data(&self) -> bool {
        matches!(
            self,
            Self::ChangeAtSource { .. }
                | Self::ChangeInCurrentCue { .. }
                | Self::AddToCurrentCue { .. }
                | Self::AddNewToCurrentCue { .. }
                | Self::UpdateExisting
                | Self::AddNew
        )
    }

    pub fn adds_data(&self) -> bool {
        matches!(
            self,
            Self::AddToCurrentCue { .. } | Self::AddNewToCurrentCue { .. } | Self::AddNew
        )
    }

    pub(super) fn changed_cue(&self) -> Option<&CueSource> {
        match self {
            Self::ChangeAtSource { source } => Some(source),
            Self::ChangeInCurrentCue { cue }
            | Self::AddToCurrentCue { cue }
            | Self::AddNewToCurrentCue { cue } => Some(cue),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UpdatePreviewItem {
    pub address: UpdateAddress,
    pub outcome: UpdateItemOutcome,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UpdatePreview {
    pub target: UpdateTargetIdentity,
    pub mode: UpdateMode,
    pub items: Vec<UpdatePreviewItem>,
}

impl UpdatePreview {
    pub fn eligible_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.outcome.is_eligible())
            .count()
    }

    pub fn changed_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.outcome.changes_data())
            .count()
    }

    pub fn added_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.outcome.adds_data())
            .count()
    }

    pub fn ignored_count(&self) -> usize {
        self.items.len().saturating_sub(self.eligible_count())
    }

    pub fn unchanged_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item.outcome, UpdateItemOutcome::Unchanged { .. }))
            .count()
    }

    pub fn has_real_change(&self) -> bool {
        self.changed_count() > 0
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateTargetFilter {
    #[default]
    EligibleForUpdateExisting,
    ShowAllActive,
}

/// One Update Update menu candidate. `existing_preview` must use the target family's
/// existing-only mode; `add_new_preview` supplies the explicit show-all mode control.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UpdateMenuEntry {
    pub target: UpdateTargetIdentity,
    pub active_or_referenced: bool,
    pub existing_preview: UpdatePreview,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub add_new_preview: Option<UpdatePreview>,
}

impl UpdateMenuEntry {
    pub fn eligible_for_update_existing(&self) -> bool {
        self.existing_preview.has_real_change()
    }

    pub fn is_no_op(&self, mode: UpdateMode) -> bool {
        let preview = if self.existing_preview.mode == mode {
            Some(&self.existing_preview)
        } else {
            self.add_new_preview
                .as_ref()
                .filter(|preview| preview.mode == mode)
        };
        preview.is_none_or(|preview| !preview.has_real_change())
    }
}

pub fn filter_update_menu(
    entries: &[UpdateMenuEntry],
    filter: UpdateTargetFilter,
) -> Vec<&UpdateMenuEntry> {
    entries
        .iter()
        .filter(|entry| match filter {
            UpdateTargetFilter::EligibleForUpdateExisting => entry.eligible_for_update_existing(),
            UpdateTargetFilter::ShowAllActive => entry.active_or_referenced,
        })
        .collect()
}
