use crate::contribution::ApplicableSequenceMaster;
use crate::{
    EngineError, RenderOptions, ResolvedAttributes, apply_safe_values, apply_safe_values_with_snap,
    blackout_raw, channel_visual_level, group_scale_for_fixture, profile_head_owner,
    profile_visual_color,
};
use light_core::{AttributeKey, AttributeValue, FixtureId, Xyz};
use light_fixture::{ChannelScales, FixtureChannel, FixtureMode, PatchedFixture, SignalLossPolicy};
use light_output::DmxFrame;
use light_programmer::GroupDefinition;
use std::collections::{HashMap, HashSet};

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_profile_split(
    frame: &mut DmxFrame,
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    split: u16,
    address: u16,
    resolved: &ResolvedAttributes,
    options: RenderOptions,
    groups: &HashMap<String, GroupDefinition>,
    group_master_flashes: &HashMap<String, f32>,
    highlighted_fixtures: &HashSet<FixtureId>,
) -> Result<(), EngineError> {
    for (head_index, _) in mode.heads.iter().enumerate().filter(|(_, head)| {
        mode.channels
            .iter()
            .any(|channel| channel.head_id == head.id && channel.split == split)
    }) {
        let output = resolve_profile_head(
            fixture,
            mode,
            head_index,
            resolved,
            options,
            groups,
            group_master_flashes,
            highlighted_fixtures,
        )?;
        encode_split_channels(frame, mode, split, address, output.channels)?;
    }
    Ok(())
}

pub(crate) struct ResolvedProfileHeadOutput {
    pub(crate) owner: FixtureId,
    pub(crate) channels: Vec<(uuid::Uuid, u32)>,
    pub(crate) intensity: f32,
    pub(crate) color: Option<Xyz>,
}

