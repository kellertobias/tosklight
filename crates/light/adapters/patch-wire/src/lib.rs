#![forbid(unsafe_code)]
//! The one mapping between the application's patched fixture and its wire shape.
//!
//! The desk's HTTP adapter and the Architect's sync client both translate patched fixtures
//! between `light-application` and `light-wire`; keeping the translation here means a fixture
//! field added to one side cannot silently drift between the two. A sync transaction addresses
//! patch fields as JSON pointers into exactly this `PatchFixtureInput`, so the Architect and the
//! desk must agree on it byte for byte.

use light_application as application;
use light_core::FixtureId;
use light_fixture as fixture;
use light_wire::v2::patch as wire;
use std::collections::BTreeMap;

pub fn application_fixture(
    input: wire::PatchFixtureInput,
) -> Result<application::PatchFixtureCandidate, String> {
    Ok(application::PatchFixtureCandidate {
        profile: fixture::PatchedFixtureProfileReference {
            profile_id: FixtureId(input.profile_id),
            profile_revision: input.profile_revision,
            mode_id: input.mode_id,
        },
        patch: fixture::PatchedFixturePatch {
            fixture_id: FixtureId(input.fixture_id),
            fixture_number: input.fixture_number,
            virtual_fixture_number: input.virtual_fixture_number,
            name: input.name,
            universe: None,
            address: None,
            split_patches: input
                .split_patches
                .into_iter()
                .map(application_split)
                .collect(),
            layer_id: input.layer_id,
            note: input.note,
            position_master: input.position_master,
            direct_control: input
                .direct_control
                .map(application_direct_control)
                .transpose()?,
            internal_bindings: fixture::InternalFixtureBindings {
                library: input.internal_bindings.library,
                output: input.internal_bindings.output,
            },
            location: application_location(input.location),
            scenery_size_metres: input.scenery_size_metres.map(application_vector),
            model_scale: input.model_scale,
            scenery_options: input
                .scenery_options
                .map(application_scenery_options)
                .unwrap_or_default(),
            rotation: application_rotation(input.rotation),
            logical_heads: Vec::new(),
            multipatch: input
                .multipatch
                .into_iter()
                .map(application_multipatch)
                .collect(),
            group_masters_enabled: input.group_masters_enabled,
            grand_master_enabled: input.grand_master_enabled,
            invert_pan: input.invert_pan,
            invert_tilt: input.invert_tilt,
            bracket_angle: input.bracket_angle,
            shaper_angle: input.shaper_angle,
            installed_appearance: application_installed_appearance(input.installed_appearance),
            move_in_black_enabled: input.move_in_black_enabled,
            move_in_black_delay_millis: input.move_in_black_delay_millis,
            highlight_overrides: application_highlights(input.highlight_overrides)?,
            freeze: Default::default(),
        },
    })
}

fn application_split(split: wire::PatchSplitAssignment) -> fixture::SplitPatch {
    fixture::SplitPatch {
        split: split.split,
        universe: split.universe,
        address: split.address,
    }
}

fn application_direct_control(
    endpoint: wire::PatchDirectControlEndpoint,
) -> Result<fixture::DirectControlEndpoint, String> {
    Ok(fixture::DirectControlEndpoint {
        protocol: match endpoint.protocol {
            wire::PatchDirectControlProtocol::Citp => fixture::DirectControlProtocol::Citp,
        },
        ip_address: endpoint
            .ip_address
            .parse()
            .map_err(|error| format!("direct-control IP address is invalid: {error}"))?,
        port: endpoint.port,
    })
}

fn application_location(location: wire::PatchFixtureLocation) -> fixture::FixtureLocation {
    fixture::FixtureLocation {
        x: location.x,
        y: location.y,
        z: location.z,
    }
}

/// A placed size, carried in millimetres exactly as a location is.
fn application_vector(size: wire::PatchFixtureLocation) -> fixture::FixtureVector {
    fixture::FixtureVector {
        x: size.x as f32,
        y: size.y as f32,
        z: size.z as f32,
    }
}

fn wire_vector(size: fixture::FixtureVector) -> wire::PatchFixtureLocation {
    wire::PatchFixtureLocation {
        x: size.x as i32,
        y: size.y as i32,
        z: size.z as i32,
    }
}

