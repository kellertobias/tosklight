use super::{
    ChannelBehavior, ChannelFunction, ChannelFunctionBehavior, ChannelScales, FixtureChannel,
    FixtureMode, ProfileError,
};
use light_core::{AttributeKey, AttributeValue};
use std::collections::{BTreeSet, HashMap};
use uuid::Uuid;

impl FixtureMode {
    pub fn primary_slots(&self) -> Result<HashMap<Uuid, u16>, ProfileError> {
        let footprint = self
            .splits
            .iter()
            .map(|split| (split.number, split.footprint))
            .collect::<HashMap<_, _>>();
        let mut reserved = HashMap::<u16, BTreeSet<u16>>::new();
        for channel in &self.channels {
            let split = channel.split;
            if channel.secondary_slots.len() + 1 != channel.resolution.bytes() {
                return Err(ProfileError::Invalid(format!(
                    "{}-bit channels require {} secondary slots",
                    channel.resolution.bytes() * 8,
                    channel.resolution.bytes() - 1
                )));
            }
            let limit = *footprint.get(&split).ok_or_else(|| {
                ProfileError::Invalid("channel references a missing split".into())
            })?;
            for slot in &channel.secondary_slots {
                if *slot == 0 || *slot > limit || !reserved.entry(split).or_default().insert(*slot)
                {
                    return Err(ProfileError::Invalid(
                        "component slots are duplicated or outside the split footprint".into(),
                    ));
                }
            }
        }
        let mut next = HashMap::<u16, u16>::new();
        let mut used = reserved.clone();
        let mut result = HashMap::new();
        for channel in &self.channels {
            let split = channel.split;
            let limit = footprint[&split];
            let cursor = next.entry(split).or_insert(1);
            while used.get(&split).is_some_and(|slots| slots.contains(cursor)) {
                *cursor += 1;
            }
            if *cursor > limit {
                return Err(ProfileError::Invalid(
                    "channel rows exceed the split footprint".into(),
                ));
            }
            used.entry(split).or_default().insert(*cursor);
            result.insert(channel.id, *cursor);
            *cursor += 1;
        }
        Ok(result)
    }

    /// Internal semantic address used by the Programmer for an atomic control-action assignment.
    /// It is channel-specific because one action may drive several channels that otherwise expose
    /// the same public attribute.
    pub fn control_action_attribute(channel_id: Uuid) -> AttributeKey {
        AttributeKey(format!("__fixture_control_channel.{channel_id}"))
    }

    /// Resolve one physical channel after normal semantic LTP/HTP resolution. Competing
    /// functions claim the channel only when their exact semantic address is explicitly present;
    /// the highest configured priority wins and release reveals the next eligible function.
    pub fn resolve_channel_raw(
        &self,
        channel: &FixtureChannel,
        values: &HashMap<AttributeKey, AttributeValue>,
        highlighted: bool,
        highlight_override: Option<u32>,
        scales: ChannelScales,
    ) -> u32 {
        let max = channel.resolution.max_raw();
        let resolved = if highlighted {
            ResolvedChannelRaw::Exact(highlight_override.unwrap_or(channel.highlight_raw))
        } else if channel.behavior == ChannelBehavior::Static {
            ResolvedChannelRaw::Exact(channel.default_raw)
        } else if let Some(AttributeValue::RawDmxExact(value)) =
            values.get(&Self::control_action_attribute(channel.id))
        {
            ResolvedChannelRaw::Exact(*value)
        } else {
            channel
                .functions
                .iter()
                .enumerate()
                .filter_map(|(index, function)| {
                    function_value(function, values)
                        .map(|raw| (function.priority, std::cmp::Reverse(index), raw))
                })
                .max_by_key(|(priority, order, _)| (*priority, *order))
                .map(|(_, _, raw)| raw)
                .or_else(|| {
                    values
                        .get(&channel.attribute)
                        .and_then(|value| mapped_raw(value, 0, max))
                })
                .unwrap_or(ResolvedChannelRaw::Exact(channel.default_raw))
        };
        let mut scale = 1.0_f64;
        if !highlighted {
            if channel.reacts_to_virtual_intensity {
                scale *= f64::from(scales.virtual_intensity.clamp(0.0, 1.0));
            }
            if channel.reacts_to_sequence_master {
                scale *= f64::from(scales.sequence_master.clamp(0.0, 1.0));
            }
            if channel.reacts_to_group_master {
                scale *= f64::from(scales.group_master.clamp(0.0, 1.0));
            }
        }
        // Grand Master is the only ordinary master above transient Highlight. Blackout and
        // hazardous safe values are enforced by the engine after this channel resolution.
        if channel.reacts_to_grand_master {
            scale *= f64::from(scales.grand_master.clamp(0.0, 1.0));
        }
        match resolved {
            ResolvedChannelRaw::Semantic { raw, from, to } => {
                let from = from.min(max);
                let to = to.min(max).max(from);
                let raw = raw.clamp(from, to);
                let scaled = (f64::from(raw - from) * scale)
                    .round()
                    .clamp(0.0, f64::from(to - from)) as u32;
                if channel.invert {
                    to.saturating_sub(scaled)
                } else {
                    from.saturating_add(scaled)
                }
            }
            ResolvedChannelRaw::Exact(raw) => {
                let raw = raw.min(max);
                if channel.invert {
                    max.saturating_sub(
                        (f64::from(max - raw) * scale)
                            .round()
                            .clamp(0.0, f64::from(max)) as u32,
                    )
                } else {
                    (f64::from(raw) * scale).round().clamp(0.0, f64::from(max)) as u32
                }
            }
        }
    }

