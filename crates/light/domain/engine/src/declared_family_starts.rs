//! TL-544 G2: where a Color, Zoom or Focus that a Cue fades in over nothing starts.
//!
//! The Position precedent (TL-552, [`DeclaredPositions`]) decodes the fixture's declared default
//! pose through the compiled physical model. This does the same for the other semantic families:
//! every channel at its profile `default_raw`, evaluated once through the generation's physical
//! projection of the root instance, then read through the family's forward model:
//!
//! - Color: the visible appearance of the default native output (before dimmer/shutter gating),
//!   adopted as a semantic intent by the shared `semantic_color_adoption` (known black stays
//!   black, unknown UV adopts off). Unknown visible appearance has no start.
//! - Zoom: the default opening in degrees with the profile's declared convention. Without a
//!   declared convention there is no start (a degree value is never guessed into a convention).
//! - Focus: the default normalized lens travel.
//!
//! The start is frame-only, the lowest-ranked value of the fade and carries no provenance: the
//! Playback evidence still names the authored endpoints alone. Owners whose default cannot be
//! decoded (no model, unresolved function, ambiguous heads) keep the existing behaviour.
//! Decoding is cached per generation, so a fading Cue reads it every tick without evaluating
//! the forward model again.
use crate::position_adoption::DeclaredPositions;
use crate::{EngineSnapshot, PhysicalInstanceOutput, PhysicalModelSupport, ProfileProjectionIndex};
use light_core::programming::{
    ColorIntent, ColorProgram, PortableColorEstimate, PortableUv, PortableVisibleColor,
    ProgrammingOwner, ScalarIntent, ZoomIntent, semantic_color_adoption,
};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_fixture::forward::OpticsForwardStatus;
use std::{collections::HashMap, sync::Arc};

/// One generation's family starts: Position delegates to the declared pose, the other families
/// decode here. Installed on the Playback engine as its [`light_playback::FamilyStartSource`].
pub(crate) struct DeclaredFamilyStarts {
    positions: Arc<DeclaredPositions>,
    snapshot: Arc<EngineSnapshot>,
    projections: Arc<ProfileProjectionIndex>,
    cache: parking_lot::Mutex<HashMap<(FixtureId, ProgrammingOwner), Option<AttributeValue>>>,
    native_transitions: parking_lot::Mutex<
        Vec<(
            AttributeValue,
            AttributeValue,
            Option<light_core::programming::CompiledProgrammingTransition>,
        )>,
    >,
}

impl DeclaredFamilyStarts {
    pub(crate) fn new(
        positions: Arc<DeclaredPositions>,
        snapshot: Arc<EngineSnapshot>,
        projections: Arc<ProfileProjectionIndex>,
    ) -> Self {
        Self {
            positions,
            snapshot,
            projections,
            cache: Default::default(),
            native_transitions: Default::default(),
        }
    }

    /// The declared default of `owner` on a programming target, decoded once per generation.
    pub(crate) fn start(
        &self,
        target: FixtureId,
        owner: ProgrammingOwner,
    ) -> Option<AttributeValue> {
        if let Some(known) = self.cache.lock().get(&(target, owner)) {
            return known.clone();
        }
        let decoded = self.decode(target, owner);
        self.cache.lock().insert((target, owner), decoded.clone());
        decoded
    }

    fn decode(&self, target: FixtureId, owner: ProgrammingOwner) -> Option<AttributeValue> {
        decode_family((&self.snapshot, &self.projections), target, owner)
    }
}

/// Decode the declared default of `owner` on `target` (see the module documentation).
fn decode_family(
    (snapshot, projections): crate::position_adoption::PositionOwnerView<'_>,
    target: FixtureId,
    owner: ProgrammingOwner,
) -> Option<AttributeValue> {
    let (root, fixture_index) = projections.owner(target)?;
    let fixture = &snapshot.fixtures[fixture_index];
    let mode = crate::fixture::profile_mode(fixture)?;
    let raw: Vec<(u32, u32)> = (0u32..)
        .zip(mode.channels.iter().map(|channel| channel.default_raw))
        .collect();
    let projection = &projections.physical;
    let mut frame = projection.take_frame();
    // Copies replay the root's raw values; the root instance is authoritative.
    projection.evaluate(root, 0, &raw, &mut frame).ok()?;
    let instance = frame
        .instances
        .iter()
        .find(|instance| instance.fixture_id == root && instance.instance_id == root.0)?;
    if !instance.complete {
        return None;
    }
    let heads: Vec<uuid::Uuid> = mode
        .heads
        .iter()
        .enumerate()
        .filter(|(index, head)| crate::fixture::profile_head_owner(fixture, *index, head) == target)
        .map(|(_, head)| head.id)
        .collect();
    match owner {
        ProgrammingOwner::Color => declared_color(instance, &heads),
        ProgrammingOwner::Zoom | ProgrammingOwner::Focus => {
            declared_optics(instance, &heads, owner)
        }
        ProgrammingOwner::Position => None,
    }
}

