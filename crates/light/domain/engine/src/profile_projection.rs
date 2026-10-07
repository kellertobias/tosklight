use crate::profile_projection_plan::{FixtureProjectionPlan, ProfileHeadPlan};
use crate::{
    EngineError, ProfileValueIndex, RenderOptions, apply_safe_values, apply_safe_values_with_snap,
    blackout_raw, channel_visual_level, profile_visual_color,
};
use light_core::{AttributeKey, AttributeValue, ColorProgrammingModel, FixtureId, Xyz};
use light_fixture::ChannelAttribute;
use light_fixture::{
    BoundFixtureModeResolution, ChannelFunctionBehavior, ChannelScales, FixtureMode,
    FixtureModeEncodingPlan, HighlightColor, HighlightLook, HighlightLookCompatibility,
    HighlightShutterPolicy, PatchedFixture, SignalLossPolicy,
};
use light_output::DmxFrame;
use light_programmer::{HighlightOutputLayer, HighlightOutputRole};
use std::collections::{HashMap, HashSet};

mod head_overlay;
mod head_values;

use head_overlay::{HeadOverlayState, channel_matches_attribute, seed_native_candidate};
pub(crate) use head_values::HeadValueStore;
use head_values::HeadValueView;

// @tour fixture-semantics:30 Resolve semantic values for every logical head
// Rendering binds the compiled mode plan, resolves each included logical head, and produces
// channel values plus visualization output without consulting the fixture library.

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_profile_fixture(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    projection: &FixtureProjectionPlan,
    included_splits: Option<&[u16]>,
    values: &ProfileValueIndex<'_>,
    options: RenderOptions,
    highlight_layers: &HashMap<FixtureId, HighlightOutputLayer>,
    highlight_look: &HighlightLook,
    axis_inversion: AxisInversion,
    instance_id: uuid::Uuid,
    native_channels: Option<&[Option<crate::native_position_projection::NativePositionInput>]>,
    // Filled rather than returned: a render resolves every fixture in turn and would otherwise
    // grow two vectors per fixture per frame.
    fixture_output: &mut ResolvedProfileFixtureOutput,
) -> Result<(), EngineError> {
    let resolution = projection
        .resolution()
        .bind(mode)
        .map_err(|error| EngineError::Invalid(error.to_string()))?;
    fixture_output.heads.clear();
    fixture_output.channels.clear();
    fixture_output.color_writes.clear();
    if fixture_output.track_color_writes {
        fixture_output.native_active_attributes.clear();
        fixture_output
            .native_active_attributes
            .resize(mode.channels.len(), None);
    }
    let frozen_channels = projection.position_freeze_inputs(instance_id);
    for head in projection
        .heads()
        .iter()
        .filter(|head| included_splits.is_none_or(|splits| head.appears_in_any_split(splits)))
    {
        let head_output = resolve_profile_head(
            fixture,
            mode,
            head,
            &resolution,
            values,
            options,
            highlight_layers,
            highlight_look,
            axis_inversion,
            native_channels,
            frozen_channels,
            &mut fixture_output.channels,
            fixture_output
                .track_color_writes
                .then_some(&mut fixture_output.color_writes),
            fixture_output
                .track_color_writes
                .then_some(fixture_output.native_active_attributes.as_mut_slice()),
        )?;
        fixture_output.heads.push(head_output);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct AxisInversion {
    pub(crate) pan: bool,
    pub(crate) tilt: bool,
}

impl AxisInversion {
    /// Installation inversion follows the head's cold compiled Position role first (TL-630), so a
    /// motor alias bound to Pan or Tilt mirrors with its physical prediction. Attributes without a
    /// compiled role keep the legacy canonical `pan`/`tilt` rule; nothing else is guessed by name.
    fn applies(self, head: &ProfileHeadPlan, attribute: &AttributeKey) -> bool {
        let (pan, tilt) = match head.axis_role(attribute) {
            Some(light_fixture::PositionAxisRole::Pan) => (true, false),
            Some(light_fixture::PositionAxisRole::Tilt) => (false, true),
            None => (
                attribute.0.eq_ignore_ascii_case("pan"),
                attribute.0.eq_ignore_ascii_case("tilt"),
            ),
        };
        (self.pan && pan) || (self.tilt && tilt)
    }

    fn any(self) -> bool {
        self.pan || self.tilt
    }
}

#[derive(Default)]
pub(crate) struct ResolvedProfileFixtureOutput {
    pub(crate) heads: Vec<ResolvedProfileHeadOutput>,
    /// Resolved raw values, each knowing which channel of the mode it is.
    ///
    /// Its position, not its identity: encoding finds where the bytes go by indexing rather than
    /// by hashing a Uuid twice, and the batch is half the size in memory.
    pub(crate) channels: Vec<(u32, u32)>,
    /// Observer-only trace of actual semantic Color writes, independent of value equality.
    pub(crate) color_writes: Vec<(FixtureId, usize)>,
    pub(crate) track_color_writes: bool,
    pub(crate) native_active_attributes: Vec<Option<AttributeKey>>,
}

impl crate::Reusable for ResolvedProfileFixtureOutput {
    fn reset(&mut self) {
        self.heads.clear();
        self.channels.clear();
        self.color_writes.clear();
        self.track_color_writes = false;
        self.native_active_attributes.clear();
    }
}

pub(crate) struct ResolvedProfileHeadOutput {
    pub(crate) owner: FixtureId,
    pub(crate) intensity: f32,
    pub(crate) color: Option<Xyz>,
}

struct ProfileHeadInputs<'v, 'a> {
    owner: FixtureId,
    head_id: uuid::Uuid,
    output_highlighted: bool,
    legacy_raw_highlight: bool,
    semantic_highlight_color: Option<HighlightColor>,
    suppressed_highlight_attributes: HashSet<AttributeKey>,
    /// Read through the frame's row of this head; only the head's own writes are held here.
    values: HeadValueView<'v, 'a>,
    held_native: bool,
}

fn look_for_role(role: HighlightOutputRole, highlight_look: &HighlightLook) -> HighlightLook {
    match role {
        HighlightOutputRole::Highlight => highlight_look.clone(),
        HighlightOutputRole::LowLight => HighlightLook {
            intensity: 0.1,
            color: Some(HighlightColor::Blue),
            ..HighlightLook::default()
        },
    }
}

fn resolved_highlight_layer(
    fixture_id: FixtureId,
    owner: FixtureId,
    layers: &HashMap<FixtureId, HighlightOutputLayer>,
) -> Option<HighlightOutputLayer> {
    if layers.is_empty() {
        return None;
    }
    let root = layers.get(&fixture_id).cloned();
    if owner == fixture_id {
        return root;
    }
    let head = layers.get(&owner).cloned();
    match (root, head) {
        (None, layer) | (layer, None) => layer,
        (Some(root), Some(head)) if root.role > head.role => Some(root),
        (Some(_), Some(head)) if head.role > HighlightOutputRole::LowLight => Some(head),
        (Some(mut root), Some(head)) => {
            root.suppressed_attributes
                .retain(|attribute| head.suppressed_attributes.contains(attribute));
            Some(root)
        }
    }
}

fn is_highlight_attribute_suppressed(inputs: &ProfileHeadInputs<'_, '_>, name: &str) -> bool {
    inputs
        .suppressed_highlight_attributes
        .iter()
        .any(|attribute| attribute.0.eq_ignore_ascii_case(name))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_profile_head(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    head: &ProfileHeadPlan,
    resolution: &BoundFixtureModeResolution<'_>,
    values: &ProfileValueIndex<'_>,
    options: RenderOptions,
    highlight_layers: &HashMap<FixtureId, HighlightOutputLayer>,
    highlight_look: &HighlightLook,
    axis_inversion: AxisInversion,
    native_channels: Option<&[Option<crate::native_position_projection::NativePositionInput>]>,
    frozen_channels: Option<&[Option<crate::native_position_projection::NativePositionInput>]>,
    channels: &mut Vec<(u32, u32)>,
    color_writes: Option<&mut Vec<(FixtureId, usize)>>,
    active_attributes: Option<&mut [Option<AttributeKey>]>,
) -> Result<ResolvedProfileHeadOutput, EngineError> {
    let owner = head.owner;
    let HeadOverlayState {
        options,
        output_highlighted,
        selected_look,
        legacy_raw_highlight,
        ..
    } = HeadOverlayState::resolve(
        fixture,
        owner,
        false,
        options,
        highlight_layers,
        highlight_look,
    );
    // Colour, level and the level's master together: three questions every head asks, answered
    // from its row in one go rather than by matching names against its whole attribute list.
    let common = values.common(owner);
    let borrowed_requested_color = common.color.and_then(|value| match value {
        AttributeValue::ColorXyz(color) => Some(*color),
        _ => None,
    });
    let no_position_inputs = |inputs: Option<&[Option<_>]>| {
        inputs.is_none_or(|inputs| {
            head.channel_indices
                .iter()
                .all(|&index| inputs[index].is_none())
        })
    };
    if no_position_inputs(native_channels)
        && no_position_inputs(frozen_channels)
        && options.control_loss_progress.is_none()
        && !(fixture.definition.hazardous && options.blackout)
        && borrowed_requested_color.is_none()
        && !axis_inversion.any()
        && (!output_highlighted || legacy_raw_highlight)
    {
        return Ok(resolve_head_without_overlays(
            HeadFastPath {
                fixture,
                mode,
                head,
                resolution,
                values,
                options,
                common,
                output_highlighted,
                legacy_raw_highlight,
                selected_look: selected_look.as_ref(),
            },
            channels,
            active_attributes,
        ));
    }

    let mut inputs = prepare_head_inputs(
        fixture,
        mode,
        head,
        values,
        options,
        highlight_layers,
        highlight_look,
        axis_inversion,
        None,
    )?;
    let virtual_intensity = virtual_intensity(&inputs, head.intensity_default);
    let requested_color = requested_color(&inputs.values);
    let mut color_attributes = Vec::new();
    resolve_requested_color(
        mode,
        &mut inputs,
        requested_color,
        options.color_model,
        color_writes.as_ref().map(|_| &mut color_attributes),
    )?;
    let channel_start = channels.len();
    resolve_channels(
        ChannelResolutionContext {
            fixture,
            mode,
            head,
            resolution,
            inputs: &inputs,
            color_attributes: &color_attributes,
            virtual_intensity,
            options,
            native_channels,
            frozen_channels,
            source_values: values,
            axis_inversion,
            highlight_layers,
            highlight_look,
        },
        channels,
        color_writes,
        active_attributes,
    )?;
    Ok(finalize_output(
        ProfileOutputContext {
            mode,
            head,
            owner: inputs.owner,
            head_id: inputs.head_id,
            virtual_intensity,
            requested_color,
            options,
        },
        &channels[channel_start..],
    ))
}

/// Everything the ordinary head needs, once the overlays have been ruled out.
struct HeadFastPath<'a> {
    fixture: &'a PatchedFixture,
    mode: &'a FixtureMode,
    head: &'a ProfileHeadPlan,
    resolution: &'a BoundFixtureModeResolution<'a>,
    values: &'a ProfileValueIndex<'a>,
    options: RenderOptions,
    common: crate::profile_value_index::HeadCommon<'a>,
    output_highlighted: bool,
    legacy_raw_highlight: bool,
    selected_look: Option<&'a HighlightLook>,
}

/// A head with no control loss, no hazardous blackout, no requested colour, no axis inversion and
/// no semantic Highlight — the state a desk is in almost all of the time. It reads its channels
/// straight through the numbering the patch compiled and never builds a map.
fn resolve_head_without_overlays(
    path: HeadFastPath<'_>,
    channels: &mut Vec<(u32, u32)>,
    mut active_attributes: Option<&mut [Option<AttributeKey>]>,
) -> ResolvedProfileHeadOutput {
    let HeadFastPath {
        fixture,
        mode,
        head,
        resolution,
        values,
        options,
        common,
        output_highlighted,
        legacy_raw_highlight,
        selected_look,
    } = path;
    let owner = head.owner;
    let channel_start = channels.len();
    let virtual_intensity = if output_highlighted {
        selected_look.map_or(1.0, |look| look.intensity)
    } else {
        common
            .intensity
            .and_then(AttributeValue::normalized)
            .unwrap_or(head.intensity_default)
    };
    let highlight_master = grand_master(fixture, options);
    // Where this head's channels read from, worked out when the patch compiled. A lookup is an
    // array index; only an attribute the patch could not number falls back to its name.
    let head_read = values.head_read(owner);
    channels.extend(head.channel_indices.iter().map(|channel_index| {
        let channel = &mode.channels[*channel_index];
        let read = |which: ChannelAttribute, attribute: &AttributeKey| {
            values.value_at(head_read, *channel_index, which, attribute)
        };
        let resolved = resolution.resolve_channel_with(
            *channel_index,
            read,
            legacy_raw_highlight,
            (!fixture.highlight_overrides.is_empty())
                .then(|| fixture.highlight_overrides.get(&channel.id).copied())
                .flatten(),
            // Masters already scaled the head's level parameters before DMX; a channel reads them
            // only through its virtual intensity, and an intensity channel is that source.
            |active| ChannelScales {
                virtual_intensity: (!active.is_some_and(|active| active.is_intensity))
                    .then_some(virtual_intensity),
                highlight_master,
            },
        );
        if let Some(active) = active_attributes.as_mut() {
            active[*channel_index] = resolved.active_attribute.cloned();
        }
        let mut raw = resolved.raw;
        if options.blackout {
            raw = blackout_raw(mode, channel, raw);
        }
        (*channel_index as u32, raw)
    }));
    finalize_output(
        ProfileOutputContext {
            mode,
            head,
            owner,
            head_id: head.head_id,
            virtual_intensity,
            requested_color: None,
            options,
        },
        &channels[channel_start..],
    )
}

pub(crate) fn encode_profile_split(
    frame: &mut DmxFrame,
    encoding: &FixtureModeEncodingPlan,
    split: u16,
    address: u16,
    output: &ResolvedProfileFixtureOutput,
) -> Result<(), EngineError> {
    encoding
        .encode_split_by_index(frame, address, split, &output.channels)
        .map_err(|error| EngineError::Invalid(error.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn prepare_head_inputs<'v, 'a>(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    head: &ProfileHeadPlan,
    values: &'v ProfileValueIndex<'a>,
    options: RenderOptions,
    highlight_layers: &HashMap<FixtureId, HighlightOutputLayer>,
    highlight_look: &HighlightLook,
    axis_inversion: AxisInversion,
    native: Option<(
        usize,
        &crate::native_position_projection::NativePositionInput,
    )>,
) -> Result<ProfileHeadInputs<'v, 'a>, EngineError> {
    let owner = head.owner;
    let HeadOverlayState {
        options,
        layer,
        output_highlighted,
        selected_look,
        legacy_raw_highlight,
    } = HeadOverlayState::resolve(
        fixture,
        owner,
        native.is_some_and(|(_, value)| value.full_freeze),
        options,
        highlight_layers,
        highlight_look,
    );
    // An explicit scalar Freeze channel uses the already Freeze-overridden ordinary input.
    // This avoids a new canonical candidate defeating a held fixture-facing alias.
    let native = native.filter(|(index, input)| {
        if input.frozen {
            return true;
        }
        fixture.freeze.targets.get(&owner).is_none_or(|frozen| {
            !frozen
                .values
                .keys()
                .any(|attribute| channel_matches_attribute(&mode.channels[*index], attribute))
        })
    });
    let mut inputs = ProfileHeadInputs {
        owner,
        head_id: head.head_id,
        output_highlighted,
        legacy_raw_highlight,
        semantic_highlight_color: None,
        suppressed_highlight_attributes: layer
            .map(|layer| layer.suppressed_attributes)
            .unwrap_or_default(),
        // Native candidates replace this channel's complete input. Preserve virtual intensity
        // without cloning every head attribute once for every fitted motor channel.
        values: if native.is_some() {
            let mut local = HeadValueView::local();
            if let Some(intensity) = values.common(owner).intensity {
                local.insert(AttributeKey::intensity(), intensity.clone());
            }
            local
        } else {
            HeadValueView::over(values, owner)
        },
        held_native: native.is_some_and(|(_, input)| input.frozen),
    };
    let derived_attributes = match native {
        // TL-639 round 2: without control loss, a hazardous blackout or a Highlight look, seeding
        // a native candidate only replaces the channel's own attribute, and nothing reads the
        // derived list; skip building it.
        Some((index, native))
            if options.control_loss_progress.is_none()
                && !(fixture.definition.hazardous && options.blackout)
                && selected_look.is_none() =>
        {
            let attribute = &mode.channels[index].attribute;
            inputs.values.remove(attribute);
            inputs
                .values
                .insert(attribute.clone(), native.value.clone());
            Vec::new()
        }
        Some((index, native)) => {
            seed_native_candidate(fixture, mode, options, &mut inputs, index, native)
        }
        None => Vec::new(),
    };
    apply_control_loss(fixture, mode, options, &mut inputs);
    apply_hazardous_blackout(fixture, options, &mut inputs.values);
    apply_axis_inversion_to_view(axis_inversion, head, &mut inputs.values);
    if let Some(look) = selected_look.as_ref() {
        let mut written = Vec::new();
        apply_semantic_highlight(
            mode,
            head,
            look,
            grand_master(fixture, options),
            &mut inputs,
            native.is_some().then_some(&mut written),
        )?;
        if let Some((index, _)) = native
            && written
                .iter()
                .any(|attribute| channel_matches_attribute(&mode.channels[index], attribute))
        {
            for attribute in derived_attributes {
                if !written.contains(&attribute) {
                    inputs.values.remove(&attribute);
                }
            }
        }
    }
    Ok(inputs)
}

fn apply_semantic_highlight(
    mode: &FixtureMode,
    head: &ProfileHeadPlan,
    look: &HighlightLook,
    grand: f32,
    inputs: &mut ProfileHeadInputs<'_, '_>,
    mut written: Option<&mut Vec<AttributeKey>>,
) -> Result<(), EngineError> {
    if !inputs.output_highlighted || look.compatibility != HighlightLookCompatibility::Semantic {
        return Ok(());
    }
    if !inputs
        .suppressed_highlight_attributes
        .contains(AttributeKey::intensity_ref())
    {
        if let Some(written) = written.as_mut() {
            written.push(AttributeKey::intensity());
        }
        // Highlight replaces the Intensity parameter after the masters; the Grand Master is the
        // only master above it.
        inputs.values.insert(
            AttributeKey::intensity(),
            AttributeValue::Normalized(look.intensity * grand),
        );
    }
    let has_authored_shutter_open = head.channel_indices.iter().any(|index| {
        mode.channels[*index].functions.iter().any(|function| {
            function.attribute.0.eq_ignore_ascii_case("shutter")
                && matches!(
                    &function.behavior,
                    ChannelFunctionBehavior::Fixed { semantic_id, .. }
                        | ChannelFunctionBehavior::Indexed { semantic_id, .. }
                        if semantic_id.eq_ignore_ascii_case("open")
                )
        })
    });
    if look.shutter == HighlightShutterPolicy::Open
        && has_authored_shutter_open
        && !is_highlight_attribute_suppressed(inputs, "shutter")
    {
        if let Some(written) = written.as_mut() {
            written.push(AttributeKey("shutter".into()));
        }
        inputs.values.insert(
            AttributeKey("shutter".into()),
            AttributeValue::Discrete("open".into()),
        );
    }
    if let Some(color) = look.color
        && !is_highlight_attribute_suppressed(inputs, "color")
    {
        let supported = !mode
            .resolve_highlight_color(inputs.head_id, color)
            .map_err(|error| EngineError::Invalid(error.to_string()))?
            .is_empty();
        if supported {
            if let Some(written) = written.as_mut() {
                written.push(AttributeKey::color());
            }
            inputs.values.insert(
                AttributeKey::color(),
                AttributeValue::ColorXyz(color.to_xyz()),
            );
            inputs.semantic_highlight_color = Some(color);
        }
    }
    for (name, value) in [
        ("iris", look.iris),
        ("zoom", look.zoom),
        ("focus", look.focus),
        ("frost", look.frost),
    ] {
        if let Some(value) = value {
            if is_highlight_attribute_suppressed(inputs, name) {
                continue;
            }
            if let Some(written) = written.as_mut() {
                written.push(AttributeKey(name.into()));
            }
            inputs
                .values
                .insert(AttributeKey(name.into()), AttributeValue::Normalized(value));
        }
    }
    Ok(())
}

/// Mirror normalized Pan/Tilt inputs of one head, once each. Explicit raw values (including
/// fitted and Frozen native words) are already native to the installation and never change.
pub(crate) fn apply_axis_inversion(
    inversion: AxisInversion,
    head: &ProfileHeadPlan,
    values: &mut crate::HeadValues,
) {
    if !inversion.any() {
        return;
    }
    for (attribute, value) in values {
        if !inversion.applies(head, attribute) {
            continue;
        }
        if let AttributeValue::Normalized(normalized) = value {
            *normalized = 1.0 - normalized.clamp(0.0, 1.0);
        }
    }
}

/// [`apply_axis_inversion`] over a head read through its frame row.
fn apply_axis_inversion_to_view(
    inversion: AxisInversion,
    head: &ProfileHeadPlan,
    values: &mut HeadValueView<'_, '_>,
) {
    if !inversion.any() {
        return;
    }
    for attribute in values.keys() {
        if !inversion.applies(head, &attribute) {
            continue;
        }
        if let Some(AttributeValue::Normalized(normalized)) = values.get(&attribute) {
            let mirrored = 1.0 - normalized.clamp(0.0, 1.0);
            values.insert(attribute, AttributeValue::Normalized(mirrored));
        }
    }
}

fn apply_control_loss(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    options: RenderOptions,
    inputs: &mut ProfileHeadInputs<'_, '_>,
) {
    let Some(progress) = options.control_loss_progress else {
        return;
    };
    match fixture.definition.effective_signal_loss_policy() {
        SignalLossPolicy::HoldLast => {}
        SignalLossPolicy::ImmediateSafe => {
            apply_safe_values(&mut inputs.values, &fixture.definition.safe_values, 1.0)
        }
        SignalLossPolicy::FadeToSafe { .. } => apply_safe_values_with_snap(
            &mut inputs.values,
            &fixture.definition.safe_values,
            progress.clamp(0.0, 1.0),
            |attribute| mode.head_attribute_is_snap(inputs.head_id, attribute),
        ),
    }
}

fn apply_hazardous_blackout(
    fixture: &PatchedFixture,
    options: RenderOptions,
    values: &mut HeadValueView<'_, '_>,
) {
    if fixture.definition.hazardous && options.blackout {
        for (attribute, value) in &fixture.definition.safe_values {
            values.insert(attribute.clone(), value.clone());
        }
    }
}

/// The head's Intensity, or its profile default while none is resolved: consistent with the
/// output-parameter stage, which holds an unprogrammed level at its mastered default.
fn virtual_intensity(inputs: &ProfileHeadInputs<'_, '_>, default: f32) -> f32 {
    inputs
        .values
        .get(AttributeKey::intensity_ref())
        .and_then(AttributeValue::normalized)
        .unwrap_or(default)
}

fn requested_color(values: &HeadValueView<'_, '_>) -> Option<Xyz> {
    values
        .get(AttributeKey::color_ref())
        .and_then(|value| match value {
            AttributeValue::ColorXyz(color) => Some(*color),
            _ => None,
        })
}

fn resolve_requested_color(
    mode: &FixtureMode,
    inputs: &mut ProfileHeadInputs<'_, '_>,
    target: Option<Xyz>,
    model: ColorProgrammingModel,
    mut color_writes: Option<&mut Vec<AttributeKey>>,
) -> Result<(), EngineError> {
    let Some(target) = target else {
        return Ok(());
    };
    let resolved = match (inputs.semantic_highlight_color, model) {
        (Some(color), _) => mode.resolve_highlight_color(inputs.head_id, color),
        // Intent shows one chromaticity at the engine's full reach; Intensity does the dimming.
        (None, ColorProgrammingModel::Intent) => {
            Ok(mode.resolve_intent(inputs.head_id, target).channels)
        }
        (None, ColorProgrammingModel::Direct) => mode.resolve_color(inputs.head_id, target),
    }
    .map_err(|error| EngineError::Invalid(error.to_string()))?;
    for (channel_id, raw) in resolved {
        let Some(channel) = mode
            .channels
            .iter()
            .find(|channel| channel.id == channel_id)
        else {
            continue;
        };
        if inputs.values.contains_key(&channel.attribute) {
            continue;
        }
        if channel.behavior != light_fixture::ChannelBehavior::Static
            && let Some(writes) = color_writes.as_mut()
        {
            writes.push(channel.attribute.clone());
        }
        inputs
            .values
            .insert(channel.attribute.clone(), AttributeValue::RawDmxExact(raw));
    }
    Ok(())
}

struct ChannelResolutionContext<'a> {
    fixture: &'a PatchedFixture,
    mode: &'a FixtureMode,
    head: &'a ProfileHeadPlan,
    resolution: &'a BoundFixtureModeResolution<'a>,
    inputs: &'a ProfileHeadInputs<'a, 'a>,
    color_attributes: &'a [AttributeKey],
    virtual_intensity: f32,
    options: RenderOptions,
    native_channels: Option<&'a [Option<crate::native_position_projection::NativePositionInput>]>,
    frozen_channels: Option<&'a [Option<crate::native_position_projection::NativePositionInput>]>,
    source_values: &'a ProfileValueIndex<'a>,
    axis_inversion: AxisInversion,
    highlight_layers: &'a HashMap<FixtureId, HighlightOutputLayer>,
    highlight_look: &'a HighlightLook,
}

fn resolve_channels(
    context: ChannelResolutionContext<'_>,
    channels: &mut Vec<(u32, u32)>,
    mut color_writes: Option<&mut Vec<(FixtureId, usize)>>,
    mut active_attributes: Option<&mut [Option<AttributeKey>]>,
) -> Result<(), EngineError> {
    let highlight_master = grand_master(context.fixture, context.options);
    // TL-639 round 4: the overlay state a fitted native channel renders under depends only on
    // its full-Freeze flag, so it is resolved at most once per flag and head.
    let mut overlays: [Option<HeadOverlayState>; 2] = [None, None];
    for channel_index in context.head.channel_indices.iter() {
        let channel = &context.mode.channels[*channel_index];
        let native_inputs;
        let native = context
            .frozen_channels
            .and_then(|channels| channels[*channel_index].as_ref())
            .or_else(|| {
                context
                    .native_channels
                    .and_then(|channels| channels[*channel_index].as_ref())
            });
        let inputs = match native {
            Some(native) => {
                match NativeChannelInputs::plain(&context, &mut overlays, *channel_index, native) {
                    Some(plain) => ChannelInputs::Native(plain),
                    None => {
                        native_inputs = prepare_head_inputs(
                            context.fixture,
                            context.mode,
                            context.head,
                            context.source_values,
                            context.options,
                            context.highlight_layers,
                            context.highlight_look,
                            context.axis_inversion,
                            Some((*channel_index, native)),
                        )?;
                        ChannelInputs::Head(&native_inputs)
                    }
                }
            }
            None => ChannelInputs::Head(context.inputs),
        };
        let resolved = context.resolution.resolve_channel_with(
            *channel_index,
            |_, attribute| inputs.value(attribute),
            inputs.legacy_raw_highlight(),
            context
                .fixture
                .highlight_overrides
                .get(&channel.id)
                .copied(),
            |active| {
                if inputs.held_native() {
                    // Captured words are the frozen parameter's own exact output.
                    return ChannelScales::default();
                }
                // Masters already scaled the head's level parameters before DMX; a channel
                // reads them only through its virtual intensity, and an intensity channel is
                // that source.
                ChannelScales {
                    virtual_intensity: (!active.is_some_and(|active| active.key.is_intensity()))
                        .then_some(context.virtual_intensity),
                    highlight_master,
                }
            },
        );
        if resolved
            .active_attribute
            .is_some_and(|attribute| context.color_attributes.contains(attribute))
            && let Some(writes) = color_writes.as_mut()
        {
            writes.push((context.inputs.owner, *channel_index));
        }
        if let Some(active) = active_attributes.as_mut() {
            active[*channel_index] = resolved.active_attribute.cloned();
        }
        let mut raw = resolved.raw;
        if context.options.blackout && !native.is_some_and(|input| input.full_freeze) {
            raw = blackout_raw(context.mode, channel, raw);
        }
        channels.push((*channel_index as u32, raw));
    }
    Ok(())
}

/// What one channel resolves from: its head's inputs, or a fitted native channel's own inputs.
enum ChannelInputs<'i, 'a> {
    Head(&'i ProfileHeadInputs<'a, 'a>),
    Native(NativeChannelInputs<'a>),
}

impl ChannelInputs<'_, '_> {
    fn value(&self, attribute: &AttributeKey) -> Option<&AttributeValue> {
        match self {
            Self::Head(inputs) => inputs.values.get(attribute),
            Self::Native(native) => native.value(attribute),
        }
    }

    fn legacy_raw_highlight(&self) -> bool {
        match self {
            Self::Head(inputs) => inputs.legacy_raw_highlight,
            Self::Native(native) => native.legacy_raw_highlight,
        }
    }

    fn held_native(&self) -> bool {
        match self {
            Self::Head(inputs) => inputs.held_native,
            Self::Native(native) => native.held,
        }
    }
}

/// The inputs `prepare_head_inputs` builds for a fitted native channel when no control loss,
/// hazardous Blackout, Highlight look or axis inversion acts on the head, and the channel's
/// candidate survives the scalar Freeze check (TL-639 round 4). Such a channel reads exactly its
/// own native value under its attribute and the head's Intensity, and nothing else; it is
/// answered here without building the per-channel input set.
struct NativeChannelInputs<'a> {
    attribute: &'a AttributeKey,
    value: &'a AttributeValue,
    intensity: Option<&'a AttributeValue>,
    held: bool,
    legacy_raw_highlight: bool,
}

impl<'a> NativeChannelInputs<'a> {
    fn plain(
        context: &ChannelResolutionContext<'a>,
        overlays: &mut [Option<HeadOverlayState>; 2],
        index: usize,
        native: &'a crate::native_position_projection::NativePositionInput,
    ) -> Option<Self> {
        let owner = context.head.owner;
        let overlay = overlays[usize::from(native.full_freeze)].get_or_insert_with(|| {
            HeadOverlayState::resolve(
                context.fixture,
                owner,
                native.full_freeze,
                context.options,
                context.highlight_layers,
                context.highlight_look,
            )
        });
        let channel = &context.mode.channels[index];
        let candidate_kept = native.frozen
            || context
                .fixture
                .freeze
                .targets
                .get(&owner)
                .is_none_or(|frozen| {
                    !frozen
                        .values
                        .keys()
                        .any(|attribute| channel_matches_attribute(channel, attribute))
                });
        let plain = candidate_kept
            && overlay.options.control_loss_progress.is_none()
            && !(context.fixture.definition.hazardous && overlay.options.blackout)
            && overlay.selected_look.is_none()
            && !context.axis_inversion.any();
        let values = context.source_values;
        plain.then(|| Self {
            attribute: &channel.attribute,
            value: &native.value,
            intensity: values.common(owner).intensity,
            held: native.frozen,
            legacy_raw_highlight: overlay.legacy_raw_highlight,
        })
    }

    fn value(&self, attribute: &AttributeKey) -> Option<&AttributeValue> {
        if attribute == self.attribute {
            Some(self.value)
        } else if *attribute.0 == *"intensity" {
            self.intensity
        } else {
            None
        }
    }
}

fn grand_master(fixture: &PatchedFixture, options: RenderOptions) -> f32 {
    if options.blackout {
        0.0
    } else if !fixture.grand_master_enabled {
        1.0
    } else {
        options.grand_master.clamp(0.0, 1.0)
    }
}

struct ProfileOutputContext<'a> {
    mode: &'a FixtureMode,
    head: &'a ProfileHeadPlan,
    owner: FixtureId,
    head_id: uuid::Uuid,
    virtual_intensity: f32,
    requested_color: Option<Xyz>,
    options: RenderOptions,
}

fn finalize_output(
    context: ProfileOutputContext<'_>,
    channels: &[(u32, u32)],
) -> ResolvedProfileHeadOutput {
    let physical_intensity = context
        .head
        .intensity_channel_indices
        .iter()
        .filter_map(|index| channel_visual_level(context.mode, channels, *index as u32))
        .reduce(f32::max);
    let mut color = profile_visual_color(
        context.mode,
        context.head_id,
        channels,
        context.requested_color,
    );
    let intensity = physical_intensity.unwrap_or_else(|| {
        visual_intensity(&mut color, context.virtual_intensity, context.options)
    });
    ResolvedProfileHeadOutput {
        owner: context.owner,
        intensity,
        color,
    }
}

fn visual_intensity(
    color: &mut Option<Xyz>,
    virtual_intensity: f32,
    options: RenderOptions,
) -> f32 {
    if options.blackout {
        return 0.0;
    }
    let brightness = color
        .map(|value| value.x.max(value.y).max(value.z).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    if brightness > f32::EPSILON {
        *color = color.map(|value| Xyz {
            x: value.x / brightness,
            y: value.y / brightness,
            z: value.z / brightness,
        });
        brightness
    } else {
        virtual_intensity
    }
}
