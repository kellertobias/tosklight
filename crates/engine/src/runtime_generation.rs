use crate::{EngineSnapshot, ProfileEncodingIndex, ProfileProjectionIndex, profile_head_owner};
use light_core::{AttributeKey, FixtureId};
use light_output::OutputRoute;
use light_playback::PlaybackEngine;
use light_programmer::{GroupDefinition, resolve_group};
use parking_lot::RwLock;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// One internally coherent engine generation.
///
/// A render retains this value for its complete lifetime, so fixture projection, Playback state,
/// Group resolution, and output routing cannot be mixed across show revisions while a new show is
/// installed concurrently.
pub(crate) struct RuntimeGeneration {
    snapshot: Arc<EngineSnapshot>,
    playback: Arc<RwLock<PlaybackEngine>>,
    groups: Arc<HashMap<String, GroupDefinition>>,
    routes: Arc<[OutputRoute]>,
    snap_attributes: Arc<HashMap<FixtureId, HashSet<AttributeKey>>>,
    group_masters: Arc<GroupMasterIndex>,
    profile_encodings: Arc<ProfileEncodingIndex>,
    profile_projections: Arc<ProfileProjectionIndex>,
}

#[derive(Clone, Copy)]
pub(crate) enum GroupMasterGenerationUpdate {
    Missing,
    Unchanged,
    Changed,
}

impl RuntimeGeneration {
    pub(crate) fn new(
        snapshot: EngineSnapshot,
        playback: PlaybackEngine,
        groups: HashMap<String, GroupDefinition>,
        profile_encodings: ProfileEncodingIndex,
        profile_projections: ProfileProjectionIndex,
    ) -> Self {
        let routes = Arc::from(snapshot.routes.clone());
        let snap_attributes = compile_snap_attributes(&snapshot);
        let group_masters = GroupMasterIndex::compile(&groups);
        Self {
            snapshot: Arc::new(snapshot),
            playback: Arc::new(RwLock::new(playback)),
            groups: Arc::new(groups),
            routes,
            snap_attributes: Arc::new(snap_attributes),
            group_masters: Arc::new(group_masters),
            profile_encodings: Arc::new(profile_encodings),
            profile_projections: Arc::new(profile_projections),
        }
    }

    pub(crate) fn with_group_master(
        current: &Arc<Self>,
        group_id: &str,
        value: f32,
    ) -> (Arc<Self>, GroupMasterGenerationUpdate) {
        let Some(group) = current.groups.get(group_id) else {
            return (Arc::clone(current), GroupMasterGenerationUpdate::Missing);
        };
        if group.master == value {
            return (Arc::clone(current), GroupMasterGenerationUpdate::Unchanged);
        }

        let mut snapshot = (*current.snapshot).clone();
        snapshot
            .groups
            .iter_mut()
            .find(|group| group.id == group_id)
            .expect("runtime groups and snapshot groups must stay aligned")
            .master = value;
        let mut groups = (*current.groups).clone();
        groups
            .get_mut(group_id)
            .expect("runtime groups and snapshot groups must stay aligned")
            .master = value;
        let group_masters = GroupMasterIndex::compile(&groups);
        (
            Arc::new(Self {
                snapshot: Arc::new(snapshot),
                playback: Arc::clone(&current.playback),
                groups: Arc::new(groups),
                routes: Arc::clone(&current.routes),
                snap_attributes: Arc::clone(&current.snap_attributes),
                group_masters: Arc::new(group_masters),
                profile_encodings: Arc::clone(&current.profile_encodings),
                profile_projections: Arc::clone(&current.profile_projections),
            }),
            GroupMasterGenerationUpdate::Changed,
        )
    }

    pub(crate) fn snapshot(&self) -> &EngineSnapshot {
        &self.snapshot
    }

    pub(crate) fn snapshot_arc(&self) -> Arc<EngineSnapshot> {
        Arc::clone(&self.snapshot)
    }

    pub(crate) fn playback(&self) -> &RwLock<PlaybackEngine> {
        &self.playback
    }

    pub(crate) fn groups(&self) -> &HashMap<String, GroupDefinition> {
        &self.groups
    }

    pub(crate) fn routes(&self) -> Arc<[OutputRoute]> {
        Arc::clone(&self.routes)
    }

    pub(crate) fn attribute_is_snap(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> bool {
        self.snap_attributes
            .get(&fixture_id)
            .is_some_and(|attributes| attributes.contains(attribute))
    }

    pub(crate) fn group_masters(&self) -> &GroupMasterIndex {
        &self.group_masters
    }

    pub(crate) fn profile_encoding(
        &self,
        fixture_id: FixtureId,
    ) -> Option<&light_fixture::FixtureModeEncodingPlan> {
        self.profile_encodings.fixture(fixture_id)
    }

    pub(crate) fn profile_projection(
        &self,
        fixture_id: FixtureId,
    ) -> Option<&crate::FixtureProjectionPlan> {
        self.profile_projections.fixture(fixture_id)
    }
}

#[derive(Default)]
pub(crate) struct GroupMasterIndex {
    masters: Vec<GroupMasterBinding>,
    fixtures: HashMap<FixtureId, Vec<usize>>,
}

struct GroupMasterBinding {
    group_id: String,
    master: f32,
}

impl GroupMasterIndex {
    fn compile(groups: &HashMap<String, GroupDefinition>) -> Self {
        let mut definitions = groups
            .values()
            .filter(|group| group.playback_fader.is_some())
            .collect::<Vec<_>>();
        definitions.sort_by(|left, right| left.id.cmp(&right.id));
        let mut index = Self::default();
        for definition in definitions {
            let Ok(fixtures) = resolve_group(&definition.id, groups) else {
                continue;
            };
            let master_index = index.masters.len();
            index.masters.push(GroupMasterBinding {
                group_id: definition.id.clone(),
                master: definition.master,
            });
            for fixture_id in fixtures {
                index
                    .fixtures
                    .entry(fixture_id)
                    .or_default()
                    .push(master_index);
            }
        }
        index
    }

    pub(crate) fn scale(&self, fixture_id: FixtureId, flashes: &HashMap<String, f32>) -> f32 {
        self.fixtures
            .get(&fixture_id)
            .into_iter()
            .flatten()
            .map(|index| &self.masters[*index])
            .map(|binding| {
                binding
                    .master
                    .max(flashes.get(&binding.group_id).copied().unwrap_or(0.0))
                    .clamp(0.0, 1.0)
            })
            .reduce(f32::max)
            .unwrap_or(1.0)
    }
}

fn compile_snap_attributes(snapshot: &EngineSnapshot) -> HashMap<FixtureId, HashSet<AttributeKey>> {
    let mut attributes = HashMap::<FixtureId, HashSet<AttributeKey>>::new();
    for fixture in &snapshot.fixtures {
        let Some(mode) = crate::fixture::profile_mode(fixture) else {
            continue;
        };
        for (head_index, head) in mode.heads.iter().enumerate() {
            let owner = profile_head_owner(fixture, head_index, head);
            for channel in mode
                .channels
                .iter()
                .filter(|channel| channel.head_id == head.id && channel.snap)
            {
                let head_attributes = attributes.entry(owner).or_default();
                head_attributes.insert(channel.attribute.clone());
                head_attributes.extend(
                    channel
                        .functions
                        .iter()
                        .map(|function| function.attribute.clone()),
                );
            }
        }
    }
    attributes
}