impl crate::Engine {
    /// The declared default of a semantic family on a programming target, as a Cue fade from
    /// nothing starts from it (TL-544 G2): every channel at its profile `default_raw`, decoded
    /// through the family's compiled forward model. It is a frame-only baseline, never authored
    /// evidence. `None` when the snapshot is not current or the default cannot be decoded.
    pub fn declared_default_family(
        &self,
        snapshot: &EngineSnapshot,
        target: FixtureId,
        owner: ProgrammingOwner,
    ) -> Option<AttributeValue> {
        let current = self.generation.load_full();
        if !std::ptr::eq(snapshot, current.snapshot()) {
            return None;
        }
        match owner {
            ProgrammingOwner::Position => {
                let pose = current.declared_positions().pose(target)?;
                Some(AttributeValue::Position(Arc::new(
                    light_core::programming::PositionIntent::angles(
                        pose.pan_degrees,
                        pose.tilt_degrees,
                    ),
                )))
            }
            _ => decode_family(current.position_owner_view(), target, owner),
        }
    }
}

/// The one owned head's visible default appearance as a semantic Color intent.
fn declared_color(
    instance: &PhysicalInstanceOutput,
    heads: &[uuid::Uuid],
) -> Option<AttributeValue> {
    if instance.color_support != PhysicalModelSupport::Compiled {
        return None;
    }
    let mut results = instance
        .colors()
        .iter()
        .filter(|result| heads.contains(&result.head_id));
    let result = results.next()?;
    if results.next().is_some() || !result.visible_complete {
        return None;
    }
    let estimate = PortableColorEstimate {
        model_revision: 0,
        visible: Some(PortableVisibleColor {
            xyz: result.known_xyz,
            relative_output: 1.0,
        }),
        uv: result.portable_uv.map(|uv| PortableUv {
            amount: uv.amount as f32,
            quality: uv.quality,
        }),
        quality: result.data_quality,
        limitations: Vec::new(),
    };
    let intent: ColorIntent = semantic_color_adoption(&estimate, None).ok()?.intent;
    Some(AttributeValue::ColorProgram(Arc::new(
        ColorProgram::Semantic { intent },
    )))
}

/// The one owned head's resolved default Zoom opening or Focus travel.
fn declared_optics(
    instance: &PhysicalInstanceOutput,
    heads: &[uuid::Uuid],
    owner: ProgrammingOwner,
) -> Option<AttributeValue> {
    if instance.optics_support != PhysicalModelSupport::Compiled {
        return None;
    }
    let zoom = owner == ProgrammingOwner::Zoom;
    let status = |result: &light_fixture::forward::OpticsForwardResult| {
        if zoom {
            result.zoom_status
        } else {
            result.focus_status
        }
    };
    let mut found = None;
    for result in instance
        .optics()
        .iter()
        .filter(|result| heads.contains(&result.head_id))
    {
        if status(result) == OpticsForwardStatus::Unsupported {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(result);
    }
    let result = found?;
    if status(result) != OpticsForwardStatus::Resolved {
        return None;
    }
    if zoom {
        let value = result.zoom?;
        let degrees = value.degrees as f32;
        let intent = ZoomIntent {
            opening_degrees: ScalarIntent::Value(degrees),
            convention: value.convention?,
        };
        intent.validate().ok()?;
        Some(AttributeValue::Zoom(Arc::new(intent)))
    } else {
        let normalized = (result.focus?.percent / 100.) as f32;
        (normalized.is_finite() && (0.0..=1.0).contains(&normalized))
            .then_some(AttributeValue::Normalized(normalized))
    }
}

/// Position keeps its declared pose; Color, Zoom and Focus start from their declared default.
impl light_playback::FamilyStartSource for DeclaredFamilyStarts {
    fn sample_native_transition(
        &self,
        from: &AttributeValue,
        to: &AttributeValue,
        progress: f32,
    ) -> Option<AttributeValue> {
        let (AttributeValue::ColorProgram(a), AttributeValue::ColorProgram(b)) = (from, to) else {
            return None;
        };
        let (ColorProgram::Direct { recipe: a, .. }, ColorProgram::Direct { recipe: b, .. }) =
            (a.as_ref(), b.as_ref())
        else {
            return None;
        };
        if a.source != b.source {
            return None;
        }
        // Bounded generation-local cache pins ORIGINAL models and endpoint correspondence.
        // Samples predict the varied recipe, never substitute a destination profile.
        let mut cache = self.native_transitions.lock();
        if let Some((_, _, compiled)) = cache.iter().find(|(a, b, _)| a == from && b == to) {
            return compiled.as_ref()?.sample(progress).ok();
        }
        let compiled = self
            .snapshot
            .native_color_sources
            .resolve(&a.source)
            .ok()
            .and_then(|model| {
                light_core::programming::CompiledProgrammingTransition::new(
                    from.clone(),
                    to.clone(),
                    Some(model),
                )
                .ok()
            });
        let sample = compiled
            .as_ref()
            .and_then(|compiled| compiled.sample(progress).ok());
        if cache.len() >= 128 {
            cache.remove(0);
        }
        cache.push((from.clone(), to.clone(), compiled));
        sample
    }

    fn family_start(&self, fixture: FixtureId, attribute: &AttributeKey) -> Option<AttributeValue> {
        match attribute.0.as_ref() {
            "position" => self.positions.family_start(fixture, attribute),
            "color" => self.start(fixture, ProgrammingOwner::Color),
            "zoom" => self.start(fixture, ProgrammingOwner::Zoom),
            "focus" => self.start(fixture, ProgrammingOwner::Focus),
            _ => None,
        }
    }
}
