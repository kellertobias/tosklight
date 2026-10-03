//! One per-frame native installation for every physical family (TL-548 C2).
//!
//! Physical adapters fit Position, Color (lamp and Media) and Focus/Zoom to exact native control
//! raws. [`PreparedStaticFamilyFrame::project_family_native`] binds all of them to the frame's
//! semantic token at once, as pre-master inputs of the ordinary renderer: masters, blackout,
//! Highlight, control loss and Freeze still act exactly once, after these writes, in their usual
//! place. Requested intent, provenance and continuity are not changed.
//!
//! The collection is validated as a whole and installed all or nothing:
//! - every write belongs to this capture and lane, addresses a captured semantic owner, a physical
//!   instance of that owner's fixture and a control of the owner's compiled footprint;
//! - channel identity, split, raw range and any selected function are exact;
//! - each `(target, family)` writes every footprint control of every physical instance once;
//! - owners of one family sharing a control must agree; two families never share a control;
//! - one installation per frame. An empty collection is a no-op.
use crate::native_position_projection::{
    NativePositionInput, NativePositionInstance, NativePositionProjection,
};
use crate::{CapturedFrameToken, EngineError, PreparedOutputFrame, PreparedStaticFamilyFrame};
use light_core::{AttributeValue, FixtureId, programming::ProgrammingOwner};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;
use uuid::Uuid;

/// One fitted native control write of one physical family owner on a physical root or copy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyNativeWrite {
    /// The family (Position, Color, Focus or Zoom) whose adapter fitted this write.
    pub owner: ProgrammingOwner,
    /// Logical owner (physical root or logical head) the family value addresses.
    pub target: FixtureId,
    /// Physical root (`root.0`) or multipatch copy id.
    pub instance_id: Uuid,
    pub channel_index: u32,
    pub channel_id: Uuid,
    pub function_id: Option<Uuid>,
    pub split: u16,
    pub raw: u32,
}

/// The validated destination of one write.
struct Destination {
    root: FixtureId,
    fixture_index: usize,
    channels: usize,
}

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::Invalid(message.into())
}

fn family(owner: ProgrammingOwner) -> &'static str {
    match owner {
        ProgrammingOwner::Position => "Position",
        ProgrammingOwner::Color => "Color",
        ProgrammingOwner::Focus => "Focus",
        ProgrammingOwner::Zoom => "Zoom",
    }
}

impl PreparedStaticFamilyFrame {
    /// Queue every family's complete fitted native writes for this frame in one call. See the
    /// module documentation for the validation contract. A rejection installs nothing.
    pub fn project_family_native(
        &mut self,
        capture: &PreparedOutputFrame,
        token: &CapturedFrameToken,
        writes: &[FamilyNativeWrite],
    ) -> Result<(), EngineError> {
        if !Arc::ptr_eq(&self.capture_identity, &capture.identity) {
            return Err(EngineError::StalePreparedFrame);
        }
        if !token.matches_static_frame(self) || token.generation() != capture.generation() {
            return Err(invalid(
                "native family projection uses another frame or lane",
            ));
        }
        if writes.is_empty() {
            return Ok(());
        }
        if !self.position_native.instances.is_empty() {
            return Err(invalid("native family projection is already installed"));
        }
        let mut candidate = NativePositionProjection::default();
        // Sized up front: each write is checked once, and owners arrive in runs.
        let mut seen = FxHashSet::with_capacity_and_hasher(writes.len(), Default::default());
        let mut counts =
            FxHashMap::<(FixtureId, ProgrammingOwner, Uuid), usize>::with_capacity_and_hasher(
                writes.len() / 2,
                Default::default(),
            );
        let mut targets = FxHashMap::<(FixtureId, ProgrammingOwner), (FixtureId, usize)>::default();
        // TL-639 round 2: an owner's writes arrive together. What depends only on the owner and
        // instance (destination, semantic owner, mode, footprint) is validated once per run.
        let mut run: Option<WriteRun<'_>> = None;
        for write in writes {
            let key = (write.target, write.owner, write.instance_id);
            if run.as_ref().is_none_or(|run| run.key != key) {
                if let Some(done) = run.take() {
                    *counts.entry(done.key).or_default() += done.count;
                }
                let (destination, mode, footprint) = self.validate_family_owner(capture, write)?;
                targets.insert(
                    (write.target, write.owner),
                    (destination.root, destination.fixture_index),
                );
                run = Some(WriteRun {
                    key,
                    destination,
                    mode,
                    footprint,
                    count: 0,
                });
            }
            let current = run.as_mut().expect("a run for this write");
            validate_family_control(current.mode, current.footprint, write)?;
            let index = write.channel_index as usize;
            if !seen.insert((write.owner, write.target, write.instance_id, index)) {
                return Err(invalid(format!(
                    "native {} owner writes a control twice",
                    family(write.owner)
                )));
            }
            current.count += 1;
            install(&mut candidate, write, &current.destination)?;
        }
        if let Some(done) = run {
            *counts.entry(done.key).or_default() += done.count;
        }
        verify_complete(capture, &targets, &counts)?;
        self.position_native = candidate;
        Ok(())
    }

