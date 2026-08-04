use super::*;

impl AttributeConfiguration {
    pub fn recommended() -> Self {
        let placements = recommended_builtin_placements();
        let mut linked_groups = vec![
            recommended_activation_group(
                "color_mix",
                "Color Mix",
                &[
                    "color",
                    "color.red",
                    "color.green",
                    "color.blue",
                    "color.amber",
                    "color.white",
                    "color.uv",
                    "color.lime",
                    "color.indigo",
                    "color.mint",
                    "color.temperature",
                ],
            ),
            recommended_activation_group(
                "color_wheel_1",
                "Color Wheel 1",
                &["color.wheel.1", "color.wheel.1.rotation"],
            ),
            recommended_activation_group(
                "color_wheel_2",
                "Color Wheel 2",
                &["color.wheel.2", "color.wheel.2.rotation"],
            ),
            recommended_activation_group(
                "position",
                "Position",
                &["pan", "tilt", "position.rotation"],
            ),
            recommended_activation_group("gobo_1", "Gobo 1", &["gobo.1", "gobo.1.rotation"]),
            recommended_activation_group("gobo_2", "Gobo 2", &["gobo.2", "gobo.2.rotation"]),
            recommended_activation_group(
                "media_source",
                "Media Source",
                &["media.folder", "media.file"],
            ),
            recommended_activation_group(
                "media_mask_source",
                "Media Mask Source",
                &["media.mask.folder", "media.mask.file"],
            ),
            recommended_activation_group(
                "shapers",
                "Shapers",
                &[
                    "iris",
                    "shaper.blade.1.position",
                    "shaper.blade.1.angle",
                    "shaper.blade.2.position",
                    "shaper.blade.2.angle",
                    "shaper.rotation",
                    "shaper.blade.3.position",
                    "shaper.blade.3.angle",
                    "shaper.blade.4.position",
                    "shaper.blade.4.angle",
                ],
            ),
        ];
        let linked_members = linked_groups
            .iter()
            .flat_map(|group| group.members.iter().map(|member| member.0.clone()))
            .collect::<HashSet<_>>();
        linked_groups.extend(
            ATTRIBUTE_REGISTRY
                .iter()
                .filter(|descriptor| {
                    descriptor.recordable
                        && !built_in_attribute_is_retired(descriptor.id)
                        && !built_in_attribute_is_special_dialog_only(descriptor.id)
                })
                .filter(|descriptor| !linked_members.contains(descriptor.id))
                .map(|descriptor| {
                    recommended_activation_group(descriptor.id, descriptor.label, &[descriptor.id])
                }),
        );
        Self {
            version: ATTRIBUTE_CONFIGURATION_VERSION,
            custom_attributes: Vec::new(),
            placements,
            activation_groups: linked_groups,
        }
    }

    /// Adds built-ins introduced after this show saved its configuration without rewriting
    /// existing custom descriptors or activation choices.
    ///
    /// Newly recommended slots remain stable. A custom descriptor that occupied a slot before it
    /// became a built-in preferred location moves to the first free page after that encoder
    /// group's built-in pages. Newly introduced attributes join an existing matching recommended
    /// group when one exists; otherwise complete new recommended groups are added, with remaining
    /// attributes receiving safe single-member groups.
    pub fn with_current_built_ins(mut self) -> Self {
        let recommended = Self::recommended();
        let built_in_ids = ATTRIBUTE_REGISTRY
            .iter()
            .map(|descriptor| descriptor.id)
            .collect::<HashSet<_>>();
        for placement in &recommended.placements {
            if let Some(existing) = self
                .placements
                .iter_mut()
                .find(|candidate| candidate.attribute == placement.attribute)
            {
                if existing.push_turn_of.is_none() {
                    existing.push_turn_of.clone_from(&placement.push_turn_of);
                }
                continue;
            }
            if let Some(occupied) = self
                .placements
                .iter()
                .position(|candidate| candidate.encoder == placement.encoder)
            {
                let occupant = &self.placements[occupied].attribute;
                if !built_in_ids.contains(occupant.0.as_str()) {
                    self.placements[occupied].encoder =
                        next_custom_encoder_slot(&self.placements, placement.encoder.group);
                }
            }
            self.placements.push(placement.clone());
        }

        let mut assigned = self
            .activation_groups
            .iter()
            .flat_map(|group| group.members.iter().cloned())
            .collect::<HashSet<_>>();
        for recommended_group in &recommended.activation_groups {
            let missing = recommended_group
                .members
                .iter()
                .filter(|member| !assigned.contains(*member))
                .cloned()
                .collect::<Vec<_>>();
            if missing.is_empty() {
                continue;
            }
            if let Some(existing) = self
                .activation_groups
                .iter_mut()
                .find(|group| group.id == recommended_group.id)
            {
                existing.members.extend(missing.iter().cloned());
                assigned.extend(missing);
                continue;
            }
            if missing.len() == recommended_group.members.len() {
                self.activation_groups.push(recommended_group.clone());
                assigned.extend(missing);
            }
        }
        for descriptor in ATTRIBUTE_REGISTRY.iter().filter(|descriptor| {
            descriptor.recordable
                && !built_in_attribute_is_retired(descriptor.id)
                && !built_in_attribute_is_special_dialog_only(descriptor.id)
        }) {
            let id = AttributeKey(descriptor.id.into());
            if assigned.insert(id.clone()) {
                self.activation_groups.push(recommended_activation_group(
                    descriptor.id,
                    descriptor.label,
                    &[descriptor.id],
                ));
            }
        }
        self
    }

