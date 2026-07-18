//! Pure planning for the Shift+Record Update workflow.
//!
//! This module deliberately has no `AppState`, database, or transport dependency. Callers resolve
//! the authoritative active playback/Cue context, build a preview, and persist the resulting
//! [`AtomicUpdatePlan`] as one normal revision-checked object write. Planning borrows programmer
//! content and never consumes it.

use light_core::{AttributeKey, AttributeValue, CueListId, FixtureId};
use light_playback::{Cue, CueChange, CueList, GroupCueChange};
use light_programmer::{
    GroupDefinition, Preset, ProgrammerFixtureUpdate, ProgrammerGroupUpdate,
    ProgrammerUpdateContent, merge_ordered_group_membership,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use uuid::Uuid;

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
    fn cue(cue_list: &CueList, target: &ResolvedCueTarget, current_cue: &Cue) -> Self {
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

    fn preset(id: &str, preset: &Preset) -> Self {
        Self {
            family: UpdateTargetFamily::Preset,
            object_id: id.to_owned(),
            name: preset.name.clone(),
            playback_number: None,
            cue: None,
        }
    }

    fn group(group: &GroupDefinition) -> Self {
        Self {
            family: UpdateTargetFamily::Group,
            object_id: group.id.clone(),
            name: group.name.clone(),
            playback_number: None,
            cue: None,
        }
    }
}

/// Authoritative concrete playback/Cue context supplied by the playback engine. Keeping the
/// playback number prevents two active instances of one Cuelist from being collapsed together.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ActiveCueContext {
    pub playback_number: u16,
    pub cue_list_id: CueListId,
    pub cue_id: Uuid,
    pub cue_number: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResolvedCueTarget {
    pub cue_list_id: CueListId,
    pub playback_number: Option<u16>,
    pub cue_id: Uuid,
    pub cue_number: f64,
}

impl From<&ActiveCueContext> for ResolvedCueTarget {
    fn from(context: &ActiveCueContext) -> Self {
        Self {
            cue_list_id: context.cue_list_id,
            playback_number: Some(context.playback_number),
            cue_id: context.cue_id,
            cue_number: context.cue_number,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CueTargetRequest {
    /// An explicit Cue is already resolved through the normal command/pool addressing path.
    Explicit(ResolvedCueTarget),
    /// A concrete playback must currently have one authoritative Cue.
    ActivePlayback { playback_number: u16 },
    /// A pool Cuelist without an explicit Cue is valid only with one concrete active context.
    PoolCueList { cue_list_id: CueListId },
}

pub fn resolve_cue_target(
    request: &CueTargetRequest,
    active: &[ActiveCueContext],
) -> Result<ResolvedCueTarget, UpdateError> {
    match request {
        CueTargetRequest::Explicit(target) => Ok(target.clone()),
        CueTargetRequest::ActivePlayback { playback_number } => {
            let matches = active
                .iter()
                .filter(|context| context.playback_number == *playback_number)
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [] => Err(UpdateError::MissingCurrentCue {
                    target: format!("playback {playback_number}"),
                }),
                [context] => Ok(ResolvedCueTarget::from(*context)),
                contexts => Err(UpdateError::AmbiguousPlaybackContext {
                    target: format!("playback {playback_number}"),
                    contexts: contexts.len(),
                }),
            }
        }
        CueTargetRequest::PoolCueList { cue_list_id } => {
            let matches = active
                .iter()
                .filter(|context| context.cue_list_id == *cue_list_id)
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [] => Err(UpdateError::MissingCurrentCue {
                    target: format!("Cuelist {}", cue_list_id.0),
                }),
                [context] => Ok(ResolvedCueTarget::from(*context)),
                contexts => Err(UpdateError::AmbiguousPlaybackContext {
                    target: format!("Cuelist {}", cue_list_id.0),
                    contexts: contexts.len(),
                }),
            }
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

    fn changed_cue(&self) -> Option<&CueSource> {
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

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum UpdateError {
    EmptyProgrammer { target_family: UpdateTargetFamily },
    MissingTarget { target: String },
    MissingCurrentCue { target: String },
    AmbiguousPlaybackContext { target: String, contexts: usize },
    StaleRevision { expected: u64, current: u64 },
    NoOp { target: UpdateTargetIdentity },
    InvalidTarget { reason: String },
}

impl fmt::Display for UpdateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProgrammer { target_family } => {
                write!(
                    formatter,
                    "the programmer has no content for {target_family:?} Update"
                )
            }
            Self::MissingTarget { target } => write!(formatter, "{target} does not exist"),
            Self::MissingCurrentCue { target } => {
                write!(
                    formatter,
                    "{target} has no current Cue; identify an explicit Cue"
                )
            }
            Self::AmbiguousPlaybackContext { target, contexts } => write!(
                formatter,
                "{target} has {contexts} active playback/Cue contexts; identify a concrete playback or Cue"
            ),
            Self::StaleRevision { expected, current } => write!(
                formatter,
                "Update target is stale: expected revision {expected}, current revision is {current}"
            ),
            Self::NoOp { target } => write!(
                formatter,
                "Update would not change {} {}",
                target.name, target.object_id
            ),
            Self::InvalidTarget { reason } => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for UpdateError {}

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
        let mut changed_cues = Vec::new();
        let mut seen = HashSet::new();
        for item in &self.preview.items {
            if let Some(cue) = item.outcome.changed_cue()
                && seen.insert(cue.cue_id)
            {
                changed_cues.push(cue.clone());
            }
        }
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

fn ensure_revision(expected: u64, current: u64) -> Result<(), UpdateError> {
    if expected == current {
        Ok(())
    } else {
        Err(UpdateError::StaleRevision { expected, current })
    }
}

#[derive(Clone, Copy)]
enum IncomingValue<'a> {
    Fixture(&'a ProgrammerFixtureUpdate),
    Group(&'a ProgrammerGroupUpdate),
}

impl IncomingValue<'_> {
    fn address(&self) -> UpdateAddress {
        match self {
            Self::Fixture(value) => UpdateAddress::FixtureAttribute {
                fixture_id: value.fixture_id,
                attribute: value.attribute.clone(),
            },
            Self::Group(value) => UpdateAddress::GroupAttribute {
                group_id: value.group_id.clone(),
                attribute: value.attribute.clone(),
            },
        }
    }

    fn value(&self) -> &AttributeValue {
        match self {
            Self::Fixture(value) => &value.value,
            Self::Group(value) => &value.value,
        }
    }

    fn fade_millis(&self) -> Option<u64> {
        match self {
            Self::Fixture(value) => value.fade_millis,
            Self::Group(value) => value.fade_millis,
        }
    }

    fn delay_millis(&self) -> Option<u64> {
        match self {
            Self::Fixture(value) => value.delay_millis,
            Self::Group(value) => value.delay_millis,
        }
    }
}

fn incoming_values(content: &ProgrammerUpdateContent) -> Vec<IncomingValue<'_>> {
    content
        .fixture_values
        .iter()
        .map(IncomingValue::Fixture)
        .chain(content.group_values.iter().map(IncomingValue::Group))
        .collect()
}

fn incoming_preset_values<'a>(
    preset: &Preset,
    content: &'a ProgrammerUpdateContent,
) -> Vec<IncomingValue<'a>> {
    incoming_values(content)
        .into_iter()
        .filter(|incoming| match incoming.address() {
            UpdateAddress::FixtureAttribute { ref attribute, .. }
            | UpdateAddress::GroupAttribute { ref attribute, .. } => {
                preset.family.accepts(attribute)
            }
            UpdateAddress::GroupMembership { .. } => false,
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
enum CueEventKind {
    Fixture,
    Group,
}

#[derive(Clone, Debug)]
struct CueEventLocation {
    cue_index: usize,
    change_index: usize,
    kind: CueEventKind,
    has_value: bool,
}

struct CueAnalysis {
    all_addresses: HashSet<UpdateAddress>,
    active_sources: HashMap<UpdateAddress, CueEventLocation>,
    current_explicit: HashMap<UpdateAddress, CueEventLocation>,
    current_any: HashMap<UpdateAddress, CueEventLocation>,
}

fn fixture_address(change: &CueChange) -> UpdateAddress {
    UpdateAddress::FixtureAttribute {
        fixture_id: change.fixture_id,
        attribute: change.attribute.clone(),
    }
}

fn group_address(change: &GroupCueChange) -> UpdateAddress {
    UpdateAddress::GroupAttribute {
        group_id: change.group_id.clone(),
        attribute: change.attribute.clone(),
    }
}

fn analyse_cue_list(cue_list: &CueList, current_index: usize) -> CueAnalysis {
    let mut analysis = CueAnalysis {
        all_addresses: HashSet::new(),
        active_sources: HashMap::new(),
        current_explicit: HashMap::new(),
        current_any: HashMap::new(),
    };
    for (cue_index, cue) in cue_list.cues.iter().enumerate() {
        for (change_index, change) in cue.changes.iter().enumerate() {
            let address = fixture_address(change);
            analysis.all_addresses.insert(address.clone());
            let location = CueEventLocation {
                cue_index,
                change_index,
                kind: CueEventKind::Fixture,
                has_value: change.value.is_some(),
            };
            if cue_index <= current_index {
                analysis
                    .active_sources
                    .insert(address.clone(), location.clone());
            }
            if cue_index == current_index {
                analysis
                    .current_any
                    .insert(address.clone(), location.clone());
                if !change.automatic_restore {
                    analysis.current_explicit.insert(address, location);
                }
            }
        }
        for (change_index, change) in cue.group_changes.iter().enumerate() {
            let address = group_address(change);
            analysis.all_addresses.insert(address.clone());
            let location = CueEventLocation {
                cue_index,
                change_index,
                kind: CueEventKind::Group,
                has_value: change.value.is_some(),
            };
            if cue_index <= current_index {
                analysis
                    .active_sources
                    .insert(address.clone(), location.clone());
            }
            if cue_index == current_index {
                analysis
                    .current_any
                    .insert(address.clone(), location.clone());
                if !change.automatic_restore {
                    analysis.current_explicit.insert(address, location);
                }
            }
        }
    }
    analysis
}

fn cue_source(cue_list: &CueList, cue_index: usize) -> CueSource {
    let cue = &cue_list.cues[cue_index];
    CueSource {
        cue_id: cue.id,
        cue_number: cue.number,
        cue_index,
    }
}

fn event_matches(
    cue_list: &CueList,
    location: &CueEventLocation,
    incoming: IncomingValue<'_>,
) -> bool {
    match location.kind {
        CueEventKind::Fixture => {
            let change = &cue_list.cues[location.cue_index].changes[location.change_index];
            change.value.as_ref() == Some(incoming.value())
                && !change.automatic_restore
                && change.fade_millis == incoming.fade_millis()
                && change.delay_millis == incoming.delay_millis()
        }
        CueEventKind::Group => {
            let change = &cue_list.cues[location.cue_index].group_changes[location.change_index];
            change.value.as_ref() == Some(incoming.value())
                && !change.automatic_restore
                && change.fade_millis == incoming.fade_millis()
                && change.delay_millis == incoming.delay_millis()
        }
    }
}

pub fn preview_cue_update(
    cue_list: &CueList,
    target: &ResolvedCueTarget,
    mode: CueUpdateMode,
    programmer: &ProgrammerUpdateContent,
) -> Result<UpdatePreview, UpdateError> {
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
    let current_index = cue_list
        .cues
        .iter()
        .position(|cue| cue.id == target.cue_id)
        .ok_or_else(|| UpdateError::MissingTarget {
            target: format!("Cue {}", target.cue_number),
        })?;
    let current = &cue_list.cues[current_index];
    let current_source = cue_source(cue_list, current_index);
    let analysis = analyse_cue_list(cue_list, current_index);
    let mut items = Vec::new();

    for incoming in incoming_values(programmer) {
        let address = incoming.address();
        let outcome = match mode {
            CueUpdateMode::ExistingOnly => match analysis.active_sources.get(&address) {
                Some(location) if location.has_value => {
                    let source = cue_source(cue_list, location.cue_index);
                    if event_matches(cue_list, location, incoming) {
                        UpdateItemOutcome::Unchanged {
                            source: Some(source),
                        }
                    } else {
                        UpdateItemOutcome::ChangeAtSource { source }
                    }
                }
                _ if analysis.all_addresses.contains(&address) => UpdateItemOutcome::Ignored {
                    reason: UpdateIgnoreReason::NotInActiveTrackedState,
                },
                _ => UpdateItemOutcome::Ignored {
                    reason: UpdateIgnoreReason::NewAddress,
                },
            },
            CueUpdateMode::ExistingInCurrentCue => match analysis.current_explicit.get(&address) {
                Some(location) if event_matches(cue_list, location, incoming) => {
                    UpdateItemOutcome::Unchanged {
                        source: Some(current_source.clone()),
                    }
                }
                Some(_) => UpdateItemOutcome::ChangeInCurrentCue {
                    cue: current_source.clone(),
                },
                None if analysis.all_addresses.contains(&address) => UpdateItemOutcome::Ignored {
                    reason: UpdateIgnoreReason::NotInCurrentCue,
                },
                None => UpdateItemOutcome::Ignored {
                    reason: UpdateIgnoreReason::NewAddress,
                },
            },
            CueUpdateMode::AddToCurrentCue => {
                if !analysis.all_addresses.contains(&address) {
                    UpdateItemOutcome::Ignored {
                        reason: UpdateIgnoreReason::NewAddress,
                    }
                } else {
                    match analysis.current_any.get(&address) {
                        Some(location) if event_matches(cue_list, location, incoming) => {
                            UpdateItemOutcome::Unchanged {
                                source: Some(current_source.clone()),
                            }
                        }
                        Some(_) => UpdateItemOutcome::ChangeInCurrentCue {
                            cue: current_source.clone(),
                        },
                        None => UpdateItemOutcome::AddToCurrentCue {
                            cue: current_source.clone(),
                        },
                    }
                }
            }
            CueUpdateMode::AddNew => match analysis.current_any.get(&address) {
                Some(location) if event_matches(cue_list, location, incoming) => {
                    UpdateItemOutcome::Unchanged {
                        source: Some(current_source.clone()),
                    }
                }
                Some(_) => UpdateItemOutcome::ChangeInCurrentCue {
                    cue: current_source.clone(),
                },
                None if analysis.all_addresses.contains(&address) => {
                    UpdateItemOutcome::AddToCurrentCue {
                        cue: current_source.clone(),
                    }
                }
                None => UpdateItemOutcome::AddNewToCurrentCue {
                    cue: current_source.clone(),
                },
            },
        };
        items.push(UpdatePreviewItem { address, outcome });
    }

    Ok(UpdatePreview {
        target: UpdateTargetIdentity::cue(cue_list, target, current),
        mode: UpdateMode::Cue(mode),
        items,
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
                return Err(UpdateError::InvalidTarget {
                    reason: "authoritative fixture source event disappeared while planning Update"
                        .into(),
                });
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
                return Err(UpdateError::InvalidTarget {
                    reason: "authoritative Group source event disappeared while planning Update"
                        .into(),
                });
            }
        }
    }
    Ok(())
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

pub fn preview_preset_update(
    preset_id: &str,
    preset: &Preset,
    mode: ExistingContentMode,
    programmer: &ProgrammerUpdateContent,
) -> Result<UpdatePreview, UpdateError> {
    if !programmer.has_values() {
        return Err(UpdateError::EmptyProgrammer {
            target_family: UpdateTargetFamily::Preset,
        });
    }
    let mut items = Vec::new();
    for incoming in incoming_preset_values(preset, programmer) {
        let address = incoming.address();
        let existing = match &address {
            UpdateAddress::FixtureAttribute {
                fixture_id,
                attribute,
            } => preset
                .values
                .get(fixture_id)
                .and_then(|attributes| attributes.get(attribute)),
            UpdateAddress::GroupAttribute {
                group_id,
                attribute,
            } => preset
                .group_values
                .get(group_id)
                .and_then(|attributes| attributes.get(attribute)),
            UpdateAddress::GroupMembership { .. } => None,
        };
        let outcome = match (mode, existing) {
            (_, Some(value)) if value == incoming.value() => {
                UpdateItemOutcome::Unchanged { source: None }
            }
            (_, Some(_)) => UpdateItemOutcome::UpdateExisting,
            (ExistingContentMode::UpdateExisting, None) => UpdateItemOutcome::Ignored {
                reason: UpdateIgnoreReason::NewAddress,
            },
            (ExistingContentMode::AddNew, None) => UpdateItemOutcome::AddNew,
        };
        items.push(UpdatePreviewItem { address, outcome });
    }
    Ok(UpdatePreview {
        target: UpdateTargetIdentity::preset(preset_id, preset),
        mode: UpdateMode::ExistingContent(mode),
        items,
    })
}

fn write_preset_value(preset: &mut Preset, incoming: IncomingValue<'_>) {
    match incoming {
        IncomingValue::Fixture(value) => {
            preset
                .values
                .entry(value.fixture_id)
                .or_default()
                .insert(value.attribute.clone(), value.value.clone());
        }
        IncomingValue::Group(value) => {
            preset
                .group_values
                .entry(value.group_id.clone())
                .or_default()
                .insert(value.attribute.clone(), value.value.clone());
        }
    }
}

pub fn plan_preset_update(
    preset_id: &str,
    preset: &Preset,
    current_revision: u64,
    expected_revision: u64,
    mode: ExistingContentMode,
    programmer: &ProgrammerUpdateContent,
) -> Result<AtomicUpdatePlan, UpdateError> {
    ensure_revision(expected_revision, current_revision)?;
    let preview = preview_preset_update(preset_id, preset, mode, programmer)?;
    if !preview.has_real_change() {
        return Err(UpdateError::NoOp {
            target: preview.target,
        });
    }
    let mut updated = preset.clone();
    for (incoming, item) in incoming_preset_values(preset, programmer)
        .into_iter()
        .zip(&preview.items)
    {
        if item.outcome.changes_data() {
            write_preset_value(&mut updated, incoming);
        }
    }
    Ok(AtomicUpdatePlan {
        target: preview.target.clone(),
        expected_revision,
        preview,
        object: PlannedUpdateObject::Preset(updated),
    })
}

pub fn preview_group_update(
    group: &GroupDefinition,
    resolved_membership: &[FixtureId],
    mode: ExistingContentMode,
    programmer: &ProgrammerUpdateContent,
) -> Result<UpdatePreview, UpdateError> {
    if !programmer.has_selection() {
        return Err(UpdateError::EmptyProgrammer {
            target_family: UpdateTargetFamily::Group,
        });
    }
    let existing = resolved_membership.iter().copied().collect::<HashSet<_>>();
    let mut selected = HashSet::new();
    let mut items = Vec::new();
    for fixture_id in &programmer.selected_fixtures {
        if !selected.insert(*fixture_id) {
            continue;
        }
        let outcome = if existing.contains(fixture_id) {
            UpdateItemOutcome::Unchanged { source: None }
        } else if mode == ExistingContentMode::AddNew {
            UpdateItemOutcome::AddNew
        } else {
            UpdateItemOutcome::Ignored {
                reason: UpdateIgnoreReason::NewGroupMember,
            }
        };
        items.push(UpdatePreviewItem {
            address: UpdateAddress::GroupMembership {
                fixture_id: *fixture_id,
            },
            outcome,
        });
    }
    Ok(UpdatePreview {
        target: UpdateTargetIdentity::group(group),
        mode: UpdateMode::ExistingContent(mode),
        items,
    })
}

pub fn plan_group_update(
    group: &GroupDefinition,
    resolved_membership: &[FixtureId],
    current_revision: u64,
    expected_revision: u64,
    mode: ExistingContentMode,
    programmer: &ProgrammerUpdateContent,
) -> Result<AtomicUpdatePlan, UpdateError> {
    ensure_revision(expected_revision, current_revision)?;
    let preview = preview_group_update(group, resolved_membership, mode, programmer)?;
    if !preview.has_real_change() {
        return Err(UpdateError::NoOp {
            target: preview.target,
        });
    }
    let mut updated = group.clone();
    updated.fixtures =
        merge_ordered_group_membership(resolved_membership, &programmer.selected_fixtures);
    // Normal Group Merge dereferences only when it actually adds membership.
    updated.derived_from = None;
    updated.frozen_from = None;
    Ok(AtomicUpdatePlan {
        target: preview.target.clone(),
        expected_revision,
        preview,
        object: PlannedUpdateObject::Group(updated),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_playback::{CueListMode, IntensityPriorityMode, RestartMode, WrapMode};

    fn fixture(id: u128) -> FixtureId {
        FixtureId(Uuid::from_u128(id))
    }

    fn attribute(name: &str) -> AttributeKey {
        AttributeKey(name.into())
    }

    fn normalized(value: f32) -> AttributeValue {
        AttributeValue::Normalized(value)
    }

    fn fixture_update(
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

    fn content(values: Vec<ProgrammerFixtureUpdate>) -> ProgrammerUpdateContent {
        ProgrammerUpdateContent {
            fixture_values: values,
            ..Default::default()
        }
    }

    fn cue(number: f64, changes: Vec<CueChange>) -> Cue {
        let mut cue = Cue::new(number);
        cue.changes = changes;
        cue
    }

    fn change(fixture_id: FixtureId, name: &str, value: f32) -> CueChange {
        CueChange::set(fixture_id, attribute(name), normalized(value))
    }

    fn cue_list(cues: Vec<Cue>) -> CueList {
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

    fn target(list: &CueList, index: usize, playback_number: Option<u16>) -> ResolvedCueTarget {
        ResolvedCueTarget {
            cue_list_id: list.id,
            playback_number,
            cue_id: list.cues[index].id,
            cue_number: list.cues[index].number,
        }
    }

    fn planned_cue_list(plan: AtomicUpdatePlan) -> CueList {
        match plan.object {
            PlannedUpdateObject::CueList(list) => list,
            _ => panic!("expected Cuelist plan"),
        }
    }

    fn planned_preset(plan: AtomicUpdatePlan) -> Preset {
        match plan.object {
            PlannedUpdateObject::Preset(preset) => preset,
            _ => panic!("expected Preset plan"),
        }
    }

    fn planned_group(plan: AtomicUpdatePlan) -> GroupDefinition {
        match plan.object {
            PlannedUpdateObject::Group(group) => group,
            _ => panic!("expected Group plan"),
        }
    }

    fn stored_value(cue: &Cue, fixture_id: FixtureId, name: &str) -> Option<f32> {
        cue.changes
            .iter()
            .find(|change| change.fixture_id == fixture_id && change.attribute == attribute(name))
            .and_then(|change| change.value.as_ref())
            .and_then(AttributeValue::normalized)
    }

    #[test]
    fn existing_only_changes_the_latest_authoritative_tracked_source() {
        let fixture = fixture(1);
        let list = cue_list(vec![
            cue(1.0, vec![change(fixture, "intensity", 0.2)]),
            cue(2.0, vec![change(fixture, "intensity", 0.4)]),
            cue(3.0, vec![]),
        ]);
        let target = target(&list, 2, Some(1));
        let programmer = content(vec![fixture_update(fixture, "intensity", 0.8, 1)]);

        let plan = plan_cue_update(
            &list,
            7,
            7,
            &target,
            CueUpdateMode::ExistingOnly,
            &programmer,
        )
        .unwrap();
        assert!(matches!(
            plan.preview.items[0].outcome,
            UpdateItemOutcome::ChangeAtSource {
                source: CueSource {
                    cue_number: 2.0,
                    cue_index: 1,
                    ..
                }
            }
        ));
        let updated = planned_cue_list(plan);
        assert_eq!(
            stored_value(&updated.cues[0], fixture, "intensity"),
            Some(0.2)
        );
        assert_eq!(
            stored_value(&updated.cues[1], fixture, "intensity"),
            Some(0.8)
        );
        assert_eq!(stored_value(&updated.cues[2], fixture, "intensity"), None);
    }

    #[test]
    fn a_later_release_prevents_existing_only_from_rewriting_an_unrelated_earlier_value() {
        let fixture = fixture(1);
        let mut release = change(fixture, "intensity", 0.0);
        release.value = None;
        let list = cue_list(vec![
            cue(1.0, vec![change(fixture, "intensity", 0.2)]),
            cue(2.0, vec![release]),
            cue(3.0, vec![]),
        ]);
        let target = target(&list, 2, Some(1));
        let programmer = content(vec![fixture_update(fixture, "intensity", 0.8, 1)]);

        let preview =
            preview_cue_update(&list, &target, CueUpdateMode::ExistingOnly, &programmer).unwrap();
        assert_eq!(preview.changed_count(), 0);
        assert_eq!(
            preview.items[0].outcome,
            UpdateItemOutcome::Ignored {
                reason: UpdateIgnoreReason::NotInActiveTrackedState
            }
        );
        assert!(matches!(
            plan_cue_update(
                &list,
                1,
                1,
                &target,
                CueUpdateMode::ExistingOnly,
                &programmer
            ),
            Err(UpdateError::NoOp { .. })
        ));
    }

    #[test]
    fn four_cue_modes_keep_tracked_source_current_cue_and_new_addresses_distinct() {
        let fixture = fixture(1);
        let list = cue_list(vec![
            cue(1.0, vec![change(fixture, "intensity", 0.5)]),
            cue(2.0, vec![change(fixture, "pan", 0.25)]),
        ]);
        let target = target(&list, 1, Some(1));
        let programmer = content(vec![
            fixture_update(fixture, "intensity", 0.8, 1),
            fixture_update(fixture, "color.red", 0.6, 2),
        ]);

        let existing = plan_cue_update(
            &list,
            1,
            1,
            &target,
            CueUpdateMode::ExistingOnly,
            &programmer,
        )
        .unwrap();
        assert_eq!(existing.preview.changed_count(), 1);
        assert_eq!(existing.preview.ignored_count(), 1);
        let existing = planned_cue_list(existing);
        assert_eq!(
            stored_value(&existing.cues[0], fixture, "intensity"),
            Some(0.8)
        );
        assert_eq!(stored_value(&existing.cues[1], fixture, "intensity"), None);

        let current = preview_cue_update(
            &list,
            &target,
            CueUpdateMode::ExistingInCurrentCue,
            &programmer,
        )
        .unwrap();
        assert_eq!(current.changed_count(), 0);
        assert_eq!(current.ignored_count(), 2);

        let add_current = plan_cue_update(
            &list,
            1,
            1,
            &target,
            CueUpdateMode::AddToCurrentCue,
            &programmer,
        )
        .unwrap();
        assert_eq!(add_current.preview.added_count(), 1);
        assert_eq!(add_current.preview.ignored_count(), 1);
        let add_current = planned_cue_list(add_current);
        assert_eq!(
            stored_value(&add_current.cues[1], fixture, "intensity"),
            Some(0.8)
        );
        assert_eq!(
            stored_value(&add_current.cues[1], fixture, "color.red"),
            None
        );

        let add_new =
            plan_cue_update(&list, 1, 1, &target, CueUpdateMode::AddNew, &programmer).unwrap();
        assert_eq!(add_new.preview.added_count(), 2);
        let add_new = planned_cue_list(add_new);
        assert_eq!(
            stored_value(&add_new.cues[1], fixture, "intensity"),
            Some(0.8)
        );
        assert_eq!(
            stored_value(&add_new.cues[1], fixture, "color.red"),
            Some(0.6)
        );
    }

    #[test]
    fn cue_eligibility_is_exact_per_fixture_and_attribute() {
        let fixtures = [fixture(1), fixture(2), fixture(3), fixture(4)];
        let list = cue_list(vec![
            cue(
                1.0,
                vec![
                    change(fixtures[0], "color.red", 0.1),
                    change(fixtures[1], "color.red", 0.1),
                ],
            ),
            cue(2.0, vec![]),
        ]);
        let target = target(&list, 1, Some(1));
        let programmer = content(
            fixtures
                .iter()
                .enumerate()
                .map(|(index, fixture_id)| {
                    fixture_update(*fixture_id, "color.red", 0.8, index as u64)
                })
                .collect(),
        );

        let preview =
            preview_cue_update(&list, &target, CueUpdateMode::AddToCurrentCue, &programmer)
                .unwrap();
        assert_eq!(preview.changed_count(), 2);
        assert_eq!(preview.ignored_count(), 2);
        let updated = planned_cue_list(
            plan_cue_update(
                &list,
                2,
                2,
                &target,
                CueUpdateMode::AddToCurrentCue,
                &programmer,
            )
            .unwrap(),
        );
        assert_eq!(updated.cues[1].changes.len(), 2);
        assert!(
            updated.cues[1]
                .changes
                .iter()
                .all(|change| fixtures[..2].contains(&change.fixture_id))
        );
    }

    #[test]
    fn preset_update_existing_and_add_new_follow_exact_addresses() {
        let fixtures = [fixture(1), fixture(2), fixture(3), fixture(4)];
        let preset = Preset {
            name: "Color 1".into(),
            family: light_programmer::PresetFamily::Color,
            number: 1,
            values: fixtures[..2]
                .iter()
                .map(|fixture_id| {
                    (
                        *fixture_id,
                        HashMap::from([(attribute("color.red"), normalized(0.1))]),
                    )
                })
                .collect(),
            group_values: HashMap::new(),
        };
        let programmer = content(
            fixtures
                .iter()
                .enumerate()
                .map(|(index, fixture_id)| {
                    fixture_update(*fixture_id, "color.red", 0.8, index as u64)
                })
                .collect(),
        );

        let existing = plan_preset_update(
            "1",
            &preset,
            4,
            4,
            ExistingContentMode::UpdateExisting,
            &programmer,
        )
        .unwrap();
        assert_eq!(existing.preview.changed_count(), 2);
        assert_eq!(existing.preview.ignored_count(), 2);
        let existing = planned_preset(existing);
        assert_eq!(existing.values.len(), 2);
        assert!(
            existing.values.values().all(|attributes| {
                attributes[&attribute("color.red")].normalized() == Some(0.8)
            })
        );

        let added = planned_preset(
            plan_preset_update("1", &preset, 4, 4, ExistingContentMode::AddNew, &programmer)
                .unwrap(),
        );
        assert_eq!(added.values.len(), 4);
    }

    #[test]
    fn group_add_new_preserves_order_and_existing_only_never_mutates_membership() {
        let first = fixture(1);
        let second = fixture(2);
        let third = fixture(3);
        let fourth = fixture(4);
        let group = GroupDefinition {
            id: "1".into(),
            name: "Group 1".into(),
            fixtures: vec![second, first],
            ..Default::default()
        };
        let programmer = ProgrammerUpdateContent {
            selected_fixtures: vec![first, third, second, fourth, third],
            ..Default::default()
        };

        let existing = preview_group_update(
            &group,
            &[second, first],
            ExistingContentMode::UpdateExisting,
            &programmer,
        )
        .unwrap();
        assert_eq!(existing.changed_count(), 0);
        assert_eq!(existing.eligible_count(), 2);
        assert_eq!(existing.ignored_count(), 2);
        assert!(matches!(
            plan_group_update(
                &group,
                &[second, first],
                3,
                3,
                ExistingContentMode::UpdateExisting,
                &programmer,
            ),
            Err(UpdateError::NoOp { .. })
        ));

        let updated = planned_group(
            plan_group_update(
                &group,
                &[second, first],
                3,
                3,
                ExistingContentMode::AddNew,
                &programmer,
            )
            .unwrap(),
        );
        assert_eq!(updated.fixtures, vec![second, first, third, fourth]);
    }

    #[test]
    fn pool_cuelist_requires_one_concrete_active_playback_context() {
        let cue_list_id = CueListId(Uuid::from_u128(10));
        let contexts = vec![
            ActiveCueContext {
                playback_number: 1,
                cue_list_id,
                cue_id: Uuid::from_u128(11),
                cue_number: 1.0,
            },
            ActiveCueContext {
                playback_number: 2,
                cue_list_id,
                cue_id: Uuid::from_u128(12),
                cue_number: 2.0,
            },
        ];
        assert_eq!(
            resolve_cue_target(&CueTargetRequest::PoolCueList { cue_list_id }, &contexts),
            Err(UpdateError::AmbiguousPlaybackContext {
                target: format!("Cuelist {}", cue_list_id.0),
                contexts: 2
            })
        );
        let concrete = resolve_cue_target(
            &CueTargetRequest::ActivePlayback { playback_number: 2 },
            &contexts,
        )
        .unwrap();
        assert_eq!(concrete.playback_number, Some(2));
        assert_eq!(concrete.cue_number, 2.0);
    }

    #[test]
    fn old_settings_receive_documented_defaults_and_confirmation_paths_are_distinct() {
        let settings: UpdateSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.cue_mode, CueUpdateMode::AddToCurrentCue);
        assert_eq!(settings.preset_mode, ExistingContentMode::UpdateExisting);
        assert!(settings.show_update_modal_on_touch);
        assert_eq!(
            settings.confirmation_behavior(&UpdateTargetFamily::Cue, UpdateConfirmationPath::Touch),
            UpdateConfirmationBehavior::OpenModal
        );
        assert_eq!(
            settings.confirmation_behavior(&UpdateTargetFamily::Cue, UpdateConfirmationPath::Enter),
            UpdateConfirmationBehavior::ApplyDefault(UpdateMode::Cue(
                CueUpdateMode::AddToCurrentCue
            ))
        );

        let settings = UpdateSettings {
            show_update_modal_on_touch: false,
            ..settings
        };
        assert_eq!(
            settings
                .confirmation_behavior(&UpdateTargetFamily::Preset, UpdateConfirmationPath::Touch),
            UpdateConfirmationBehavior::ApplyDefault(UpdateMode::ExistingContent(
                ExistingContentMode::UpdateExisting
            ))
        );
    }

    #[test]
    fn stale_and_no_op_updates_produce_no_mutation_plan() {
        let fixture = fixture(1);
        let preset = Preset {
            name: "Intensity".into(),
            family: light_programmer::PresetFamily::Intensity,
            number: 1,
            values: HashMap::from([(
                fixture,
                HashMap::from([(attribute("intensity"), normalized(0.5))]),
            )]),
            group_values: HashMap::new(),
        };
        let changed = content(vec![fixture_update(fixture, "intensity", 0.8, 1)]);
        assert!(matches!(
            plan_preset_update(
                "1",
                &preset,
                9,
                8,
                ExistingContentMode::UpdateExisting,
                &changed
            ),
            Err(UpdateError::StaleRevision {
                expected: 8,
                current: 9
            })
        ));

        let unchanged = content(vec![fixture_update(fixture, "intensity", 0.5, 1)]);
        assert!(matches!(
            plan_preset_update(
                "1",
                &preset,
                9,
                9,
                ExistingContentMode::UpdateExisting,
                &unchanged
            ),
            Err(UpdateError::NoOp { .. })
        ));
        assert_eq!(
            preset.values[&fixture][&attribute("intensity")].normalized(),
            Some(0.5)
        );
    }

    #[test]
    fn preset_update_ignores_attributes_outside_the_stored_family() {
        let fixture = fixture(1);
        let preset = Preset {
            name: "Color".into(),
            family: light_programmer::PresetFamily::Color,
            number: 1,
            values: HashMap::from([(
                fixture,
                HashMap::from([(attribute("color.red"), normalized(0.2))]),
            )]),
            group_values: HashMap::new(),
        };
        let programmer = content(vec![
            fixture_update(fixture, "color.red", 0.8, 1),
            fixture_update(fixture, "pan", 0.6, 2),
        ]);

        let plan = plan_preset_update(
            "2.1",
            &preset,
            3,
            3,
            ExistingContentMode::AddNew,
            &programmer,
        )
        .unwrap();
        assert_eq!(plan.preview.items.len(), 1);
        let PlannedUpdateObject::Preset(updated) = plan.object else {
            panic!("expected preset update")
        };
        assert_eq!(updated.values[&fixture].len(), 1);
        assert_eq!(
            updated.values[&fixture][&attribute("color.red")],
            normalized(0.8)
        );
    }

    #[test]
    fn one_atomic_cuelist_plan_reports_every_changed_source_and_retains_programmer_values() {
        let first = fixture(1);
        let second = fixture(2);
        let list = cue_list(vec![
            cue(1.0, vec![change(first, "intensity", 0.2)]),
            cue(2.0, vec![change(second, "pan", 0.3)]),
            cue(3.0, vec![]),
        ]);
        let target = target(&list, 2, Some(1));
        let programmer = content(vec![
            fixture_update(first, "intensity", 0.8, 1),
            fixture_update(second, "pan", 0.9, 2),
        ]);
        let plan = plan_cue_update(
            &list,
            11,
            11,
            &target,
            CueUpdateMode::ExistingOnly,
            &programmer,
        )
        .unwrap();
        assert_eq!(plan.object_kind(), "cue_list");
        assert!(plan.body().is_ok());
        let result = plan.complete(12);
        assert_eq!(result.changed_count, 2);
        assert_eq!(result.changed_cues.len(), 2);
        assert_eq!(result.revision_before, 11);
        assert_eq!(result.revision_after, 12);
        assert!(result.programmer_values_retained);
        assert_eq!(programmer.fixture_values.len(), 2);
    }

    #[test]
    fn eligible_menu_filter_excludes_no_ops_but_show_all_keeps_them_distinguishable() {
        let fixture = fixture(1);
        let preset = Preset {
            name: "Intensity".into(),
            family: light_programmer::PresetFamily::Intensity,
            number: 1,
            values: HashMap::from([(
                fixture,
                HashMap::from([(attribute("intensity"), normalized(0.5))]),
            )]),
            group_values: HashMap::new(),
        };
        let changed = content(vec![fixture_update(fixture, "intensity", 0.8, 1)]);
        let unchanged = content(vec![fixture_update(fixture, "intensity", 0.5, 1)]);
        let changed_preview =
            preview_preset_update("1", &preset, ExistingContentMode::UpdateExisting, &changed)
                .unwrap();
        let no_op_preview = preview_preset_update(
            "2",
            &preset,
            ExistingContentMode::UpdateExisting,
            &unchanged,
        )
        .unwrap();
        let entries = vec![
            UpdateMenuEntry {
                target: changed_preview.target.clone(),
                active_or_referenced: true,
                existing_preview: changed_preview,
                add_new_preview: None,
            },
            UpdateMenuEntry {
                target: no_op_preview.target.clone(),
                active_or_referenced: true,
                existing_preview: no_op_preview,
                add_new_preview: None,
            },
        ];
        assert_eq!(
            filter_update_menu(&entries, UpdateTargetFilter::EligibleForUpdateExisting).len(),
            1
        );
        assert_eq!(
            filter_update_menu(&entries, UpdateTargetFilter::ShowAllActive).len(),
            2
        );
        assert!(entries[1].is_no_op(UpdateMode::ExistingContent(
            ExistingContentMode::UpdateExisting
        )));
    }

    #[test]
    fn cue_fixture_and_group_addresses_track_independently() {
        let fixture = fixture(1);
        let mut first = cue(1.0, vec![change(fixture, "intensity", 0.2)]);
        first.group_changes.push(GroupCueChange {
            group_id: "front".into(),
            attribute: attribute("intensity"),
            value: Some(normalized(0.4)),
            automatic_restore: false,
            fade_millis: None,
            delay_millis: None,
        });
        let list = cue_list(vec![first, cue(2.0, vec![])]);
        let target = target(&list, 1, Some(1));
        let programmer = ProgrammerUpdateContent {
            fixture_values: vec![fixture_update(fixture, "intensity", 0.8, 1)],
            group_values: vec![ProgrammerGroupUpdate {
                group_id: "front".into(),
                attribute: attribute("intensity"),
                value: normalized(0.9),
                programmer_order: 2,
                fade_millis: None,
                delay_millis: None,
            }],
            selected_fixtures: vec![],
        };
        let updated = planned_cue_list(
            plan_cue_update(
                &list,
                1,
                1,
                &target,
                CueUpdateMode::ExistingOnly,
                &programmer,
            )
            .unwrap(),
        );
        assert_eq!(
            stored_value(&updated.cues[0], fixture, "intensity"),
            Some(0.8)
        );
        assert_eq!(
            updated.cues[0].group_changes[0]
                .value
                .as_ref()
                .and_then(AttributeValue::normalized),
            Some(0.9)
        );
    }

    #[test]
    fn existing_in_current_cue_treats_explicit_release_as_stored_but_not_generated_restore() {
        let explicit_fixture = fixture(1);
        let generated_fixture = fixture(2);
        let mut explicit_release = change(explicit_fixture, "intensity", 0.0);
        explicit_release.value = None;
        let mut generated = change(generated_fixture, "intensity", 0.2);
        generated.automatic_restore = true;
        let list = cue_list(vec![cue(1.0, vec![explicit_release, generated])]);
        let target = target(&list, 0, Some(1));
        let programmer = content(vec![
            fixture_update(explicit_fixture, "intensity", 0.8, 1),
            fixture_update(generated_fixture, "intensity", 0.9, 2),
        ]);
        let preview = preview_cue_update(
            &list,
            &target,
            CueUpdateMode::ExistingInCurrentCue,
            &programmer,
        )
        .unwrap();
        assert!(matches!(
            preview.items[0].outcome,
            UpdateItemOutcome::ChangeInCurrentCue { .. }
        ));
        assert_eq!(
            preview.items[1].outcome,
            UpdateItemOutcome::Ignored {
                reason: UpdateIgnoreReason::NotInCurrentCue
            }
        );
    }
}
