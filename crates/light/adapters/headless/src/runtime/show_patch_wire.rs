use light_application as application;
use light_core::{FixtureId, ShowId};
use light_fixture as fixture;
use light_wire::v2::patch as wire;
use std::collections::BTreeMap;
use uuid::Uuid;

pub(crate) fn application_command(
    show_id: ShowId,
    request: wire::PatchFixturesRequest,
) -> Result<application::PatchFixturesCommand, String> {
    Ok(application::PatchFixturesCommand {
        show_id,
        fixtures: request
            .fixtures
            .into_iter()
            .map(application_fixture)
            .collect::<Result<_, _>>()?,
        remove_fixture_ids: request
            .remove_fixture_ids
            .into_iter()
            .map(FixtureId)
            .collect(),
        placements: request
            .placements
            .into_iter()
            .map(application_placement)
            .collect(),
        vector_spreads: request
            .vector_spreads
            .into_iter()
            .map(application_vector_spread)
            .collect(),
        fixture_updates: Vec::new(),
    })
}

pub(crate) fn application_update_command(
    show_id: ShowId,
    fixture_id: Uuid,
    request: wire::PatchFixtureUpdateRequest,
) -> Result<application::PatchFixturesCommand, String> {
    let action = match request.action {
        wire::PatchFixtureUpdateAction::SetMasters {
            group_masters_enabled,
            grand_master_enabled,
        } => application::PatchFixtureUpdateAction::SetMasters {
            group_masters_enabled,
            grand_master_enabled,
        },
        wire::PatchFixtureUpdateAction::SetPanTilt {
            invert_pan,
            invert_tilt,
        } => application::PatchFixtureUpdateAction::SetPanTilt {
            invert_pan,
            invert_tilt,
        },
        wire::PatchFixtureUpdateAction::SetMoveInBlack {
            enabled,
            delay_millis,
        } => application::PatchFixtureUpdateAction::SetMoveInBlack {
            enabled,
            delay_millis,
        },
        wire::PatchFixtureUpdateAction::SetLocationAxis { axis, millimetres } => {
            application::PatchFixtureUpdateAction::SetLocationAxis {
                axis: application_update_axis(axis),
                millimetres,
            }
        }
        wire::PatchFixtureUpdateAction::SetRotationAxis { axis, degrees } => {
            application::PatchFixtureUpdateAction::SetRotationAxis {
                axis: application_update_axis(axis),
                degrees,
            }
        }
        wire::PatchFixtureUpdateAction::SetBracketAngle { degrees } => {
            application::PatchFixtureUpdateAction::SetBracketAngle { degrees }
        }
        wire::PatchFixtureUpdateAction::SetShaperModuleRotation { degrees } => {
            application::PatchFixtureUpdateAction::SetShaperModuleAngle { degrees }
        }
        wire::PatchFixtureUpdateAction::SetStaticShaperAngle { element, degrees } => {
            application::PatchFixtureUpdateAction::SetStaticShaperAngle { element, degrees }
        }
        wire::PatchFixtureUpdateAction::SetInstalledAppearance { appearance } => {
            application::PatchFixtureUpdateAction::SetInstalledAppearance {
                appearance: application_installed_appearance(appearance),
            }
        }
    };
    Ok(application::PatchFixturesCommand {
        show_id,
        fixtures: Vec::new(),
        remove_fixture_ids: Vec::new(),
        placements: Vec::new(),
        vector_spreads: Vec::new(),
        fixture_updates: vec![application::PatchFixtureUpdateIntent {
            fixture_id: FixtureId(fixture_id),
            expected_fixture_revision: request.expected_fixture_revision,
            expected_show_revision: light_show::PortableShowRevision::from_value(
                request.expected_show_revision,
            ),
            multipatch_instance_id: request.multipatch_instance_id,
            action,
        }],
    })
}

