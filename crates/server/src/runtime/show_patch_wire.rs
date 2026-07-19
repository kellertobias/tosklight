use light_application as application;
use light_core::{FixtureId, ShowId};
use light_fixture as fixture;
use light_wire::v2::patch as wire;
use std::collections::BTreeMap;

pub(super) fn application_command(
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
    })
}

pub(super) fn wire_outcome(result: application::PatchFixturesResult) -> wire::PatchFixturesOutcome {
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
            direct_control: input
                .direct_control
                .map(application_direct_control)
                .transpose()?,
            location: application_location(input.location),
            rotation: application_rotation(input.rotation),
            logical_heads: Vec::new(),
            multipatch: input
                .multipatch
                .into_iter()
                .map(application_multipatch)
                .collect(),
            move_in_black_enabled: input.move_in_black_enabled,
            move_in_black_delay_millis: input.move_in_black_delay_millis,
            highlight_overrides: application_highlights(input.highlight_overrides)?,
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
    }
}
