//! TL-559 native Color descriptors, reference head, page overflow and representation adoption.
//!
//! Everything here consumes the TL-595 identity layer; nothing adds an identity heuristic:
//! - A destination head's identity comes from its fixture's runtime Color context, which was
//!   derived from the complete immutable profile before compaction. Names, canonical aliases,
//!   channel counts and DMX slots never decide identity or compatibility.
//! - Inspecting pages, descriptors or current native values is read-only. Only a real native
//!   edit (`adopt_native_edit`) turns a Semantic value into Direct: it captures the published
//!   premaster writes of the reference head under their own frame token through
//!   `PreparedOutputFrame::capture_direct_color`, then applies the edit through the complete
//!   family operation `edit_family`. The active function of every control is derived from the
//!   head's authoritative mode descriptor and the premaster raw value; a published write without
//!   a function (a parked write) never becomes a Direct control by itself, no UUID is invented.
//! - The first semantic edit of a Direct value (`adopt_semantic_edit`) derives its starting
//!   intent once from the source estimate (forward-evaluated by the original model when it is
//!   available) through `light_core::programming::semantic_color_adoption`, the same conversion
//!   incompatible replay fits, and applies the edit through `edit_family`.
use super::*;
use light_core::programming::{
    ColorAuthoringModel, ComponentEdit, FamilyEditContext, IntentError,
    NativeColorComponentDescriptor, NativeColorEditModel, SemanticColorAdoption, edit_family,
    semantic_color_adoption,
};
use light_core::{AttributeKey, NativeColorBinding, NativeColorValue};
use light_dynamics::{DynamicNativeModelResolver, NativeColorModelCapability};
use light_engine::{CapturedFrameToken, DirectColorObservation};
use light_fixture::{
    ChannelFunctionBehavior, OpticalEmitterBand, OpticalSource, native_color_function_allowed,
};

/// Encoder pages 3/4 show at most this many native controls; the rest are overflow controls
/// for the full Color modal. Nothing is silently omitted.
pub(in crate::runtime) const NATIVE_PAGE_CONTROLS: usize = 8;

/// One control of a head's native Color path, as the verified profile declares it.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct NativeColorControl {
    pub channel_id: Uuid,
    /// Mode channel index (the adapter's native raw space) and split.
    pub channel_index: u32,
    pub split: u16,
    /// Display label only; never used for identity or compatibility.
    pub attribute: AttributeKey,
    pub raw_max: u32,
    /// The profile declares an ultraviolet emitter on this channel (UV is independent of
    /// visible appearance and never held from an earlier frame).
    pub ultraviolet: bool,
    /// Every recordable function of the channel (service/control functions excluded).
    pub functions: Box<[NativeColorComponentDescriptor]>,
}

impl NativeColorControl {
    /// The single recordable function containing `raw`, from the authoritative descriptor.
    pub fn function_for(&self, raw: u32) -> Result<NativeColorBinding, IntentError> {
        let mut found = self
            .functions
            .iter()
            .filter(|f| (f.raw_from.min(f.raw_to)..=f.raw_from.max(f.raw_to)).contains(&raw));
        match (found.next(), found.next()) {
            (Some(function), None) => Ok(function.binding),
            (None, _) => Err(IntentError(
                "premaster native value lies outside every recordable function of its channel"
                    .into(),
            )),
            (Some(_), Some(_)) => Err(IntentError(
                "premaster native value matches several recordable functions".into(),
            )),
        }
    }
}

