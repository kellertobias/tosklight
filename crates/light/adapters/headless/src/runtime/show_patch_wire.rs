use light_application as application;
use light_core::{FixtureId, ShowId};
use light_fixture as fixture;
use light_wire::v2::patch as wire;
use uuid::Uuid;

pub(crate) use light_patch_wire::{
    application_color_calibration, application_fixture, application_installed_appearance,
    application_position_calibration, wire_fixture, wire_profile,
};

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
        wire::PatchFixtureUpdateAction::ReplaceProfile {
            profile_id,
            profile_revision,
            mode_id,
            head_mapping,
            root_programming_mapping,
        } => application::PatchFixtureUpdateAction::ReplaceProfile {
            profile: fixture::PatchedFixtureProfileReference {
                profile_id: FixtureId(profile_id),
                profile_revision,
                mode_id,
            },
            root_programming_mapping: root_programming_mapping
                .into_iter()
                .map(|entry| application::PatchRootProgramReplacement {
                    source_profile_head_id: entry.source_profile_head_id,
                    attribute: light_core::AttributeKey(entry.attribute.into()),
                    target_profile_head_ids: entry.target_profile_head_ids,
                })
                .collect(),
            head_mapping: head_mapping
                .into_iter()
                .map(|entry| application::PatchHeadReplacement {
                    fixture_id: FixtureId(entry.fixture_id),
                    target_profile_head_id: entry.target_profile_head_id,
                })
                .collect(),
        },
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
        wire::PatchFixtureUpdateAction::SetColorCalibration { calibration } => {
            application::PatchFixtureUpdateAction::SetColorCalibration {
                calibration: calibration.map(application_color_calibration),
            }
        }
        wire::PatchFixtureUpdateAction::SetPositionCalibration { calibration } => {
            application::PatchFixtureUpdateAction::SetPositionCalibration {
                calibration: calibration.map(application_position_calibration),
            }
        }
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
                .any(fixture::FixtureChannel::follows_masters)
            {
                return Err("fixture mode has no Group Master eligible channels".into());
            }
            candidate.patch.group_masters_enabled = controlled;
        }
        wire::PatchFixturePolicyAction::SetGrandMaster { controlled } => {
            if !mode
                .channels
                .iter()
                .any(fixture::FixtureChannel::follows_masters)
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
