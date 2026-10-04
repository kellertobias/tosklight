use crate::{EngineSnapshot, ProfileEncodingIndex, ProfileProjectionIndex, profile_head_owner};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_output::OutputRoute;
use light_playback::{PlaybackEngine, PlaybackTarget};
use light_programmer::{GroupDefinition, resolve_group, resolve_group_spatial};
use parking_lot::RwLock;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// An output generation changes even when its dense slot numbering remains valid.
static NEXT_RUNTIME_GENERATION: AtomicU64 = AtomicU64::new(1);

fn next_runtime_generation() -> u64 {
    NEXT_RUNTIME_GENERATION.fetch_add(1, Ordering::Relaxed)
}

/// One internally coherent engine generation.
///
/// Every construction and replacement receives a fresh `identity`, including a same-show reload
/// or a calibration/rebind edit that keeps slot numbering. [`crate::CapturedFrameToken`] carries
/// this identity so physical destination descriptors compiled for one generation cannot be
/// used with a capture of another.
///
/// A render retains this value for its complete lifetime, so fixture projection, Playback state,
/// Group resolution, and output routing cannot be mixed across show revisions while a new show is
/// installed concurrently.
pub(crate) struct RuntimeGeneration {
    identity: u64,
    snapshot: Arc<EngineSnapshot>,
    playback: Arc<RwLock<PlaybackEngine>>,
    groups: Arc<HashMap<String, GroupDefinition>>,
    routes: Arc<[OutputRoute]>,
    snap_attributes: Arc<HashMap<FixtureId, HashSet<AttributeKey>>>,
    default_values: Arc<crate::ResolvedValues>,
    group_rankings: Arc<HashMap<String, light_dynamics::RankedSelection>>,
    group_masters: Arc<GroupMasterIndex>,
    profile_encodings: Arc<ProfileEncodingIndex>,
    profile_projections: Arc<ProfileProjectionIndex>,
    /// The fixed shape of every frame this generation renders.
    slots: Arc<crate::SlotTable>,
    /// Where this generation's frame buffers wait between frames.
    frames: Arc<crate::FramePool>,
    /// Where each channel of each head reads its attributes from.
    channel_slots: Arc<crate::ChannelSlotIndex>,
    /// Every Group's programming, as the slots it lands in.
    group_plan: Arc<crate::group_plan::GroupContributionPlan>,
    point_projection: Arc<crate::point_projection::PointProjectionIndex>,
    mount_projection: Arc<crate::mount_projection::MountProjectionIndex>,
    /// Declared default poses, decoded lazily against exactly this generation's projections. The
    /// Playback engine starts a Position that fades in from nothing here (TL-552).
    declared_positions: Arc<crate::position_adoption::DeclaredPositions>,
    /// Each fixture's position in its profile's modes, found on first use (TL-639 round 4).
    /// `crate::fixture::profile_mode` searches the modes by id on every call.
    mode_indices: Arc<std::sync::OnceLock<Box<[Option<u32>]>>>,
}

/// Where a replacement generation's Group Master levels come from.
pub(crate) enum GroupMasterLevels {
    /// Portable seeds only (Release policy).
    Released,
    /// The current generation's levels for surviving Group Master bindings.
    Preserved,
    /// An index already compiled against the replacement snapshot and Groups, adopted verbatim.
    Prepared(Arc<GroupMasterIndex>),
}

#[derive(Clone, Copy)]
pub(crate) enum GroupMasterGenerationUpdate {
    Missing,
    Unchanged,
    Changed,
}

impl RuntimeGeneration {
    pub(crate) fn physical_projection(
        &self,
    ) -> &crate::physical_projection::PhysicalProjectionIndex {
        &self.profile_projections.physical
    }

