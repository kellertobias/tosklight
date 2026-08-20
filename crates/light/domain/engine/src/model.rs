use crate::{ResolvedChangedAt, ResolvedValues};
use light_core::{AttributeKey, AttributeValue, FixtureId, Universe};
use light_dynamics::{DynamicDefinition, validate_definition};
use light_fixture::{PatchedFixture, validate_patch};
use light_output::{DmxFrame, OutputRoute};
use light_playback::{
    AutomaticPlaybackTransition, CueList, CueNumber, PlaybackDefinition, PlaybackPage,
    PlaybackTarget,
};
use light_programmer::{GroupDefinition, GroupFixtureSource, resolve_group};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EngineSnapshot {
    pub fixtures: Arc<Vec<PatchedFixture>>,
    pub cue_lists: Arc<Vec<CueList>>,
    #[serde(default)]
    pub dynamics: Arc<Vec<DynamicDefinition>>,
    /// Replaceable target-ordering input for Dynamics. Definitions do not persist a private grid.
    #[serde(default)]
    pub dynamic_stage_positions: Arc<HashMap<FixtureId, light_dynamics::SpatialPosition>>,
    #[serde(default)]
    pub playbacks: Arc<Vec<PlaybackDefinition>>,
    #[serde(default)]
    pub playback_pages: Arc<Vec<PlaybackPage>>,
    pub routes: Arc<Vec<OutputRoute>>,
    pub control_mappings: Arc<Vec<light_control::ControlMapping>>,
    #[serde(default)]
    pub groups: Arc<Vec<GroupDefinition>>,
    pub revision: u64,
}

impl EngineSnapshot {
    pub fn validate(&self) -> Result<(), EngineError> {
        self.validate_changed(None)
    }

    pub(crate) fn validate_changed(&self, previous: Option<&Self>) -> Result<(), EngineError> {
        if previous.is_none_or(|previous| !Arc::ptr_eq(&self.fixtures, &previous.fixtures)) {
            validate_patch(&self.fixtures)?;
        }
        let groups_changed =
            previous.is_none_or(|previous| !Arc::ptr_eq(&self.groups, &previous.groups));
        let cue_lists_changed =
            previous.is_none_or(|previous| !Arc::ptr_eq(&self.cue_lists, &previous.cue_lists));
        let dynamics_changed =
            previous.is_none_or(|previous| !Arc::ptr_eq(&self.dynamics, &previous.dynamics));
        let playbacks_changed =
            previous.is_none_or(|previous| !Arc::ptr_eq(&self.playbacks, &previous.playbacks));
        let pages_changed = previous
            .is_none_or(|previous| !Arc::ptr_eq(&self.playback_pages, &previous.playback_pages));
        let routes_changed =
            previous.is_none_or(|previous| !Arc::ptr_eq(&self.routes, &previous.routes));
        let groups = self
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        for group in self.groups.iter().filter(|_| groups_changed) {
            match group.source.as_ref() {
                Some(GroupFixtureSource::References { references }) => {
                    for reference in references {
                        reference.rule.validate().map_err(EngineError::Invalid)?;
                    }
                }
                Some(GroupFixtureSource::Explicit { .. }) => {}
                None => {
                    if let Some(derived) = &group.derived_from {
                        derived.rule.validate().map_err(EngineError::Invalid)?;
                    }
                }
            }
            if let Some(mapping) = group.mapping.as_ref() {
                light_dynamics::evaluate_spatial_mapping(mapping, &[])
                    .map_err(|error| EngineError::Invalid(error.to_string()))?;
            }
            resolve_group(&group.id, &groups).map_err(EngineError::Invalid)?;
        }
        for cue_list in self.cue_lists.iter().filter(|_| cue_lists_changed) {
            cue_list.validate().map_err(EngineError::Invalid)?;
        }
        if dynamics_changed {
            let mut ids = std::collections::HashSet::new();
            let mut pool_numbers = std::collections::HashSet::new();
            for dynamic in self.dynamics.iter() {
                validate_definition(dynamic)
                    .map_err(|error| EngineError::Invalid(error.to_string()))?;
                if !ids.insert(dynamic.id) {
                    return Err(EngineError::Invalid("duplicate Dynamic identity".into()));
                }
                if !pool_numbers.insert(dynamic.pool_number) {
                    return Err(EngineError::Invalid("duplicate Dynamic pool number".into()));
                }
                if let light_dynamics::DynamicTargetBinding::LiveGroup { group_id } =
                    &dynamic.target_binding
                    && !groups.contains_key(group_id)
                {
                    return Err(EngineError::Invalid(format!(
                        "Dynamic {} references a missing Group",
                        dynamic.pool_number
                    )));
                }
            }
        }
        let mut playback_numbers = std::collections::HashSet::new();
        let validate_playbacks = playbacks_changed || cue_lists_changed || groups_changed;
        for playback in self.playbacks.iter() {
            let unique_number = playback_numbers.insert(playback.number);
            if !validate_playbacks {
                continue;
            }
            playback.validate().map_err(EngineError::Invalid)?;
            if !unique_number {
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
                PlaybackTarget::Group { group_id, .. }
                    if !self.groups.iter().any(|group| group.id == *group_id) =>
                {
                    return Err(EngineError::Invalid(
                        "playback references a missing group".into(),
                    ));
                }
                _ => {}
            }
        }
        for page in self
            .playback_pages
            .iter()
            .filter(|_| pages_changed || playbacks_changed)
        {
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
            for playback in page.virtual_playbacks.values() {
                match &playback.target {
                    PlaybackTarget::CueList { cue_list_id }
                        if !self.cue_lists.iter().any(|cue| cue.id == *cue_list_id) =>
                    {
                        return Err(EngineError::Invalid(
                            "virtual playback references a missing cue list".into(),
                        ));
                    }
                    PlaybackTarget::Group { group_id, .. }
                        if !self.groups.iter().any(|group| group.id == *group_id) =>
                    {
                        return Err(EngineError::Invalid(
                            "virtual playback references a missing group".into(),
                        ));
                    }
                    _ => {}
                }
            }
        }
        for route in self.routes.iter().filter(|_| routes_changed) {
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
    /// The authoritative semantic values used to produce `universes`. Keeping this immutable
    /// snapshot with the render result lets observational consumers follow output without
    /// resolving the engine a second time.
    pub resolved_values: Arc<ResolvedValues>,
    /// Winning LTP timestamps for edge-sensitive internal services.
    pub resolved_changed_at: Arc<ResolvedChangedAt>,
    /// Profile-head values resolved while producing the same output frame.
    pub profile_visualization_values: Arc<ResolvedValues>,
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
    pub current_cue_number: CueNumber,
    pub target_cue_id: uuid::Uuid,
    pub target_cue_number: CueNumber,
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
