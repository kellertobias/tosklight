use crate::contribution::ApplicableSequenceMaster;
use crate::profile_projection_plan::{FixtureProjectionPlan, ProfileHeadPlan};
use crate::{
    EngineError, GroupMasterIndex, ProfileValueIndex, RenderOptions, apply_safe_values,
    apply_safe_values_with_snap, blackout_raw, channel_visual_level, profile_visual_color,
};
use light_core::{AttributeKey, AttributeValue, FixtureId, Xyz};
use light_fixture::ChannelAttribute;
use light_fixture::{
    BoundFixtureModeResolution, ChannelFunctionBehavior, ChannelScales, FixtureChannel,
    FixtureMode, FixtureModeEncodingPlan, HighlightColor, HighlightLook,
    HighlightLookCompatibility, HighlightShutterPolicy, PatchedFixture, SignalLossPolicy,
};
use light_output::DmxFrame;
use light_programmer::{HighlightOutputLayer, HighlightOutputRole};
use std::collections::{HashMap, HashSet};

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
    group_masters: &GroupMasterIndex,
    group_master_flashes: &HashMap<String, f32>,
    highlight_layers: &HashMap<FixtureId, HighlightOutputLayer>,
    highlight_look: &HighlightLook,
    axis_inversion: AxisInversion,
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
            group_masters,
            group_master_flashes,
            highlight_layers,
            highlight_look,
            axis_inversion,
            &mut fixture_output.channels,
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
    fn applies(self, attribute: &AttributeKey) -> bool {
        (self.pan && attribute.0.eq_ignore_ascii_case("pan"))
            || (self.tilt && attribute.0.eq_ignore_ascii_case("tilt"))
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
}

pub(crate) struct ResolvedProfileHeadOutput {
    pub(crate) owner: FixtureId,
    pub(crate) intensity: f32,
    pub(crate) color: Option<Xyz>,
}

