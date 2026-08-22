use super::{
    ChannelBehavior, ChannelScales, FixtureMode, ProfileError,
    resolution::{ResolvedChannelRaw, function_value_for, mapped_canonical_raw, scale_channel_raw},
};
use light_core::{AttributeKey, AttributeValue};
use std::collections::HashMap;
use uuid::Uuid;

/// Immutable semantic lookup metadata for one validated fixture mode.
///
/// The plan keeps control-action keys and function priority order out of the frame loop. It must
/// be used with the same immutable mode from which it was compiled.
#[derive(Clone, Debug)]
pub struct FixtureModeResolutionPlan {
    mode_id: Uuid,
    channels: Box<[CompiledChannelResolution]>,
}

#[derive(Clone, Debug)]
struct CompiledChannelResolution {
    channel_id: Uuid,
    control_attribute: AttributeKey,
    functions_by_priority: Box<[usize]>,
}

/// One resolved physical channel and the semantic address which owns its sequence master.
#[derive(Clone, Copy, Debug)]
pub struct PlannedChannelResolution<'a> {
    pub active_attribute: Option<&'a AttributeKey>,
    pub raw: u32,
}

/// A resolution plan paired once with the immutable mode it was compiled from.
#[derive(Clone, Copy, Debug)]
pub struct BoundFixtureModeResolution<'a> {
    mode: &'a FixtureMode,
    plan: &'a FixtureModeResolutionPlan,
}

impl FixtureMode {
    pub fn compile_resolution_plan(&self) -> FixtureModeResolutionPlan {
        let channels = self
            .channels
            .iter()
            .map(|channel| {
                let mut functions = (0..channel.functions.len()).collect::<Vec<_>>();
                functions.sort_unstable_by(|left, right| {
                    channel.functions[*right]
                        .priority
                        .cmp(&channel.functions[*left].priority)
                        .then_with(|| left.cmp(right))
                });
                CompiledChannelResolution {
                    channel_id: channel.id,
                    control_attribute: Self::control_action_attribute(channel.id),
                    functions_by_priority: functions.into_boxed_slice(),
                }
            })
            .collect();
        FixtureModeResolutionPlan {
            mode_id: self.id,
            channels,
        }
    }
}

/// Which of a channel's attributes a resolution lookup is for.
///
/// A channel reads at most four kinds of thing, and which one is being asked for is known before
/// the show runs. Naming them lets a caller answer from a place it worked out when the patch
/// compiled instead of from the attribute's name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelAttribute {
    /// One of this channel's functions, by its index in `channel.functions`.
    Function(usize),
    /// The channel's canonical attribute.
    Canonical,
    /// The manufacturer attribute behind the canonical one.
    Fixture,
    /// The attribute that decides which of the channel's functions is in control.
    Control,
}

impl FixtureModeResolutionPlan {
    pub fn bind<'a>(
        &'a self,
        mode: &'a FixtureMode,
    ) -> Result<BoundFixtureModeResolution<'a>, ProfileError> {
        if mode.id != self.mode_id || mode.channels.len() != self.channels.len() {
            return Err(ProfileError::Invalid(
                "fixture resolution plan does not match its immutable mode".into(),
            ));
        }
        Ok(BoundFixtureModeResolution { mode, plan: self })
    }
}

