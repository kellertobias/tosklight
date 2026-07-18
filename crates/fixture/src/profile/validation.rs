use super::color_model::valid_measured_xyz;
use super::{
    ChannelFunctionBehavior, ColorSystem, ControlActionKind, FIXTURE_PROFILE_SCHEMA_VERSION,
    FixtureChannel, FixtureMode, FixtureProfile, PatchPolicy, ProfileError,
};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

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
        Ok(())
    }
}

impl FixtureChannel {
    pub fn validate(&self) -> Result<(), ProfileError> {
        let max = self.resolution.max_raw();
        if self.attribute.0.trim().is_empty() || self.default_raw > max || self.highlight_raw > max
        {
            return Err(ProfileError::Invalid(
                "channel attribute or raw values are invalid".into(),
            ));
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

fn validate_positive(name: &str, value: Option<f32>) -> Result<(), ProfileError> {
    if value.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        Err(ProfileError::Invalid(format!("{name} must be positive")))
    } else {
        Ok(())
    }
}