fn application_scenery_options(input: wire::PatchSceneryOptions) -> fixture::SceneryOptions {
    fixture::SceneryOptions {
        colour_srgb: input.colour_srgb,
        chain_top: input.chain_top.map(|end| match end {
            wire::PatchChainTopEnd::Motor => fixture::ChainTopEnd::Motor,
            wire::PatchChainTopEnd::Direct => fixture::ChainTopEnd::Direct,
            wire::PatchChainTopEnd::SteelflexLoop => fixture::ChainTopEnd::SteelflexLoop,
        }),
        chain_bottom: input.chain_bottom.map(|end| match end {
            wire::PatchChainBottomEnd::Direct => fixture::ChainBottomEnd::Direct,
            wire::PatchChainBottomEnd::SteelflexLoop => fixture::ChainBottomEnd::SteelflexLoop,
            wire::PatchChainBottomEnd::Motor => fixture::ChainBottomEnd::Motor,
        }),
        handrails: input.handrails.map(|sides| match sides {
            wire::PatchStairHandrails::None => fixture::StairHandrails::None,
            wire::PatchStairHandrails::Left => fixture::StairHandrails::Left,
            wire::PatchStairHandrails::Right => fixture::StairHandrails::Right,
            wire::PatchStairHandrails::Both => fixture::StairHandrails::Both,
        }),
    }
}

fn wire_scenery_options(options: &fixture::SceneryOptions) -> Option<wire::PatchSceneryOptions> {
    (!options.is_empty()).then(|| wire::PatchSceneryOptions {
        colour_srgb: options.colour_srgb.clone(),
        chain_top: options.chain_top.map(|end| match end {
            fixture::ChainTopEnd::Motor => wire::PatchChainTopEnd::Motor,
            fixture::ChainTopEnd::Direct => wire::PatchChainTopEnd::Direct,
            fixture::ChainTopEnd::SteelflexLoop => wire::PatchChainTopEnd::SteelflexLoop,
        }),
        chain_bottom: options.chain_bottom.map(|end| match end {
            fixture::ChainBottomEnd::Direct => wire::PatchChainBottomEnd::Direct,
            fixture::ChainBottomEnd::SteelflexLoop => wire::PatchChainBottomEnd::SteelflexLoop,
            fixture::ChainBottomEnd::Motor => wire::PatchChainBottomEnd::Motor,
        }),
        handrails: options.handrails.map(|sides| match sides {
            fixture::StairHandrails::None => wire::PatchStairHandrails::None,
            fixture::StairHandrails::Left => wire::PatchStairHandrails::Left,
            fixture::StairHandrails::Right => wire::PatchStairHandrails::Right,
            fixture::StairHandrails::Both => wire::PatchStairHandrails::Both,
        }),
    })
}

fn application_rotation(rotation: wire::PatchFixtureRotation) -> fixture::FixtureVector {
    fixture::FixtureVector {
        x: rotation.x,
        y: rotation.y,
        z: rotation.z,
    }
}

fn application_multipatch(input: wire::PatchMultiPatchInput) -> fixture::MultiPatchInstance {
    fixture::MultiPatchInstance {
        scenery_size_metres: None,
        id: input.id,
        name: input.name,
        universe: None,
        address: None,
        split_patches: input
            .split_patches
            .into_iter()
            .map(application_split)
            .collect(),
        location: application_location(input.location),
        rotation: application_rotation(input.rotation),
        invert_pan: input.invert_pan,
        invert_tilt: input.invert_tilt,
        bracket_angle: input.bracket_angle,
        shaper_angle: input.shaper_angle,
        installed_appearance: application_installed_appearance(input.installed_appearance),
    }
}