/// Identity and native controls of one destination head. The identity is withheld (None, so
/// Direct values fall back) when the runtime Color context is missing or its path control is
/// not part of the fitter's complete footprint.
pub(super) fn head_native(
    fixture: &PatchedFixture,
    head_id: Uuid,
    footprint: &[ColorFitControl],
) -> (Option<NativeColorIdentity>, Box<[NativeColorControl]>) {
    let none = || (None, Box::default());
    let Some(context) = fixture.definition.runtime_color_context.as_deref() else {
        return none();
    };
    let Some(identity) = context.identities().iter().find(|i| i.head_id == head_id) else {
        return none();
    };
    let mode = context.mode();
    let Some(path) = mode
        .color_physical
        .as_ref()
        .and_then(|model| model.paths.iter().find(|p| p.id == identity.path_id))
    else {
        return none();
    };
    let mut controls = Vec::with_capacity(path.controls.len());
    for id in &path.controls {
        let (Some(channel), Some(fit)) = (
            mode.channels.iter().find(|c| c.id == *id),
            footprint.iter().find(|c| c.channel_id == *id),
        ) else {
            return none();
        };
        let functions = channel
            .functions
            .iter()
            .filter(|function| native_color_function_allowed(channel, function))
            .map(|function| NativeColorComponentDescriptor {
                binding: NativeColorBinding {
                    channel_id: channel.id,
                    function_id: function.id,
                },
                raw_from: function.dmx_from,
                raw_to: function.dmx_to,
                continuous: matches!(
                    function.behavior,
                    ChannelFunctionBehavior::Continuous { .. }
                ),
            })
            .collect();
        let ultraviolet = match &path.source {
            OpticalSource::Additive { emitters } => emitters.iter().any(|e| {
                e.binding.channel_id == channel.id && e.band == OpticalEmitterBand::Ultraviolet
            }),
            _ => false,
        };
        controls.push(NativeColorControl {
            channel_id: channel.id,
            channel_index: fit.channel_index,
            split: fit.split,
            attribute: channel.attribute.clone(),
            raw_max: fit.raw_max,
            ultraviolet,
            functions,
        });
    }
    (Some(identity.clone()), controls.into_boxed_slice())
}

/// One destination head of one programming target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct ColorHeadRef {
    pub target: FixtureId,
    pub destination: FixtureId,
    pub head_id: Uuid,
}

/// Native page layout of one verified head: pages 3/4 plus the modal overflow, in path order.
#[derive(Clone, Copy, Debug)]
pub(in crate::runtime) struct NativeColorPages<'a> {
    pub head: ColorHeadRef,
    pub identity: &'a NativeColorIdentity,
    pub pages: &'a [NativeColorControl],
    pub overflow: &'a [NativeColorControl],
}

impl ColorDescriptor {
    /// Root-instance head `head_id` (multipatch copies are output destinations, not authoring
    /// references).
    pub fn root_head(&self, head_id: Uuid) -> Option<&ColorHeadDescriptor> {
        self.heads
            .iter()
            .find(|h| h.destination == self.root && h.head_id == head_id)
    }

    /// Pages/overflow of a verified root head. Read-only.
    pub fn native_pages(&self, target: FixtureId, head_id: Uuid) -> Option<NativeColorPages<'_>> {
        let head = self.root_head(head_id)?;
        let identity = head.native.as_ref()?;
        let split = head.native_controls.len().min(NATIVE_PAGE_CONTROLS);
        Some(NativeColorPages {
            head: ColorHeadRef {
                target,
                destination: head.destination,
                head_id,
            },
            identity,
            pages: &head.native_controls[..split],
            overflow: &head.native_controls[split..],
        })
    }
}

/// The first eligible head (verified native identity) in the stable ordered selection.
/// Selection changes never reseed an existing Direct value; callers pick again explicitly.
pub(in crate::runtime) fn reference_head<'a>(
    selection: impl IntoIterator<Item = (FixtureId, &'a ColorDescriptor)>,
) -> Option<ColorHeadRef> {
    selection.into_iter().find_map(|(target, descriptor)| {
        descriptor
            .heads
            .iter()
            .find(|h| {
                h.destination == descriptor.root
                    && h.native.is_some()
                    && !h.native_controls.is_empty()
            })
            .map(|h| ColorHeadRef {
                target,
                destination: h.destination,
                head_id: h.head_id,
            })
    })
}

/// The published output of one target from one accepted frame.
#[derive(Clone, Copy)]
pub(in crate::runtime) struct PublishedColorHead<'a> {
    pub token: &'a CapturedFrameToken,
    pub target: FixtureId,
    /// The composed family value the frame resolved.
    pub value: &'a AttributeValue,
    pub writes: &'a [NativeControlWrite],
}

