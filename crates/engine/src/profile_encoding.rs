use crate::{EngineError, EngineSnapshot, fixture::profile_mode};
use light_core::FixtureId;
use light_fixture::FixtureModeEncodingPlan;
use std::collections::HashMap;

/// Physical DMX layouts compiled alongside the immutable engine generation.
#[derive(Debug, Default)]
pub(crate) struct ProfileEncodingIndex {
    fixtures: HashMap<FixtureId, FixtureModeEncodingPlan>,
}

impl ProfileEncodingIndex {
    pub(crate) fn compile(snapshot: &EngineSnapshot) -> Result<Self, EngineError> {
        let mut fixtures = HashMap::new();
        for fixture in &snapshot.fixtures {
            let Some(mode) = profile_mode(fixture) else {
                continue;
            };
            let plan = mode
                .compile_encoding_plan()
                .map_err(|error| EngineError::Invalid(error.to_string()))?;
            fixtures.insert(fixture.fixture_id, plan);
        }
        Ok(Self { fixtures })
    }

    pub(crate) fn fixture(&self, fixture_id: FixtureId) -> Option<&FixtureModeEncodingPlan> {
        self.fixtures.get(&fixture_id)
    }
}
