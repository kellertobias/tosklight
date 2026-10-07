use crate::{EngineError, EngineSnapshot, fixture::profile_mode, profile_head_owner};
use light_core::{AttributeKey, FixtureId};
use light_fixture::{
    CompiledPositionFitting, FixtureMode, FixtureModeResolutionPlan, PatchedFixture,
    PositionAxisRole,
};
// Per-frame lookups by fixture and owner: hashed for speed, never for adversaries (TL-553).
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use uuid::Uuid;

/// Immutable semantic projection metadata compiled with an engine generation.
#[derive(Debug, Default)]
pub(crate) struct ProfileProjectionIndex {
    fixtures: HashMap<FixtureId, FixtureProjectionPlan>,
    /// Physical root or logical head owner to its root fixture and snapshot position.
    /// The first head wins for an owner shared by several heads, as in channel addressing.
    owners: HashMap<FixtureId, (FixtureId, usize)>,
    pub(crate) physical: crate::physical_projection::PhysicalProjectionIndex,
    /// Color and Focus/Zoom native footprints per root fixture, compiled on the first native
    /// family installation that addresses the fixture in this generation (TL-548 C2).
    family_footprints:
        HashMap<FixtureId, std::sync::OnceLock<crate::native_family_footprint::FamilyFootprints>>,
}

#[derive(Debug)]
pub(crate) struct FixtureProjectionPlan {
    resolution: FixtureModeResolutionPlan,
    heads: Box<[ProfileHeadPlan]>,
    native_dependencies: HashMap<(FixtureId, AttributeKey), Box<[usize]>>,
    native_virtual_intensity: Box<[bool]>,
    position_footprints: HashMap<FixtureId, Box<[usize]>>,
    position_adoption_emitters: HashMap<FixtureId, Box<[PositionAdoptionEmitter]>>,
    position_freeze_signatures: HashMap<Uuid, Box<[Option<String>]>>,
    position_freeze_inputs:
        HashMap<Uuid, Box<[Option<crate::native_position_projection::NativePositionInput>]>>,
}

/// Cold emitter-to-command mapping, shared with all installation-specific forward models.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PositionAdoptionEmitter {
    pub(crate) emitter_id: Uuid,
    /// Pan then Tilt; absence explicitly retains missing or ambiguous role information.
    pub(crate) commands: Option<[(usize, Uuid); 2]>,
}

#[derive(Debug)]
pub(crate) struct ProfileHeadPlan {
    pub(crate) owner: FixtureId,
    pub(crate) head_id: Uuid,
    pub(crate) channel_indices: Box<[usize]>,
    pub(crate) intensity_channel_indices: Box<[usize]>,
    /// The head's Intensity parameter default (canonical), what its virtual intensity is while no
    /// Intensity value is resolved: the same default the output-parameter stage masters for an
    /// unprogrammed level. The abstract virtual dimmer defaults to full.
    pub(crate) intensity_default: f32,
    splits: Box<[u16]>,
    /// Cold Pan/Tilt role of every attribute that selects a Position-bound channel function of
    /// this head. Empty without a validated physical Position model; canonical names then apply.
    axis_roles: Box<[(AttributeKey, PositionAxisRole)]>,
}

impl ProfileProjectionIndex {
    pub(crate) fn compile(snapshot: &EngineSnapshot) -> Result<Self, EngineError> {
        let mut fixtures = HashMap::default();
        let mut owners = HashMap::default();
        for (index, fixture) in snapshot.fixtures.iter().enumerate() {
            let Some(mode) = profile_mode(fixture) else {
                continue;
            };
            let plan = FixtureProjectionPlan::compile(fixture, mode)?;
            // A physical root remains a native destination even when all of its heads
            // are logical owners. Geometry can also assign an unheaded emitter to it.
            owners
                .entry(fixture.fixture_id)
                .or_insert((fixture.fixture_id, index));
            for head in plan.heads.iter() {
                owners
                    .entry(head.owner)
                    .or_insert((fixture.fixture_id, index));
            }
            fixtures.insert(fixture.fixture_id, plan);
        }
        Ok(Self {
            family_footprints: fixtures
                .keys()
                .map(|id| (*id, Default::default()))
                .collect(),
            fixtures,
            owners,
            physical: crate::physical_projection::PhysicalProjectionIndex::compile(snapshot),
        })
    }

