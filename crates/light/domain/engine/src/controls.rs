use crate::engine::GroupMasterTransition;
use crate::{Engine, EngineError, GroupMasterGenerationUpdate, RuntimeGeneration};
use light_core::FixtureId;
use std::cell::Cell;

impl Engine {
    /// Updates one output-runtime Group master without rebuilding Playback or refreshing live
    /// Programmer selections. Group membership and every other immutable generation component
    /// remain unchanged.
    pub fn set_group_master(&self, group_id: &str, value: f32) -> Result<bool, EngineError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(EngineError::Invalid(
                "group master must be within 0-1".into(),
            ));
        }
        let outcome = Cell::new(GroupMasterGenerationUpdate::Missing);
        self.generation.rcu(|current| {
            let (generation, update) =
                RuntimeGeneration::with_group_master(current, group_id, value);
            outcome.set(update);
            generation
        });
        match outcome.get() {
            GroupMasterGenerationUpdate::Missing => Err(EngineError::Invalid(format!(
                "group {group_id} does not exist"
            ))),
            GroupMasterGenerationUpdate::Unchanged => Ok(false),
            GroupMasterGenerationUpdate::Changed => Ok(true),
        }
    }

    pub fn set_group_master_transition(
        &self,
        group_id: &str,
        value: f32,
        duration_millis: u64,
    ) -> Result<bool, EngineError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) || duration_millis > 60_000 {
            return Err(EngineError::Invalid(
                "group master transition must use level 0-1 and duration 0-60000 milliseconds"
                    .into(),
            ));
        }
        let current = self
            .group_master(group_id)
            .ok_or_else(|| EngineError::Invalid(format!("group {group_id} does not exist")))?;
        if duration_millis == 0 {
            self.group_master_transitions.lock().remove(group_id);
            return self.set_group_master(group_id, value);
        }
        let transition = GroupMasterTransition {
            from: current,
            to: value,
            started_at: self.clock.now(),
            duration_millis,
        };
        self.group_master_transitions
            .lock()
            .insert(group_id.to_owned(), transition);
        Ok(current != value)
    }

    pub(crate) fn advance_group_master_transitions(&self) {
        let now = self.clock.now();
        let updates = {
            let mut transitions = self.group_master_transitions.lock();
            let mut updates = Vec::with_capacity(transitions.len());
            transitions.retain(|group_id, transition| {
                let elapsed = (now - transition.started_at).num_milliseconds().max(0) as f32;
                let progress = (elapsed / transition.duration_millis.max(1) as f32).clamp(0.0, 1.0);
                updates.push((
                    group_id.clone(),
                    transition.from + (transition.to - transition.from) * progress,
                ));
                progress < 1.0
            });
            updates
        };
        for (group_id, value) in updates {
            let _ = self.set_group_master(&group_id, value);
        }
    }

    /// Sets a transient group flash level without changing the group's fader value.
    pub fn set_group_master_flash(&self, group_id: String, value: f32) {
        let mut flashes = self.group_master_flashes.write();
        if value <= 0.0 {
            flashes.remove(&group_id);
        } else {
            flashes.insert(group_id, value.clamp(0.0, 1.0));
        }
    }

    pub fn group_master_flash(&self, group_id: &str) -> f32 {
        self.group_master_flashes
            .read()
            .get(group_id)
            .copied()
            .unwrap_or(0.0)
    }

    pub fn group_master(&self, group_id: &str) -> Option<f32> {
        self.generation.load().group_masters().master(group_id)
    }

    /// Desired durable value. During an engine-owned transition the persisted target remains the
    /// final level rather than whichever intermediate sample happened to be rendered.
    pub fn group_master_for_persistence(&self, group_id: &str) -> Option<f32> {
        self.group_master_transitions
            .lock()
            .get(group_id)
            .map(|transition| transition.to)
            .or_else(|| self.group_master(group_id))
    }

    /// Replace the transient Highlight output set. This deliberately does not touch programmer
    /// state, undo history, or the persisted engine snapshot.
    pub fn set_highlighted_fixtures(&self, fixtures: impl IntoIterator<Item = FixtureId>) {
        *self.highlighted_fixtures.write() = fixtures.into_iter().collect();
    }

    pub fn clear_highlighted_fixtures(&self) {
        self.highlighted_fixtures.write().clear();
    }

    pub fn highlighted_fixtures(&self) -> Vec<FixtureId> {
        let mut fixtures = self
            .highlighted_fixtures
            .read()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        fixtures.sort_by_key(|fixture| fixture.0);
        fixtures
    }
}