pub fn application_installed_appearance(
    input: wire::PatchInstalledFixtureAppearance,
) -> fixture::InstalledFixtureAppearance {
    fixture::InstalledFixtureAppearance {
        light_source: match input.light_source {
            wire::PatchInstalledLightSource::ProfileDefault => {
                fixture::InstalledLightSource::ProfileDefault
            }
            wire::PatchInstalledLightSource::Tungsten => fixture::InstalledLightSource::Tungsten,
            wire::PatchInstalledLightSource::Halogen => fixture::InstalledLightSource::Halogen,
            wire::PatchInstalledLightSource::Discharge => fixture::InstalledLightSource::Discharge,
            wire::PatchInstalledLightSource::Led => fixture::InstalledLightSource::Led,
            wire::PatchInstalledLightSource::Fluorescent => {
                fixture::InstalledLightSource::Fluorescent
            }
            wire::PatchInstalledLightSource::Arc => fixture::InstalledLightSource::Arc,
            wire::PatchInstalledLightSource::Other { label } => {
                fixture::InstalledLightSource::Other { label }
            }
        },
        color_temperature_kelvin: input.color_temperature_kelvin,
        luminous_output_lumens: input.luminous_output_lumens,
        gel: match input.gel {
            wire::PatchGelAssignment::OpenWhite => fixture::GelAssignment::OpenWhite,
            wire::PatchGelAssignment::BuiltIn {
                catalog_id,
                entry_id,
                embedded_fallback,
            } => fixture::GelAssignment::BuiltIn {
                catalog_id,
                entry_id,
                embedded_fallback: fixture::GelDefinitionSnapshot {
                    number: embedded_fallback.number,
                    name: embedded_fallback.name,
                    display_srgb: embedded_fallback.display_srgb,
                    visualizer_srgb: embedded_fallback.visualizer_srgb,
                },
            },
            wire::PatchGelAssignment::Custom {
                name,
                color_srgb,
                note,
            } => fixture::GelAssignment::Custom {
                name,
                color_srgb,
                note,
            },
        },
        shaper_angles_degrees: input.shaper_angles_degrees,
    }
}

fn application_highlights(
    highlights: Vec<wire::PatchHighlightOverrideInput>,
) -> Result<BTreeMap<uuid::Uuid, u32>, String> {
    let mut values = BTreeMap::new();
    for highlight in highlights {
        if values
            .insert(highlight.channel_id, highlight.raw_value)
            .is_some()
        {
            return Err("Highlight override channel identities must be unique".into());
        }
    }
    Ok(values)
}

pub fn wire_fixture(input: &application::PatchFixtureProjection) -> wire::PatchFixtureProjection {
    let patch = &input.patch;
    wire::PatchFixtureProjection {
        fixture_id: patch.fixture_id.0,
        fixture_revision: input.fixture_revision,
        fixture_number: patch.fixture_number,
        virtual_fixture_number: patch.virtual_fixture_number,
        name: patch.name.clone(),
        profile_id: input.profile.profile_id.0,
        profile_revision: input.profile.profile_revision,
        mode_id: input.profile.mode_id,
        split_patches: patch.split_patches.iter().map(wire_split).collect(),
        layer_id: patch.layer_id.clone(),
        direct_control: patch.direct_control.as_ref().map(wire_direct_control),
        internal_bindings: wire::PatchInternalFixtureBindings {
            library: patch.internal_bindings.library.clone(),
            output: patch.internal_bindings.output.clone(),
        },
        location: wire_location(patch.location),
        scenery_size_metres: patch.scenery_size_metres.map(wire_vector),
        scenery_options: wire_scenery_options(&patch.scenery_options),
        model_scale: patch.model_scale,
        rotation: wire_rotation(patch.rotation),
        note: patch.note.clone(),
        position_master: patch.position_master,
        logical_heads: patch
            .logical_heads
            .iter()
            .map(|head| wire::PatchLogicalHeadProjection {
                profile_head_id: head.profile_head_id,
                head_index: head.head_index,
                fixture_id: head.fixture_id.0,
            })
            .collect(),
        multipatch: patch.multipatch.iter().map(wire_multipatch).collect(),
        group_masters_enabled: patch.group_masters_enabled,
        grand_master_enabled: patch.grand_master_enabled,
        invert_pan: patch.invert_pan,
        invert_tilt: patch.invert_tilt,
        bracket_angle: patch.bracket_angle,
        shaper_angle: patch.shaper_angle,
        installed_appearance: wire_installed_appearance(&patch.installed_appearance),
        move_in_black_enabled: patch.move_in_black_enabled,
        move_in_black_delay_millis: patch.move_in_black_delay_millis,
        highlight_overrides: patch
            .highlight_overrides
            .iter()
            .map(
                |(channel_id, raw_value)| wire::PatchHighlightOverrideProjection {
                    channel_id: *channel_id,
                    raw_value: *raw_value,
                },
            )
            .collect(),
        freeze_targets: patch
            .freeze
            .targets
            .iter()
            .map(
                |(fixture_id, target)| wire::PatchFixtureFreezeTargetProjection {
                    fixture_id: fixture_id.0,
                    full: target.full,
                    families: target
                        .families
                        .iter()
                        .map(|family| match family {
                            fixture::FreezeFamily::Intensity => {
                                wire::PatchFixtureFreezeFamily::Intensity
                            }
                            fixture::FreezeFamily::Color => wire::PatchFixtureFreezeFamily::Color,
                            fixture::FreezeFamily::Position => {
                                wire::PatchFixtureFreezeFamily::Position
                            }
                            fixture::FreezeFamily::Beam => wire::PatchFixtureFreezeFamily::Beam,
                        })
                        .collect(),
                },
            )
            .collect(),
    }
}

