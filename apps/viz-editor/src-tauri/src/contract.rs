//! The shape `@tosklight/patch` consumes.
//!
//! The patch sheet is host-agnostic: it speaks one feature contract and leaves the transport to
//! whoever hosts it. The desk reaches that contract over HTTP with its own wire DTOs; this
//! application is a single process, so its Tauri commands emit the contract directly. Nothing here
//! is a second definition of what a patch *is* — that stays in the application services — only of
//! how it crosses this one boundary.

use light_application::{
    PatchChange, PatchFixtureCandidate, PatchFixtureProjection, PatchFixturesCommand,
    PatchModeProjection, PatchProfileRevisionProjection, PatchSnapshot,
};
use light_core::{FixtureId, ShowId};
use light_fixture::{
    DirectControlEndpoint, FixtureLocation, FixtureVector, GelAssignment, GelDefinitionSnapshot,
    InstalledFixtureAppearance, InstalledLightSource, MultiPatchInstance, PatchedFixturePatch,
    PatchedFixtureProfileReference, PatchedHead, SplitPatch,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDto {
    pub show_id: Uuid,
    pub show_revision: u64,
    pub patch_revision: u64,
    pub cursor: u64,
    pub fixtures: Vec<FixtureDto>,
    pub profile_revisions: Vec<ProfileRevisionDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeDto {
    pub show_id: Uuid,
    pub show_revision: u64,
    pub patch_revision: u64,
    pub event_sequence: Option<u64>,
    pub fixtures: Vec<FixtureDto>,
    pub removed_fixture_ids: Vec<Uuid>,
    pub profile_revisions: Vec<ProfileRevisionDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeDto {
    #[serde(flatten)]
    pub change: ChangeDto,
    pub request_id: String,
    pub replayed: bool,
    pub changed: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitDto {
    pub split: u16,
    pub universe: Option<u16>,
    pub address: Option<u16>,
}

/// Position in whole millimetres, matching the stored patch. Rotation uses its own type.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationDto {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Rotation in degrees.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorDto {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectControlDto {
    pub protocol: String,
    pub ip_address: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightOverrideDto {
    pub channel_id: Uuid,
    pub raw_value: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledAppearanceDto {
    #[serde(default)]
    pub light_source: InstalledLightSourceDto,
    #[serde(default)]
    pub color_temperature_kelvin: Option<u32>,
    #[serde(default)]
    pub gel: GelAssignmentDto,
    #[serde(default)]
    pub shaper_angles_degrees: [f32; 4],
}

impl Default for InstalledAppearanceDto {
    fn default() -> Self {
        Self {
            light_source: InstalledLightSourceDto::ProfileDefault,
            color_temperature_kelvin: None,
            gel: GelAssignmentDto::OpenWhite,
            shaper_angles_degrees: [0.0; 4],
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstalledLightSourceDto {
    #[default]
    ProfileDefault,
    Tungsten,
    Halogen,
    Discharge,
    Led,
    Fluorescent,
    Arc,
    Other {
        label: String,
    },
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GelAssignmentDto {
    #[default]
    OpenWhite,
    BuiltIn {
        #[serde(rename = "catalogId")]
        catalog_id: String,
        #[serde(rename = "entryId")]
        entry_id: String,
        #[serde(rename = "embeddedFallback")]
        embedded_fallback: GelDefinitionSnapshotDto,
    },
    Custom {
        name: String,
        #[serde(rename = "colorSrgb")]
        color_srgb: String,
        note: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GelDefinitionSnapshotDto {
    pub number: String,
    pub name: String,
    pub display_srgb: String,
    pub visualizer_srgb: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultipatchDto {
    pub id: Uuid,
    pub name: String,
    pub split_patches: Vec<SplitDto>,
    pub location: LocationDto,
    pub rotation: VectorDto,
    #[serde(default)]
    pub invert_pan: bool,
    #[serde(default)]
    pub invert_tilt: bool,
    #[serde(default)]
    pub bracket_angle: f32,
    #[serde(default)]
    pub shaper_angle: Option<f32>,
    #[serde(default)]
    pub installed_appearance: InstalledAppearanceDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalHeadDto {
    pub profile_head_id: Option<Uuid>,
    pub head_index: u32,
    pub fixture_id: Uuid,
}

/// One patched fixture, in both directions: the sheet writes this shape and reads it back.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureDto {
    pub fixture_id: Uuid,
    pub fixture_number: Option<u32>,
    pub virtual_fixture_number: Option<u32>,
    pub name: String,
    pub profile_id: Uuid,
    pub profile_revision: u64,
    pub mode_id: Uuid,
    pub split_patches: Vec<SplitDto>,
    pub layer_id: String,
    pub direct_control: Option<DirectControlDto>,
    pub location: LocationDto,
    pub rotation: VectorDto,
    #[serde(default)]
    pub multipatch: Vec<MultipatchDto>,
    #[serde(default = "yes")]
    pub group_masters_enabled: bool,
    #[serde(default = "yes")]
    pub grand_master_enabled: bool,
    #[serde(default)]
    pub invert_pan: bool,
    #[serde(default)]
    pub invert_tilt: bool,
    /// Degrees the mounting bracket is set to, positive nose-down.
    #[serde(default)]
    pub bracket_angle: f32,
    /// Degrees a fitted shaper or barn-door module is turned to, absent when none is fitted.
    #[serde(default)]
    pub shaper_angle: Option<f32>,
    #[serde(default)]
    pub installed_appearance: InstalledAppearanceDto,
    #[serde(default = "yes")]
    pub move_in_black_enabled: bool,
    #[serde(default)]
    pub move_in_black_delay_millis: u64,
    #[serde(default)]
    pub highlight_overrides: Vec<HighlightOverrideDto>,
    /// Read-only projections the sheet displays but never writes.
    #[serde(default, skip_deserializing)]
    pub fixture_revision: u64,
    #[serde(default, skip_deserializing)]
    pub logical_heads: Vec<LogicalHeadDto>,
}

const fn yes() -> bool {
    true
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeDto {
    pub mode_id: Uuid,
    pub name: String,
    pub splits: Vec<ModeSplitDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeSplitDto {
    pub split: u16,
    pub footprint: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRevisionDto {
    pub profile_id: Uuid,
    pub profile_revision: u64,
    pub content_digest: String,
    pub manufacturer: String,
    pub name: String,
    pub fixture_type: String,
    pub patch_policy: String,
    pub referenced_modes: Vec<ModeDto>,
    pub profile_snapshot: serde_json::Value,
}

/// One requested patch, as the sheet submits it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationDto {
    pub request_id: String,
    #[serde(default)]
    pub fixtures: Vec<FixtureDto>,
    #[serde(default)]
    pub remove_fixture_ids: Vec<Uuid>,
}

impl From<PatchSnapshot> for SnapshotDto {
    fn from(snapshot: PatchSnapshot) -> Self {
        Self {
            show_id: snapshot.show_id.0,
            show_revision: snapshot.show_revision.value(),
            patch_revision: snapshot.patch_revision.value(),
            cursor: snapshot.event_sequence,
            fixtures: snapshot
                .fixtures
                .into_iter()
                .map(FixtureDto::from)
                .collect(),
            profile_revisions: snapshot
                .profile_revisions
                .into_iter()
                .map(ProfileRevisionDto::from)
                .collect(),
        }
    }
}

impl ChangeDto {
    pub fn new(change: PatchChange, event_sequence: Option<u64>) -> Self {
        Self {
            show_id: change.show_id.0,
            show_revision: change.show_revision.value(),
            patch_revision: change.patch_revision.value(),
            event_sequence,
            fixtures: change.fixtures.into_iter().map(FixtureDto::from).collect(),
            removed_fixture_ids: change
                .removed_fixture_ids
                .into_iter()
                .map(|id| id.0)
                .collect(),
            profile_revisions: change
                .profile_revisions
                .into_iter()
                .map(ProfileRevisionDto::from)
                .collect(),
        }
    }
}

impl From<PatchFixtureProjection> for FixtureDto {
    fn from(projection: PatchFixtureProjection) -> Self {
        let patch = projection.patch;
        Self {
            fixture_id: patch.fixture_id.0,
            fixture_number: patch.fixture_number,
            virtual_fixture_number: patch.virtual_fixture_number,
            name: patch.name,
            profile_id: projection.profile.profile_id.0,
            profile_revision: projection.profile.profile_revision,
            mode_id: projection.profile.mode_id,
            split_patches: patch.split_patches.iter().map(SplitDto::from).collect(),
            layer_id: patch.layer_id,
            direct_control: patch.direct_control.as_ref().map(DirectControlDto::from),
            location: LocationDto::from(&patch.location),
            rotation: VectorDto::from(&patch.rotation),
            multipatch: patch.multipatch.iter().map(MultipatchDto::from).collect(),
            group_masters_enabled: patch.group_masters_enabled,
            grand_master_enabled: patch.grand_master_enabled,
            invert_pan: patch.invert_pan,
            invert_tilt: patch.invert_tilt,
            bracket_angle: patch.bracket_angle,
            shaper_angle: patch.shaper_angle,
            installed_appearance: InstalledAppearanceDto::from(&patch.installed_appearance),
            move_in_black_enabled: patch.move_in_black_enabled,
            move_in_black_delay_millis: patch.move_in_black_delay_millis,
            highlight_overrides: patch
                .highlight_overrides
                .iter()
                .map(|(channel_id, raw_value)| HighlightOverrideDto {
                    channel_id: *channel_id,
                    raw_value: *raw_value,
                })
                .collect(),
            fixture_revision: projection.fixture_revision,
            logical_heads: patch
                .logical_heads
                .iter()
                .enumerate()
                .map(|(index, head)| LogicalHeadDto {
                    profile_head_id: head.profile_head_id,
                    head_index: u32::try_from(index).unwrap_or(0),
                    fixture_id: head.fixture_id.0,
                })
                .collect(),
        }
    }
}

impl From<PatchProfileRevisionProjection> for ProfileRevisionDto {
    fn from(profile: PatchProfileRevisionProjection) -> Self {
        Self {
            profile_id: profile.profile_id.0,
            profile_revision: profile.profile_revision,
            content_digest: profile.content_digest,
            manufacturer: profile.manufacturer,
            name: profile.name,
            fixture_type: profile.fixture_type,
            patch_policy: match profile.patch_policy {
                light_fixture::PatchPolicy::Dmx => "dmx".to_owned(),
                light_fixture::PatchPolicy::VisualOnly => "visual_only".to_owned(),
            },
            referenced_modes: profile
                .referenced_modes
                .into_iter()
                .map(ModeDto::from)
                .collect(),
            profile_snapshot: profile.profile_snapshot,
        }
    }
}

impl From<PatchModeProjection> for ModeDto {
    fn from(mode: PatchModeProjection) -> Self {
        Self {
            mode_id: mode.mode_id,
            name: mode.name,
            splits: mode
                .splits
                .into_iter()
                .map(|split| ModeSplitDto {
                    split: split.number,
                    footprint: split.footprint,
                })
                .collect(),
        }
    }
}

impl From<&SplitPatch> for SplitDto {
    fn from(patch: &SplitPatch) -> Self {
        Self {
            split: patch.split,
            universe: patch.universe,
            address: patch.address,
        }
    }
}

impl From<&FixtureLocation> for LocationDto {
    fn from(value: &FixtureLocation) -> Self {
        Self {
            x: value.x,
            y: value.y,
            z: value.z,
        }
    }
}

impl From<&FixtureVector> for VectorDto {
    fn from(value: &FixtureVector) -> Self {
        Self {
            x: value.x,
            y: value.y,
            z: value.z,
        }
    }
}

impl From<&DirectControlEndpoint> for DirectControlDto {
    fn from(value: &DirectControlEndpoint) -> Self {
        Self {
            protocol: "citp".to_owned(),
            ip_address: value.ip_address.to_string(),
            port: value.port,
        }
    }
}

impl From<&MultiPatchInstance> for MultipatchDto {
    fn from(value: &MultiPatchInstance) -> Self {
        Self {
            id: value.id,
            name: value.name.clone(),
            split_patches: value.split_patches.iter().map(SplitDto::from).collect(),
            location: LocationDto::from(&value.location),
            rotation: VectorDto::from(&value.rotation),
            invert_pan: value.invert_pan,
            invert_tilt: value.invert_tilt,
            bracket_angle: value.bracket_angle,
            shaper_angle: value.shaper_angle,
            installed_appearance: InstalledAppearanceDto::from(&value.installed_appearance),
        }
    }
}

impl MutationDto {
    /// Turns one submitted mutation into the application command.
    pub fn into_command(self, show_id: ShowId) -> PatchFixturesCommand {
        PatchFixturesCommand {
            show_id,
            fixtures: self
                .fixtures
                .into_iter()
                .map(PatchFixtureCandidate::from)
                .collect(),
            remove_fixture_ids: self.remove_fixture_ids.into_iter().map(FixtureId).collect(),
            placements: Vec::new(),
            vector_spreads: Vec::new(),
            fixture_updates: Vec::new(),
        }
    }
}

impl From<FixtureDto> for PatchFixtureCandidate {
    fn from(dto: FixtureDto) -> Self {
        Self {
            profile: PatchedFixtureProfileReference {
                profile_id: FixtureId(dto.profile_id),
                profile_revision: dto.profile_revision,
                mode_id: dto.mode_id,
            },
            patch: PatchedFixturePatch {
                fixture_id: FixtureId(dto.fixture_id),
                fixture_number: dto.fixture_number,
                virtual_fixture_number: dto.virtual_fixture_number,
                name: dto.name,
                universe: dto.split_patches.first().and_then(|split| split.universe),
                address: dto.split_patches.first().and_then(|split| split.address),
                split_patches: dto.split_patches.iter().map(SplitPatch::from).collect(),
                layer_id: dto.layer_id,
                // An unparseable address drops the endpoint rather than failing the whole patch;
                // direct control is an optional extra, not the fixture's identity.
                direct_control: dto.direct_control.and_then(|value| {
                    value
                        .ip_address
                        .parse()
                        .ok()
                        .map(|ip_address| DirectControlEndpoint {
                            protocol: light_fixture::DirectControlProtocol::Citp,
                            ip_address,
                            port: value.port,
                        })
                }),
                location: FixtureLocation {
                    x: dto.location.x,
                    y: dto.location.y,
                    z: dto.location.z,
                },
                rotation: FixtureVector {
                    x: dto.rotation.x,
                    y: dto.rotation.y,
                    z: dto.rotation.z,
                },
                logical_heads: Vec::<PatchedHead>::new(),
                multipatch: dto
                    .multipatch
                    .into_iter()
                    .map(MultiPatchInstance::from)
                    .collect(),
                group_masters_enabled: dto.group_masters_enabled,
                grand_master_enabled: dto.grand_master_enabled,
                invert_pan: dto.invert_pan,
                invert_tilt: dto.invert_tilt,
                bracket_angle: dto.bracket_angle,
                shaper_angle: dto.shaper_angle,
                installed_appearance: InstalledFixtureAppearance::from(dto.installed_appearance),
                move_in_black_enabled: dto.move_in_black_enabled,
                move_in_black_delay_millis: dto.move_in_black_delay_millis,
                highlight_overrides: dto
                    .highlight_overrides
                    .into_iter()
                    .map(|value| (value.channel_id, value.raw_value))
                    .collect::<BTreeMap<_, _>>(),
            },
        }
    }
}

impl From<&SplitDto> for SplitPatch {
    fn from(dto: &SplitDto) -> Self {
        Self {
            split: dto.split,
            universe: dto.universe,
            address: dto.address,
        }
    }
}

impl From<MultipatchDto> for MultiPatchInstance {
    fn from(dto: MultipatchDto) -> Self {
        Self {
            id: dto.id,
            name: dto.name,
            // Split 1 remains the canonical legacy address for an instance, as it is for a fixture.
            universe: dto.split_patches.first().and_then(|split| split.universe),
            address: dto.split_patches.first().and_then(|split| split.address),
            split_patches: dto.split_patches.iter().map(SplitPatch::from).collect(),
            location: FixtureLocation {
                x: dto.location.x,
                y: dto.location.y,
                z: dto.location.z,
            },
            rotation: FixtureVector {
                x: dto.rotation.x,
                y: dto.rotation.y,
                z: dto.rotation.z,
            },
            invert_pan: dto.invert_pan,
            invert_tilt: dto.invert_tilt,
            bracket_angle: dto.bracket_angle,
            shaper_angle: dto.shaper_angle,
            installed_appearance: InstalledFixtureAppearance::from(dto.installed_appearance),
        }
    }
}

impl From<&InstalledFixtureAppearance> for InstalledAppearanceDto {
    fn from(value: &InstalledFixtureAppearance) -> Self {
        Self {
            light_source: match &value.light_source {
                InstalledLightSource::ProfileDefault => InstalledLightSourceDto::ProfileDefault,
                InstalledLightSource::Tungsten => InstalledLightSourceDto::Tungsten,
                InstalledLightSource::Halogen => InstalledLightSourceDto::Halogen,
                InstalledLightSource::Discharge => InstalledLightSourceDto::Discharge,
                InstalledLightSource::Led => InstalledLightSourceDto::Led,
                InstalledLightSource::Fluorescent => InstalledLightSourceDto::Fluorescent,
                InstalledLightSource::Arc => InstalledLightSourceDto::Arc,
                InstalledLightSource::Other { label } => InstalledLightSourceDto::Other {
                    label: label.clone(),
                },
            },
            color_temperature_kelvin: value.color_temperature_kelvin,
            gel: match &value.gel {
                GelAssignment::OpenWhite => GelAssignmentDto::OpenWhite,
                GelAssignment::BuiltIn {
                    catalog_id,
                    entry_id,
                    embedded_fallback,
                } => GelAssignmentDto::BuiltIn {
                    catalog_id: catalog_id.clone(),
                    entry_id: entry_id.clone(),
                    embedded_fallback: GelDefinitionSnapshotDto {
                        number: embedded_fallback.number.clone(),
                        name: embedded_fallback.name.clone(),
                        display_srgb: embedded_fallback.display_srgb.clone(),
                        visualizer_srgb: embedded_fallback.visualizer_srgb.clone(),
                    },
                },
                GelAssignment::Custom {
                    name,
                    color_srgb,
                    note,
                } => GelAssignmentDto::Custom {
                    name: name.clone(),
                    color_srgb: color_srgb.clone(),
                    note: note.clone(),
                },
            },
            shaper_angles_degrees: value.shaper_angles_degrees,
        }
    }
}

impl From<InstalledAppearanceDto> for InstalledFixtureAppearance {
    fn from(value: InstalledAppearanceDto) -> Self {
        Self {
            light_source: match value.light_source {
                InstalledLightSourceDto::ProfileDefault => InstalledLightSource::ProfileDefault,
                InstalledLightSourceDto::Tungsten => InstalledLightSource::Tungsten,
                InstalledLightSourceDto::Halogen => InstalledLightSource::Halogen,
                InstalledLightSourceDto::Discharge => InstalledLightSource::Discharge,
                InstalledLightSourceDto::Led => InstalledLightSource::Led,
                InstalledLightSourceDto::Fluorescent => InstalledLightSource::Fluorescent,
                InstalledLightSourceDto::Arc => InstalledLightSource::Arc,
                InstalledLightSourceDto::Other { label } => InstalledLightSource::Other { label },
            },
            color_temperature_kelvin: value.color_temperature_kelvin,
            gel: match value.gel {
                GelAssignmentDto::OpenWhite => GelAssignment::OpenWhite,
                GelAssignmentDto::BuiltIn {
                    catalog_id,
                    entry_id,
                    embedded_fallback,
                } => GelAssignment::BuiltIn {
                    catalog_id,
                    entry_id,
                    embedded_fallback: GelDefinitionSnapshot {
                        number: embedded_fallback.number,
                        name: embedded_fallback.name,
                        display_srgb: embedded_fallback.display_srgb,
                        visualizer_srgb: embedded_fallback.visualizer_srgb,
                    },
                },
                GelAssignmentDto::Custom {
                    name,
                    color_srgb,
                    note,
                } => GelAssignment::Custom {
                    name,
                    color_srgb,
                    note,
                },
            },
            shaper_angles_degrees: value.shaper_angles_degrees,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_appearance_crosses_the_planning_contract_without_identity_loss() {
        let appearance = InstalledFixtureAppearance {
            light_source: InstalledLightSource::Tungsten,
            color_temperature_kelvin: Some(3_200),
            gel: GelAssignment::BuiltIn {
                catalog_id: "touring-gels".into(),
                entry_id: "deep-red".into(),
                embedded_fallback: GelDefinitionSnapshot {
                    number: "R1".into(),
                    name: "Deep red".into(),
                    display_srgb: "#D92838".into(),
                    visualizer_srgb: "#C01020".into(),
                },
            },
            shaper_angles_degrees: [-10.0, 20.0, 0.0, 179.0],
        };

        let value = serde_json::to_value(InstalledAppearanceDto::from(&appearance)).unwrap();
        assert_eq!(value["lightSource"]["type"], "tungsten");
        assert_eq!(value["colorTemperatureKelvin"], 3_200);
        assert_eq!(value["gel"]["catalogId"], "touring-gels");
        assert_eq!(
            value["gel"]["embeddedFallback"]["visualizerSrgb"],
            "#C01020"
        );
        let decoded: InstalledAppearanceDto = serde_json::from_value(value).unwrap();
        assert_eq!(InstalledFixtureAppearance::from(decoded), appearance);
    }
}