    /// Rewrites retired emitter/operator controls to their canonical identities.
    ///
    /// Older recommended configurations contained both sets. Those untouched duplicate CMY
    /// placements and singleton activation groups are removed. Authored layouts that assign a
    /// different meaning to both identities are rejected so loading cannot silently choose one.
    pub fn migrate_canonical_attributes(mut self) -> Result<Self, AttributeConfigurationError> {
        for (source, target, legacy_encoder) in [
            (
                "color.cyan",
                "color.red",
                EncoderPlacement::new(EncoderGroup::Color, 4, 1),
            ),
            (
                "color.magenta",
                "color.green",
                EncoderPlacement::new(EncoderGroup::Color, 4, 2),
            ),
            (
                "color.yellow",
                "color.blue",
                EncoderPlacement::new(EncoderGroup::Color, 4, 3),
            ),
            (
                "color.cold_white",
                "color.white",
                EncoderPlacement::new(EncoderGroup::Color, 2, 1),
            ),
            (
                "color.warm_white",
                "color.amber",
                EncoderPlacement::new(EncoderGroup::Color, 2, 2),
            ),
            (
                "frost.1",
                "softness",
                EncoderPlacement::new(EncoderGroup::Focus, 1, 3),
            ),
            (
                "beam.edge",
                "softness",
                EncoderPlacement::new(EncoderGroup::Focus, 1, 5),
            ),
            (
                "pan.time",
                "position.movement",
                EncoderPlacement::new(EncoderGroup::Position, 1, 5),
            ),
            (
                "tilt.time",
                "position.movement",
                EncoderPlacement::new(EncoderGroup::Position, 1, 6),
            ),
            (
                "position.time",
                "position.movement",
                EncoderPlacement::new(EncoderGroup::Position, 1, 5),
            ),
            (
                "position.speed",
                "position.movement",
                EncoderPlacement::new(EncoderGroup::Position, 2, 1),
            ),
        ] {
            self.migrate_canonical_configuration_pair(source, target, legacy_encoder)?;
        }
        self.remove_legacy_default_whole_color_encoder();
        self.remove_legacy_default_tint_encoder();
        Ok(self)
    }

    fn remove_legacy_default_whole_color_encoder(&mut self) {
        let color = AttributeKey("color".into());
        if let Some(index) = self.placements.iter().position(|placement| {
            placement.attribute == color
                && placement.encoder == EncoderPlacement::new(EncoderGroup::Color, 3, 2)
                && placement.push_turn_of.is_none()
        }) {
            self.placements.remove(index);
        }
    }

    fn remove_legacy_default_tint_encoder(&mut self) {
        let tint = AttributeKey("color.tint".into());
        if let Some(index) = self.placements.iter().position(|placement| {
            placement.attribute == tint
                && placement.encoder == EncoderPlacement::new(EncoderGroup::Color, 3, 1)
                && placement.push_turn_of.is_none()
        }) {
            self.placements.remove(index);
        }
        if self.attribute_placement_for(&tint).is_some() {
            return;
        }
        for group in &mut self.activation_groups {
            group.members.retain(|member| member != &tint);
        }
        self.activation_groups
            .retain(|group| !group.members.is_empty());
    }

