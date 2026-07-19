use crate::{EngineError, EngineSnapshot, fixture::profile_mode, profile_head_owner};
use light_core::FixtureId;
use light_fixture::{FixtureMode, FixtureModeResolutionPlan, PatchedFixture};
use std::collections::HashMap;
use uuid::Uuid;

/// Immutable semantic projection metadata compiled with an engine generation.
#[derive(Debug, Default)]
pub(crate) struct ProfileProjectionIndex {
    fixtures: HashMap<FixtureId, FixtureProjectionPlan>,
}

#[derive(Debug)]
pub(crate) struct FixtureProjectionPlan {
    resolution: FixtureModeResolutionPlan,
    heads: Box<[ProfileHeadPlan]>,
}

#[derive(Debug)]
pub(crate) struct ProfileHeadPlan {
    pub(crate) owner: FixtureId,
    pub(crate) head_id: Uuid,
    pub(crate) channel_indices: Box<[usize]>,
    pub(crate) intensity_channel_indices: Box<[usize]>,
    splits: Box<[u16]>,
}

impl ProfileProjectionIndex {
    pub(crate) fn compile(snapshot: &EngineSnapshot) -> Result<Self, EngineError> {
        let mut fixtures = HashMap::new();
        for fixture in &snapshot.fixtures {
            let Some(mode) = profile_mode(fixture) else {
                continue;
            };
            fixtures.insert(
                fixture.fixture_id,
                FixtureProjectionPlan::compile(fixture, mode)?,
            );
        }
        Ok(Self { fixtures })
    }

    pub(crate) fn fixture(&self, fixture_id: FixtureId) -> Option<&FixtureProjectionPlan> {
        self.fixtures.get(&fixture_id)
    }
}

impl FixtureProjectionPlan {
    fn compile(fixture: &PatchedFixture, mode: &FixtureMode) -> Result<Self, EngineError> {
        let mut channels = vec![Vec::new(); mode.heads.len()];
        let mut intensity_channels = vec![Vec::new(); mode.heads.len()];
        let mut splits = vec![Vec::new(); mode.heads.len()];
        let head_indices = mode
            .heads
            .iter()
            .enumerate()
            .map(|(index, head)| (head.id, index))
            .collect::<HashMap<_, _>>();
        for (channel_index, channel) in mode.channels.iter().enumerate() {
            let head_index = head_indices.get(&channel.head_id).copied().ok_or_else(|| {
                EngineError::Invalid("profile channel references a missing head".into())
            })?;
            channels[head_index].push(channel_index);
            if channel.attribute.is_intensity() {
                intensity_channels[head_index].push(channel_index);
            }
            if !splits[head_index].contains(&channel.split) {
                splits[head_index].push(channel.split);
            }
        }
        let heads = mode
            .heads
            .iter()
            .enumerate()
            .map(|(head_index, head)| ProfileHeadPlan {
                owner: profile_head_owner(fixture, head_index, head),
                head_id: head.id,
                channel_indices: std::mem::take(&mut channels[head_index]).into_boxed_slice(),
                intensity_channel_indices: std::mem::take(&mut intensity_channels[head_index])
                    .into_boxed_slice(),
                splits: std::mem::take(&mut splits[head_index]).into_boxed_slice(),
            })
            .collect();
        Ok(Self {
            resolution: mode.compile_resolution_plan(),
            heads,
        })
    }

    pub(crate) fn heads(&self) -> &[ProfileHeadPlan] {
        &self.heads
    }

    pub(crate) fn resolution(&self) -> &FixtureModeResolutionPlan {
        &self.resolution
    }
}

impl ProfileHeadPlan {
    pub(crate) fn appears_in_any_split(&self, splits: &[u16]) -> bool {
        self.splits
            .iter()
            .any(|split| splits.iter().any(|candidate| candidate == split))
    }
}
