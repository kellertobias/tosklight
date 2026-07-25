use super::{
    ChannelBehavior, ChannelScales, FixtureMode, ProfileError,
    resolution::{ResolvedChannelRaw, function_value, mapped_raw, scale_channel_raw},
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
    pub fn resolve_channel(
        &self,
        channel_index: usize,
        values: &HashMap<AttributeKey, AttributeValue>,
        highlighted: bool,
        highlight_override: Option<u32>,
        scales: impl FnOnce(Option<&AttributeKey>) -> ChannelScales,
    ) -> PlannedChannelResolution<'_> {
        let channel = &self.mode.channels[channel_index];
        let compiled = &self.plan.channels[channel_index];
        debug_assert_eq!(channel.id, compiled.channel_id);
        let winning_function = compiled
            .functions_by_priority
            .iter()
            .filter_map(|index| channel.functions.get(*index))
            .find_map(|function| function_value(function, values).map(|raw| (function, raw)));
        let control_value = values.get(&compiled.control_attribute);
        let attribute_value = values.get(&channel.attribute);
        let active_attribute = if channel.behavior == ChannelBehavior::Static {
            None
        } else if control_value.is_some() {
            Some(&compiled.control_attribute)
        } else if let Some((function, _)) = winning_function {
            Some(&function.attribute)
        } else {
            attribute_value.is_some().then_some(&channel.attribute)
        };
        let resolved = resolved_raw(
            channel,
            control_value,
            attribute_value,
            highlighted,
            highlight_override,
            winning_function.map(|(_, raw)| raw),
        );
        PlannedChannelResolution {
            active_attribute,
            raw: scale_channel_raw(channel, highlighted, resolved, scales(active_attribute)),
        }
    }
}

fn resolved_raw(
    channel: &super::FixtureChannel,
    control_value: Option<&AttributeValue>,
    attribute_value: Option<&AttributeValue>,
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
            attribute_value.and_then(|value| mapped_raw(value, 0, channel.resolution.max_raw()))
        })
        .unwrap_or(ResolvedChannelRaw::Exact(channel.default_raw))
}