    pub(crate) fn new(
        snapshot: EngineSnapshot,
        playback: Arc<RwLock<PlaybackEngine>>,
        groups: Arc<HashMap<String, GroupDefinition>>,
        profile_encodings: Arc<ProfileEncodingIndex>,
        profile_projections: Arc<ProfileProjectionIndex>,
    ) -> Self {
        let routes = Arc::from(snapshot.routes.as_slice());
        let snap_attributes = compile_snap_attributes(&snapshot);
        let default_values = compile_default_values(&snapshot);
        let group_rankings = compile_group_rankings(&groups, &snapshot);
        let group_masters = GroupMasterIndex::compile(&groups, &snapshot, None);
        let slots = Arc::new(crate::SlotTable::compile(
            crate::next_generation(),
            &snapshot.fixtures,
        ));
        let frames = Arc::new(crate::FramePool::for_generation(
            slots.generation(),
            slots.len(),
        ));
        let channel_slots = Arc::new(crate::ChannelSlotIndex::compile(&snapshot.fixtures, &slots));
        let group_plan = Arc::new(crate::group_plan::GroupContributionPlan::compile(
            &snapshot.groups,
            &group_rankings,
            &slots,
        ));
        let point_projection = Arc::new(crate::point_projection::PointProjectionIndex::compile(
            &snapshot.fixtures,
            &slots,
        ));
        let mount_projection = Arc::new(crate::mount_projection::MountProjectionIndex::compile(
            &snapshot.fixtures,
            &point_projection,
        ));
        let snapshot = Arc::new(snapshot);
        let declared_positions =
            install_playback_frame(&playback, &slots, &snapshot, &profile_projections);
        Self {
            identity: next_runtime_generation(),
            snapshot,
            playback,
            groups,
            routes,
            snap_attributes: Arc::new(snap_attributes),
            default_values: Arc::new(default_values),
            group_rankings: Arc::new(group_rankings),
            group_masters: Arc::new(group_masters),
            profile_encodings,
            profile_projections,
            slots,
            frames,
            channel_slots,
            group_plan,
            point_projection,
            mount_projection,
            declared_positions,
            mode_indices: Arc::default(),
        }
    }