    fn migrate_canonical_configuration_pair(
        &mut self,
        source: &str,
        target: &str,
        legacy_encoder: EncoderPlacement,
    ) -> Result<(), AttributeConfigurationError> {
        let source_key = AttributeKey(source.into());
        let target_key = AttributeKey(target.into());
        let source_placement = self
            .placements
            .iter()
            .position(|placement| placement.attribute == source_key);
        let target_placement = self
            .placements
            .iter()
            .position(|placement| placement.attribute == target_key);
        match (source_placement, target_placement) {
            (Some(source_index), Some(_))
                if self.placements[source_index].encoder == legacy_encoder
                    && self.placements[source_index].push_turn_of.is_none() =>
            {
                self.placements.remove(source_index);
            }
            (Some(_), Some(_)) => {
                return Err(AttributeConfigurationError::CanonicalPlacementConflict {
                    legacy: source.into(),
                    canonical: target.into(),
                });
            }
            (Some(source_index), None)
                if target == "position.movement"
                    && self.placements[source_index].encoder == legacy_encoder
                    && self.placements[source_index].push_turn_of.is_none() =>
            {
                let recommended = recommended_builtin_placements()
                    .into_iter()
                    .find(|placement| placement.attribute == target_key)
                    .expect("canonical migration target has a recommended placement");
                self.placements[source_index] = recommended;
            }
            (Some(source_index), None) => {
                self.placements[source_index].attribute = target_key.clone();
            }
            (None, _) => {}
        }

        for placement in &mut self.placements {
            if placement.push_turn_of.as_ref() == Some(&source_key) {
                placement.push_turn_of = Some(target_key.clone());
            }
        }

        let source_group = self
            .activation_groups
            .iter()
            .position(|group| group.members.contains(&source_key));
        let target_group = self
            .activation_groups
            .iter()
            .position(|group| group.members.contains(&target_key));
        match (source_group, target_group) {
            (Some(source_index), Some(target_index)) if source_index == target_index => {
                let members = &mut self.activation_groups[source_index].members;
                members.retain(|member| member != &source_key);
            }
            (Some(source_index), Some(_))
                if legacy_singleton_group(&self.activation_groups[source_index], source) =>
            {
                self.activation_groups.remove(source_index);
            }
            (Some(_), Some(_)) => {
                return Err(
                    AttributeConfigurationError::CanonicalActivationGroupConflict {
                        legacy: source.into(),
                        canonical: target.into(),
                    },
                );
            }
            (Some(source_index), None) => {
                let members = &mut self.activation_groups[source_index].members;
                for member in members {
                    if member == &source_key {
                        *member = target_key.clone();
                    }
                }
            }
            (None, _) => {}
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), AttributeConfigurationError> {
        if self.version != ATTRIBUTE_CONFIGURATION_VERSION {
            return Err(AttributeConfigurationError::UnsupportedVersion {
                actual: self.version,
                expected: ATTRIBUTE_CONFIGURATION_VERSION,
            });
        }
        let mut descriptors = ATTRIBUTE_REGISTRY
            .iter()
            .map(|descriptor| {
                (
                    descriptor.id,
                    (descriptor.value_type, descriptor.recordable),
                )
            })
            .collect::<HashMap<_, _>>();
        let built_in_ids = descriptors.keys().copied().collect::<HashSet<_>>();
        for descriptor in &self.custom_attributes {
            validate_custom_descriptor(descriptor, &built_in_ids)?;
            if descriptors
                .insert(
                    descriptor.id.0.as_str(),
                    (descriptor.value_type, descriptor.recordable),
                )
                .is_some()
            {
                return Err(AttributeConfigurationError::DuplicateCustomAttribute(
                    descriptor.id.0.clone(),
                ));
            }
        }

        let mut placements = HashMap::new();
        let mut occupied = HashSet::new();
        for placement in &self.placements {
            let id = placement.attribute.0.as_str();
            if !descriptors.contains_key(id) {
                return Err(AttributeConfigurationError::UnknownPlacedAttribute(
                    id.to_owned(),
                ));
            }
            if !placement.encoder.is_valid() {
                return Err(AttributeConfigurationError::InvalidPlacement(id.to_owned()));
            }
            if placements.insert(id, placement.encoder).is_some() {
                return Err(AttributeConfigurationError::DuplicatePlacement(
                    id.to_owned(),
                ));
            }
            if !occupied.insert(placement.encoder) {
                return Err(AttributeConfigurationError::OccupiedPlacement {
                    group: placement.encoder.group,
                    page: placement.encoder.page,
                    slot: placement.encoder.slot,
                });
            }
        }
        for id in descriptors.keys() {
            if !built_in_attribute_is_retired(id)
                && !built_in_attribute_is_projection_only(id)
                && !built_in_attribute_is_special_dialog_only(id)
                && !placements.contains_key(id)
            {
                return Err(AttributeConfigurationError::MissingPlacement(
                    (*id).to_owned(),
                ));
            }
        }

        let mut push_turn_parents = HashSet::new();
        for placement in &self.placements {
            let Some(parent_id) = placement.push_turn_of.as_ref() else {
                continue;
            };
            let attribute = placement.attribute.0.as_str();
            let parent = parent_id.0.as_str();
            let Some(parent_placement) = self
                .placements
                .iter()
                .find(|candidate| candidate.attribute == *parent_id)
            else {
                return Err(AttributeConfigurationError::InvalidPushTurnParent {
                    attribute: attribute.to_owned(),
                    parent: parent.to_owned(),
                });
            };
            if attribute == parent || parent_placement.push_turn_of.is_some() {
                return Err(AttributeConfigurationError::InvalidPushTurnParent {
                    attribute: attribute.to_owned(),
                    parent: parent.to_owned(),
                });
            }
            if placement.encoder.group != parent_placement.encoder.group {
                return Err(AttributeConfigurationError::CrossEncoderPushTurn {
                    parent: parent.to_owned(),
                    attribute: attribute.to_owned(),
                });
            }
            if !push_turn_parents.insert(parent) {
                return Err(AttributeConfigurationError::DuplicatePushTurnCompanion {
                    parent: parent.to_owned(),
                });
            }
        }

        let mut group_ids = HashSet::new();
        let mut activated = HashSet::new();
        for group in &self.activation_groups {
            if group.id.trim().is_empty() || !group_ids.insert(group.id.as_str()) {
                return Err(AttributeConfigurationError::InvalidActivationGroupId(
                    group.id.clone(),
                ));
            }
            if group.label.trim().is_empty() {
                return Err(AttributeConfigurationError::EmptyActivationGroupLabel(
                    group.id.clone(),
                ));
            }
            if group.members.is_empty() {
                return Err(AttributeConfigurationError::EmptyActivationGroup(
                    group.id.clone(),
                ));
            }
            let mut members = HashSet::new();
            let mut encoder_group = None;
            for member in &group.members {
                let id = member.0.as_str();
                if !members.insert(id) {
                    return Err(AttributeConfigurationError::DuplicateActivationMember {
                        group: group.id.clone(),
                        attribute: id.to_owned(),
                    });
                }
                let Some((value_type, recordable)) = descriptors.get(id).copied() else {
                    return Err(AttributeConfigurationError::UnknownActivationMember {
                        group: group.id.clone(),
                        attribute: id.to_owned(),
                    });
                };
                if !recordable || value_type == AttributeValueType::Control {
                    return Err(AttributeConfigurationError::IneligibleActivationMember(
                        id.to_owned(),
                    ));
                }
                let member_encoder_group = placements
                    .get(id)
                    .map(|placement| placement.group)
                    .or_else(|| (id == "color").then_some(EncoderGroup::Color))
                    .ok_or_else(|| AttributeConfigurationError::MissingPlacement(id.to_owned()))?;
                if encoder_group
                    .replace(member_encoder_group)
                    .is_some_and(|expected| expected != member_encoder_group)
                {
                    return Err(AttributeConfigurationError::CrossEncoderActivationGroup {
                        group: group.id.clone(),
                        attribute: id.to_owned(),
                    });
                }
                if !activated.insert(id) {
                    return Err(AttributeConfigurationError::OverlappingActivationGroup(
                        id.to_owned(),
                    ));
                }
            }
        }
        for (id, (value_type, recordable)) in descriptors {
            if !built_in_attribute_is_retired(id)
                && !built_in_attribute_is_special_dialog_only(id)
                && recordable
                && value_type != AttributeValueType::Control
                && !activated.contains(id)
            {
                return Err(AttributeConfigurationError::MissingActivationGroup(
                    id.to_owned(),
                ));
            }
        }
        Ok(())
    }

    pub fn placement_for(&self, attribute: &AttributeKey) -> Option<EncoderPlacement> {
        self.attribute_placement_for(attribute)
            .map(|placement| placement.encoder)
    }

    pub fn attribute_placement_for(&self, attribute: &AttributeKey) -> Option<&AttributePlacement> {
        self.placements
            .iter()
            .find(|placement| placement.attribute == *attribute)
    }

    pub fn activation_group_for(
        &self,
        attribute: &AttributeKey,
    ) -> Option<&AttributeActivationGroup> {
        self.activation_groups
            .iter()
            .find(|group| group.members.contains(attribute))
    }

    /// Symmetric linked-member lookup in the stable order authored for each activation group.
    /// Single-member groups intentionally produce an empty link list.
    pub fn activation_links(&self) -> HashMap<AttributeKey, Vec<AttributeKey>> {
        self.activation_groups
            .iter()
            .flat_map(|group| {
                group.members.iter().cloned().map(|member| {
                    let linked = group
                        .members
                        .iter()
                        .filter(|candidate| **candidate != member)
                        .cloned()
                        .collect();
                    (member, linked)
                })
            })
            .collect()
    }
}

fn legacy_singleton_group(group: &AttributeActivationGroup, source: &str) -> bool {
    group.id == source
        && group.members.as_slice() == [AttributeKey(source.into())]
        && group.label == attribute_descriptor(&AttributeKey(source.into())).label
}

fn next_custom_encoder_slot(
    placements: &[AttributePlacement],
    group: EncoderGroup,
) -> EncoderPlacement {
    let first_custom_page = recommended_builtin_placements()
        .into_iter()
        .filter(|placement| placement.encoder.group == group)
        .map(|placement| placement.encoder.page)
        .max()
        .unwrap_or(0)
        + 1;
    for page in first_custom_page..=u16::MAX {
        for slot in 1..=ENCODER_SLOTS_PER_PAGE {
            let candidate = EncoderPlacement::new(group, page, slot);
            if placements
                .iter()
                .all(|placement| placement.encoder != candidate)
            {
                return candidate;
            }
        }
    }
    unreachable!("u16 encoder pages cannot be exhausted by an in-memory configuration")
}

impl Default for AttributeConfiguration {
    fn default() -> Self {
        Self::recommended()
    }
}

fn validate_custom_descriptor(
    descriptor: &CustomAttributeDescriptor,
    built_in_ids: &HashSet<&str>,
) -> Result<(), AttributeConfigurationError> {
    let id = descriptor.id.0.as_str();
    if built_in_ids.contains(id) {
        return Err(AttributeConfigurationError::BuiltInShadow(id.to_owned()));
    }
    if !valid_custom_attribute_id(id) {
        return Err(AttributeConfigurationError::InvalidCustomId(id.to_owned()));
    }
    if descriptor.label.trim().is_empty() {
        return Err(AttributeConfigurationError::EmptyCustomLabel(id.to_owned()));
    }
    if descriptor.value_type == AttributeValueType::Control && descriptor.recordable {
        return Err(AttributeConfigurationError::RecordableControl(
            id.to_owned(),
        ));
    }
    if descriptor
        .normalized_bounds
        .into_iter()
        .chain(descriptor.domain_bounds)
        .any(|bounds| {
            !bounds.min.is_finite() || !bounds.max.is_finite() || bounds.min >= bounds.max
        })
        || (descriptor.normalized_bounds.is_some()
            && descriptor.value_type != AttributeValueType::Continuous)
    {
        return Err(AttributeConfigurationError::InvalidCustomBounds(
            id.to_owned(),
        ));
    }
    Ok(())
}

pub(super) fn valid_custom_attribute_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.split('.').count() >= 2
        && id.split('.').all(|segment| {
            !segment.is_empty()
                && segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'_' | b'-')
                })
        })
}

