//! Runtime-only Angle adoption from final commanded physical output. No Target solve,
//! fitted native replay, source mutation or persisted copy-specific representation lives here.
use crate::{
    Engine, PhysicalForwardFrame, PhysicalInstanceOutput, PhysicalModelSupport,
    ProfileProjectionIndex,
};
use light_core::{FixtureId, programming::JointAngles};
use light_fixture::{PositionAxisRole, forward::PositionForwardFlags};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

/// One actual emitter's commanded joints in programming coordinates. Installation calibration
/// is already reflected in the pair; mounting remains in the frame's separate lens pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionCommandedReadout {
    pub destination: FixtureId,
    pub emitter_id: Uuid,
    pub pan_degrees: f64,
    pub tilt_degrees: f64,
}

/// Bulk readout preserves requested owner order and represents unsupported owners explicitly.
#[derive(Clone, Debug, PartialEq)]
pub struct PositionCommandedOwnerReadout {
    pub owner: FixtureId,
    pub commands: Option<Vec<PositionCommandedReadout>>,
}

impl PositionCommandedOwnerReadout {
    /// One common authoring pair only when every returned command agrees exactly in f64.
    /// This conversion provides no additional acceptance/generation authority.
    pub fn common_angles(&self) -> Option<JointAngles> {
        let mut common = None;
        for command in self.commands.as_ref()? {
            agree_common_angles(&mut common, *command)?;
        }
        author_angles(common)
    }
}

/// The engine snapshot and projections one Position owner decodes against.
pub(crate) type PositionOwnerView<'a> = (&'a crate::EngineSnapshot, &'a ProfileProjectionIndex);

/// Each owner's declared default pose for one generation's snapshot and projections, decoded
/// once on first use. A Cue that fades a Position in from nothing reads it every tick (TL-552),
/// so the forward model is not re-evaluated per frame. `None` (unknown, unsupported, disagreeing
/// copies) is cached too. The Playback engine holds it as its family start.
pub(crate) struct DeclaredPositions {
    snapshot: Arc<crate::EngineSnapshot>,
    projections: Arc<ProfileProjectionIndex>,
    cache: parking_lot::Mutex<HashMap<FixtureId, Option<JointAngles>>>,
}

impl DeclaredPositions {
    pub(crate) fn new(
        snapshot: Arc<crate::EngineSnapshot>,
        projections: Arc<ProfileProjectionIndex>,
    ) -> Self {
        Self {
            snapshot,
            projections,
            cache: Default::default(),
        }
    }

    /// See [`Engine::declared_default_position`].
    pub(crate) fn pose(&self, owner: FixtureId) -> Option<JointAngles> {
        if let Some(known) = self.cache.lock().get(&owner) {
            return *known;
        }
        let decoded = self.decode(owner);
        self.cache.lock().insert(owner, decoded);
        decoded
    }

    fn decode(&self, owner: FixtureId) -> Option<JointAngles> {
        let (root, fixture_index) = self.projections.owner(owner)?;
        let fixture = &self.snapshot.fixtures[fixture_index];
        let mode = fixture
            .definition
            .profile_snapshot
            .as_deref()?
            .mode(fixture.definition.mode_id?)?;
        let raw: Vec<(u32, u32)> = (0u32..)
            .zip(mode.channels.iter().map(|c| c.default_raw))
            .collect();
        let projection = &self.projections.physical;
        let mut frame = projection.take_frame();
        for instance in 0..=fixture.multipatch.len() {
            projection.evaluate(root, instance, &raw, &mut frame).ok()?;
        }
        let mut common = None;
        Engine::visit_position_owner_commands(
            (&self.snapshot, &self.projections),
            owner,
            |root, instance_id| {
                frame
                    .instances
                    .iter()
                    .find(|i| i.fixture_id == root && i.instance_id == instance_id)
            },
            |readout| agree_common_angles(&mut common, readout),
        )?;
        author_angles(common)
    }
}

/// A Position fading in over nothing starts from the declared default pose; nothing else does.
impl light_playback::FamilyStartSource for DeclaredPositions {
    fn family_start(
        &self,
        fixture: FixtureId,
        attribute: &light_core::AttributeKey,
    ) -> Option<light_core::AttributeValue> {
        if *attribute != light_core::programming::ProgrammingOwner::Position.key() {
            return None;
        }
        let pose = self.pose(fixture)?;
        Some(light_core::AttributeValue::Position(Arc::new(
            light_core::programming::PositionIntent::angles(pose.pan_degrees, pose.tilt_degrees),
        )))
    }
}

