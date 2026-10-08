use light_application::show_sync as application;
use light_wire::v2::show_sync as wire;

pub(super) fn wire_commit(change: &application::ShowSyncCommitChange) -> wire::ShowSyncCommit {
    wire::ShowSyncCommit {
        show_id: change.show_id.0,
        show_revision: change.show_revision,
        previous_show_revision: change.previous_show_revision,
        patch_revision: change.patch_revision,
        request_id: change.request_id.clone(),
        association_id: change.association_id,
        objects: change
            .objects
            .iter()
            .map(|object| wire::ShowSyncCommittedObject {
                kind: object.kind.clone(),
                id: object.id.clone(),
                revision: object.revision,
                deleted: object.deleted,
                body: object.body.clone(),
                body_omitted: object.body_omitted,
            })
            .collect(),
        metadata: change
            .metadata
            .iter()
            .map(|(key, value)| wire::ShowSyncMetadataChange {
                key: key.clone(),
                value: value.clone(),
            })
            .collect(),
        profile_revisions: change
            .profile_revisions
            .iter()
            .map(|profile| wire::ShowSyncProfileReference {
                profile_id: profile.profile_id,
                revision: profile.revision,
                content_digest: profile.content_digest.clone(),
            })
            .collect(),
        desk_only_changes: change.desk_only_changes,
    }
}

pub(super) fn wire_gap(gap: &application::ShowSyncGapChange) -> wire::ShowSyncGap {
    wire::ShowSyncGap {
        show_id: gap.show_id.0,
        show_revision: gap.show_revision,
        reason: match gap.reason {
            application::ShowSyncGapReason::BulkCommit => wire::ShowSyncGapReason::BulkCommit,
            application::ShowSyncGapReason::OutOfBandWrite => {
                wire::ShowSyncGapReason::OutOfBandWrite
            }
            application::ShowSyncGapReason::ShowReplaced => wire::ShowSyncGapReason::ShowReplaced,
        },
    }
}
