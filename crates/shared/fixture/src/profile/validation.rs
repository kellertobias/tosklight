use super::color_model::valid_measured_xyz;
use super::{
    CanonicalTransform, ChannelFunctionBehavior, ColorSystem, ControlActionKind,
    FIXTURE_PROFILE_SCHEMA_VERSION, FixtureChannel, FixtureMode, FixtureProfile, PatchPolicy,
    ProfileError,
};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

/// The highest slot a gobo wheel may declare.
///
/// Wheels on real fixtures run to a dozen or so; this is only here so a mistyped slot cannot ask
/// the visualizer to divide a channel into thousands of positions nobody can select.
const MAX_GOBO_SLOT: u32 = 63;

impl FixtureProfile {
    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.schema_version != FIXTURE_PROFILE_SCHEMA_VERSION {
            return Err(ProfileError::Invalid("unsupported schema version".into()));
        }
        if self.manufacturer.trim().is_empty() || self.name.trim().is_empty() {
            return Err(ProfileError::Invalid(
                "manufacturer and fixture name are required".into(),
            ));
        }
        if self.modes.is_empty() {
            return Err(ProfileError::Invalid(
                "at least one mode is required".into(),
            ));
        }
        validate_positive("width", self.physical.width_millimetres)?;
        validate_positive("height", self.physical.height_millimetres)?;
        validate_positive("depth", self.physical.depth_millimetres)?;
        validate_positive("weight", self.physical.weight_kilograms)?;
        validate_positive("power", self.physical.power_watts)?;
        validate_positive("color temperature", self.physical.color_temperature_kelvin)?;
        validate_positive("luminous output", self.physical.luminous_output_lumens)?;
        validate_positive("beam angle", self.physical.beam_angle_degrees)?;
        if let Some(cri) = self.physical.color_rendering_index
            && (!cri.is_finite() || !(0.0..=100.0).contains(&cri))
        {
            return Err(ProfileError::Invalid(
                "color rendering index must be from 0 to 100".into(),
            ));
        }
        validate_positive("relative output", self.optics.output)?;
        validate_fraction("sharpness", self.optics.sharpness)?;
        validate_fraction("uniformity", self.optics.uniformity)?;
        if let Some(source) = self.optics.light_source {
            validate_positive("light source width", Some(source.width_millimetres))?;
            validate_positive("light source height", Some(source.height_millimetres))?;
        }
        // A wheel is read by slot, so two slots with the same number is an authoring mistake that
        // would otherwise silently drop one of them.
        let mut gobo_slots = HashSet::new();
        for gobo in &self.gobos {
            if gobo.slot > MAX_GOBO_SLOT {
                return Err(ProfileError::Invalid(format!(
                    "gobo slot {} is beyond the {MAX_GOBO_SLOT} a wheel can hold",
                    gobo.slot
                )));
            }
            if !gobo_slots.insert(gobo.slot) {
                return Err(ProfileError::Invalid(format!(
                    "gobo slot {} is declared twice",
                    gobo.slot
                )));
            }
        }
        let mut mode_ids = HashSet::new();
        for mode in &self.modes {
            if !mode_ids.insert(mode.id) {
                return Err(ProfileError::Invalid("mode IDs must be unique".into()));
            }
            mode.validate_for_patch_policy(self.patch_policy)?;
        }
        Ok(())
    }
}

impl FixtureMode {
    pub fn validate(&self) -> Result<(), ProfileError> {
        self.validate_for_patch_policy(PatchPolicy::Dmx)
    }

