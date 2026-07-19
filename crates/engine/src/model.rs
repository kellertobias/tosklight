use light_core::{AttributeKey, AttributeValue, FixtureId, Universe};
use light_fixture::{PatchedFixture, validate_patch};
use light_output::{DmxFrame, OutputRoute};
use light_playback::{
    AutomaticPlaybackTransition, CueList, PlaybackDefinition, PlaybackPage, PlaybackTarget,
};
use light_programmer::{GroupDefinition, resolve_group};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EngineSnapshot {
    pub fixtures: Vec<PatchedFixture>,
    pub cue_lists: Vec<CueList>,
    #[serde(default)]
    pub playbacks: Vec<PlaybackDefinition>,
    #[serde(default)]
    pub playback_pages: Vec<PlaybackPage>,
    pub routes: Vec<OutputRoute>,
    pub control_mappings: Vec<light_control::ControlMapping>,
    #[serde(default)]
    pub groups: Vec<GroupDefinition>,
    pub revision: u64,
}

impl EngineSnapshot {
    pub fn validate(&self) -> Result<(), EngineError> {
        validate_patch(&self.fixtures)?;
        let groups = self
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        for group in &self.groups {
            if let Some(derived) = &group.derived_from {
                derived.rule.validate().map_err(EngineError::Invalid)?;
            }
            if !group.master.is_finite() || !(0.0..=1.0).contains(&group.master) {
                return Err(EngineError::Invalid(format!(
                    "group {} master must be within 0-1",
                    group.id
                )));
            }
            resolve_group(&group.id, &groups).map_err(EngineError::Invalid)?;
        }
        for cue_list in &self.cue_lists {
            cue_list.validate().map_err(EngineError::Invalid)?;
        }
        let mut playback_numbers = std::collections::HashSet::new();
        for playback in &self.playbacks {
            playback.validate().map_err(EngineError::Invalid)?;
            if !playback_numbers.insert(playback.number) {
                return Err(EngineError::Invalid("duplicate playback number".into()));
            }
            match &playback.target {
                PlaybackTarget::CueList { cue_list_id }
                    if !self.cue_lists.iter().any(|cue| cue.id == *cue_list_id) =>
                {
                    return Err(EngineError::Invalid(
                        "playback references a missing cue list".into(),
                    ));
                }
                PlaybackTarget::Group { group_id }
                    if !self.groups.iter().any(|group| group.id == *group_id) =>
                {
                    return Err(EngineError::Invalid(
                        "playback references a missing group".into(),
                    ));
                }
                _ => {}
            }
        }
        for page in &self.playback_pages {
            page.validate().map_err(EngineError::Invalid)?;
            if page
                .slots
                .values()
                .any(|number| !playback_numbers.contains(number))
            {
                return Err(EngineError::Invalid(
                    "page references a missing playback".into(),
                ));
            }
        }
        for route in &self.routes {
            if route.destination_universe == 0 || route.logical_universe == 0 {
                return Err(EngineError::Invalid(
                    "universe zero is not valid for show routes".into(),
                ));
            }
            if !(1..=light_output::DMX_SLOTS as u16).contains(&route.minimum_slots) {
                return Err(EngineError::Invalid(
                    "route minimum slots must be within 1-512".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RenderOptions {
    pub grand_master: f32,
    pub blackout: bool,
    pub control_loss_progress: Option<f32>,
}
impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            grand_master: 1.0,
            blackout: false,
            control_loss_progress: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderResult {
    pub universes: HashMap<Universe, DmxFrame>,
    /// Highest patched slot for each logical universe. This is kept separately from values so a
    /// patched channel whose default is zero still extends the network payload.
    pub patched_slots: HashMap<Universe, u16>,
    pub revision: u64,
    /// Output routes compiled from the same generation as `universes`.
    pub routes: Arc<[OutputRoute]>,
    /// Scheduler transitions collected under the playback lock and returned for publication only
    /// after rendering has left the domain lock boundary.
    pub automatic_playback_transitions: Vec<AutomaticPlaybackTransition>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveInBlackState {
    Disabled,
    Blocked,
    Delaying,
    Moving,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize)]
pub struct MoveInBlackPosition {
    pub attribute: AttributeKey,
    pub current: AttributeValue,
    pub target: AttributeValue,
}

#[derive(Clone, Debug, Serialize)]
pub struct MoveInBlackDiagnostic {
    pub fixture_id: FixtureId,
    pub playback_number: Option<u16>,
    pub cue_list_id: light_core::CueListId,
    pub current_cue_id: uuid::Uuid,
    pub current_cue_number: f64,
    pub target_cue_id: uuid::Uuid,
    pub target_cue_number: f64,
    pub state: MoveInBlackState,
    pub positions: Vec<MoveInBlackPosition>,
    pub dark_since: Option<chrono::DateTime<chrono::Utc>>,
    pub delay_deadline: Option<chrono::DateTime<chrono::Utc>>,
    pub movement_started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub movement_ends_at: Option<chrono::DateTime<chrono::Utc>>,
    pub cancellation_reason: Option<String>,
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("snapshot validation failed: {0}")]
    Invalid(String),
    #[error(transparent)]
    Fixture(#[from] light_fixture::FixtureError),
}