fn wire_split(split: &fixture::SplitPatch) -> wire::PatchSplitAssignment {
    wire::PatchSplitAssignment {
        split: split.split,
        universe: split.universe,
        address: split.address,
    }
}

fn wire_direct_control(
    endpoint: &fixture::DirectControlEndpoint,
) -> wire::PatchDirectControlEndpoint {
    wire::PatchDirectControlEndpoint {
        protocol: match endpoint.protocol {
            fixture::DirectControlProtocol::Citp => wire::PatchDirectControlProtocol::Citp,
        },
        ip_address: endpoint.ip_address.to_string(),
        port: endpoint.port,
    }
}

fn wire_location(location: fixture::FixtureLocation) -> wire::PatchFixtureLocation {
    wire::PatchFixtureLocation {
        x: location.x,
        y: location.y,
        z: location.z,
    }
}

fn wire_rotation(rotation: fixture::FixtureVector) -> wire::PatchFixtureRotation {
    wire::PatchFixtureRotation {
        x: rotation.x,
        y: rotation.y,
        z: rotation.z,
    }
}

fn wire_multipatch(instance: &fixture::MultiPatchInstance) -> wire::PatchMultiPatchProjection {
    wire::PatchMultiPatchProjection {
        id: instance.id,
        name: instance.name.clone(),
        split_patches: instance.split_patches.iter().map(wire_split).collect(),
        location: wire_location(instance.location),
        scenery_size_metres: instance.scenery_size_metres.map(wire_vector),
        rotation: wire_rotation(instance.rotation),
        invert_pan: instance.invert_pan,
        invert_tilt: instance.invert_tilt,
        bracket_angle: instance.bracket_angle,
        shaper_angle: instance.shaper_angle,
        installed_appearance: wire_installed_appearance(&instance.installed_appearance),
    }
}

fn wire_installed_appearance(
    input: &fixture::InstalledFixtureAppearance,
) -> wire::PatchInstalledFixtureAppearance {
    wire::PatchInstalledFixtureAppearance {
        light_source: match &input.light_source {
            fixture::InstalledLightSource::ProfileDefault => {
                wire::PatchInstalledLightSource::ProfileDefault
            }
            fixture::InstalledLightSource::Tungsten => wire::PatchInstalledLightSource::Tungsten,
            fixture::InstalledLightSource::Halogen => wire::PatchInstalledLightSource::Halogen,
            fixture::InstalledLightSource::Discharge => wire::PatchInstalledLightSource::Discharge,
            fixture::InstalledLightSource::Led => wire::PatchInstalledLightSource::Led,
            fixture::InstalledLightSource::Fluorescent => {
                wire::PatchInstalledLightSource::Fluorescent
            }
            fixture::InstalledLightSource::Arc => wire::PatchInstalledLightSource::Arc,
            fixture::InstalledLightSource::Other { label } => {
                wire::PatchInstalledLightSource::Other {
                    label: label.clone(),
                }
            }
        },
        color_temperature_kelvin: input.color_temperature_kelvin,
        luminous_output_lumens: input.luminous_output_lumens,
        gel: match &input.gel {
            fixture::GelAssignment::OpenWhite => wire::PatchGelAssignment::OpenWhite,
            fixture::GelAssignment::BuiltIn {
                catalog_id,
                entry_id,
                embedded_fallback,
            } => wire::PatchGelAssignment::BuiltIn {
                catalog_id: catalog_id.clone(),
                entry_id: entry_id.clone(),
                embedded_fallback: wire::PatchGelDefinitionSnapshot {
                    number: embedded_fallback.number.clone(),
                    name: embedded_fallback.name.clone(),
                    display_srgb: embedded_fallback.display_srgb.clone(),
                    visualizer_srgb: embedded_fallback.visualizer_srgb.clone(),
                },
            },
            fixture::GelAssignment::Custom {
                name,
                color_srgb,
                note,
            } => wire::PatchGelAssignment::Custom {
                name: name.clone(),
                color_srgb: color_srgb.clone(),
                note: note.clone(),
            },
        },
        shaper_angles_degrees: input.shaper_angles_degrees,
    }
}