impl<'a> PublishedColorHead<'a> {
    pub fn from_result(result: &'a PhysicalHeadResult<ColorAdapter>) -> Self {
        Self {
            token: &result.token,
            target: result.target,
            value: &result.value,
            writes: &result.writes,
        }
    }
}

/// Current premaster native values of one head from a published frame, each with the function
/// derived from the authoritative descriptor. Read-only inspection: nothing is adopted.
pub(in crate::runtime) fn inspect_native_values(
    head: &ColorHeadDescriptor,
    published: &PublishedColorHead<'_>,
) -> Result<Vec<NativeColorValue>, TransitionError> {
    if head.native.is_none() {
        return Err(TransitionError::Requires(
            TransitionRequirement::NativeColorModel,
        ));
    }
    head.native_controls
        .iter()
        .map(|control| {
            let write = published
                .writes
                .iter()
                .find(|w| {
                    w.slot.destination == head.destination
                        && w.slot.channel_index == control.channel_index
                        && w.channel_id == control.channel_id
                })
                .ok_or_else(|| invalid("published Color output lacks a native path control"))?;
            let binding = control.function_for(write.raw)?;
            if write
                .function_id
                .is_some_and(|id| id != binding.function_id)
            {
                return Err(invalid(
                    "published Color write disagrees with its descriptor function",
                ));
            }
            Ok(NativeColorValue {
                channel_id: binding.channel_id,
                function_id: binding.function_id,
                raw: write.raw,
            })
        })
        .collect()
}

fn require_native_edits(edits: &[ComponentEdit]) -> Result<(), TransitionError> {
    if edits.is_empty()
        || !edits
            .iter()
            .all(|edit| matches!(edit, ComponentEdit::Native { .. }))
    {
        return Err(invalid(
            "only a real native edit adopts a Direct Color recipe",
        ));
    }
    Ok(())
}

/// A real native edit on the reference head. An existing Direct value of the same source is
/// edited in place (never reseeded). Anything else is captured once from the published
/// premaster writes of that frame, then edited, as one complete Color family value.
pub(in crate::runtime) fn adopt_native_edit(
    capture: &PreparedOutputFrame,
    descriptor: &ColorDescriptor,
    published: PublishedColorHead<'_>,
    head_id: Uuid,
    edits: &[ComponentEdit],
) -> Result<AttributeValue, TransitionError> {
    require_native_edits(edits)?;
    let head = descriptor
        .root_head(head_id)
        .ok_or_else(|| invalid("reference Color head is not part of this target"))?;
    // One authoritative guard for BOTH branches: the published output must belong to this
    // capture, this target and this descriptor before any value (captured or existing) is edited.
    authoritative_publication(capture, descriptor, &published, head_id)?;
    let source = head.native.clone().ok_or(TransitionError::Requires(
        TransitionRequirement::NativeColorModel,
    ))?;
    let catalogue = Arc::clone(&capture.snapshot().native_color_sources);
    let model = match catalogue.resolve_capability(&source)? {
        NativeColorModelCapability::Available(model) => model,
        NativeColorModelCapability::Unavailable(_) => {
            return Err(TransitionError::Requires(
                TransitionRequirement::NativeColorModel,
            ));
        }
    };
    let base = match published.value {
        AttributeValue::ColorProgram(program) if matches!(program.as_ref(), ColorProgram::Direct { recipe, .. } if recipe.source == source) => {
            published.value.clone()
        }
        _ => {
            let values = inspect_native_values(head, &published)?;
            let mut captured = capture.capture_direct_color(vec![DirectColorObservation {
                token: published.token.clone(),
                target: published.target,
                native: light_core::programming::NativeColorObservation { source, values },
            }])?;
            captured.remove(0).capture.into_value()
        }
    };
    let context = FamilyEditContext {
        native_model: Some(model.as_ref() as &dyn NativeColorEditModel),
        ..FamilyEditContext::default()
    };
    Ok(edit_family(&base, edits, &context)?)
}