impl BoundFixtureModeResolution<'_> {
    /// Resolve a channel without rebuilding its control key or rescanning functions twice.
    #[inline]
    /// Generic over the hasher: the caller decides how its own values are hashed, and a frame's
    /// values never arrive from outside the desk.
    pub fn resolve_channel<S: std::hash::BuildHasher>(
        &self,
        channel_index: usize,
        values: &HashMap<AttributeKey, AttributeValue, S>,
        highlighted: bool,
        highlight_override: Option<u32>,
        scales: impl FnOnce(Option<&AttributeKey>) -> ChannelScales,
    ) -> PlannedChannelResolution<'_> {
        let scales = |active: Option<(ChannelAttribute, &AttributeKey)>| {
            scales(active.map(|(_, attribute)| attribute))
        };
        self.resolve_channel_with(
            channel_index,
            |_, attribute| values.get(attribute),
            highlighted,
            highlight_override,
            scales,
        )
    }

    /// Resolve a channel from a borrowed semantic lookup without requiring a per-head owned map.
    ///
    /// The lookup is told which of the channel's attributes it is being asked for as well as its
    /// name. A caller that has already worked out where each of them lives can answer from that
    /// rather than from the name, which is the difference between an array index and a hash of a
    /// string for every channel of every fixture of every frame.
    #[inline]
    pub fn resolve_channel_with<'values>(
        &self,
        channel_index: usize,
        value: impl Fn(ChannelAttribute, &AttributeKey) -> Option<&'values AttributeValue>,
        highlighted: bool,
        highlight_override: Option<u32>,
        scales: impl FnOnce(Option<(ChannelAttribute, &AttributeKey)>) -> ChannelScales,
    ) -> PlannedChannelResolution<'_> {
        let channel = &self.mode.channels[channel_index];
        let compiled = &self.plan.channels[channel_index];
        debug_assert_eq!(channel.id, compiled.channel_id);
        let winning_function = compiled
            .functions_by_priority
            .iter()
            .filter_map(|index| {
                channel
                    .functions
                    .get(*index)
                    .map(|function| (*index, function))
            })
            .find_map(|(index, function)| {
                if let Some(found) = value(ChannelAttribute::Function(index), &function.attribute) {
                    function_value_for(function, Some(found), channel.canonical_transform).map(
                        |raw| {
                            (
                                (ChannelAttribute::Function(index), &function.attribute),
                                raw,
                            )
                        },
                    )
                } else if function.attribute == channel.attribute
                    && channel.fixture_attribute != channel.attribute
                {
                    function_value_for(
                        function,
                        value(ChannelAttribute::Fixture, &channel.fixture_attribute),
                        super::CanonicalTransform::Identity,
                    )
                    .map(|raw| ((ChannelAttribute::Fixture, &channel.fixture_attribute), raw))
                } else {
                    None
                }
            });
        let control_value = value(ChannelAttribute::Control, &compiled.control_attribute);
        let (attribute_value, active_channel_attribute, attribute_transform) =
            if let Some(found) = value(ChannelAttribute::Canonical, &channel.attribute) {
                (
                    Some(found),
                    Some((ChannelAttribute::Canonical, &channel.attribute)),
                    channel.canonical_transform,
                )
            } else if channel.fixture_attribute != channel.attribute {
                (
                    value(ChannelAttribute::Fixture, &channel.fixture_attribute),
                    Some((ChannelAttribute::Fixture, &channel.fixture_attribute)),
                    super::CanonicalTransform::Identity,
                )
            } else {
                (None, None, channel.canonical_transform)
            };
        let active_attribute = if channel.behavior == ChannelBehavior::Static {
            None
        } else if control_value.is_some() {
            Some((ChannelAttribute::Control, &compiled.control_attribute))
        } else if let Some((attribute, _)) = winning_function {
            Some(attribute)
        } else {
            attribute_value.and(active_channel_attribute)
        };
        let resolved = resolved_raw(
            channel,
            control_value,
            attribute_value,
            attribute_transform,
            highlighted,
            highlight_override,
            winning_function.map(|(_, raw)| raw),
        );
        PlannedChannelResolution {
            active_attribute: active_attribute.map(|(_, attribute)| attribute),
            raw: scale_channel_raw(channel, highlighted, resolved, scales(active_attribute)),
        }
    }
}

fn resolved_raw(
    channel: &super::FixtureChannel,
    control_value: Option<&AttributeValue>,
    attribute_value: Option<&AttributeValue>,
    attribute_transform: super::CanonicalTransform,
    highlighted: bool,
    highlight_override: Option<u32>,
    function_raw: Option<ResolvedChannelRaw>,
) -> ResolvedChannelRaw {
    if highlighted {
        return ResolvedChannelRaw::Exact(highlight_override.unwrap_or(channel.highlight_raw));
    }
    if channel.behavior == ChannelBehavior::Static {
        return ResolvedChannelRaw::Exact(channel.default_raw);
    }
    if let Some(AttributeValue::RawDmxExact(value)) = control_value {
        return ResolvedChannelRaw::Exact(*value);
    }
    function_raw
        .or_else(|| {
            attribute_value.and_then(|value| {
                mapped_canonical_raw(value, 0, channel.resolution.max_raw(), attribute_transform)
            })
        })
        .unwrap_or(ResolvedChannelRaw::Exact(channel.default_raw))
}