pub fn wire_profile(
    profile: &application::PatchProfileRevisionProjection,
) -> wire::PatchProfileRevisionProjection {
    wire::PatchProfileRevisionProjection {
        profile_id: profile.profile_id.0,
        profile_revision: profile.profile_revision,
        content_digest: profile.content_digest.clone(),
        manufacturer: profile.manufacturer.clone(),
        name: profile.name.clone(),
        fixture_type: profile.fixture_type.clone(),
        patch_policy: match profile.patch_policy {
            fixture::PatchPolicy::Dmx => wire::PatchProfilePolicy::Dmx,
            fixture::PatchPolicy::VisualOnly => wire::PatchProfilePolicy::VisualOnly,
            fixture::PatchPolicy::Internal => wire::PatchProfilePolicy::Internal,
        },
        referenced_modes: profile
            .referenced_modes
            .iter()
            .map(|mode| wire::PatchModeProjection {
                mode_id: mode.mode_id,
                name: mode.name.clone(),
                splits: mode
                    .splits
                    .iter()
                    .map(|split| wire::PatchModeSplitProjection {
                        split: split.number,
                        footprint: split.footprint,
                    })
                    .collect(),
            })
            .collect(),
        profile_snapshot: profile.profile_snapshot.clone(),
    }
}

/// The input that recreates a projected fixture: the document a sync transaction's patch field
/// edits address.
pub fn patch_input(projection: wire::PatchFixtureProjection) -> wire::PatchFixtureInput {
    wire::PatchFixtureInput {
        fixture_id: projection.fixture_id,
        fixture_number: projection.fixture_number,
        virtual_fixture_number: projection.virtual_fixture_number,
        name: projection.name,
        profile_id: projection.profile_id,
        profile_revision: projection.profile_revision,
        mode_id: projection.mode_id,
        split_patches: projection.split_patches,
        layer_id: projection.layer_id,
        direct_control: projection.direct_control,
        internal_bindings: projection.internal_bindings,
        location: projection.location,
        scenery_size_metres: projection.scenery_size_metres,
        scenery_options: projection.scenery_options,
        model_scale: projection.model_scale,
        rotation: projection.rotation,
        note: projection.note,
        position_master: projection.position_master,
        multipatch: projection
            .multipatch
            .into_iter()
            .map(|copy| wire::PatchMultiPatchInput {
                id: copy.id,
                name: copy.name,
                split_patches: copy.split_patches,
                location: copy.location,
                scenery_size_metres: copy.scenery_size_metres,
                rotation: copy.rotation,
                invert_pan: copy.invert_pan,
                invert_tilt: copy.invert_tilt,
                bracket_angle: copy.bracket_angle,
                shaper_angle: copy.shaper_angle,
                installed_appearance: copy.installed_appearance,
            })
            .collect(),
        group_masters_enabled: projection.group_masters_enabled,
        grand_master_enabled: projection.grand_master_enabled,
        invert_pan: projection.invert_pan,
        invert_tilt: projection.invert_tilt,
        bracket_angle: projection.bracket_angle,
        shaper_angle: projection.shaper_angle,
        installed_appearance: projection.installed_appearance,
        move_in_black_enabled: projection.move_in_black_enabled,
        move_in_black_delay_millis: projection.move_in_black_delay_millis,
        highlight_overrides: projection
            .highlight_overrides
            .into_iter()
            .map(|value| wire::PatchHighlightOverrideInput {
                channel_id: value.channel_id,
                raw_value: value.raw_value,
            })
            .collect(),
    }
}

/// The sync document of one stored `patched_fixture` record: the raw body a show stores and the
/// sync feed carries, read as the `PatchFixtureInput` its field edits address.
pub fn stored_fixture_input(
    body: serde_json::Value,
    revision: u64,
) -> Result<wire::PatchFixtureInput, String> {
    let record = fixture::PortablePatchedFixtureRecord::decode(body).map_err(|e| e.to_string())?;
    let profile = record
        .selected_profile_reference()
        .map_err(|e| e.to_string())?
        .ok_or("patched fixture has no portable profile reference")?;
    let projection = application::PatchFixtureProjection {
        fixture_revision: revision,
        profile,
        patch: record.patch().map_err(|e| e.to_string())?,
    };
    Ok(patch_input(wire_fixture(&projection)))
}