    fn validate_for_patch_policy(&self, patch_policy: PatchPolicy) -> Result<(), ProfileError> {
        if self.name.trim().is_empty() {
            return Err(ProfileError::Invalid("mode name is required".into()));
        }
        if self.splits.is_empty() || self.heads.is_empty() {
            return Err(ProfileError::Invalid(
                "a mode needs a split and a head".into(),
            ));
        }
        let split_map = self
            .splits
            .iter()
            .map(|split| (split.number, split.footprint))
            .collect::<BTreeMap<_, _>>();
        let invalid_split = self.splits.iter().any(|split| {
            split.number == 0
                || match patch_policy {
                    PatchPolicy::Dmx => !(1..=512).contains(&split.footprint),
                    PatchPolicy::VisualOnly => split.footprint != 0,
                }
        });
        if split_map.len() != self.splits.len() || invalid_split {
            return Err(ProfileError::Invalid(
                match patch_policy {
                    PatchPolicy::Dmx => "split numbers must be unique and footprints must be 1-512",
                    PatchPolicy::VisualOnly => {
                        "visual-only split numbers must be unique and footprints must be zero"
                    }
                }
                .into(),
            ));
        }
        if patch_policy == PatchPolicy::VisualOnly
            && (!self.channels.is_empty()
                || !self.color_systems.is_empty()
                || !self.control_actions.is_empty())
        {
            return Err(ProfileError::Invalid(
                "visual-only modes cannot define DMX channels, color systems, or control actions"
                    .into(),
            ));
        }
        let mut head_ids = HashSet::new();
        let mut masters = 0;
        for head in &self.heads {
            if head.name.trim().is_empty() || !head_ids.insert(head.id) {
                return Err(ProfileError::Invalid(
                    "head names and IDs must be unique".into(),
                ));
            }
            masters += usize::from(head.master_shared);
        }
        if masters > 1 {
            return Err(ProfileError::Invalid(
                "at most one head can be master/shared".into(),
            ));
        }
        let mut channel_ids = HashSet::new();
        for channel in &self.channels {
            if !channel_ids.insert(channel.id) || !head_ids.contains(&channel.head_id) {
                return Err(ProfileError::Invalid(
                    "channel IDs must be unique and reference an existing head".into(),
                ));
            }
            if !split_map.contains_key(&channel.split) {
                return Err(ProfileError::Invalid(
                    "channel references a missing split".into(),
                ));
            }
            channel.validate()?;
        }
        for channel in &self.channels {
            let Some((canonical, transform)) =
                light_core::canonical_attribute_migration(&channel.fixture_attribute)
            else {
                continue;
            };
            if transform != light_core::CanonicalAttributeTransform::Identity
                || canonical == channel.fixture_attribute
            {
                continue;
            }
            if self.channels.iter().any(|candidate| {
                candidate.id != channel.id
                    && candidate.split == channel.split
                    && candidate.head_id == channel.head_id
                    && candidate.attribute == channel.attribute
            }) {
                return Err(ProfileError::Invalid(format!(
                    "split {} maps more than one channel on the same head to canonical attribute `{}`",
                    channel.split, channel.attribute.0
                )));
            }
        }
        self.primary_slots()?;
        for head in &self.heads {
            if !head_ids.contains(&head.id) {
                return Err(ProfileError::Invalid("invalid head".into()));
            }
        }
        let action_ids = self
            .control_actions
            .iter()
            .map(|action| action.id)
            .collect::<HashSet<_>>();
        if action_ids.len() != self.control_actions.len() {
            return Err(ProfileError::Invalid(
                "control action IDs must be unique".into(),
            ));
        }
        for action in &self.control_actions {
            if action.assignments.is_empty() {
                return Err(ProfileError::Invalid(
                    "control actions need assignments".into(),
                ));
            }
            if action.kind == ControlActionKind::TimedPulse
                && action.duration_millis.is_none_or(|duration| duration == 0)
            {
                return Err(ProfileError::Invalid(
                    "timed pulse actions need a positive duration".into(),
                ));
            }
            for assignment in &action.assignments {
                let channel = self
                    .channels
                    .iter()
                    .find(|channel| channel.id == assignment.channel_id)
                    .ok_or_else(|| {
                        ProfileError::Invalid("action references a missing channel".into())
                    })?;
                if assignment.active_raw > channel.resolution.max_raw()
                    || assignment.inactive_raw > channel.resolution.max_raw()
                {
                    return Err(ProfileError::Invalid(
                        "action raw value is out of range".into(),
                    ));
                }
            }
        }
        for channel in &self.channels {
            for function in &channel.functions {
                if let ChannelFunctionBehavior::Control { action_id } = function.behavior
                    && !action_ids.contains(&action_id)
                {
                    return Err(ProfileError::Invalid(
                        "channel function references a missing control action".into(),
                    ));
                }
            }
        }
        self.validate_color_systems(&head_ids, &channel_ids)?;
        self.geometry.validate(&head_ids)?;
        Ok(())
    }