fn agree_common_angles(
    common: &mut Option<[f64; 2]>,
    command: PositionCommandedReadout,
) -> Option<()> {
    let pair = [command.pan_degrees, command.tilt_degrees];
    if common.is_some_and(|common| common != pair) {
        return None;
    }
    *common = Some(pair);
    Some(())
}

fn author_angles(common: Option<[f64; 2]>) -> Option<JointAngles> {
    let [pan, tilt] = common?;
    let result = JointAngles {
        pan_degrees: pan as f32,
        tilt_degrees: tilt as f32,
    };
    (result.pan_degrees.is_finite() && result.tilt_degrees.is_finite()).then_some(result)
}

impl Engine {
    /// Cold geometry-derived Position ownership for command planning. The exact snapshot guard
    /// prevents capability reads from a newer patch being joined to an older environment.
    /// Presence establishes controls, not a usable pose or successful Angle adoption.
    pub fn position_has_native_controls(
        &self,
        snapshot: &crate::EngineSnapshot,
        owner: FixtureId,
    ) -> bool {
        let current = self.generation.load_full();
        if !std::ptr::eq(snapshot, current.snapshot()) {
            return false;
        }
        let Some((root, _)) = current.profile_owner(owner) else {
            return false;
        };
        current
            .profile_projection(root)
            .and_then(|profile| profile.position_footprint(owner))
            .is_some_and(|channels| !channels.is_empty())
    }

    /// The fixture's declared default pose: every channel at its profile `default_raw`, decoded
    /// through the compiled Position model of the root and every physical copy (TL-552). It is
    /// the static pre-Dynamic baseline of an owner nothing has programmed, never authored
    /// evidence. Unsupported or disagreeing copies return None: unknown stays unknown, never 0°.
    pub fn declared_default_position(
        &self,
        snapshot: &crate::EngineSnapshot,
        owner: FixtureId,
    ) -> Option<JointAngles> {
        let current = self.generation.load_full();
        if !std::ptr::eq(snapshot, current.snapshot()) {
            return None;
        }
        current.declared_positions().pose(owner)
    }

    /// Read one common calibrated, unwrapped commanded Pan/Tilt pair from a final
    /// physical frame of the exact current generation and model layout. The caller supplies the
    /// accepted output for its Normal, Blind or Preload lane; this method does not infer a lane
    /// or establish that an arbitrary observational frame was accepted.
    ///
    /// Every owned emitter and physical copy must agree. Missing, velocity, ambiguous or
    /// unsupported axes/poses return `None`; optical equivalence modulo 360 is never equality.
    /// Equality is deliberately strict in the decoded f64 domain: differently quantized or
    /// calibrated copies may be withheld even when they look alike. This API never approximates
    /// them by their root or first emitter. The final f32 authoring conversion is checked finite.
    pub fn position_angles_from_physical(
        &self,
        generation: u64,
        physical: &PhysicalForwardFrame,
        owner: FixtureId,
    ) -> Option<JointAngles> {
        let mut common: Option<[f64; 2]> = None;
        self.visit_position_commands_from_physical(generation, physical, owner, |readout| {
            agree_common_angles(&mut common, readout)
        })?;
        author_angles(common)
    }

    /// Collect commanded joints for a selected programming owner from its supplied accepted
    /// physical output. Each owned emitter is returned for the root and every physical copy,
    /// including different calibrated, unwrapped pairs. This read establishes neither frame
    /// acceptance nor a lane; it performs no solve, resampling or mutation.
    ///
    /// This selected-owner API uses the existing strict generation/model/pose validation and
    /// is not a bulk hot-render projection. Rows are ordered root then copies, with each
    /// instance's compiled owned-emitter order. Unsupported or incomplete output returns None.
    pub fn position_commanded_readout_from_physical(
        &self,
        generation: u64,
        physical: &PhysicalForwardFrame,
        owner: FixtureId,
    ) -> Option<Vec<PositionCommandedReadout>> {
        let mut readouts = Vec::new();
        self.visit_position_commands_from_physical(generation, physical, owner, |readout| {
            readouts.push(readout);
            Some(())
        })?;
        Some(readouts)
    }

