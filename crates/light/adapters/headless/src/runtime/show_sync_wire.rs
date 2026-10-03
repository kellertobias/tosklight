//! Translation between the `show_sync` wire contract and the application sync command.

use light_application::{self as application, show_sync as sync};
use light_core::{FixtureId, ShowId};
use light_wire::v2::{patch as patch_wire, show_sync as wire};
use sha2::{Digest, Sha256};

pub(super) fn application_command(
    show_id: ShowId,
    request: &wire::ShowSyncTransactionRequest,
) -> Result<sync::ShowSyncCommand, String> {
    Ok(sync::ShowSyncCommand {
        show_id,
        association_id: request.association_id,
        request_id: request.request_id.clone(),
        signature: signature(request)?,
        operations: request
            .operations
            .iter()
            .map(application_operation)
            .collect::<Result<_, _>>()?,
    })
}

/// Digest of everything a retry must repeat exactly: the show, the association and the edits.
fn signature(request: &wire::ShowSyncTransactionRequest) -> Result<String, String> {
    let canonical =
        serde_json::to_vec(&(request.show_id, request.association_id, &request.operations))
            .map_err(|error| error.to_string())?;
    let digest = Sha256::digest(canonical);
    Ok(format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

fn application_operation(
    operation: &wire::ShowSyncOperation,
) -> Result<sync::ShowSyncOperation, String> {
    use sync::ShowSyncOperation as Operation;
    Ok(match operation {
        wire::ShowSyncOperation::UpdateObject { kind, id, fields } => Operation::UpdateObject {
            kind: kind.clone(),
            id: id.clone(),
            fields: fields.iter().map(application_field).collect(),
        },
        wire::ShowSyncOperation::CreateObject { kind, id, body } => Operation::CreateObject {
            kind: kind.clone(),
            id: id.clone(),
            body: body.clone(),
        },
        wire::ShowSyncOperation::DeleteObject {
            kind,
            id,
            base_object_revision,
        } => Operation::DeleteObject {
            kind: kind.clone(),
            id: id.clone(),
            base_object_revision: *base_object_revision,
        },
        wire::ShowSyncOperation::UpdatePatchFixture { fixture_id, fields } => {
            Operation::UpdatePatchFixture {
                fixture_id: FixtureId(*fixture_id),
                fields: fields.iter().map(application_field).collect(),
            }
        }
        wire::ShowSyncOperation::CreatePatchFixture { fixture } => Operation::CreatePatchFixture {
            fixture_id: FixtureId(fixture.fixture_id),
            document: serde_json::to_value(fixture.as_ref()).map_err(|error| error.to_string())?,
        },
        wire::ShowSyncOperation::RemovePatchFixture {
            fixture_id,
            base_fixture_revision,
        } => Operation::RemovePatchFixture {
            fixture_id: FixtureId(*fixture_id),
            base_fixture_revision: *base_fixture_revision,
        },
        wire::ShowSyncOperation::RetainProfileRevision {
            profile_id,
            revision,
            profile,
        } => Operation::RetainProfileRevision {
            profile_id: FixtureId(*profile_id),
            revision: *revision,
            profile: profile.clone(),
        },
        wire::ShowSyncOperation::SetMetadata { key, base, value } => Operation::SetMetadata {
            key: key.clone(),
            base: base.clone(),
            value: value.clone(),
        },
    })
}

fn application_field(field: &wire::ShowSyncFieldEdit) -> sync::ShowSyncFieldEdit {
    sync::ShowSyncFieldEdit {
        path: field.path.clone(),
        base: field.base.clone(),
        value: field.value.clone(),
    }
}

pub(super) fn wire_outcome(result: sync::ShowSyncResult) -> wire::ShowSyncTransactionOutcome {
    wire::ShowSyncTransactionOutcome {
        status: if result.is_conflicted() {
            wire::ShowSyncStatus::Conflicted
        } else {
            wire::ShowSyncStatus::Accepted
        },
        request_id: result.request_id,
        association_id: result.association_id,
        show_id: result.show_id,
        replayed: result.replayed,
        show_revision: result.show_revision,
        patch_revision: result.patch_revision,
        applied: result
            .applied
            .into_iter()
            .map(|object| wire::ShowSyncAppliedObject {
                kind: object.kind,
                id: object.id,
                revision: object.revision,
                deleted: object.deleted,
            })
            .collect(),
        metadata: result.metadata,
        conflicts: result.conflicts.into_iter().map(wire_conflict).collect(),
        event_sequence: result.event_sequence,
    }
}

fn wire_conflict(conflict: sync::ShowSyncConflict) -> wire::ShowSyncConflict {
    wire::ShowSyncConflict {
        kind: conflict.kind,
        id: conflict.id,
        path: conflict.path,
        base: conflict.base,
        mine: conflict.mine,
        theirs: conflict.theirs,
        theirs_revision: conflict.theirs_revision,
        reason: match conflict.reason {
            sync::ShowSyncConflictReason::FieldChanged => {
                wire::ShowSyncConflictReason::FieldChanged
            }
            sync::ShowSyncConflictReason::ObjectDeleted => {
                wire::ShowSyncConflictReason::ObjectDeleted
            }
            sync::ShowSyncConflictReason::ObjectModified => {
                wire::ShowSyncConflictReason::ObjectModified
            }
            sync::ShowSyncConflictReason::ObjectExists => {
                wire::ShowSyncConflictReason::ObjectExists
            }
        },
    }
}

/// The document a client's patch field edits address: the fixture as a `PatchFixtureInput`, the
/// same shape it sends to create one.
pub(super) fn patch_fixture_document(
    fixture: &application::PatchFixtureProjection,
) -> Result<serde_json::Value, String> {
    let projection = super::show_patch_wire::wire_fixture(fixture);
    serde_json::to_value(patch_input(projection)).map_err(|error| error.to_string())
}

pub(super) fn patch_fixture_candidate(
    document: serde_json::Value,
) -> Result<application::PatchFixtureCandidate, String> {
    let input: patch_wire::PatchFixtureInput = serde_json::from_value(document)
        .map_err(|error| format!("patched fixture document is invalid: {error}"))?;
    super::show_patch_wire::application_fixture(input)
}

fn patch_input(projection: patch_wire::PatchFixtureProjection) -> patch_wire::PatchFixtureInput {
    patch_wire::PatchFixtureInput {
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
            .map(|copy| patch_wire::PatchMultiPatchInput {
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
            .map(|value| patch_wire::PatchHighlightOverrideInput {
                channel_id: value.channel_id,
                raw_value: value.raw_value,
            })
            .collect(),
    }
}
