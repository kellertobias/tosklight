//! TL-554 production wiring of the first native edit (`adopt_native_edit`'s seed half).
//!
//! The edit transport captures, once per gesture, everything a native edit needs and hands it to
//! the shared Programmer planner, which applies the edit itself (`edit_family` with the pinned
//! model) for every target of the ordered selection:
//! - the reference head: the operator's explicit choice when it is a verified root head of the
//!   selection, otherwise the first verified root head in selection order (`reference_head`);
//! - its verified original model from the runtime generation's retained catalogue (never the
//!   current library, never a name or slot heuristic);
//! - the complete Direct value captured from that head's *published* premaster output in one
//!   accepted Live frame (the displayed frame, or the latest), guarded exactly like
//!   `adopt_native_edit`: the output must belong to that frame, its target must own the head and
//!   its writes must be the descriptor's complete footprint. A published value that is already
//!   Direct of the same source is kept as it is (never reseeded).
//!
//! Reading pages, values or descriptors never calls anything here.
use super::native::{
    NativeColorControl, PublishedColorHead, inspect_native_values, reference_head,
};
use super::*;
use light_core::programming::{NativeColorEditModel, NativeColorObservation};
use light_dynamics::NativeColorModelCapability;
use light_engine::CapturedFrameLane;

/// A verified pinned model of one native Color source.
pub(in crate::runtime) type NativeModel = Arc<dyn NativeColorEditModel + Send + Sync>;

/// The compiled descriptor of the selection's reference head.
pub(in crate::runtime) struct NativeReference {
    pub target: FixtureId,
    pub head_id: Uuid,
    /// The operator chose this head (rather than the first verified one).
    pub chosen: bool,
    pub descriptor: ColorDescriptor,
}

impl NativeReference {
    pub fn head(&self) -> &ColorHeadDescriptor {
        self.descriptor
            .root_head(self.head_id)
            .expect("a reference names a verified root head of its descriptor")
    }

    pub fn identity(&self) -> &NativeColorIdentity {
        self.head()
            .native
            .as_ref()
            .expect("a reference head is verified")
    }

    pub fn controls(&self) -> &[NativeColorControl] {
        &self.head().native_controls
    }
}

fn verified_root_head(descriptor: &ColorDescriptor, head_id: Uuid) -> bool {
    descriptor
        .root_head(head_id)
        .is_some_and(|head| head.native.is_some() && !head.native_controls.is_empty())
}

/// The reference head of `members` (ordered). An explicit choice that is not a verified root
/// head of the selection is ignored when `fallback` is set, else it yields `None`.
pub(in crate::runtime) fn native_reference(
    snapshot: &EngineSnapshot,
    members: &[FixtureId],
    explicit: Option<(FixtureId, Option<Uuid>)>,
    fallback: bool,
) -> Option<NativeReference> {
    let adapter = ColorAdapter::default();
    let compile = |target: FixtureId| adapter.compile(snapshot, target).ok().flatten();
    if let Some((target, head)) = explicit.filter(|(target, _)| members.contains(target)) {
        let chosen = compile(target).and_then(|descriptor| {
            let head_id = head.or_else(|| {
                reference_head([(target, &descriptor)]).map(|reference| reference.head_id)
            })?;
            verified_root_head(&descriptor, head_id).then_some(NativeReference {
                target,
                head_id,
                chosen: true,
                descriptor,
            })
        });
        if chosen.is_some() || !fallback {
            return chosen;
        }
    } else if explicit.is_some() && !fallback {
        return None;
    }
    let mut seen = Vec::new();
    members.iter().find_map(|target| {
        if seen.contains(target) {
            return None;
        }
        seen.push(*target);
        let descriptor = compile(*target)?;
        let head = reference_head([(*target, &descriptor)])?;
        Some(NativeReference {
            target: *target,
            head_id: head.head_id,
            chosen: false,
            descriptor,
        })
    })
}

/// The verified original model of `source` in this generation's retained catalogue.
pub(in crate::runtime) fn native_source_model(
    snapshot: &EngineSnapshot,
    source: &NativeColorIdentity,
) -> Option<NativeModel> {
    match snapshot
        .native_color_sources
        .resolve_capability(source)
        .ok()?
    {
        NativeColorModelCapability::Available(model) => Some(model),
        NativeColorModelCapability::Unavailable(_) => None,
    }
}