    /// Read a list of programming owners with one generation/layout validation and one
    /// physical-row lookup. Invalid frame identity returns None; unavailable owners remain
    /// present with commands None. Caller order and duplicates are preserved. Duplicate actual
    /// physical row keys make that instance unavailable rather than selecting a winner.
    pub fn position_commanded_readouts_from_physical(
        &self,
        generation: u64,
        physical: &PhysicalForwardFrame,
        owners: &[FixtureId],
    ) -> Option<Vec<PositionCommandedOwnerReadout>> {
        let current = self.generation.load_full();
        if current.identity() != generation
            || !physical.belongs_to_generation(generation)
            || !current.physical_projection().layout_matches(physical)
        {
            return None;
        }
        if owners.is_empty() {
            return Some(Vec::new());
        }
        let mut instances = HashMap::with_capacity(physical.instances.len());
        for instance in &physical.instances {
            instances
                .entry((instance.fixture_id, instance.instance_id))
                .and_modify(|entry| *entry = None)
                .or_insert(Some(instance));
        }
        Some(
            owners
                .iter()
                .map(|&owner| {
                    let mut commands = Vec::new();
                    let available = Self::visit_position_owner_commands(
                        current.position_owner_view(),
                        owner,
                        |root, instance_id| instances.get(&(root, instance_id)).copied().flatten(),
                        |command| {
                            commands.push(command);
                            Some(())
                        },
                    )
                    .is_some();
                    PositionCommandedOwnerReadout {
                        owner,
                        commands: available.then_some(commands),
                    }
                })
                .collect(),
        )
    }

    // Shared decoder deliberately keeps equality policy outside the frame/pose validation.
    // The adoption caller can refuse a divergent pair before any f32 conversion or allocation.
    fn visit_position_commands_from_physical(
        &self,
        generation: u64,
        physical: &PhysicalForwardFrame,
        owner: FixtureId,
        visit: impl FnMut(PositionCommandedReadout) -> Option<()>,
    ) -> Option<()> {
        let current = self.generation.load_full();
        if current.identity() != generation
            || !physical.belongs_to_generation(generation)
            || !current.physical_projection().layout_matches(physical)
        {
            return None;
        }
        Self::visit_position_owner_commands(
            current.position_owner_view(),
            owner,
            |root, instance_id| {
                let mut instances = physical.instances.iter().filter(|instance| {
                    instance.fixture_id == root && instance.instance_id == instance_id
                });
                let instance = instances.next()?;
                instances.next().is_none().then_some(instance)
            },
            visit,
        )
    }

    fn visit_position_owner_commands<'a>(
        (snapshot, projections): PositionOwnerView<'_>,
        owner: FixtureId,
        mut instance_for: impl FnMut(FixtureId, Uuid) -> Option<&'a PhysicalInstanceOutput>,
        mut visit: impl FnMut(PositionCommandedReadout) -> Option<()>,
    ) -> Option<()> {
        let (root, fixture_index) = projections.owner(owner)?;
        let fixture = &snapshot.fixtures[fixture_index];
        let emitters = projections
            .fixture(root)?
            .position_adoption_emitters(owner)?;
        if emitters.is_empty() {
            return None;
        }
        for instance_id in
            std::iter::once(root.0).chain(fixture.multipatch.iter().map(|copy| copy.id))
        {
            let instance = instance_for(root, instance_id)?;
            if !instance.complete || instance.position_support != PhysicalModelSupport::Compiled {
                return None;
            }
            for emitter in emitters {
                let commands = emitter.commands?;
                let mut lenses = instance
                    .lenses()
                    .iter()
                    .filter(|lens| lens.emitter_id == emitter.emitter_id);
                let lens = lenses.next()?;
                if lenses.next().is_some()
                    || lens.local.is_none()
                    || lens.world.is_none()
                    || lens.flags != PositionForwardFlags::default()
                {
                    return None;
                }
                let mut pair = [0.; 2];
                for (axis_index, ((index, node_id), role)) in commands
                    .into_iter()
                    .zip([PositionAxisRole::Pan, PositionAxisRole::Tilt])
                    .enumerate()
                {
                    let command = instance.axes().get(index)?;
                    if command.node_id != node_id || command.role != Some(role) {
                        return None;
                    }
                    pair[axis_index] = command.absolute_degrees()?;
                    if !pair[axis_index].is_finite() {
                        return None;
                    }
                }
                visit(PositionCommandedReadout {
                    destination: FixtureId(instance_id),
                    emitter_id: emitter.emitter_id,
                    pan_degrees: pair[0],
                    tilt_degrees: pair[1],
                })?;
            }
        }
        Some(())
    }
}