struct ProfileHeadInputs {
    owner: FixtureId,
    head_id: uuid::Uuid,
    output_highlighted: bool,
    group_scale: f32,
    values: HashMap<AttributeKey, AttributeValue>,
    sequence_masters: HashMap<AttributeKey, ApplicableSequenceMaster>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_profile_head(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    head_index: usize,
    resolved: &ResolvedAttributes,
    options: RenderOptions,
    groups: &HashMap<String, GroupDefinition>,
    group_master_flashes: &HashMap<String, f32>,
    highlighted_fixtures: &HashSet<FixtureId>,
) -> Result<ResolvedProfileHeadOutput, EngineError> {
    let mut inputs = prepare_head_inputs(
        fixture,
        mode,
        head_index,
        resolved,
        options,
        groups,
        group_master_flashes,
        highlighted_fixtures,
    )?;
    let virtual_intensity = virtual_intensity(&inputs);
    let requested_color = requested_color(&inputs.values);
    resolve_requested_color(mode, &mut inputs, requested_color)?;
    let channels = resolve_channels(fixture, mode, &inputs, virtual_intensity, options);
    Ok(finalize_output(
        mode,
        inputs,
        channels,
        virtual_intensity,
        requested_color,
        options,
    ))
}

fn encode_split_channels(
    frame: &mut DmxFrame,
    mode: &FixtureMode,
    split: u16,
    address: u16,
    channels: Vec<(uuid::Uuid, u32)>,
) -> Result<(), EngineError> {
    for (channel_id, raw) in channels {
        let channel = mode
            .channels
            .iter()
            .find(|channel| channel.id == channel_id)
            .ok_or_else(|| EngineError::Invalid("resolved profile channel is missing".into()))?;
        if channel.split == split {
            mode.encode_channel(frame, address, channel, raw)
                .map_err(|error| EngineError::Invalid(error.to_string()))?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn prepare_head_inputs(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    head_index: usize,
    resolved: &ResolvedAttributes,
    options: RenderOptions,
    groups: &HashMap<String, GroupDefinition>,
    group_master_flashes: &HashMap<String, f32>,
    highlighted_fixtures: &HashSet<FixtureId>,
) -> Result<ProfileHeadInputs, EngineError> {
    let head = mode
        .heads
        .get(head_index)
        .ok_or_else(|| EngineError::Invalid("fixture profile head is missing".into()))?;
    let owner = profile_head_owner(fixture, head_index, head);
    let highlighted =
        highlighted_fixtures.contains(&fixture.fixture_id) || highlighted_fixtures.contains(&owner);
    let output_highlighted = highlighted && !(fixture.definition.hazardous && options.blackout);
    let group_scale = if output_highlighted {
        1.0
    } else {
        group_scale_for_fixture(owner, groups, group_master_flashes)
    };
    let mut inputs = ProfileHeadInputs {
        owner,
        head_id: head.id,
        output_highlighted,
        group_scale,
        values: fixture_values(&resolved.values, owner),
        sequence_masters: fixture_values(&resolved.sequence_masters, owner),
    };
    apply_control_loss(fixture, mode, options, &mut inputs);
    apply_hazardous_blackout(fixture, options, &mut inputs.values);
    Ok(inputs)
}

fn fixture_values<T: Clone>(
    values: &HashMap<(FixtureId, AttributeKey), T>,
    owner: FixtureId,
) -> HashMap<AttributeKey, T> {
    values
        .iter()
        .filter(|((fixture_id, _), _)| *fixture_id == owner)
        .map(|((_, attribute), value)| (attribute.clone(), value.clone()))
        .collect()
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
    values: &mut HashMap<AttributeKey, AttributeValue>,
) {
    if fixture.definition.hazardous && options.blackout {
        for (attribute, value) in &fixture.definition.safe_values {
            values.insert(attribute.clone(), value.clone());
        }
    }
}

fn virtual_intensity(inputs: &ProfileHeadInputs) -> f32 {
    if inputs.output_highlighted {
        1.0
    } else {
        inputs
            .values
            .get(&AttributeKey::intensity())
            .and_then(AttributeValue::normalized)
            .unwrap_or(1.0)
    }
}

fn requested_color(values: &HashMap<AttributeKey, AttributeValue>) -> Option<Xyz> {
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
    for (channel_id, raw) in mode
        .resolve_color(inputs.head_id, target)
        .map_err(|error| EngineError::Invalid(error.to_string()))?
    {
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

fn resolve_channels(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    inputs: &ProfileHeadInputs,
    virtual_intensity: f32,
    options: RenderOptions,
) -> Vec<(uuid::Uuid, u32)> {
    let intensity_master = inputs
        .sequence_masters
        .get(&AttributeKey::intensity())
        .copied();
    mode.channels
        .iter()
        .filter(|channel| channel.head_id == inputs.head_id)
        .map(|channel| {
            let active = mode.active_attribute_for_channel(channel, &inputs.values);
            let sequence_master = sequence_master_scale(channel, active, inputs, intensity_master);
            let channel_intensity = if active.is_some_and(AttributeKey::is_intensity) {
                1.0
            } else {
                virtual_intensity
            };
            let mut raw = mode.resolve_channel_raw(
                channel,
                &inputs.values,
                inputs.output_highlighted,
                fixture.highlight_overrides.get(&channel.id).copied(),
                ChannelScales {
                    virtual_intensity: channel_intensity,
                    sequence_master,
                    group_master: inputs.group_scale,
                    grand_master: grand_master(options),
                },
            );
            if options.blackout {
                raw = blackout_raw(mode, channel, raw);
            }
            (channel.id, raw)
        })
        .collect()
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

fn grand_master(options: RenderOptions) -> f32 {
    if options.blackout {
        0.0
    } else {
        options.grand_master.clamp(0.0, 1.0)
    }
}

fn finalize_output(
    mode: &FixtureMode,
    inputs: ProfileHeadInputs,
    channels: Vec<(uuid::Uuid, u32)>,
    virtual_intensity: f32,
    requested_color: Option<Xyz>,
    options: RenderOptions,
) -> ResolvedProfileHeadOutput {
    let channel_map = channels.iter().copied().collect::<HashMap<_, _>>();
    let physical_intensity = mode
        .channels
        .iter()
        .filter(|channel| channel.head_id == inputs.head_id && channel.attribute.is_intensity())
        .filter_map(|channel| channel_visual_level(mode, &channel_map, channel.id))
        .reduce(f32::max);
    let mut color = profile_visual_color(mode, inputs.head_id, &channel_map, requested_color);
    let intensity = physical_intensity.unwrap_or_else(|| {
        visual_intensity(&mut color, virtual_intensity, inputs.group_scale, options)
    });
    ResolvedProfileHeadOutput {
        owner: inputs.owner,
        channels,
        intensity,
        color,
    }
}

fn visual_intensity(
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
        virtual_intensity * group_scale * options.grand_master.clamp(0.0, 1.0)
    }
}