fn application_update_axis(axis: wire::PatchVectorAxis) -> application::PatchFixtureAxis {
    match axis {
        wire::PatchVectorAxis::X => application::PatchFixtureAxis::X,
        wire::PatchVectorAxis::Y => application::PatchFixtureAxis::Y,
        wire::PatchVectorAxis::Z => application::PatchFixtureAxis::Z,
    }
}

pub(crate) fn application_policy_command(
    show_id: ShowId,
    fixture_id: Uuid,
    request: wire::PatchFixturePolicyActionRequest,
    snapshot: &application::PatchSnapshot,
) -> Result<application::PatchFixturesCommand, String> {
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.patch.fixture_id.0 == fixture_id)
        .ok_or_else(|| "fixture does not exist".to_owned())?;
    let mut candidate = application::PatchFixtureCandidate {
        profile: fixture.profile,
        patch: fixture.patch.clone(),
    };
    let profile = snapshot
        .profile_revisions
        .iter()
        .find(|profile| {
            profile.profile_id == fixture.profile.profile_id
                && profile.profile_revision == fixture.profile.profile_revision
        })
        .ok_or_else(|| "fixture profile revision is missing from the Patch snapshot".to_owned())?;
    let profile_snapshot: fixture::FixtureProfile =
        serde_json::from_value(profile.profile_snapshot.clone())
            .map_err(|error| format!("fixture profile snapshot is invalid: {error}"))?;
    let mode = profile_snapshot
        .mode(fixture.profile.mode_id)
        .ok_or_else(|| "fixture mode is missing from its profile snapshot".to_owned())?;
    match request.action {
        wire::PatchFixturePolicyAction::SetGroupMasters { controlled } => {
            if !mode
                .channels
                .iter()
                .any(|channel| channel.reacts_to_group_master)
            {
                return Err("fixture mode has no Group Master eligible channels".into());
            }
            candidate.patch.group_masters_enabled = controlled;
        }
        wire::PatchFixturePolicyAction::SetGrandMaster { controlled } => {
            if !mode
                .channels
                .iter()
                .any(|channel| channel.reacts_to_grand_master)
            {
                return Err("fixture mode has no Grand Master eligible channels".into());
            }
            candidate.patch.grand_master_enabled = controlled;
        }
        wire::PatchFixturePolicyAction::SetAxisInversion {
            axis,
            inverted,
            multipatch_instance_id,
        } => {
            let attribute = match axis {
                wire::PatchFixtureAxis::Pan => "pan",
                wire::PatchFixtureAxis::Tilt => "tilt",
            };
            let applicable = profile.patch_policy == fixture::PatchPolicy::Dmx
                && mode.channels.iter().any(|channel| {
                    channel.attribute.0.eq_ignore_ascii_case(attribute)
                        || channel
                            .functions
                            .iter()
                            .any(|function| function.attribute.0.eq_ignore_ascii_case(attribute))
                });
            if !applicable {
                return Err(format!("fixture mode has no applicable {attribute} axis"));
            }
            if let Some(instance_id) = multipatch_instance_id {
                let instance = candidate
                    .patch
                    .multipatch
                    .iter_mut()
                    .find(|instance| instance.id == instance_id)
                    .ok_or_else(|| "multi-patch instance does not exist".to_owned())?;
                match axis {
                    wire::PatchFixtureAxis::Pan => instance.invert_pan = inverted,
                    wire::PatchFixtureAxis::Tilt => instance.invert_tilt = inverted,
                }
            } else {
                match axis {
                    wire::PatchFixtureAxis::Pan => candidate.patch.invert_pan = inverted,
                    wire::PatchFixtureAxis::Tilt => candidate.patch.invert_tilt = inverted,
                }
            }
        }
    }
    Ok(application::PatchFixturesCommand {
        show_id,
        fixtures: vec![candidate],
        remove_fixture_ids: Vec::new(),
        placements: Vec::new(),
        vector_spreads: Vec::new(),
        fixture_updates: Vec::new(),
    })
}