/// The published head must come from `capture` (same capture and generation, Live or one of
/// its Preload branches), name a programming target whose root/head is this descriptor's, and
/// carry exactly this descriptor's complete native footprint.
fn authoritative_publication(
    capture: &PreparedOutputFrame,
    descriptor: &ColorDescriptor,
    published: &PublishedColorHead<'_>,
    head_id: Uuid,
) -> Result<(), TransitionError> {
    let frame = capture.frame_token();
    if !published.token.same_capture(&frame) || published.token.generation() != capture.generation()
    {
        return Err(invalid(
            "published Color output belongs to another capture or generation",
        ));
    }
    let snapshot = capture.snapshot();
    if !light_engine::profile_head_destinations(&snapshot, published.target)
        .iter()
        .any(|d| d.destination == descriptor.root && d.head_id == head_id)
    {
        return Err(invalid(
            "published Color target does not own the reference head of this descriptor",
        ));
    }
    validate_complete_writes(&descriptor.footprint, published.writes)
}

/// One real native edit for an ordered (possibly mixed) selection: capture and edit once on the
/// reference head, then assign the same tagged value to every target. Other fixture types
/// replay it through the adapter's exact/fallback path.
pub(in crate::runtime) fn adopt_native_edit_for_selection(
    capture: &PreparedOutputFrame,
    reference: (&ColorDescriptor, PublishedColorHead<'_>, Uuid),
    targets: &[FixtureId],
    edits: &[ComponentEdit],
) -> Result<Vec<(FixtureId, AttributeValue)>, TransitionError> {
    let (descriptor, published, head_id) = reference;
    let value = adopt_native_edit(capture, descriptor, published, head_id, edits)?;
    Ok(targets
        .iter()
        .map(|target| (*target, value.clone()))
        .collect())
}

/// Source estimate of a Direct value for adoption: the original model's prediction of the
/// reference recipe (spreads removed, as authoring predicts), else the recorded estimate.
pub(super) fn adoption_estimate(
    program: &ColorProgram,
    models: &dyn DynamicNativeModelResolver,
) -> Result<PortableColorEstimate, TransitionError> {
    let ColorProgram::Direct { recipe, portable } = program else {
        return Err(invalid("semantic adoption requires a Direct Color value"));
    };
    match models.resolve_capability(&recipe.source)? {
        NativeColorModelCapability::Available(model) => {
            let reference = light_core::programming::NativeColorRecipe {
                spreads: Vec::new(),
                ..recipe.clone()
            };
            Ok(model.predict(&reference)?)
        }
        NativeColorModelCapability::Unavailable(_) => Ok(portable.clone()),
    }
}

/// The first semantic edit of a Color value. Semantic values are edited directly. A Direct value
/// is adopted once from its source estimate (unknown appearance needs `explicit_start`) and the
/// edit is applied through the complete family operation. Returns the adoption for display.
pub(in crate::runtime) fn adopt_semantic_edit(
    value: &AttributeValue,
    edits: &[ComponentEdit],
    explicit_start: Option<&ColorIntent>,
    models: &dyn DynamicNativeModelResolver,
    color_model: &dyn ColorAuthoringModel,
) -> Result<(AttributeValue, Option<SemanticColorAdoption>), TransitionError> {
    if edits.is_empty()
        || edits
            .iter()
            .any(|edit| matches!(edit, ComponentEdit::Native { .. }))
    {
        return Err(invalid("semantic adoption requires a semantic Color edit"));
    }
    let adoption = match value {
        AttributeValue::ColorProgram(program)
            if matches!(program.as_ref(), ColorProgram::Direct { .. }) =>
        {
            let estimate = adoption_estimate(program, models)?;
            Some(semantic_color_adoption(&estimate, explicit_start)?)
        }
        _ => None,
    };
    let context = FamilyEditContext {
        semantic_color_adoption: adoption.as_ref().map(|a| &a.intent),
        color_model: Some(color_model),
        ..FamilyEditContext::default()
    };
    Ok((edit_family(value, edits, &context)?, adoption))
}