    fn validate_color_systems(
        &self,
        head_ids: &HashSet<Uuid>,
        channel_ids: &HashSet<Uuid>,
    ) -> Result<(), ProfileError> {
        for system in &self.color_systems {
            if !head_ids.contains(&system.head_id) {
                return Err(ProfileError::Invalid(
                    "color system references a missing head".into(),
                ));
            }
            if system
                .correction_matrix
                .iter()
                .flatten()
                .any(|value| !value.is_finite())
            {
                return Err(ProfileError::Invalid(
                    "color correction matrix is invalid".into(),
                ));
            }
            let references = match &system.system {
                ColorSystem::Additive { emitters } => {
                    if emitters.is_empty()
                        || emitters.iter().any(|emitter| {
                            !valid_measured_xyz(emitter.xyz)
                                || !emitter.maximum_level.is_finite()
                                || emitter.maximum_level <= 0.0
                                || emitter.maximum_level > 1.0
                                || !emitter.response_curve.is_finite()
                                || emitter.response_curve <= 0.0
                        })
                    {
                        return Err(ProfileError::Invalid(
                            "additive emitter calibration is invalid".into(),
                        ));
                    }
                    emitters
                        .iter()
                        .map(|emitter| emitter.channel_id)
                        .collect::<Vec<_>>()
                }
                ColorSystem::Subtractive {
                    cyan_channel_id,
                    magenta_channel_id,
                    yellow_channel_id,
                } => {
                    vec![*cyan_channel_id, *magenta_channel_id, *yellow_channel_id]
                }
                ColorSystem::HueSaturation {
                    hue_channel_id,
                    saturation_channel_id,
                    intensity_channel_id,
                } => {
                    let mut references = vec![*hue_channel_id, *saturation_channel_id];
                    references.extend(intensity_channel_id);
                    if references.iter().collect::<HashSet<_>>().len() != references.len() {
                        return Err(ProfileError::Invalid(
                            "hue/saturation color system channels must be distinct".into(),
                        ));
                    }
                    if references.iter().any(|id| {
                        self.channels
                            .iter()
                            .find(|channel| channel.id == *id)
                            .is_some_and(|channel| channel.head_id != system.head_id)
                    }) {
                        return Err(ProfileError::Invalid(
                            "hue/saturation color system channels must belong to its head".into(),
                        ));
                    }
                    references
                }
                ColorSystem::DiscreteWheel { channel_id, slots } => {
                    let Some(channel) = self
                        .channels
                        .iter()
                        .find(|channel| channel.id == *channel_id)
                    else {
                        return Err(ProfileError::Invalid(
                            "color system references a missing channel".into(),
                        ));
                    };
                    let mut semantic_ids = HashSet::new();
                    if slots.is_empty()
                        || slots.iter().any(|slot| {
                            let semantic_id = slot.semantic_id.trim();
                            semantic_id.is_empty()
                                || !semantic_ids.insert(semantic_id)
                                || slot.label.trim().is_empty()
                                || slot.dmx_from > slot.dmx_to
                                || slot.dmx_to > channel.resolution.max_raw()
                                || slot
                                    .measured_xyz
                                    .is_some_and(|xyz| !valid_measured_xyz(xyz))
                        })
                    {
                        return Err(ProfileError::Invalid(
                            "color wheel slot metadata is invalid".into(),
                        ));
                    }
                    if slots
                        .windows(2)
                        .any(|pair| pair[0].dmx_to >= pair[1].dmx_from)
                    {
                        return Err(ProfileError::Invalid(
                            "color wheel slots must be sorted and non-overlapping".into(),
                        ));
                    }
                    vec![*channel_id]
                }
            };
            if references.iter().any(|id| !channel_ids.contains(id)) {
                return Err(ProfileError::Invalid(
                    "color system references a missing channel".into(),
                ));
            }
        }
        for channel in &self.channels {
            let bound = self.color_systems.iter().any(|system| {
                if system.head_id != channel.head_id {
                    return false;
                }
                let ColorSystem::HueSaturation {
                    hue_channel_id,
                    saturation_channel_id,
                    intensity_channel_id,
                } = &system.system
                else {
                    return false;
                };
                match channel.attribute.0.as_str() {
                    "color.hue" => *hue_channel_id == channel.id,
                    "color.saturation" => *saturation_channel_id == channel.id,
                    "color.brightness" => *intensity_channel_id == Some(channel.id),
                    _ => false,
                }
            });
            if light_core::built_in_attribute_is_projection_only(&channel.attribute.0) && !bound {
                return Err(ProfileError::Invalid(format!(
                    "projection-only channel `{}` is not bound to its color system",
                    channel.attribute.0
                )));
            }
        }
        Ok(())
    }
}