fn application_vector_spread(
    input: wire::PatchVectorSpreadIntent,
) -> application::PatchVectorSpreadIntent {
    application::PatchVectorSpreadIntent {
        fixture_ids: input.fixture_ids.into_iter().map(FixtureId).collect(),
        kind: match input.kind {
            wire::PatchVectorKind::Location => application::PatchVectorKind::Location,
            wire::PatchVectorKind::Rotation => application::PatchVectorKind::Rotation,
        },
        axis: match input.axis {
            wire::PatchVectorAxis::X => application::PatchVectorAxis::X,
            wire::PatchVectorAxis::Y => application::PatchVectorAxis::Y,
            wire::PatchVectorAxis::Z => application::PatchVectorAxis::Z,
        },
        points: input.points,
    }
}

fn application_placement(input: wire::PatchPlacementIntent) -> application::PatchPlacementIntent {
    application::PatchPlacementIntent {
        fixture_ids: input.fixture_ids.into_iter().map(FixtureId).collect(),
        splits: input
            .splits
            .into_iter()
            .map(|split| application::PatchSplitPlacementIntent {
                split: split.split,
                universe: split.universe,
                address: split.address,
                mode: match split.mode {
                    wire::PatchSplitPlacementMode::Consecutive => {
                        application::PatchSplitPlacementMode::Consecutive
                    }
                    wire::PatchSplitPlacementMode::OperatorOverrides { overrides } => {
                        application::PatchSplitPlacementMode::OperatorOverrides(
                            overrides
                                .into_iter()
                                .map(|override_| application::PatchOperatorAddressOverride {
                                    fixture_id: FixtureId(override_.fixture_id),
                                    universe: override_.universe,
                                    address: override_.address,
                                })
                                .collect(),
                        )
                    }
                },
            })
            .collect(),
    }
}

pub(crate) fn wire_outcome(result: application::PatchFixturesResult) -> wire::PatchFixturesOutcome {
    wire::PatchFixturesOutcome {
        request_id: result.request_id,
        replayed: result.replayed,
        changed: result.changed,
        delta: wire_delta(&result.change, result.event_sequence),
    }
}

pub(super) fn wire_snapshot(snapshot: application::PatchSnapshot) -> wire::PatchSnapshot {
    wire::PatchSnapshot {
        show_id: snapshot.show_id.0,
        show_revision: snapshot.show_revision.value(),
        patch_revision: snapshot.patch_revision.value(),
        cursor: light_wire::v2::events::EventSnapshotCursor {
            sequence: snapshot.event_sequence,
        },
        fixtures: snapshot.fixtures.iter().map(wire_fixture).collect(),
        profile_revisions: snapshot
            .profile_revisions
            .iter()
            .map(wire_profile)
            .collect(),
    }
}

pub(super) fn wire_delta(
    change: &application::PatchChange,
    event_sequence: Option<u64>,
) -> wire::PatchDelta {
    wire::PatchDelta {
        show_id: change.show_id.0,
        show_revision: change.show_revision.value(),
        patch_revision: change.patch_revision.value(),
        event_sequence,
        fixtures: change.fixtures.iter().map(wire_fixture).collect(),
        removed_fixture_ids: change
            .removed_fixture_ids
            .iter()
            .map(|fixture| fixture.0)
            .collect(),
        profile_revisions: change.profile_revisions.iter().map(wire_profile).collect(),
    }
}

fn application_fixture(
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

fn application_rotation(rotation: wire::PatchFixtureRotation) -> fixture::FixtureVector {
    fixture::FixtureVector {
        x: rotation.x,
        y: rotation.y,
        z: rotation.z,
    }
}

fn application_multipatch(input: wire::PatchMultiPatchInput) -> fixture::MultiPatchInstance {
    fixture::MultiPatchInstance {
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

fn application_installed_appearance(
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

fn wire_fixture(input: &application::PatchFixtureProjection) -> wire::PatchFixtureProjection {
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
        rotation: wire_rotation(patch.rotation),
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

fn wire_profile(
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