fn recommended_activation_group(
    id: &str,
    label: &str,
    members: &[&str],
) -> AttributeActivationGroup {
    AttributeActivationGroup {
        id: id.to_owned(),
        label: label.to_owned(),
        members: members
            .iter()
            .map(|member| AttributeKey((*member).to_owned()))
            .collect(),
    }
}

mod placements;
use placements::recommended_builtin_placements;

/// Built-in attribute registry. Custom attributes remain valid and use their persisted identifier
/// as the operator label until a desk extension supplies richer metadata.
pub const ATTRIBUTE_REGISTRY: &[AttributeDescriptor] = &[
    continuous(
        "intensity",
        "Intensity",
        AttributeClass::Intensity,
        "percent",
    ),
    indexed("shutter", "Shutter / Strobe", AttributeClass::Intensity),
    continuous("strobe", "Strobe", AttributeClass::Intensity, "hz"),
    continuous("volume", "Volume", AttributeClass::Intensity, "percent"),
    color("color", "Color", AttributeClass::Color),
    continuous("color.red", "Red", AttributeClass::Color, "percent"),
    continuous("color.green", "Green", AttributeClass::Color, "percent"),
    continuous("color.blue", "Blue", AttributeClass::Color, "percent"),
    continuous("color.cyan", "Cyan", AttributeClass::Color, "percent"),
    continuous("color.magenta", "Magenta", AttributeClass::Color, "percent"),
    continuous("color.yellow", "Yellow", AttributeClass::Color, "percent"),
    continuous("color.amber", "Amber", AttributeClass::Color, "percent"),
    continuous("color.white", "White", AttributeClass::Color, "percent"),
    continuous("color.uv", "UV", AttributeClass::Color, "percent"),
    continuous(
        "color.cold_white",
        "Cold White",
        AttributeClass::Color,
        "percent",
    ),
    continuous(
        "color.warm_white",
        "Warm White",
        AttributeClass::Color,
        "percent",
    ),
    continuous("color.lime", "Lime", AttributeClass::Color, "percent"),
    continuous("color.indigo", "Indigo", AttributeClass::Color, "percent"),
    continuous("color.mint", "Mint", AttributeClass::Color, "percent"),
    continuous(
        "color.temperature",
        "Color Temperature",
        AttributeClass::Color,
        "K",
    ),
    continuous("color.tint", "Tint", AttributeClass::Color, "percent"),
    projection_continuous("color.hue", "Hue", AttributeClass::Color, "deg"),
    projection_continuous(
        "color.saturation",
        "Saturation",
        AttributeClass::Color,
        "percent",
    ),
    projection_continuous(
        "color.brightness",
        "Color Brightness",
        AttributeClass::Color,
        "percent",
    ),
    indexed("color.wheel.1", "Color Wheel 1", AttributeClass::Color),
    continuous(
        "color.wheel.1.rotation",
        "Color Wheel 1 Rotation",
        AttributeClass::Color,
        "percent",
    ),
    indexed("color.wheel.2", "Color Wheel 2", AttributeClass::Color),
    continuous(
        "color.wheel.2.rotation",
        "Color Wheel 2 Rotation",
        AttributeClass::Color,
        "percent",
    ),
    continuous("pan", "Pan", AttributeClass::Position, "deg"),
    continuous("tilt", "Tilt", AttributeClass::Position, "deg"),
    continuous(
        "pan.continuous",
        "Continuous Pan",
        AttributeClass::Position,
        "percent",
    ),
    continuous(
        "tilt.continuous",
        "Continuous Tilt",
        AttributeClass::Position,
        "percent",
    ),
    continuous("pan.time", "Pan Time", AttributeClass::Position, "s"),
    continuous("tilt.time", "Tilt Time", AttributeClass::Position, "s"),
    continuous(
        "position.time",
        "Pan/Tilt Time",
        AttributeClass::Position,
        "s",
    ),
    continuous(
        "position.speed",
        "Position Speed",
        AttributeClass::Position,
        "percent",
    ),
    indexed("position.mode", "Position Mode", AttributeClass::Position),
    continuous(
        "position.movement",
        "Position Movement",
        AttributeClass::Position,
        "percent",
    ),
    cyclic_continuous(
        "position.rotation",
        "Head Rotation",
        AttributeClass::Position,
        "deg",
    ),
    indexed("gobo.1", "Gobo 1", AttributeClass::Beam),
    continuous(
        "gobo.1.rotation",
        "Gobo 1 Rotation",
        AttributeClass::Beam,
        "percent",
    ),
    indexed("gobo.2", "Gobo 2", AttributeClass::Beam),
    continuous(
        "gobo.2.rotation",
        "Gobo 2 Rotation",
        AttributeClass::Beam,
        "percent",
    ),
    indexed("prism.1", "Prism 1", AttributeClass::Beam),
    continuous(
        "prism.1.rotation",
        "Prism 1 Rotation",
        AttributeClass::Beam,
        "percent",
    ),
    indexed("prism.2", "Prism 2", AttributeClass::Beam),
    continuous(
        "prism.2.rotation",
        "Prism 2 Rotation",
        AttributeClass::Beam,
        "percent",
    ),
    indexed("animation.1", "Animation Wheel 1", AttributeClass::Beam),
    continuous(
        "animation.1.rotation",
        "Animation Rotation 1",
        AttributeClass::Beam,
        "percent",
    ),
    indexed("beam.effect.1", "Beam Effect 1", AttributeClass::Beam),
    indexed("beam.effect.2", "Beam Effect 2", AttributeClass::Beam),
    continuous("beam", "Beam", AttributeClass::Beam, "percent"),
    continuous("iris", "Iris", AttributeClass::Shapers, "percent"),
    continuous(
        "shaper.blade.1.position",
        "Blade 1 Position",
        AttributeClass::Shapers,
        "percent",
    ),
    continuous(
        "shaper.blade.1.angle",
        "Blade 1 Angle",
        AttributeClass::Shapers,
        "deg",
    ),
    continuous(
        "shaper.blade.2.position",
        "Blade 2 Position",
        AttributeClass::Shapers,
        "percent",
    ),
    continuous(
        "shaper.blade.2.angle",
        "Blade 2 Angle",
        AttributeClass::Shapers,
        "deg",
    ),
    cyclic_continuous(
        "shaper.rotation",
        "Shaper Rotation",
        AttributeClass::Shapers,
        "deg",
    ),
    continuous(
        "shaper.blade.3.position",
        "Blade 3 Position",
        AttributeClass::Shapers,
        "percent",
    ),
    continuous(
        "shaper.blade.3.angle",
        "Blade 3 Angle",
        AttributeClass::Shapers,
        "deg",
    ),
    continuous(
        "shaper.blade.4.position",
        "Blade 4 Position",
        AttributeClass::Shapers,
        "percent",
    ),
    continuous(
        "shaper.blade.4.angle",
        "Blade 4 Angle",
        AttributeClass::Shapers,
        "deg",
    ),
    continuous(
        "shaper.keystone.x",
        "Keystone X",
        AttributeClass::Shapers,
        "percent",
    ),
    continuous(
        "shaper.keystone.y",
        "Keystone Y",
        AttributeClass::Shapers,
        "percent",
    ),
    continuous("focus", "Focus", AttributeClass::Focus, "percent"),
    continuous("zoom", "Zoom", AttributeClass::Focus, "deg"),
    continuous("softness", "Softness", AttributeClass::Focus, "percent"),
    continuous("frost.1", "Frost 1", AttributeClass::Focus, "percent"),
    continuous("frost.2", "Frost 2", AttributeClass::Focus, "percent"),
    continuous("beam.edge", "Beam Edge", AttributeClass::Focus, "percent"),
    indexed("control.mode", "Fixture Mode", AttributeClass::Control),
    continuous(
        "control.speed",
        "Fixture Control Speed",
        AttributeClass::Control,
        "percent",
    ),
    control("control", "Control", AttributeClass::Control),
    indexed("media.folder", "Media Folder", AttributeClass::Media),
    indexed("media.file", "Media File", AttributeClass::Media),
    indexed("media.mask.folder", "Mask Folder", AttributeClass::Media),
    indexed("media.mask.file", "Mask File", AttributeClass::Media),
    continuous(
        "media.opacity",
        "Layer Opacity",
        AttributeClass::Media,
        "percent",
    ),
    color("media.tint", "Layer Tint", AttributeClass::Media),
    indexed("media.play_mode", "Play Mode", AttributeClass::Media),
    continuous(
        "media.playback_speed",
        "Playback Speed",
        AttributeClass::Media,
        "percent",
    ),
    continuous(
        "media.playback_bpm",
        "Playback BPM",
        AttributeClass::Media,
        "bpm",
    ),
    continuous(
        "media.grayscale",
        "Grayscale",
        AttributeClass::Media,
        "percent",
    ),
    indexed("media.scaling_mode", "Scaling Mode", AttributeClass::Media),
    cyclic_continuous(
        "media.rotation",
        "Layer Rotation",
        AttributeClass::Media,
        "deg",
    ),
    continuous(
        "media.position.x",
        "Position X",
        AttributeClass::Media,
        "percent",
    ),
    continuous(
        "media.position.y",
        "Position Y",
        AttributeClass::Media,
        "percent",
    ),
    continuous("media.scale.x", "Scale X", AttributeClass::Media, "percent"),
    continuous("media.scale.y", "Scale Y", AttributeClass::Media, "percent"),
    continuous(
        "media.mask.opacity",
        "Mask Opacity",
        AttributeClass::Media,
        "percent",
    ),
    indexed("media.mask.invert", "Invert Mask", AttributeClass::Media),
    indexed("media.effect.1", "Media Effect 1", AttributeClass::Media),
    indexed("media.effect.2", "Media Effect 2", AttributeClass::Media),
    indexed("media.effect.3", "Media Effect 3", AttributeClass::Media),
    indexed("media.effect.4", "Media Effect 4", AttributeClass::Media),
];

