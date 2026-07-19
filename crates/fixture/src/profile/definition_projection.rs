use super::{
    ColorSystem, FIXTURE_PROFILE_SCHEMA_VERSION, FixtureChannel, FixtureMode, FixtureProfile,
    ProfileError,
};
use crate::{
    ByteOrder, ChannelComponent, ColorCalibration, EmitterCalibration, FixtureDefinition,
    FixturePhysicalProperties, LogicalHead, Parameter, ParameterMetadata,
};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

impl FixtureProfile {
    pub fn resolved_definition(&self, mode_id: Uuid) -> Result<FixtureDefinition, ProfileError> {
        self.validate()?;
        self.project_definition(mode_id, SnapshotScope::Full)
    }

    /// Projects one mode after the caller has validated this profile.
    ///
    /// The runtime snapshot contains only the selected mode. This is intended for compilers that
    /// cache a validated immutable profile revision and must not clone every unrelated mode for
    /// each patched fixture.
    pub(crate) fn compact_resolved_definition_from_validated_profile(
        &self,
        mode_id: Uuid,
    ) -> Result<FixtureDefinition, ProfileError> {
        self.project_definition(mode_id, SnapshotScope::SelectedMode)
    }

    fn project_definition(
        &self,
        mode_id: Uuid,
        snapshot_scope: SnapshotScope,
    ) -> Result<FixtureDefinition, ProfileError> {
        let mode = self.required_mode(mode_id)?;
        let primary_slots = mode.primary_slots()?;
        let snapshot = self.snapshot_for(mode, snapshot_scope);
        Ok(build_definition(self, mode, &primary_slots, snapshot))
    }

    fn required_mode(&self, mode_id: Uuid) -> Result<&FixtureMode, ProfileError> {
        self.mode(mode_id)
            .ok_or_else(|| ProfileError::Invalid("mode does not exist".into()))
    }

    fn snapshot_for(&self, mode: &FixtureMode, scope: SnapshotScope) -> FixtureProfile {
        let modes = match scope {
            SnapshotScope::Full => self.modes.clone(),
            SnapshotScope::SelectedMode => vec![mode.clone()],
        };
        self.snapshot_with_modes(modes)
    }

    fn snapshot_with_modes(&self, modes: Vec<FixtureMode>) -> FixtureProfile {
        FixtureProfile {
            schema_version: self.schema_version,
            id: self.id,
            revision: self.revision,
            manufacturer: self.manufacturer.clone(),
            name: self.name.clone(),
            short_name: self.short_name.clone(),
            fixture_type: self.fixture_type.clone(),
            patch_policy: self.patch_policy,
            notes: self.notes.clone(),
            photograph_asset: self.photograph_asset.clone(),
            stage_icon_asset: self.stage_icon_asset.clone(),
            model_asset: self.model_asset.clone(),
            model_units: self.model_units,
            physical: self.physical.clone(),
            modes,
            hazardous: self.hazardous,
            direct_control_protocols: self.direct_control_protocols.clone(),
            signal_loss_policy: self.signal_loss_policy,
            reserved_source: self.reserved_source.clone(),
        }
    }
}

#[derive(Clone, Copy)]
enum SnapshotScope {
    Full,
    SelectedMode,
}

fn build_definition(
    profile: &FixtureProfile,
    mode: &FixtureMode,
    primary_slots: &HashMap<Uuid, u16>,
    snapshot: FixtureProfile,
) -> FixtureDefinition {
    FixtureDefinition {
        schema_version: FIXTURE_PROFILE_SCHEMA_VERSION,
        id: profile.id,
        revision: profile.revision,
        manufacturer: profile.manufacturer.clone(),
        device_type: profile.fixture_type.clone(),
        name: profile.name.clone(),
        model: profile.short_name.clone(),
        mode: mode.name.clone(),
        footprint: mode_footprint(mode),
        heads: logical_heads(mode, primary_slots),
        color_calibration: color_calibration(mode),
        physical: physical_properties(profile),
        model_asset: profile.model_asset.clone(),
        icon_asset: profile.stage_icon_asset.clone(),
        hazardous: profile.hazardous,
        direct_control_protocols: profile.direct_control_protocols.clone(),
        signal_loss_policy: profile.signal_loss_policy,
        safe_values: BTreeMap::new(),
        profile_id: Some(profile.id),
        mode_id: Some(mode.id),
        profile_snapshot: Some(Box::new(snapshot)),
    }
}

fn logical_heads(mode: &FixtureMode, primary_slots: &HashMap<Uuid, u16>) -> Vec<LogicalHead> {
    mode.heads
        .iter()
        .enumerate()
        .map(|(index, head)| LogicalHead {
            index: index as u16,
            name: head.name.clone(),
            shared: head.master_shared,
            parameters: mode
                .channels
                .iter()
                .filter(|channel| channel.head_id == head.id)
                .map(|channel| parameter(channel, primary_slots))
                .collect(),
        })
        .collect()
}

fn parameter(channel: &FixtureChannel, primary_slots: &HashMap<Uuid, u16>) -> Parameter {
    let slots =
        std::iter::once(primary_slots[&channel.id]).chain(channel.secondary_slots.iter().copied());
    let max = channel.resolution.max_raw();
    Parameter {
        attribute: channel.attribute.clone(),
        components: slots
            .map(|slot| ChannelComponent {
                offset: slot - 1,
                byte_order: ByteOrder::MsbFirst,
            })
            .collect(),
        default: channel.default_raw as f32 / max as f32,
        virtual_dimmer: channel.reacts_to_virtual_intensity,
        metadata: parameter_metadata(channel),
        capabilities: Vec::new(),
    }
}

fn parameter_metadata(channel: &FixtureChannel) -> ParameterMetadata {
    ParameterMetadata {
        physical_min: channel.physical_min.unwrap_or(0.0),
        physical_max: channel.physical_max.unwrap_or(1.0),
        unit: channel.unit.clone(),
        invert: channel.invert,
        ..Default::default()
    }
}

fn color_calibration(mode: &FixtureMode) -> Option<ColorCalibration> {
    mode.color_systems.iter().find_map(|system| {
        let ColorSystem::Additive { emitters } = &system.system else {
            return None;
        };
        Some(ColorCalibration {
            emitters: emitters
                .iter()
                .map(|emitter| EmitterCalibration {
                    name: emitter.name.clone(),
                    xyz: emitter.xyz,
                    limit: emitter.maximum_level,
                })
                .collect(),
            correction_matrix: system.correction_matrix,
        })
    })
}

fn mode_footprint(mode: &FixtureMode) -> u16 {
    mode.splits
        .iter()
        .find(|split| split.number == 1)
        .or_else(|| mode.splits.first())
        .map(|split| split.footprint)
        .unwrap_or(1)
}

fn physical_properties(profile: &FixtureProfile) -> FixturePhysicalProperties {
    FixturePhysicalProperties {
        width_millimetres: profile.physical.width_millimetres,
        height_millimetres: profile.physical.height_millimetres,
        depth_millimetres: profile.physical.depth_millimetres,
        weight_kilograms: profile.physical.weight_kilograms,
        power_watts: profile.physical.power_watts,
        ..Default::default()
    }
}