    /// Native footprint of `owner` (any family) on one physical instance of root `fixture`.
    /// Position is geometry-derived and identical on every instance; the other families come
    /// from the compiled Color, Media and optics models (`native_family_footprint`).
    pub(crate) fn family_footprint(
        &self,
        fixture: &PatchedFixture,
        mode: &FixtureMode,
        (target, owner): (FixtureId, light_core::programming::ProgrammingOwner),
        instance: Uuid,
    ) -> Option<&[usize]> {
        let plan = self.fixtures.get(&fixture.fixture_id)?;
        if owner == light_core::programming::ProgrammingOwner::Position {
            return plan.position_footprint(target);
        }
        self.family_footprints
            .get(&fixture.fixture_id)?
            .get_or_init(|| {
                crate::native_family_footprint::FamilyFootprints::compile(
                    fixture,
                    mode,
                    &plan.heads,
                    &self.physical,
                )
            })
            .get(target, owner, instance)
    }

    pub(crate) fn fixture(&self, fixture_id: FixtureId) -> Option<&FixtureProjectionPlan> {
        self.fixtures.get(&fixture_id)
    }

    /// Root fixture and its snapshot position for a physical root or logical head owner.
    pub(crate) fn owner(&self, owner: FixtureId) -> Option<(FixtureId, usize)> {
        self.owners.get(&owner).copied()
    }
}

impl FixtureProjectionPlan {
    fn compile(fixture: &PatchedFixture, mode: &FixtureMode) -> Result<Self, EngineError> {
        let mut heads = compile_heads(fixture, mode)?;
        let dependencies = compile_native_dependencies(mode, &heads);
        let (position_footprints, position_adoption_emitters) =
            compile_position_ownership(fixture, mode, &mut heads);
        let position_freeze_signatures =
            compile_position_freeze_signatures(fixture, mode, &position_footprints);
        let position_freeze_inputs = compile_position_freeze_inputs(
            fixture,
            mode,
            &position_footprints,
            &position_freeze_signatures,
        );
        Ok(Self {
            position_freeze_inputs,
            position_freeze_signatures,
            position_footprints,
            position_adoption_emitters,
            resolution: mode.compile_resolution_plan(),
            heads,
            native_dependencies: dependencies,
            native_virtual_intensity: mode
                .channels
                .iter()
                .map(|c| c.reacts_to_virtual_intensity)
                .collect(),
        })
    }

    pub(crate) fn native_ownership(
        &self,
        previewed: &std::collections::HashSet<(FixtureId, AttributeKey)>,
        color_writes: &[(FixtureId, usize)],
        values: &crate::ResolvedValues,
        active_attributes: &[Option<AttributeKey>],
    ) -> Option<Box<[bool]>> {
        let mut owned = vec![false; self.native_virtual_intensity.len()];
        for key in previewed {
            if let Some(channels) = self.native_dependencies.get(key) {
                for &index in channels {
                    // An absent key is an explicit Off/Release fallback. A present key must
                    // actually win arbitration, except intensity also scales its dependants.
                    if !values.contains_key(key)
                        || (key.1.is_intensity() && self.native_virtual_intensity[index])
                        || active_attributes.get(index).and_then(|v| v.as_ref()) == Some(&key.1)
                    {
                        owned[index] = true;
                    }
                }
            }
        }
        for &(owner, index) in color_writes {
            if previewed.contains(&(owner, AttributeKey::color())) {
                owned[index] = true;
            }
        }
        owned.iter().any(|v| *v).then(|| owned.into_boxed_slice())
    }

    pub(crate) fn position_footprint(&self, owner: FixtureId) -> Option<&[usize]> {
        self.position_footprints.get(&owner).map(AsRef::as_ref)
    }

    pub(crate) fn position_adoption_emitters(
        &self,
        owner: FixtureId,
    ) -> Option<&[PositionAdoptionEmitter]> {
        self.position_adoption_emitters
            .get(&owner)
            .map(AsRef::as_ref)
    }

