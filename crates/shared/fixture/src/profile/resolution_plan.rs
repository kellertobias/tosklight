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
    /// Whether the manufacturer's name for this channel differs from its canonical one, so a
    /// canonical miss has a second name to try.
    fixture_attribute_differs: bool,
    canonical_is_intensity: bool,
    fixture_is_intensity: bool,
    control_is_intensity: bool,
    /// What each function's attribute is, indexed like `channel.functions`.
    functions: Box<[CompiledFunctionResolution]>,
}

/// The two questions asked of a function's name on every channel of every frame, answered
/// when the mode is compiled. An attribute key is a heap string; comparing two of them per
/// function per channel per frame was most of what resolving a channel cost.
#[derive(Clone, Copy, Debug)]
struct CompiledFunctionResolution {
    /// The function is named after the channel itself, so a fixture-facing value can drive it.
    aliases_channel: bool,
    is_intensity: bool,
}

/// The attribute a channel ended up reading, as told to the scale lookup.
#[derive(Clone, Copy, Debug)]
pub struct ActiveAttribute<'a> {
    pub which: ChannelAttribute,
    pub key: &'a AttributeKey,
    /// Decided when the mode was compiled; the same answer `key.is_intensity()` would give.
    pub is_intensity: bool,
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
                let control_attribute = Self::control_action_attribute(channel.id);
                CompiledChannelResolution {
                    channel_id: channel.id,
                    fixture_attribute_differs: channel.fixture_attribute != channel.attribute,
                    canonical_is_intensity: channel.attribute.is_intensity(),
                    fixture_is_intensity: channel.fixture_attribute.is_intensity(),
                    control_is_intensity: control_attribute.is_intensity(),
                    functions: channel
                        .functions
                        .iter()
                        .map(|function| CompiledFunctionResolution {
                            aliases_channel: function.attribute == channel.attribute,
                            is_intensity: function.attribute.is_intensity(),
                        })
                        .collect(),
                    control_attribute,
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
        let scales = |active: Option<ActiveAttribute<'_>>| scales(active.map(|active| active.key));
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
        scales: impl FnOnce(Option<ActiveAttribute<'_>>) -> ChannelScales,
    ) -> PlannedChannelResolution<'_> {
        let channel = &self.mode.channels[channel_index];
        let compiled = &self.plan.channels[channel_index];
        debug_assert_eq!(channel.id, compiled.channel_id);
        let fixture_facing = ActiveAttribute {
            which: ChannelAttribute::Fixture,
            key: &channel.fixture_attribute,
            is_intensity: compiled.fixture_is_intensity,
        };
        let winning_function = compiled
            .functions_by_priority
            .iter()
            .filter_map(|index| {
                channel
                    .functions
                    .get(*index)
                    .map(|function| (*index, function, compiled.functions[*index]))
            })
            .find_map(|(index, function, flags)| {
                if let Some(found) = value(ChannelAttribute::Function(index), &function.attribute) {
                    function_value_for(function, Some(found), channel.canonical_transform).map(
                        |raw| {
                            (
                                ActiveAttribute {
                                    which: ChannelAttribute::Function(index),
                                    key: &function.attribute,
                                    is_intensity: flags.is_intensity,
                                },
                                raw,
                            )
                        },
                    )
                } else if flags.aliases_channel && compiled.fixture_attribute_differs {
                    function_value_for(
                        function,
                        value(ChannelAttribute::Fixture, &channel.fixture_attribute),
                        super::CanonicalTransform::Identity,
                    )
                    .map(|raw| (fixture_facing, raw))
                } else {
                    None
                }
            });
        let control_value = value(ChannelAttribute::Control, &compiled.control_attribute);
        let (attribute_value, active_channel_attribute, attribute_transform) =
            if let Some(found) = value(ChannelAttribute::Canonical, &channel.attribute) {
                (
                    Some(found),
                    Some(ActiveAttribute {
                        which: ChannelAttribute::Canonical,
                        key: &channel.attribute,
                        is_intensity: compiled.canonical_is_intensity,
                    }),
                    channel.canonical_transform,
                )
            } else if compiled.fixture_attribute_differs {
                (
                    value(ChannelAttribute::Fixture, &channel.fixture_attribute),
                    Some(fixture_facing),
                    super::CanonicalTransform::Identity,
                )
            } else {
                (None, None, channel.canonical_transform)
            };
        let active_attribute = if channel.behavior == ChannelBehavior::Static {
            None
        } else if control_value.is_some() {
            Some(ActiveAttribute {
                which: ChannelAttribute::Control,
                key: &compiled.control_attribute,
                is_intensity: compiled.control_is_intensity,
            })
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
            active_attribute: active_attribute.map(|active| active.key),
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

#[cfg(test)]
mod compiled_flag_tests {

    /// Every flag the plan compiles must be the answer the string comparison it replaces would
    /// give, for every mode of every shipped fixture package.
    #[test]
    fn compiled_flags_agree_with_the_names_for_every_shipped_fixture() {
        let library = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("assets/fixture-library");
        let mut modes = 0usize;
        for entry in std::fs::read_dir(&library).expect("the shipped fixture library exists") {
            let path = entry.unwrap().path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("toskfixture") {
                continue;
            }
            let profile = crate::read_fixture_package(&std::fs::read(&path).unwrap())
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            for mode in &profile.modes {
                let plan = mode.compile_resolution_plan();
                for (channel, compiled) in mode.channels.iter().zip(plan.channels.iter()) {
                    assert_eq!(compiled.channel_id, channel.id);
                    assert_eq!(
                        compiled.fixture_attribute_differs,
                        channel.fixture_attribute != channel.attribute
                    );
                    assert_eq!(
                        compiled.canonical_is_intensity,
                        channel.attribute.is_intensity()
                    );
                    assert_eq!(
                        compiled.fixture_is_intensity,
                        channel.fixture_attribute.is_intensity()
                    );
                    assert_eq!(
                        compiled.control_is_intensity,
                        compiled.control_attribute.is_intensity()
                    );
                    assert_eq!(compiled.functions.len(), channel.functions.len());
                    for (function, flags) in channel.functions.iter().zip(compiled.functions.iter())
                    {
                        assert_eq!(
                            flags.aliases_channel,
                            function.attribute == channel.attribute
                        );
                        assert_eq!(flags.is_intensity, function.attribute.is_intensity());
                    }
                }
                modes += 1;
            }
        }
        assert!(modes > 0, "no fixture modes were checked");
    }
}