struct ProfileHeadInputs {
    owner: FixtureId,
    head_id: uuid::Uuid,
    output_highlighted: bool,
    legacy_raw_highlight: bool,
    semantic_highlight_color: Option<HighlightColor>,
    suppressed_highlight_attributes: HashSet<AttributeKey>,
    group_scale: f32,
    /// Hashed for speed rather than against an adversary: a head's values are read several times
    /// per channel and never arrive from outside this desk.
    values: crate::HeadValues,
    sequence_masters: crate::HeadSequenceMasters,
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

fn is_highlight_attribute_suppressed(inputs: &ProfileHeadInputs, name: &str) -> bool {
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
    group_masters: &GroupMasterIndex,
    group_master_flashes: &HashMap<String, f32>,
    highlight_layers: &HashMap<FixtureId, HighlightOutputLayer>,
    highlight_look: &HighlightLook,
    axis_inversion: AxisInversion,
    channels: &mut Vec<(u32, u32)>,
) -> Result<ResolvedProfileHeadOutput, EngineError> {
    let owner = head.owner;
    // Nothing frozen, nothing highlighted and nothing flashing is the ordinary state of a desk, so
    // each of these asks whether there is anything to look up before hashing this head's identity.
    let full_freeze = !fixture.freeze.targets.is_empty()
        && fixture
            .freeze
            .targets
            .get(&owner)
            .is_some_and(|target| target.full);
    let options = if full_freeze {
        RenderOptions {
            grand_master: 1.0,
            blackout: false,
            control_loss_progress: None,
        }
    } else {
        options
    };
    let layer = (!full_freeze)
        .then(|| resolved_highlight_layer(fixture.fixture_id, owner, highlight_layers))
        .flatten();
    let output_highlighted = layer.is_some() && !(fixture.definition.hazardous && options.blackout);
    let selected_look = layer
        .as_ref()
        .map(|layer| look_for_role(layer.role, highlight_look));
    let legacy_raw_highlight = output_highlighted
        && selected_look
            .as_ref()
            .is_some_and(|look| look.compatibility != HighlightLookCompatibility::Semantic);
    let group_scale = if full_freeze || output_highlighted || !fixture.group_masters_enabled {
        1.0
    } else {
        group_masters.scale(owner, group_master_flashes)
    };
    // Colour, level and the level's master together: three questions every head asks, answered
    // from its row in one go rather than by matching names against its whole attribute list.
    let common = values.common(owner);
    let borrowed_requested_color = common.color.and_then(|value| match value {
        AttributeValue::ColorXyz(color) => Some(*color),
        _ => None,
    });
    if options.control_loss_progress.is_none()
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
                group_scale,
                output_highlighted,
                legacy_raw_highlight,
                selected_look: selected_look.as_ref(),
            },
            channels,
        ));
    }

    let mut inputs = prepare_head_inputs(
        fixture,
        mode,
        head,
        values,
        options,
        group_masters,
        group_master_flashes,
        highlight_layers,
        highlight_look,
        axis_inversion,
    )?;
    let virtual_intensity = virtual_intensity(&inputs);
    let requested_color = requested_color(&inputs.values);
    resolve_requested_color(mode, &mut inputs, requested_color)?;
    let channel_start = channels.len();
    resolve_channels(
        ChannelResolutionContext {
            fixture,
            mode,
            head,
            resolution,
            inputs: &inputs,
            virtual_intensity,
            options,
        },
        channels,
    );
    Ok(finalize_output(
        ProfileOutputContext {
            fixture,
            mode,
            head,
            owner: inputs.owner,
            head_id: inputs.head_id,
            group_scale: inputs.group_scale,
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
    group_scale: f32,
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
) -> ResolvedProfileHeadOutput {
    let HeadFastPath {
        fixture,
        mode,
        head,
        resolution,
        values,
        options,
        common,
        group_scale,
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
            .unwrap_or(1.0)
    };
    let intensity_master = common.intensity_master;
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
            |active| {
                let sequence_master = active
                    .filter(|(_, attribute)| !attribute.is_intensity())
                    .and_then(|(which, attribute)| {
                        values.sequence_master_at(head_read, *channel_index, which, attribute)
                    })
                    .filter(|master| {
                        !channel.reacts_to_virtual_intensity
                            || intensity_master
                                .is_none_or(|intensity| intensity.source != master.source)
                    })
                    .map(|master| master.scale)
                    .unwrap_or(1.0);
                ChannelScales {
                    virtual_intensity: if active
                        .is_some_and(|(_, attribute)| attribute.is_intensity())
                    {
                        1.0
                    } else {
                        virtual_intensity
                    },
                    sequence_master,
                    group_master: group_scale,
                    grand_master: grand_master(fixture, options),
                }
            },
        );
        let mut raw = resolved.raw;
        if options.blackout {
            raw = blackout_raw(mode, channel, raw);
        }
        (*channel_index as u32, raw)
    }));
    finalize_output(
        ProfileOutputContext {
            fixture,
            mode,
            head,
            owner,
            head_id: head.head_id,
            group_scale,
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
fn prepare_head_inputs(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    head: &ProfileHeadPlan,
    values: &ProfileValueIndex<'_>,
    options: RenderOptions,
    group_masters: &GroupMasterIndex,
    group_master_flashes: &HashMap<String, f32>,
    highlight_layers: &HashMap<FixtureId, HighlightOutputLayer>,
    highlight_look: &HighlightLook,
    axis_inversion: AxisInversion,
) -> Result<ProfileHeadInputs, EngineError> {
    let owner = head.owner;
    // Nothing frozen, nothing highlighted and nothing flashing is the ordinary state of a desk, so
    // each of these asks whether there is anything to look up before hashing this head's identity.
    let full_freeze = !fixture.freeze.targets.is_empty()
        && fixture
            .freeze
            .targets
            .get(&owner)
            .is_some_and(|target| target.full);
    let options = if full_freeze {
        RenderOptions {
            grand_master: 1.0,
            blackout: false,
            control_loss_progress: None,
        }
    } else {
        options
    };
    let layer = (!full_freeze)
        .then(|| resolved_highlight_layer(fixture.fixture_id, owner, highlight_layers))
        .flatten();
    let output_highlighted = layer.is_some() && !(fixture.definition.hazardous && options.blackout);
    let selected_look = layer
        .as_ref()
        .map(|layer| look_for_role(layer.role, highlight_look));
    let legacy_raw_highlight = output_highlighted
        && selected_look
            .as_ref()
            .is_some_and(|look| look.compatibility != HighlightLookCompatibility::Semantic);
    let group_scale = if full_freeze || output_highlighted || !fixture.group_masters_enabled {
        1.0
    } else {
        group_masters.scale(owner, group_master_flashes)
    };
    let mut inputs = ProfileHeadInputs {
        owner,
        head_id: head.head_id,
        output_highlighted,
        legacy_raw_highlight,
        semantic_highlight_color: None,
        suppressed_highlight_attributes: layer
            .map(|layer| layer.suppressed_attributes)
            .unwrap_or_default(),
        group_scale,
        values: values.values(owner),
        sequence_masters: values.sequence_masters(owner),
    };
    apply_control_loss(fixture, mode, options, &mut inputs);
    apply_hazardous_blackout(fixture, options, &mut inputs.values);
    apply_axis_inversion(axis_inversion, &mut inputs.values);
    if let Some(look) = selected_look.as_ref() {
        apply_semantic_highlight(mode, head, look, &mut inputs)?;
    }
    Ok(inputs)
}

fn apply_semantic_highlight(
    mode: &FixtureMode,
    head: &ProfileHeadPlan,
    look: &HighlightLook,
    inputs: &mut ProfileHeadInputs,
) -> Result<(), EngineError> {
    if !inputs.output_highlighted || look.compatibility != HighlightLookCompatibility::Semantic {
        return Ok(());
    }
    if !inputs
        .suppressed_highlight_attributes
        .contains(&AttributeKey::intensity())
    {
        inputs.values.insert(
            AttributeKey::intensity(),
            AttributeValue::Normalized(look.intensity),
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
            inputs.values.insert(
                AttributeKey("color".into()),
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
            inputs
                .values
                .insert(AttributeKey(name.into()), AttributeValue::Normalized(value));
        }
    }
    Ok(())
}

fn apply_axis_inversion(inversion: AxisInversion, values: &mut crate::HeadValues) {
    for (attribute, value) in values {
        if !inversion.applies(attribute) {
            continue;
        }
        if let AttributeValue::Normalized(normalized) = value {
            *normalized = 1.0 - normalized.clamp(0.0, 1.0);
        }
    }
}

fn apply_control_loss(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    options: RenderOptions,
    inputs: &mut ProfileHeadInputs,
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
    values: &mut crate::HeadValues,
) {
    if fixture.definition.hazardous && options.blackout {
        for (attribute, value) in &fixture.definition.safe_values {
            values.insert(attribute.clone(), value.clone());
        }
    }
}

fn virtual_intensity(inputs: &ProfileHeadInputs) -> f32 {
    inputs
        .values
        .get(&AttributeKey::intensity())
        .and_then(AttributeValue::normalized)
        .unwrap_or(1.0)
}

fn requested_color(values: &crate::HeadValues) -> Option<Xyz> {
    values
        .get(&AttributeKey("color".into()))
        .and_then(|value| match value {
            AttributeValue::ColorXyz(color) => Some(*color),
            _ => None,
        })
}

fn resolve_requested_color(
    mode: &FixtureMode,
    inputs: &mut ProfileHeadInputs,
    target: Option<Xyz>,
) -> Result<(), EngineError> {
    let Some(target) = target else {
        return Ok(());
    };
    let color_attribute = AttributeKey("color".into());
    let color_master = inputs.sequence_masters.get(&color_attribute).copied();
    let resolved = match inputs.semantic_highlight_color {
        Some(color) => mode.resolve_highlight_color(inputs.head_id, color),
        None => mode.resolve_color(inputs.head_id, target),
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
        inputs
            .values
            .insert(channel.attribute.clone(), AttributeValue::RawDmxExact(raw));
        if let Some(master) = color_master {
            inputs
                .sequence_masters
                .insert(channel.attribute.clone(), master);
        }
    }
    Ok(())
}

struct ChannelResolutionContext<'a> {
    fixture: &'a PatchedFixture,
    mode: &'a FixtureMode,
    head: &'a ProfileHeadPlan,
    resolution: &'a BoundFixtureModeResolution<'a>,
    inputs: &'a ProfileHeadInputs,
    virtual_intensity: f32,
    options: RenderOptions,
}

fn resolve_channels(context: ChannelResolutionContext<'_>, channels: &mut Vec<(u32, u32)>) {
    let intensity_master = context
        .inputs
        .sequence_masters
        .get(&AttributeKey::intensity())
        .copied();
    channels.extend(context.head.channel_indices.iter().map(|channel_index| {
        let channel = &context.mode.channels[*channel_index];
        let resolved = context.resolution.resolve_channel(
            *channel_index,
            &context.inputs.values,
            context.inputs.legacy_raw_highlight,
            context
                .fixture
                .highlight_overrides
                .get(&channel.id)
                .copied(),
            |active| {
                let sequence_master =
                    sequence_master_scale(channel, active, context.inputs, intensity_master);
                let channel_intensity = if active.is_some_and(AttributeKey::is_intensity) {
                    1.0
                } else {
                    context.virtual_intensity
                };
                ChannelScales {
                    virtual_intensity: channel_intensity,
                    sequence_master,
                    group_master: context.inputs.group_scale,
                    grand_master: grand_master(context.fixture, context.options),
                }
            },
        );
        let mut raw = resolved.raw;
        if context.options.blackout {
            raw = blackout_raw(context.mode, channel, raw);
        }
        (*channel_index as u32, raw)
    }))
}

fn sequence_master_scale(
    channel: &FixtureChannel,
    active: Option<&AttributeKey>,
    inputs: &ProfileHeadInputs,
    intensity: Option<ApplicableSequenceMaster>,
) -> f32 {
    active
        .filter(|attribute| !attribute.is_intensity())
        .and_then(|attribute| inputs.sequence_masters.get(attribute).copied())
        .filter(|master| {
            !channel.reacts_to_virtual_intensity
                || intensity.is_none_or(|intensity| intensity.source != master.source)
        })
        .map(|master| master.scale)
        .unwrap_or(1.0)
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
    fixture: &'a PatchedFixture,
    mode: &'a FixtureMode,
    head: &'a ProfileHeadPlan,
    owner: FixtureId,
    head_id: uuid::Uuid,
    group_scale: f32,
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
        visual_intensity(
            context.fixture,
            &mut color,
            context.virtual_intensity,
            context.group_scale,
            context.options,
        )
    });
    ResolvedProfileHeadOutput {
        owner: context.owner,
        intensity,
        color,
    }
}

fn visual_intensity(
    fixture: &PatchedFixture,
    color: &mut Option<Xyz>,
    virtual_intensity: f32,
    group_scale: f32,
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
        virtual_intensity * group_scale * grand_master(fixture, options)
    }
}