    /// Returns the semantic attribute that currently owns a physical channel. Defaults and Static
    /// behavior deliberately return `None`: neither represents an explicitly active source and
    /// therefore neither should acquire a source-specific sequence-master scale.
    pub fn active_attribute_for_channel<'a>(
        &'a self,
        channel: &'a FixtureChannel,
        values: &'a HashMap<AttributeKey, AttributeValue>,
    ) -> Option<&'a AttributeKey> {
        if channel.behavior == ChannelBehavior::Static {
            return None;
        }
        values
            .get_key_value(&Self::control_action_attribute(channel.id))
            .map(|(attribute, _)| attribute)
            .or_else(|| {
                channel
                    .functions
                    .iter()
                    .enumerate()
                    .filter(|(_, function)| function_value(function, values).is_some())
                    .max_by_key(|(index, function)| (function.priority, std::cmp::Reverse(*index)))
                    .map(|(_, function)| &function.attribute)
            })
            .or_else(|| {
                values
                    .contains_key(&channel.attribute)
                    .then_some(&channel.attribute)
            })
    }

    /// Whether a semantic address belonging to one logical head must bypass fades. A channel can
    /// claim an attribute either directly or through one of its multi-function ranges.
    pub fn head_attribute_is_snap(&self, head_id: Uuid, attribute: &AttributeKey) -> bool {
        self.channels.iter().any(|channel| {
            channel.head_id == head_id
                && channel.snap
                && (channel.attribute == *attribute
                    || channel
                        .functions
                        .iter()
                        .any(|function| function.attribute == *attribute))
        })
    }

    pub fn encode_channel(
        &self,
        frame: &mut [u8; 512],
        base: u16,
        channel: &FixtureChannel,
        raw: u32,
    ) -> Result<(), ProfileError> {
        let primary = self.primary_slots()?[&channel.id];
        let mut slots = vec![primary];
        slots.extend(channel.secondary_slots.iter().copied());
        if slots.len() != channel.resolution.bytes() {
            return Err(ProfileError::Invalid(
                "channel component count is invalid".into(),
            ));
        }
        for (index, slot) in slots.into_iter().enumerate() {
            let absolute =
                usize::from(base.saturating_sub(1)) + usize::from(slot.saturating_sub(1));
            if base == 0 || absolute >= frame.len() {
                return Err(ProfileError::Invalid(
                    "encoded channel exceeds its universe".into(),
                ));
            }
            let shift = 8 * (channel.resolution.bytes() - index - 1);
            frame[absolute] = ((raw >> shift) & 0xff) as u8;
        }
        Ok(())
    }

    pub fn control_action_values(
        &self,
        action_id: Uuid,
        active: bool,
    ) -> Result<Vec<(Uuid, u32)>, ProfileError> {
        let action = self
            .control_actions
            .iter()
            .find(|action| action.id == action_id)
            .ok_or_else(|| ProfileError::Invalid("control action does not exist".into()))?;
        Ok(action
            .assignments
            .iter()
            .map(|assignment| {
                (
                    assignment.channel_id,
                    if active {
                        assignment.active_raw
                    } else {
                        assignment.inactive_raw
                    },
                )
            })
            .collect())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolvedChannelRaw {
    /// A semantic continuous value. Masters scale its distance from the function's zero endpoint,
    /// then inversion is applied inside that function range.
    Semantic { raw: u32, from: u32, to: u32 },
    /// A fixture-manual/raw value. Inversion does not reinterpret the value; opted-in masters
    /// instead move it toward the channel's configured physical-off endpoint.
    Exact(u32),
}

fn function_value(
    function: &ChannelFunction,
    values: &HashMap<AttributeKey, AttributeValue>,
) -> Option<ResolvedChannelRaw> {
    let value = values.get(&function.attribute)?;
    match (&function.behavior, value) {
        (ChannelFunctionBehavior::Continuous { .. }, value) => {
            mapped_raw(value, function.dmx_from, function.dmx_to)
        }
        (
            ChannelFunctionBehavior::Fixed {
                semantic_id,
                raw_value,
                ..
            },
            AttributeValue::Discrete(value),
        )
        | (
            ChannelFunctionBehavior::Indexed {
                semantic_id,
                raw_value,
                ..
            },
            AttributeValue::Discrete(value),
        ) if value == semantic_id => Some(ResolvedChannelRaw::Exact(*raw_value)),
        (ChannelFunctionBehavior::Control { action_id }, AttributeValue::Discrete(value))
            if value == &action_id.to_string() =>
        {
            Some(ResolvedChannelRaw::Exact(function.dmx_to))
        }
        _ => None,
    }
}

fn mapped_raw(value: &AttributeValue, from: u32, to: u32) -> Option<ResolvedChannelRaw> {
    match value {
        AttributeValue::Normalized(value) => Some(ResolvedChannelRaw::Semantic {
            raw: (f64::from(from) + f64::from(to - from) * f64::from(value.clamp(0.0, 1.0))).round()
                as u32,
            from,
            to,
        }),
        AttributeValue::RawDmx(value) => Some(ResolvedChannelRaw::Semantic {
            raw: from + ((u64::from(to - from) * u64::from(*value) + 127) / 255) as u32,
            from,
            to,
        }),
        // RawDmxExact is a physical channel value, not a semantic point in this function range.
        // Resolution clamping happens once in resolve_channel_raw.
        AttributeValue::RawDmxExact(value) => Some(ResolvedChannelRaw::Exact(*value)),
        _ => None,
    }
}