    pub(crate) fn replacing(
        current: &Arc<Self>,
        snapshot: Arc<EngineSnapshot>,
        playback: Arc<RwLock<PlaybackEngine>>,
        groups: Arc<HashMap<String, GroupDefinition>>,
        profile_encodings: Arc<ProfileEncodingIndex>,
        profile_projections: Arc<ProfileProjectionIndex>,
        group_master_levels: GroupMasterLevels,
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
        let default_values = if fixtures_changed {
            Arc::new(compile_default_values(&snapshot))
        } else {
            Arc::clone(&current.default_values)
        };
        // A Release-policy destination may carry a Group Master index already compiled against
        // this exact snapshot and Group table, with persisted levels applied before install.
        let group_masters = match group_master_levels {
            GroupMasterLevels::Prepared(prepared) => prepared,
            GroupMasterLevels::Released => {
                Arc::new(GroupMasterIndex::compile(&groups, &snapshot, None))
            }
            GroupMasterLevels::Preserved
                if playbacks_changed || playback_pages_changed || groups_changed =>
            {
                Arc::new(GroupMasterIndex::compile(
                    &groups,
                    &snapshot,
                    Some(current.group_masters.as_ref()),
                ))
            }
            GroupMasterLevels::Preserved => Arc::clone(&current.group_masters),
        };
        let (slots, frames, channel_slots) = if fixtures_changed {
            let slots = Arc::new(crate::SlotTable::compile(
                crate::next_generation(),
                &snapshot.fixtures,
            ));
            let frames = Arc::new(crate::FramePool::for_generation(
                slots.generation(),
                slots.len(),
            ));
            let channel_slots =
                Arc::new(crate::ChannelSlotIndex::compile(&snapshot.fixtures, &slots));
            (slots, frames, channel_slots)
        } else {
            (
                Arc::clone(&current.slots),
                Arc::clone(&current.frames),
                Arc::clone(&current.channel_slots),
            )
        };
        let group_rankings = if groups_changed || stage_positions_changed {
            Arc::new(compile_group_rankings(&groups, &snapshot))
        } else {
            Arc::clone(&current.group_rankings)
        };
        let group_plan = if groups_changed || fixtures_changed || stage_positions_changed {
            Arc::new(crate::group_plan::GroupContributionPlan::compile(
                &snapshot.groups,
                &group_rankings,
                &slots,
            ))
        } else {
            Arc::clone(&current.group_plan)
        };
        let point_projection = if fixtures_changed {
            Arc::new(crate::point_projection::PointProjectionIndex::compile(
                &snapshot.fixtures,
                &slots,
            ))
        } else {
            Arc::clone(&current.point_projection)
        };
        let mount_projection = if fixtures_changed {
            Arc::new(crate::mount_projection::MountProjectionIndex::compile(
                &snapshot.fixtures,
                &point_projection,
            ))
        } else {
            Arc::clone(&current.mount_projection)
        };
        let declared_positions =
            install_playback_frame(&playback, &slots, &snapshot, &profile_projections);
        Self {
            identity: next_runtime_generation(),
            snapshot,
            playback,
            groups,
            routes,
            snap_attributes,
            default_values,
            group_rankings,
            group_masters,
            profile_encodings,
            profile_projections,
            slots,
            frames,
            channel_slots,
            group_plan,
            point_projection,
            mount_projection,
            declared_positions,
            mode_indices: Arc::default(),
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
                identity: next_runtime_generation(),
                snapshot: Arc::clone(&current.snapshot),
                playback: Arc::clone(&current.playback),
                groups: Arc::clone(&current.groups),
                routes: Arc::clone(&current.routes),
                snap_attributes: Arc::clone(&current.snap_attributes),
                default_values: Arc::clone(&current.default_values),
                group_rankings: Arc::clone(&current.group_rankings),
                group_masters: Arc::new(group_masters),
                profile_encodings: Arc::clone(&current.profile_encodings),
                profile_projections: Arc::clone(&current.profile_projections),
                slots: Arc::clone(&current.slots),
                frames: Arc::clone(&current.frames),
                channel_slots: Arc::clone(&current.channel_slots),
                group_plan: Arc::clone(&current.group_plan),
                point_projection: Arc::clone(&current.point_projection),
                mount_projection: Arc::clone(&current.mount_projection),
                declared_positions: Arc::clone(&current.declared_positions),
                // Same snapshot, same modes.
                mode_indices: Arc::clone(&current.mode_indices),
            }),
            GroupMasterGenerationUpdate::Changed,
        )
    }

    /// The slot numbering every frame of this generation is addressed by.
    pub(crate) fn slots(&self) -> &Arc<crate::SlotTable> {
        &self.slots
    }

    /// Where each channel of each head reads its attributes from.
    pub(crate) fn channel_slots(&self) -> &crate::ChannelSlotIndex {
        &self.channel_slots
    }

    /// This generation's frame buffers.
    pub(crate) fn frames(&self) -> &Arc<crate::FramePool> {
        &self.frames
    }

    pub(crate) fn snapshot(&self) -> &EngineSnapshot {
        &self.snapshot
    }

    /// `crate::fixture::profile_mode` of the snapshot's fixture at `fixture_index`, without
    /// searching the profile's modes again (TL-639 round 4).
    pub(crate) fn fixture_mode(&self, fixture_index: usize) -> Option<&light_fixture::FixtureMode> {
        let indices = self.mode_indices.get_or_init(|| {
            self.snapshot
                .fixtures
                .iter()
                .map(|fixture| {
                    let mode = crate::fixture::profile_mode(fixture)?;
                    let profile = fixture.definition.profile_snapshot.as_deref()?;
                    let index = profile
                        .modes
                        .iter()
                        .position(|candidate| std::ptr::eq(candidate, mode))?;
                    u32::try_from(index).ok()
                })
                .collect()
        });
        let fixture = self.snapshot.fixtures.get(fixture_index)?;
        let index = (*indices.get(fixture_index)?)?;
        fixture
            .definition
            .profile_snapshot
            .as_deref()?
            .modes
            .get(index as usize)
    }

    pub(crate) fn snapshot_arc(&self) -> Arc<EngineSnapshot> {
        Arc::clone(&self.snapshot)
    }

    pub(crate) fn identity(&self) -> u64 {
        self.identity
    }

    pub(crate) fn point_projection(&self) -> &crate::point_projection::PointProjectionIndex {
        &self.point_projection
    }

    pub(crate) fn mount_projection(&self) -> &crate::mount_projection::MountProjectionIndex {
        &self.mount_projection
    }

    pub(crate) fn playback(&self) -> &RwLock<PlaybackEngine> {
        &self.playback
    }

    pub(crate) fn playback_arc(&self) -> Arc<RwLock<PlaybackEngine>> {
        Arc::clone(&self.playback)
    }

    pub(crate) fn group_plan(&self) -> &crate::group_plan::GroupContributionPlan {
        &self.group_plan
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

    pub(crate) fn default_value(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&AttributeValue> {
        self.default_values.get(&(fixture_id, attribute.clone()))
    }

    pub(crate) fn declared_positions(&self) -> &crate::position_adoption::DeclaredPositions {
        &self.declared_positions
    }

    pub(crate) fn position_owner_view(&self) -> crate::position_adoption::PositionOwnerView<'_> {
        (&self.snapshot, &self.profile_projections)
    }

    pub(crate) fn group_masters(&self) -> &GroupMasterIndex {
        &self.group_masters
    }

    pub(crate) fn group_rankings_arc(
        &self,
    ) -> Arc<HashMap<String, light_dynamics::RankedSelection>> {
        Arc::clone(&self.group_rankings)
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

    pub(crate) fn profile_owner(&self, owner: FixtureId) -> Option<(FixtureId, usize)> {
        self.profile_projections.owner(owner)
    }

    /// Native footprint of one family owner on one physical instance (TL-548 C2).
    pub(crate) fn family_footprint(
        &self,
        fixture: &light_fixture::PatchedFixture,
        mode: &light_fixture::FixtureMode,
        owner: (FixtureId, light_core::programming::ProgrammingOwner),
        instance: uuid::Uuid,
    ) -> Option<&[usize]> {
        self.profile_projections
            .family_footprint(fixture, mode, owner, instance)
    }
}

/// Every compiled cue list learns where this generation keeps its pairs, so a playback
/// contribution is offered by number rather than by name on every tick, and where a Position
/// that fades in from nothing starts: this generation's declared default poses (TL-552).
fn install_playback_frame(
    playback: &RwLock<PlaybackEngine>,
    slots: &Arc<crate::SlotTable>,
    snapshot: &Arc<EngineSnapshot>,
    profile_projections: &Arc<ProfileProjectionIndex>,
) -> Arc<crate::position_adoption::DeclaredPositions> {
    let declared = Arc::new(crate::position_adoption::DeclaredPositions::new(
        Arc::clone(snapshot),
        Arc::clone(profile_projections),
    ));
    // TL-544 G2: Color, Zoom and Focus fade in from their declared defaults too.
    let starts = crate::declared_family_starts::DeclaredFamilyStarts::new(
        Arc::clone(&declared),
        Arc::clone(snapshot),
        Arc::clone(profile_projections),
    );
    let mut playback = playback.write();
    playback.resolve_frame_addresses(&crate::FrameAddresser::new(Arc::clone(slots)));
    playback.set_family_start(Some(Arc::new(starts) as _));
    declared
}

fn compile_default_values(snapshot: &EngineSnapshot) -> crate::ResolvedValues {
    let mut values = crate::ResolvedValues::default();
    for fixture in snapshot.fixtures.iter() {
        for parameter in fixture
            .definition
            .heads
            .iter()
            .filter(|head| head.shared)
            .flat_map(|head| &head.parameters)
        {
            values.insert(
                (fixture.fixture_id, parameter.attribute.clone()),
                AttributeValue::Normalized(parameter.default),
            );
        }
        for logical in &fixture.logical_heads {
            let Some(head) = fixture
                .definition
                .heads
                .iter()
                .find(|head| head.index == logical.head_index)
            else {
                continue;
            };
            for parameter in &head.parameters {
                values.insert(
                    (logical.fixture_id, parameter.attribute.clone()),
                    AttributeValue::Normalized(parameter.default),
                );
            }
        }
    }
    values
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

#[derive(Clone, Debug, Default)]
pub(crate) struct GroupMasterIndex {
    masters: Vec<GroupMasterBinding>,
    fixtures: HashMap<FixtureId, Vec<usize>>,
}

#[derive(Clone, Debug)]
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
        // A show with no Group Master assigned asks this for every head of every frame.
        if self.fixtures.is_empty() {
            return 1.0;
        }
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

    /// The index a Release-policy installation compiles: portable seeds only, no Live levels.
    pub(crate) fn compile_released(
        groups: &HashMap<String, GroupDefinition>,
        snapshot: &EngineSnapshot,
    ) -> Self {
        Self::compile(groups, snapshot, None)
    }

    /// Sets an existing binding in place. `None` means the Group has no Group Master binding in
    /// this index (unassigned, deleted, or unresolvable), matching `with_master`'s missing case.
    pub(crate) fn set_existing_master(&mut self, group_id: &str, value: f32) -> Option<bool> {
        let binding = self
            .masters
            .iter_mut()
            .find(|binding| binding.group_id == group_id)?;
        let changed = binding.master != value;
        binding.master = value;
        Some(changed)
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