    pub(crate) fn position_freeze_signature(&self, instance: Uuid, channel: usize) -> Option<&str> {
        self.position_freeze_signatures
            .get(&instance)?
            .get(channel)?
            .as_deref()
    }

    pub(crate) fn position_freeze_inputs(
        &self,
        instance: Uuid,
    ) -> Option<&[Option<crate::native_position_projection::NativePositionInput>]> {
        self.position_freeze_inputs
            .get(&instance)
            .map(AsRef::as_ref)
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

    /// The compiled Pan/Tilt role of an attribute of this head, if it selects a bound function.
    pub(crate) fn axis_role(&self, attribute: &AttributeKey) -> Option<PositionAxisRole> {
        self.axis_roles
            .iter()
            .find(|(key, _)| key == attribute)
            .map(|(_, role)| *role)
    }
}

/// Each head's channels, intensity channels and splits, in mode channel order.
fn compile_heads(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
) -> Result<Box<[ProfileHeadPlan]>, EngineError> {
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
    let heads: Box<[ProfileHeadPlan]> = mode
        .heads
        .iter()
        .enumerate()
        .map(|(head_index, head)| ProfileHeadPlan {
            owner: profile_head_owner(fixture, head_index, head),
            head_id: head.id,
            channel_indices: std::mem::take(&mut channels[head_index]).into_boxed_slice(),
            intensity_channel_indices: std::mem::take(&mut intensity_channels[head_index])
                .into_boxed_slice(),
            intensity_default: intensity_default(fixture, head_index),
            splits: std::mem::take(&mut splits[head_index]).into_boxed_slice(),
            axis_roles: Box::default(),
        })
        .collect();
    Ok(heads)
}

/// The Intensity parameter default the resolved definition declares for one head, or full when it
/// declares none.
fn intensity_default(fixture: &PatchedFixture, head_index: usize) -> f32 {
    fixture
        .definition
        .heads
        .get(head_index)
        .and_then(|head| {
            head.parameters
                .iter()
                .find(|parameter| parameter.attribute.is_intensity())
        })
        .map_or(1.0, |parameter| parameter.default.clamp(0.0, 1.0))
}

/// The channels each owner's attribute reaches, for native ownership of previewed values.
fn compile_native_dependencies(
    mode: &FixtureMode,
    heads: &[ProfileHeadPlan],
) -> HashMap<(FixtureId, AttributeKey), Box<[usize]>> {
    let mut dependencies: HashMap<(FixtureId, AttributeKey), Vec<usize>> = HashMap::default();
    let mut add = |owner, attribute: AttributeKey, index| {
        let channels = dependencies.entry((owner, attribute)).or_default();
        if !channels.contains(&index) {
            channels.push(index);
        }
    };
    for head in heads {
        for &index in &head.channel_indices {
            let channel = &mode.channels[index];
            if channel.behavior == light_fixture::ChannelBehavior::Static {
                continue;
            }
            add(head.owner, channel.attribute.clone(), index);
            add(head.owner, channel.fixture_attribute.clone(), index);
            add(
                head.owner,
                FixtureMode::control_action_attribute(channel.id),
                index,
            );
            for function in &channel.functions {
                add(head.owner, function.attribute.clone(), index);
            }
            if channel.reacts_to_virtual_intensity {
                add(head.owner, AttributeKey::intensity(), index);
            }
        }
    }
    dependencies
        .into_iter()
        .map(|(key, channels)| (key, channels.into_boxed_slice()))
        .collect()
}

type PositionOwnership = (
    HashMap<FixtureId, Box<[usize]>>,
    HashMap<FixtureId, Box<[PositionAdoptionEmitter]>>,
);

/// Position footprints and adoption emitters per owner, assigning each head's axis roles.
fn compile_position_ownership(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    heads: &mut [ProfileHeadPlan],
) -> PositionOwnership {
    // Ownership is geometry-derived, including root-owned shared ancestral motors.
    // This cold-only compilation reuses the existing fitter's exact footprint rules.
    let mut position_footprints = HashMap::default();
    let mut position_adoption_emitters = HashMap::default();
    if let Some(profile) = fixture.definition.profile_snapshot.as_deref()
        && let Ok(Some(model)) = light_fixture::CompiledPositionFitting::compile(
            profile,
            mode.id,
            light_fixture::forward::PositionInstallation::default(),
        )
    {
        assign_axis_roles(mode, &model, heads);
        let owners: HashSet<_> = std::iter::once(fixture.fixture_id)
            .chain(heads.iter().map(|head| head.owner))
            .collect();
        for owner in owners {
            let mut channels = HashSet::default();
            let mut adoption = Vec::new();
            for emitter in model.emitters().filter(|emitter| {
                emitter
                    .head_id
                    .map_or(owner == fixture.fixture_id, |head_id| {
                        heads
                            .iter()
                            .any(|head| head.owner == owner && head.head_id == head_id)
                    })
            }) {
                adoption.push(PositionAdoptionEmitter {
                    emitter_id: emitter.emitter_id,
                    commands: emitter
                        .command_indices
                        .map(|indices| indices.map(|index| (index, model.axes()[index].node_id))),
                });
                for &axis in emitter.ancestor_axes {
                    let axis = &model.axes()[axis];
                    if matches!(
                        axis.role,
                        Some(
                            light_fixture::PositionAxisRole::Pan
                                | light_fixture::PositionAxisRole::Tilt
                        )
                    ) {
                        channels.extend(
                            axis.controls
                                .iter()
                                .map(|control| control.channel_index as usize),
                        );
                    }
                }
            }
            if !adoption.is_empty() {
                position_adoption_emitters.insert(owner, adoption.into_boxed_slice());
            }
            if !channels.is_empty() {
                let mut channels: Vec<_> = channels.into_iter().collect();
                channels.sort_unstable();
                position_footprints.insert(owner, channels.into_boxed_slice());
            }
        }
    }
    (position_footprints, position_adoption_emitters)
}

/// Cold Position Freeze compatibility signature of every footprint control per instance.
fn compile_position_freeze_signatures(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    position_footprints: &HashMap<FixtureId, Box<[usize]>>,
) -> HashMap<Uuid, Box<[Option<String>]>> {
    let mut position_freeze_signatures = HashMap::default();
    if !position_footprints.is_empty()
        && let Some(profile) = fixture.definition.profile_snapshot.as_deref()
    {
        let controls: HashSet<_> = position_footprints
            .values()
            .flat_map(|indices| indices.iter().copied())
            .collect();
        let instances = std::iter::once((
            fixture.fixture_id.0,
            light_fixture::forward::PositionInstallation {
                calibration: fixture.position_calibration.as_ref(),
                invert_pan: fixture.invert_pan,
                invert_tilt: fixture.invert_tilt,
                bracket_degrees: f64::from(fixture.bracket_angle),
            },
        ))
        .chain(fixture.multipatch.iter().map(|copy| {
            (
                copy.id,
                light_fixture::forward::PositionInstallation {
                    calibration: copy.position_calibration.as_ref(),
                    invert_pan: copy.invert_pan,
                    invert_tilt: copy.invert_tilt,
                    bracket_degrees: f64::from(copy.bracket_angle),
                },
            )
        }));
        for (instance, installed) in instances {
            let mut signatures = vec![None; mode.channels.len()];
            for &index in &controls {
                // Compatibility is cold-only. An unsupported interpretation cannot be
                // captured; it does not prevent ordinary show activation or output.
                signatures[index] = light_fixture::position_freeze_control_signature(
                    profile,
                    mode.id,
                    mode.channels[index].id,
                    installed,
                )
                .ok()
                .flatten();
            }
            position_freeze_signatures.insert(instance, signatures.into_boxed_slice());
        }
    }
    position_freeze_signatures
}

/// Assign each head the attributes that select its Pan/Tilt-bound channel functions (TL-630).
///
/// Roles come only from the validated cold forward model: its role-bearing axes, the mode's
/// motion bindings for those axes and the exact bound function of each driving channel. An
/// attribute reaches the bound function either as the function's own attribute or, for a
/// function named after its channel, as the channel's distinct fixture-facing attribute (the
/// same rule the compiled resolution plan uses). Other functions of the same channel, other
/// heads' owners and attributes bound to both roles stay unresolved, so the legacy canonical
/// `pan`/`tilt` rule applies to them unchanged.
fn assign_axis_roles(
    mode: &FixtureMode,
    model: &CompiledPositionFitting,
    heads: &mut [ProfileHeadPlan],
) {
    let Some(physical) = mode.position_physical.as_ref() else {
        return;
    };
    let mut roles: Vec<Vec<(AttributeKey, Option<PositionAxisRole>)>> =
        heads.iter().map(|_| Vec::new()).collect();
    for axis in model.axes() {
        let Some(role) = axis.role else {
            continue;
        };
        for binding in physical
            .bindings
            .iter()
            .filter(|binding| binding.node_id == axis.node_id && binding.role == role)
        {
            let Some(index) = axis
                .controls
                .iter()
                .find(|control| control.channel_id == binding.channel_id)
                .map(|control| control.channel_index as usize)
            else {
                continue;
            };
            let channel = &mode.channels[index];
            let (Some(function), Some(head)) = (
                channel
                    .functions
                    .iter()
                    .find(|function| function.id == binding.function_id),
                heads
                    .iter()
                    .position(|head| head.channel_indices.contains(&index)),
            ) else {
                continue;
            };
            let aliases = (function.attribute == channel.attribute
                && channel.fixture_attribute != channel.attribute)
                .then_some(&channel.fixture_attribute);
            for attribute in std::iter::once(&function.attribute).chain(aliases) {
                match roles[head].iter_mut().find(|(key, _)| key == attribute) {
                    // One attribute driving both roles is ambiguous and stays unresolved.
                    Some((_, existing)) if *existing != Some(role) => *existing = None,
                    Some(_) => {}
                    None => roles[head].push((attribute.clone(), Some(role))),
                }
            }
        }
    }
    for (head, roles) in heads.iter_mut().zip(roles) {
        head.axis_roles = roles
            .into_iter()
            .filter_map(|(attribute, role)| Some((attribute, role?)))
            .collect();
    }
}

/// One incompatible control withholds the entire owner/physical-instance pair. New copies have
/// no implicit inherited hold; stale identity/interpretation remains portable but never replayed.
fn compile_position_freeze_inputs(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    footprints: &HashMap<FixtureId, Box<[usize]>>,
    signatures: &HashMap<Uuid, Box<[Option<String>]>>,
) -> HashMap<Uuid, Box<[Option<crate::native_position_projection::NativePositionInput>]>> {
    use crate::native_position_projection::NativePositionInput;
    let mut output: HashMap<Uuid, Box<[Option<NativePositionInput>]>> = HashMap::default();
    let mut owners: Vec<_> = fixture.freeze.targets.iter().collect();
    owners.sort_by_key(|(owner, _)| owner.0);
    for (owner, target) in owners {
        let Some(payload) = &target.position_native else {
            continue;
        };
        if payload.version != 1
            || (!target.full
                && !target
                    .families
                    .contains(&light_fixture::FreezeFamily::Position))
        {
            continue;
        }
        let Some(footprint) = footprints.get(owner) else {
            continue;
        };
        for instance in &payload.instances {
            let Some(expected) = signatures.get(&instance.instance_id) else {
                continue;
            };
            if footprint.is_empty() || instance.controls.len() != footprint.len() {
                continue;
            }
            let controls: HashMap<_, _> = instance
                .controls
                .iter()
                .map(|control| (control.channel_id, control))
                .collect();
            if controls.len() != footprint.len()
                || !footprint.iter().all(|&index| {
                    let channel = &mode.channels[index];
                    controls.get(&channel.id).is_some_and(|control| {
                        control.raw <= channel.resolution.max_raw()
                            && expected[index].as_deref() == Some(control.signature.as_str())
                    })
                })
            {
                continue;
            }
            let row = output
                .entry(instance.instance_id)
                .or_insert_with(|| vec![None; mode.channels.len()].into_boxed_slice());
            for &index in footprint.iter() {
                let control = controls[&mode.channels[index].id];
                // Central patch validation rejects conflicting shared holds. Full ownership on
                // either agreeing owner retains the existing full Freeze overlay bypass.
                let full =
                    target.full || row[index].as_ref().is_some_and(|value| value.full_freeze);
                row[index] = Some(NativePositionInput::frozen(control.raw, full));
            }
        }
    }
    output
}