/// The complete Direct value a first native edit starts from: the reference head's published
/// premaster output in the accepted Live frame `(generation, sampled_at)`, captured once with
/// the pinned original model. A published Direct value of the same source is returned as is.
pub(in crate::runtime) fn published_native_seed(
    snapshot: &EngineSnapshot,
    frame: (u64, chrono::DateTime<chrono::Utc>),
    reference: &NativeReference,
    published: PublishedColorHead<'_>,
) -> Result<AttributeValue, TransitionError> {
    let (generation, sampled_at) = frame;
    if published.token.generation() != generation
        || published.token.sampled_at() != sampled_at
        || !matches!(published.token.lane(), CapturedFrameLane::Live)
        || published.target != reference.target
    {
        return Err(invalid(
            "published Color output belongs to another frame, lane or target",
        ));
    }
    if !profile_head_destinations(snapshot, published.target)
        .iter()
        .any(|d| d.destination == reference.descriptor.root && d.head_id == reference.head_id)
    {
        return Err(invalid(
            "published Color target does not own the reference head of this descriptor",
        ));
    }
    validate_complete_writes(&reference.descriptor.footprint, published.writes)?;
    let source = reference.identity().clone();
    if let AttributeValue::ColorProgram(program) = published.value
        && matches!(program.as_ref(), ColorProgram::Direct { recipe, .. } if recipe.source == source)
    {
        return Ok(published.value.clone());
    }
    let values = inspect_native_values(reference.head(), &published)?;
    let captured = snapshot
        .native_color_sources
        .capture_direct(NativeColorObservation { source, values })?;
    Ok(captured.into_value())
}

/// G5: the native values an idle reference head outputs (no Color sidecar in the frame): every
/// path control at its profile default, with the function the descriptor derives for that raw.
/// `None` when a default lies outside every recordable function (nothing is invented).
pub(in crate::runtime) fn idle_native_values(
    reference: &NativeReference,
) -> Option<Vec<light_core::NativeColorValue>> {
    reference
        .controls()
        .iter()
        .map(|control| {
            let binding = control.function_for(control.default_raw).ok()?;
            Some(light_core::NativeColorValue {
                channel_id: binding.channel_id,
                function_id: binding.function_id,
                raw: control.default_raw,
            })
        })
        .collect()
}

/// G5: the complete Direct value a first native edit on an idle head starts from: the head's
/// profile-default output, captured once with the pinned original model.
pub(in crate::runtime) fn idle_native_seed(
    snapshot: &EngineSnapshot,
    reference: &NativeReference,
) -> Option<AttributeValue> {
    let values = idle_native_values(reference)?;
    snapshot
        .native_color_sources
        .capture_direct(NativeColorObservation {
            source: reference.identity().clone(),
            values,
        })
        .ok()
        .map(|captured| captured.into_value())
}

/// G5: the complete Direct value a first native edit in Preload starts from: the reference
/// head's output in the Programmer's accepted Pending (After) branch. Guarded like
/// [`published_native_seed`], except that the output belongs to the Preload lane.
pub(in crate::runtime) fn pending_native_seed(
    snapshot: &EngineSnapshot,
    reference: &NativeReference,
    published: PublishedColorHead<'_>,
) -> Result<AttributeValue, TransitionError> {
    if !matches!(published.token.lane(), CapturedFrameLane::Preload { .. })
        || published.target != reference.target
    {
        return Err(invalid(
            "Pending Color output belongs to another lane or target",
        ));
    }
    if !profile_head_destinations(snapshot, published.target)
        .iter()
        .any(|d| d.destination == reference.descriptor.root && d.head_id == reference.head_id)
    {
        return Err(invalid(
            "Pending Color target does not own the reference head of this descriptor",
        ));
    }
    validate_complete_writes(&reference.descriptor.footprint, published.writes)?;
    let source = reference.identity().clone();
    if let AttributeValue::ColorProgram(program) = published.value
        && matches!(program.as_ref(), ColorProgram::Direct { recipe, .. } if recipe.source == source)
    {
        return Ok(published.value.clone());
    }
    let values = inspect_native_values(reference.head(), &published)?;
    let captured = snapshot
        .native_color_sources
        .capture_direct(NativeColorObservation { source, values })?;
    Ok(captured.into_value())
}
