use crate::{EngineSnapshot, ProfileEncodingIndex, ProfileProjectionIndex, profile_head_owner};
use light_core::{AttributeKey, FixtureId};
use light_output::OutputRoute;
use light_playback::{PlaybackEngine, PlaybackTarget};
use light_programmer::{GroupDefinition, resolve_group, resolve_group_spatial};
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
    group_rankings: Arc<HashMap<String, light_dynamics::RankedSelection>>,
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
        playback: Arc<RwLock<PlaybackEngine>>,
        groups: Arc<HashMap<String, GroupDefinition>>,
        profile_encodings: Arc<ProfileEncodingIndex>,
        profile_projections: Arc<ProfileProjectionIndex>,
    ) -> Self {
        let routes = Arc::from(snapshot.routes.as_slice());
        let snap_attributes = compile_snap_attributes(&snapshot);
        let group_rankings = compile_group_rankings(&groups, &snapshot);
        let group_masters = GroupMasterIndex::compile(&groups, &snapshot, None);
        Self {
            snapshot: Arc::new(snapshot),
            playback,
            groups,
            routes,
            snap_attributes: Arc::new(snap_attributes),
            group_rankings: Arc::new(group_rankings),
            group_masters: Arc::new(group_masters),
            profile_encodings,
            profile_projections,
        }
    }

    pub(crate) fn replacing(
        current: &Arc<Self>,
        snapshot: EngineSnapshot,
        playback: Arc<RwLock<PlaybackEngine>>,
        groups: Arc<HashMap<String, GroupDefinition>>,
        profile_encodings: Arc<ProfileEncodingIndex>,
        profile_projections: Arc<ProfileProjectionIndex>,
        preserve_group_master_levels: bool,
    ) -> Self {
        let fixtures_changed = !Arc::ptr_eq(&snapshot.fixtures, &current.snapshot.fixtures);
        let playbacks_changed = !Arc::ptr_eq(&snapshot.playbacks, &current.snapshot.playbacks);
        let playback_pages_changed =
            !Arc::ptr_eq(&snapshot.playback_pages, &current.snapshot.playback_pages);
        let groups_changed = !Arc::ptr_eq(&snapshot.groups, &current.snapshot.groups);
        let stage_positions_changed = !Arc::ptr_eq(
            &snapshot.dynamic_stage_positions,
            &current.snapshot.dynamic_stage_positions,
        );
        let routes = if Arc::ptr_eq(&snapshot.routes, &current.snapshot.routes) {
            Arc::clone(&current.routes)
        } else {
            Arc::from(snapshot.routes.as_slice())
        };
        let snap_attributes = if fixtures_changed {
            Arc::new(compile_snap_attributes(&snapshot))
        } else {
            Arc::clone(&current.snap_attributes)
        };
        let group_masters = if !preserve_group_master_levels
            || playbacks_changed
            || playback_pages_changed
            || groups_changed
        {
            Arc::new(GroupMasterIndex::compile(
                &groups,
                &snapshot,
                preserve_group_master_levels.then_some(current.group_masters.as_ref()),
            ))
        } else {
            Arc::clone(&current.group_masters)
        };
        let group_rankings = if groups_changed || stage_positions_changed {
            Arc::new(compile_group_rankings(&groups, &snapshot))
        } else {
            Arc::clone(&current.group_rankings)
        };
        Self {
            snapshot: Arc::new(snapshot),
            playback,
            groups,
            routes,
            snap_attributes,
            group_rankings,
            group_masters,
            profile_encodings,
            profile_projections,
        }
    }

    pub(crate) fn with_group_master(
        current: &Arc<Self>,
        group_id: &str,
        value: f32,
    ) -> (Arc<Self>, GroupMasterGenerationUpdate) {
        let Some(group_masters) = current.group_masters.with_master(group_id, value) else {
            return (Arc::clone(current), GroupMasterGenerationUpdate::Missing);
        };
        if current.group_masters.master(group_id) == Some(value) {
            return (Arc::clone(current), GroupMasterGenerationUpdate::Unchanged);
        }
        (
            Arc::new(Self {
                snapshot: Arc::clone(&current.snapshot),
                playback: Arc::clone(&current.playback),
                groups: Arc::clone(&current.groups),
                routes: Arc::clone(&current.routes),
                snap_attributes: Arc::clone(&current.snap_attributes),
                group_rankings: Arc::clone(&current.group_rankings),
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

    pub(crate) fn playback_arc(&self) -> Arc<RwLock<PlaybackEngine>> {
        Arc::clone(&self.playback)
    }

    pub(crate) fn groups(&self) -> &HashMap<String, GroupDefinition> {
        &self.groups
    }

    pub(crate) fn groups_arc(&self) -> Arc<HashMap<String, GroupDefinition>> {
        Arc::clone(&self.groups)
    }

    pub(crate) fn profile_encodings_arc(&self) -> Arc<ProfileEncodingIndex> {
        Arc::clone(&self.profile_encodings)
    }

    pub(crate) fn profile_projections_arc(&self) -> Arc<ProfileProjectionIndex> {
        Arc::clone(&self.profile_projections)
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

    pub(crate) fn group_ranking(&self, group_id: &str) -> Option<&light_dynamics::RankedSelection> {
        self.group_rankings.get(group_id)
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

fn compile_group_rankings(
    groups: &HashMap<String, GroupDefinition>,
    snapshot: &EngineSnapshot,
) -> HashMap<String, light_dynamics::RankedSelection> {
    let positions = group_stage_positions(snapshot);
    groups
        .keys()
        .map(|group_id| {
            let resolved = resolve_group_spatial(group_id, groups, &positions)
                .expect("validated Group spatial mapping must resolve");
            (group_id.clone(), resolved.ranked_selection)
        })
        .collect()
}

pub(crate) fn group_stage_positions(
    snapshot: &EngineSnapshot,
) -> HashMap<FixtureId, light_dynamics::Position3d> {
    snapshot
        .dynamic_stage_positions
        .iter()
        .map(|(fixture_id, position)| {
            (
                *fixture_id,
                light_dynamics::Position3d {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                    z: f64::from(position.z),
                },
            )
        })
        .collect()
}

#[derive(Clone, Default)]
pub(crate) struct GroupMasterIndex {
    masters: Vec<GroupMasterBinding>,
    fixtures: HashMap<FixtureId, Vec<usize>>,
}

#[derive(Clone)]
struct GroupMasterBinding {
    group_id: String,
    master: f32,
}

impl GroupMasterIndex {
    fn compile(
        groups: &HashMap<String, GroupDefinition>,
        snapshot: &EngineSnapshot,
        preserved: Option<&Self>,
    ) -> Self {
        let initial_masters = initial_group_masters(snapshot);
        let assigned_groups = snapshot
            .playbacks
            .iter()
            .chain(
                snapshot
                    .playback_pages
                    .iter()
                    .flat_map(|page| page.virtual_playbacks.values()),
            )
            .filter_map(|playback| match &playback.target {
                PlaybackTarget::Group { group_id, .. } => Some(group_id.as_str()),
                _ => None,
            })
            .collect::<HashSet<_>>();
        let mut definitions = groups
            .values()
            .filter(|group| assigned_groups.contains(group.id.as_str()))
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
                master: preserved
                    .and_then(|current| current.master(&definition.id))
                    .or_else(|| initial_masters.get(&definition.id).copied())
                    .unwrap_or(1.0),
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

    pub(crate) fn master(&self, group_id: &str) -> Option<f32> {
        self.masters
            .iter()
            .find(|binding| binding.group_id == group_id)
            .map(|binding| binding.master)
    }

    fn with_master(&self, group_id: &str, value: f32) -> Option<Self> {
        let mut updated = self.clone();
        updated
            .masters
            .iter_mut()
            .find(|binding| binding.group_id == group_id)?
            .master = value;
        Some(updated)
    }
}

/// Resolve portable Group Master seeds by stable Playback address. Physical assignments precede
/// virtual assignments; each class is ordered by its operator-visible address. Migration writes
/// one reconciled value to every assignment, but retaining the rule here makes direct snapshots
/// deterministic and keeps malformed divergent input from becoming iteration-order dependent.
fn initial_group_masters(snapshot: &EngineSnapshot) -> HashMap<String, f32> {
    let mut assignments = snapshot
        .playbacks
        .iter()
        .filter_map(|playback| match &playback.target {
            PlaybackTarget::Group {
                group_id,
                initial_master: Some(master),
            } => Some(((0_u8, 0_u8, playback.number), group_id.clone(), *master)),
            _ => None,
        })
        .chain(snapshot.playback_pages.iter().flat_map(|page| {
            page.virtual_playbacks
                .values()
                .filter_map(move |playback| match &playback.target {
                    PlaybackTarget::Group {
                        group_id,
                        initial_master: Some(master),
                    } => Some((
                        (1_u8, page.number, playback.number),
                        group_id.clone(),
                        *master,
                    )),
                    _ => None,
                })
        }))
        .collect::<Vec<_>>();
    assignments.sort_by_key(|(address, _, _)| *address);
    let mut levels = HashMap::new();
    for (_, group_id, master) in assignments {
        levels.entry(group_id).or_insert(master);
    }
    levels
}

fn compile_snap_attributes(snapshot: &EngineSnapshot) -> HashMap<FixtureId, HashSet<AttributeKey>> {
    let mut attributes = HashMap::<FixtureId, HashSet<AttributeKey>>::new();
    for fixture in snapshot.fixtures.iter() {
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