const fn continuous(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
    unit: &'static str,
) -> AttributeDescriptor {
    descriptor(
        id,
        label,
        family,
        AttributeValueType::Continuous,
        Some(unit),
    )
}

const fn projection_continuous(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
    display_unit: &'static str,
) -> AttributeDescriptor {
    let mut descriptor = continuous(id, label, family, display_unit);
    descriptor.recordable = false;
    descriptor.normalized_bounds = None;
    descriptor
}

const fn cyclic_continuous(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
    unit: &'static str,
) -> AttributeDescriptor {
    let mut result = continuous(id, label, family, unit);
    result.cyclic = true;
    result
}

const fn indexed(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
) -> AttributeDescriptor {
    descriptor(id, label, family, AttributeValueType::Indexed, None)
}

const fn color(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
) -> AttributeDescriptor {
    descriptor(id, label, family, AttributeValueType::Color, None)
}

const fn control(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
) -> AttributeDescriptor {
    descriptor(id, label, family, AttributeValueType::Control, None)
}

const fn descriptor(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
    value_type: AttributeValueType,
    default_unit: Option<&'static str>,
) -> AttributeDescriptor {
    let normalized_bounds = match value_type {
        AttributeValueType::Continuous => Some(AttributeBounds { min: 0.0, max: 1.0 }),
        AttributeValueType::Color | AttributeValueType::Indexed | AttributeValueType::Control => {
            None
        }
    };
    AttributeDescriptor {
        id,
        label,
        family,
        value_type,
        default_unit,
        display_unit: default_unit,
        physical_unit: default_unit,
        normalized_bounds,
        domain_bounds: None,
        cyclic: false,
        recordable: !matches!(value_type, AttributeValueType::Control),
    }
}

