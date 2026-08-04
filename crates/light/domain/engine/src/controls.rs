use crate::engine::GroupMasterTransition;
use crate::{Engine, EngineError, GroupMasterGenerationUpdate, RuntimeGeneration};
use light_core::FixtureId;
use light_fixture::{ChannelFunctionBehavior, HighlightLook};
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
        let changed = match outcome.get() {
            GroupMasterGenerationUpdate::Missing => Err(EngineError::Invalid(format!(
                "group {group_id} does not exist"
            ))),
            GroupMasterGenerationUpdate::Unchanged => Ok(false),
            GroupMasterGenerationUpdate::Changed => Ok(true),
        }?;
        self.generation
            .load()
            .playback()
            .write()
            .retarget_group_physical_controls(group_id, value, None);
        Ok(changed)
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

    /// Replace the installation-owned semantic Highlight look without touching show or
    /// programmer state.
    pub fn set_highlight_look(&self, look: HighlightLook) -> Result<(), EngineError> {
        look.validate()
            .map_err(|error| EngineError::Invalid(error.to_string()))?;
        *self.highlight_look.write() = look;
        Ok(())
    }

    pub fn highlight_look(&self) -> HighlightLook {
        self.highlight_look.read().clone()
    }

    /// Fixture-authoring feedback for the current semantic Highlight configuration. This is a
    /// read-only projection and never changes show or runtime state.
    pub fn highlight_look_warnings(&self, look: &HighlightLook) -> Vec<String> {
        if look.compatibility != light_fixture::HighlightLookCompatibility::Semantic {
            return Vec::new();
        }
        let snapshot = self.generation.load();
        snapshot
            .snapshot()
            .fixtures
            .iter()
            .filter_map(|fixture| {
                let profile = fixture.definition.profile_snapshot.as_deref()?;
                let mode = profile.mode(fixture.definition.mode_id?)?;
                let has_attribute = |name: &str| {
                    mode.channels.iter().any(|channel| {
                        channel.attribute.0.eq_ignore_ascii_case(name)
                            || channel
                                .functions
                                .iter()
                                .any(|function| function.attribute.0.eq_ignore_ascii_case(name))
                    })
                };
                let mut issues = Vec::new();
                if has_attribute("shutter")
                    && !mode.channels.iter().any(|channel| {
                        channel.functions.iter().any(|function| {
                            function.attribute.0.eq_ignore_ascii_case("shutter")
                                && matches!(
                                    &function.behavior,
                                    ChannelFunctionBehavior::Fixed { semantic_id, .. }
                                        | ChannelFunctionBehavior::Indexed { semantic_id, .. }
                                        if semantic_id.eq_ignore_ascii_case("open")
                                )
                        })
                    })
                {
                    issues.push("Shutter Open is not authored");
                }
                if let Some(color) = look.color
                    && !mode.heads.iter().any(|head| {
                        mode.resolve_highlight_color(head.id, color)
                            .is_ok_and(|values| !values.is_empty())
                    })
                {
                    issues.push("Color is unavailable");
                }
                for (label, configured) in [
                    ("Iris", look.iris.is_some()),
                    ("Zoom", look.zoom.is_some()),
                    ("Focus", look.focus.is_some()),
                    ("Frost", look.frost.is_some()),
                ] {
                    if configured && !has_attribute(&label.to_ascii_lowercase()) {
                        issues.push(match label {
                            "Iris" => "Iris is unavailable",
                            "Zoom" => "Zoom is unavailable",
                            "Focus" => "Focus is unavailable",
                            _ => "Frost is unavailable",
                        });
                    }
                }
                (!issues.is_empty()).then(|| {
                    let identity = fixture
                        .fixture_number
                        .map(|number| format!("Fixture {number}"))
                        .unwrap_or_else(|| format!("Fixture {}", fixture.fixture_id.0));
                    format!("{identity} {}: {}", fixture.name, issues.join("; "))
                })
            })
            .collect()
    }
}
