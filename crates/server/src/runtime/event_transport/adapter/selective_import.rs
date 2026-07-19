use light_application as application;
use light_wire::v2::events as wire;

pub(super) fn wire_change(
    change: &application::SelectiveShowImportChange,
) -> wire::SelectiveImportChange {
    wire::SelectiveImportChange {
        show_id: change.show_id.0,
        show_revision: change.show_revision.value(),
        objects: change.objects.iter().map(wire_object_change).collect(),
        profile_revisions: change
            .profiles
            .iter()
            .map(|profile| wire_profile_identity(profile.destination))
            .collect(),
        managed_assets: change
            .managed_assets
            .iter()
            .map(|asset| wire::ManagedAssetReference {
                asset_id: asset.id.0,
                revision: asset.revision.0,
            })
            .collect(),
    }
}

fn wire_object_change(
    change: &application::SelectiveShowObjectChange,
) -> wire::SelectiveImportObjectChange {
    wire::SelectiveImportObjectChange {
        kind: change.key.kind().into(),
        object_id: change.key.id().into(),
        object_revision: change.object_revision,
        body: change.body.clone(),
    }
}

fn wire_profile_identity(identity: application::ImportProfileKey) -> wire::FixtureProfileIdentity {
    wire::FixtureProfileIdentity {
        profile_id: identity.profile_id.0,
        revision: identity.revision,
    }
}