pub fn attribute_descriptor<'a>(key: &'a AttributeKey) -> ResolvedAttributeDescriptor<'a> {
    ATTRIBUTE_REGISTRY
        .iter()
        .find(|descriptor| descriptor.id == key.0)
        .map(resolved_descriptor)
        .unwrap_or_else(|| custom_descriptor(key))
}

const fn resolved_descriptor(
    descriptor: &'static AttributeDescriptor,
) -> ResolvedAttributeDescriptor<'static> {
    ResolvedAttributeDescriptor {
        id: descriptor.id,
        label: descriptor.label,
        family: descriptor.family,
        value_type: descriptor.value_type,
        display_unit: descriptor.display_unit,
        physical_unit: descriptor.physical_unit,
        normalized_bounds: descriptor.normalized_bounds,
        domain_bounds: descriptor.domain_bounds,
        cyclic: descriptor.cyclic,
        recordable: descriptor.recordable,
        built_in: true,
    }
}

fn custom_descriptor(key: &AttributeKey) -> ResolvedAttributeDescriptor<'_> {
    ResolvedAttributeDescriptor {
        id: &key.0,
        label: &key.0,
        family: AttributeClass::Custom,
        value_type: AttributeValueType::Continuous,
        display_unit: None,
        physical_unit: None,
        normalized_bounds: None,
        domain_bounds: None,
        cyclic: false,
        recordable: false,
        built_in: false,
    }
}