impl FixtureChannel {
    pub fn validate(&self) -> Result<(), ProfileError> {
        let max = self.resolution.max_raw();
        if self.fixture_attribute.0.trim().is_empty()
            || self.attribute.0.trim().is_empty()
            || self.default_raw > max
            || self.highlight_raw > max
        {
            return Err(ProfileError::Invalid(
                "channel attribute or raw values are invalid".into(),
            ));
        }
        if self.canonical_transform == CanonicalTransform::InvertNormalized {
            let descriptor = light_core::attribute_descriptor(&self.attribute);
            if descriptor.normalized_bounds.is_none() || !descriptor.recordable {
                return Err(ProfileError::Invalid(
                    "normalized inversion requires a recordable continuous canonical attribute"
                        .into(),
                ));
            }
        }
        if self
            .physical_min
            .zip(self.physical_max)
            .is_some_and(|(min, max)| !min.is_finite() || !max.is_finite() || min >= max)
        {
            return Err(ProfileError::Invalid(
                "channel physical range is invalid".into(),
            ));
        }
        let mut function_ids = HashSet::new();
        let mut ranges = self.functions.iter().collect::<Vec<_>>();
        ranges.sort_by_key(|function| function.dmx_from);
        for (index, function) in ranges.iter().enumerate() {
            if !function_ids.insert(function.id)
                || function.name.trim().is_empty()
                || function.attribute.0.trim().is_empty()
                || function.dmx_from > function.dmx_to
                || function.dmx_to > max
            {
                return Err(ProfileError::Invalid("channel function is invalid".into()));
            }
            if index > 0 && ranges[index - 1].dmx_to >= function.dmx_from {
                return Err(ProfileError::Invalid(
                    "channel function ranges overlap".into(),
                ));
            }
            match &function.behavior {
                ChannelFunctionBehavior::Continuous {
                    physical_min,
                    physical_max,
                    ..
                } if !physical_min.is_finite()
                    || !physical_max.is_finite()
                    || physical_min >= physical_max =>
                {
                    return Err(ProfileError::Invalid(
                        "continuous function range is invalid".into(),
                    ));
                }
                ChannelFunctionBehavior::Fixed { raw_value, .. }
                | ChannelFunctionBehavior::Indexed { raw_value, .. }
                    if *raw_value < function.dmx_from || *raw_value > function.dmx_to =>
                {
                    return Err(ProfileError::Invalid(
                        "fixed value is outside its function range".into(),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// A declared `0..=1` optical figure. Out of range is a mistake worth reporting rather than
/// silently clamping: a sharpness of `40` is a transcription error, not a very hard edge.
fn validate_fraction(name: &str, value: Option<f32>) -> Result<(), ProfileError> {
    if value.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
        Err(ProfileError::Invalid(format!("{name} must be from 0 to 1")))
    } else {
        Ok(())
    }
}

fn validate_positive(name: &str, value: Option<f32>) -> Result<(), ProfileError> {
    if value.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        Err(ProfileError::Invalid(format!("{name} must be positive")))
    } else {
        Ok(())
    }
}