    /// Validate what one write's owner and instance determine: destination, semantic owner,
    /// instance, mode and footprint. Returns the destination, mode and footprint.
    fn validate_family_owner<'g>(
        &self,
        capture: &'g PreparedOutputFrame,
        write: &FamilyNativeWrite,
    ) -> Result<(Destination, &'g light_fixture::FixtureMode, &'g [usize]), EngineError> {
        let name = family(write.owner);
        let generation = &capture.generation;
        let (root, fixture_index) = generation
            .profile_owner(write.target)
            .ok_or_else(|| invalid(format!("native {name} target has no profile destination")))?;
        self.validate_semantic_owner(write)?;
        let fixture = &generation.snapshot().fixtures[fixture_index];
        if write.instance_id != root.0
            && !fixture
                .multipatch
                .iter()
                .any(|copy| copy.id == write.instance_id)
        {
            return Err(invalid(format!(
                "native {name} instance does not belong to target"
            )));
        }
        let mode = crate::fixture::profile_mode(fixture)
            .ok_or_else(|| invalid(format!("native {name} has no profile mode")))?;
        let footprint = generation
            .family_footprint(
                fixture,
                mode,
                (write.target, write.owner),
                write.instance_id,
            )
            .ok_or_else(|| invalid(format!("native {name} target has no verified footprint")))?;
        Ok((
            Destination {
                root,
                fixture_index,
                channels: mode.channels.len(),
            },
            mode,
            footprint,
        ))
    }

    /// The owner must carry a captured semantic value on this frame: queued by `project_family`
    /// or resolved by the static baseline. Position values must also be valid Position intent.
    fn validate_semantic_owner(&self, write: &FamilyNativeWrite) -> Result<(), EngineError> {
        let value = self
            .projections
            .get(&(write.target, write.owner))
            .map(|(value, _)| value)
            .or_else(|| self.value(write.target, &write.owner.key()));
        match (write.owner, value) {
            (ProgrammingOwner::Position, Some(AttributeValue::Position(value))) => value
                .validate()
                .map_err(|error| EngineError::Invalid(error.to_string())),
            (ProgrammingOwner::Position, _) => Err(invalid(
                "native Position requires a captured semantic owner",
            )),
            (_, Some(_)) => Ok(()),
            (owner, None) => Err(invalid(format!(
                "native {} requires a captured semantic owner",
                family(owner)
            ))),
        }
    }
}

/// The owner and instance of consecutive writes, validated once for the run.
struct WriteRun<'g> {
    key: (FixtureId, ProgrammingOwner, Uuid),
    destination: Destination,
    mode: &'g light_fixture::FixtureMode,
    footprint: &'g [usize],
    count: usize,
}

