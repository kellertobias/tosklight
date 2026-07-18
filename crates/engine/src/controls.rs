use crate::Engine;
use light_core::FixtureId;

impl Engine {
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