/// Validate one write against its owner's mode and footprint: footprint membership, channel
/// identity, raw range and function.
fn validate_family_control(
    mode: &light_fixture::FixtureMode,
    footprint: &[usize],
    write: &FamilyNativeWrite,
) -> Result<(), EngineError> {
    let name = family(write.owner);
    let index = write.channel_index as usize;
    if !footprint.contains(&index) {
        return Err(invalid(format!(
            "native {name} writes outside its owner's footprint"
        )));
    }
    let channel = &mode.channels[index];
    if channel.id != write.channel_id
        || channel.split != write.split
        || write.raw > channel.resolution.max_raw()
    {
        return Err(invalid(format!(
            "native {name} channel identity or raw range is invalid"
        )));
    }
    if let Some(function_id) = write.function_id {
        let function = channel.functions.iter().any(|function| {
            function.id == function_id && (function.dmx_from..=function.dmx_to).contains(&write.raw)
        });
        let bound = write.owner != ProgrammingOwner::Position
            || mode.position_physical.as_ref().is_some_and(|model| {
                model.bindings.iter().any(|binding| {
                    binding.channel_id == write.channel_id && binding.function_id == function_id
                })
            });
        if !function || !bound {
            return Err(invalid(if write.owner == ProgrammingOwner::Position {
                "native Position function is not a motion binding".to_owned()
            } else {
                format!("native {name} function does not select this raw")
            }));
        }
    }
    Ok(())
}

/// Place one validated write. Same-family owners may share an agreeing control; a control already
/// claimed by another family rejects the collection whatever its value.
fn install(
    candidate: &mut NativePositionProjection,
    write: &FamilyNativeWrite,
    destination: &Destination,
) -> Result<(), EngineError> {
    let index = write.channel_index as usize;
    let row = candidate
        .instances
        .entry(write.instance_id)
        .or_insert_with(|| NativePositionInstance {
            root: destination.root,
            channels: vec![None; destination.channels].into_boxed_slice(),
            claims: Vec::new(),
        });
    let input = NativePositionInput {
        value: AttributeValue::RawDmxExact(write.raw),
        function_id: write.function_id,
        frozen: false,
        full_freeze: false,
        family: Some(write.owner),
    };
    match &row.channels[index] {
        Some(previous) if previous.family != input.family => {
            return Err(invalid(
                "native owners of different families claim one control",
            ));
        }
        Some(previous)
            if previous.value != input.value || previous.function_id != input.function_id =>
        {
            return Err(invalid(format!(
                "native {} owners disagree on a shared control",
                family(write.owner)
            )));
        }
        Some(_) => {}
        None => row.channels[index] = Some(input),
    }
    row.claims.push((write.target, write.owner, index));
    Ok(())
}

/// Every addressed `(target, family)` writes its complete footprint on every physical instance.
fn verify_complete(
    capture: &PreparedOutputFrame,
    targets: &FxHashMap<(FixtureId, ProgrammingOwner), (FixtureId, usize)>,
    counts: &FxHashMap<(FixtureId, ProgrammingOwner, Uuid), usize>,
) -> Result<(), EngineError> {
    let generation = &capture.generation;
    for (&(target, owner), &(root, fixture_index)) in targets {
        let fixture = &generation.snapshot().fixtures[fixture_index];
        let mode = crate::fixture::profile_mode(fixture)
            .ok_or_else(|| invalid("native family has no profile mode"))?;
        for instance in std::iter::once(root.0).chain(fixture.multipatch.iter().map(|c| c.id)) {
            let expected = generation
                .family_footprint(fixture, mode, (target, owner), instance)
                .map_or(0, <[usize]>::len);
            if expected == 0 || counts.get(&(target, owner, instance)) != Some(&expected) {
                return Err(invalid(format!(
                    "native {} requires every control of every physical instance",
                    family(owner)
                )));
            }
        }
    }
    Ok(())
}
