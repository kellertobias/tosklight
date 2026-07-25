//! Lossless conversion between v2 selective-import DTOs and application models.

use light_application as app;
use light_core::{FixtureId, ShowId};
use light_show::{PortableShowDocument, PortableShowObjectKey, PortableShowRevision};
use light_wire::v2::selective_import as wire;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn application_request(
    source_show_id: ShowId,
    target_show_id: ShowId,
    selection: wire::SelectiveImportSelection,
) -> Result<app::SelectiveShowImportRequest, String> {
    if selection.selected_objects.len() > 4_096
        || selection.conflict_resolutions.len() > 4_096
        || selection.profile_conflict_resolutions.len() > 4_096
    {
        return Err("selective import requests are limited to 4096 entries per section".into());
    }
    let selected_objects = unique_object_keys(selection.selected_objects, "selected object")?;
    let mut conflict_resolutions = BTreeMap::new();
    for choice in selection.conflict_resolutions {
        let key = application_key(choice.key)?;
        if conflict_resolutions
            .insert(key, conflict_resolution(choice.resolution))
            .is_some()
        {
            return Err("an object conflict has more than one resolution".into());
        }
    }
    let mut profile_conflict_resolutions = BTreeMap::new();
    for choice in selection.profile_conflict_resolutions {
        let key = application_profile_key(choice.key);
        if profile_conflict_resolutions
            .insert(key, profile_resolution(choice.resolution))
            .is_some()
        {
            return Err("a profile conflict has more than one resolution".into());
        }
    }
    Ok(app::SelectiveShowImportRequest {
        source_show_id,
        target_show_id,
        selected_objects,
        conflict_resolutions,
        profile_conflict_resolutions,
    })
}

pub(super) fn catalog(document: &PortableShowDocument) -> wire::SelectiveImportCatalog {
    wire::SelectiveImportCatalog {
        source_show_id: document.id().0,
        source_show_name: document.name().into(),
        source_revision: document.revision().value(),
        objects: document
            .objects()
            .map(|object| wire::SelectiveImportCatalogObject {
                key: wire_key(object.key()),
                object_revision: object.revision(),
                display_name: display_name(object.key(), object.body()),
            })
            .collect(),
    }
}

pub(super) fn preview(preview: app::SelectiveShowImportPreview) -> wire::SelectiveImportPreview {
    let can_apply = preview.can_apply();
    wire::SelectiveImportPreview {
        source_show_id: preview.request.source_show_id.0,
        target_show_id: preview.request.target_show_id.0,
        source_revision: preview.source_revision.value(),
        target_revision: preview.target_revision.value(),
        objects: preview.objects.into_iter().map(object_preview).collect(),
        dependencies: preview
            .dependencies
            .into_iter()
            .map(|dependency| wire::SelectiveImportDependency {
                owner: wire_key(&dependency.owner),
                dependency: wire_key(&dependency.dependency),
                disposition: dependency_disposition(dependency.disposition),
            })
            .collect(),
        conflicts: preview
            .conflicts
            .into_iter()
            .map(|conflict| wire::SelectiveImportConflict {
                key: wire_key(&conflict.key),
                resolution: conflict.resolution.map(wire_conflict_resolution),
            })
            .collect(),
        profiles: preview.profiles.into_iter().map(profile_preview).collect(),
        managed_assets: preview
            .managed_assets
            .into_iter()
            .map(|asset| wire::SelectiveImportManagedAssetPreview {
                asset: wire_asset(asset.asset),
                action: managed_asset_action(asset.action),
            })
            .collect(),
        blockers: preview.blockers.into_iter().map(blocker).collect(),
        can_apply,
    }
}

pub(super) fn outcome(result: app::SelectiveShowImportResult) -> wire::SelectiveImportOutcome {
    wire::SelectiveImportOutcome {
        request_id: result.context.request_id.clone().unwrap_or_default(),
        correlation_id: result.context.correlation_id,
        changed: result.changed,
        show_id: result.change.show_id.0,
        show_revision: result.change.show_revision.value(),
        event_sequence: result.event_sequence,
        outcomes: result
            .change
            .outcomes
            .into_iter()
            .map(object_preview)
            .collect(),
        objects: result
            .change
            .objects
            .into_iter()
            .map(|object| wire::SelectiveImportOutcomeObjectChange {
                key: wire_key(&object.key),
                object_revision: object.object_revision,
                body: object.body,
            })
            .collect(),
        profiles: result
            .change
            .profiles
            .into_iter()
            .map(|profile| wire::SelectiveImportProfileChange {
                source: wire_profile_key(profile.source),
                destination: wire_profile_key(profile.destination),
                digest: profile.digest,
            })
            .collect(),
        managed_assets: result
            .change
            .managed_assets
            .into_iter()
            .map(wire_asset)
            .collect(),
    }
}

pub(super) fn expected_revision(value: u64) -> PortableShowRevision {
    PortableShowRevision::from_value(value)
}

fn unique_object_keys(
    keys: Vec<wire::SelectiveImportObjectKey>,
    label: &str,
) -> Result<BTreeSet<PortableShowObjectKey>, String> {
    let mut output = BTreeSet::new();
    for key in keys {
        if !output.insert(application_key(key)?) {
            return Err(format!("duplicate {label}"));
        }
    }
    Ok(output)
}

fn application_key(key: wire::SelectiveImportObjectKey) -> Result<PortableShowObjectKey, String> {
    for (label, value) in [("object kind", &key.kind), ("object id", &key.id)] {
        if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
            return Err(format!("{label} must contain 1-256 printable characters"));
        }
    }
    Ok(PortableShowObjectKey::new(key.kind, key.id))
}

fn application_profile_key(key: wire::SelectiveImportProfileKey) -> app::ImportProfileKey {
    app::ImportProfileKey {
        profile_id: FixtureId(key.profile_id),
        revision: key.revision,
    }
}

fn conflict_resolution(
    value: wire::SelectiveImportConflictResolution,
) -> app::ImportConflictResolution {
    match value {
        wire::SelectiveImportConflictResolution::KeepDestination => {
            app::ImportConflictResolution::KeepDestination
        }
        wire::SelectiveImportConflictResolution::ReplaceDestination => {
            app::ImportConflictResolution::ReplaceDestination
        }
        wire::SelectiveImportConflictResolution::Duplicate => {
            app::ImportConflictResolution::Duplicate
        }
    }
}

fn profile_resolution(
    value: wire::SelectiveImportProfileConflictResolution,
) -> app::ImportProfileConflictResolution {
    match value {
        wire::SelectiveImportProfileConflictResolution::KeepDestination => {
            app::ImportProfileConflictResolution::KeepDestination
        }
        wire::SelectiveImportProfileConflictResolution::Duplicate => {
            app::ImportProfileConflictResolution::Duplicate
        }
    }
}

fn wire_conflict_resolution(
    value: app::ImportConflictResolution,
) -> wire::SelectiveImportConflictResolution {
    match value {
        app::ImportConflictResolution::KeepDestination => {
            wire::SelectiveImportConflictResolution::KeepDestination
        }
        app::ImportConflictResolution::ReplaceDestination => {
            wire::SelectiveImportConflictResolution::ReplaceDestination
        }
        app::ImportConflictResolution::Duplicate => {
            wire::SelectiveImportConflictResolution::Duplicate
        }
    }
}

fn object_preview(value: app::AppliedImportObject) -> wire::SelectiveImportObjectPreview {
    let action = match value.action {
        app::ImportObjectAction::ImportPreservingId => {
            wire::SelectiveImportObjectAction::ImportPreservingId
        }
        app::ImportObjectAction::SkipIdentical => wire::SelectiveImportObjectAction::SkipIdentical,
        app::ImportObjectAction::KeepDestination => {
            wire::SelectiveImportObjectAction::KeepDestination
        }
        app::ImportObjectAction::ReplaceDestination => {
            wire::SelectiveImportObjectAction::ReplaceDestination
        }
        app::ImportObjectAction::Duplicate { destination } => {
            wire::SelectiveImportObjectAction::Duplicate {
                destination: wire_key(&destination),
            }
        }
        app::ImportObjectAction::BlockedConflict => {
            wire::SelectiveImportObjectAction::BlockedConflict
        }
    };
    wire::SelectiveImportObjectPreview {
        source: wire_key(&value.source),
        destination: wire_key(&value.destination),
        action,
    }
}

fn dependency_disposition(
    value: app::ImportDependencyDisposition,
) -> wire::SelectiveImportDependencyDisposition {
    match value {
        app::ImportDependencyDisposition::Selected => {
            wire::SelectiveImportDependencyDisposition::Selected
        }
        app::ImportDependencyDisposition::Included => {
            wire::SelectiveImportDependencyDisposition::Included
        }
        app::ImportDependencyDisposition::BoundToDestination => {
            wire::SelectiveImportDependencyDisposition::BoundToDestination
        }
        app::ImportDependencyDisposition::Missing => {
            wire::SelectiveImportDependencyDisposition::Missing
        }
    }
}

fn profile_preview(value: app::ImportProfilePreview) -> wire::SelectiveImportProfilePreview {
    let action = match value.action {
        app::ImportProfileAction::Copy => wire::SelectiveImportProfileAction::Copy,
        app::ImportProfileAction::SkipIdentical => {
            wire::SelectiveImportProfileAction::SkipIdentical
        }
        app::ImportProfileAction::KeepDestination => {
            wire::SelectiveImportProfileAction::KeepDestination
        }
        app::ImportProfileAction::Duplicate { destination } => {
            wire::SelectiveImportProfileAction::Duplicate {
                destination: wire_profile_key(destination),
            }
        }
        app::ImportProfileAction::BlockedConflict => {
            wire::SelectiveImportProfileAction::BlockedConflict
        }
        app::ImportProfileAction::Missing => wire::SelectiveImportProfileAction::Missing,
    };
    wire::SelectiveImportProfilePreview {
        source: wire_profile_key(value.source),
        destination: wire_profile_key(value.destination),
        action,
    }
}

fn managed_asset_action(
    value: app::ImportManagedAssetAction,
) -> wire::SelectiveImportManagedAssetAction {
    match value {
        app::ImportManagedAssetAction::Copy => wire::SelectiveImportManagedAssetAction::Copy,
        app::ImportManagedAssetAction::SkipIdentical => {
            wire::SelectiveImportManagedAssetAction::SkipIdentical
        }
        app::ImportManagedAssetAction::Missing => wire::SelectiveImportManagedAssetAction::Missing,
        app::ImportManagedAssetAction::BlockedConflict => {
            wire::SelectiveImportManagedAssetAction::BlockedConflict
        }
    }
}

fn blocker(value: app::ImportBlocker) -> wire::SelectiveImportBlocker {
    use app::ImportBlocker as A;
    use wire::SelectiveImportBlocker as W;
    match value {
        A::EmptySelection => W::EmptySelection,
        A::SameShow => W::SameShow,
        A::UnsupportedObject { key } => W::UnsupportedObject {
            key: wire_key(&key),
        },
        A::MissingObject { key, required_by } => W::MissingObject {
            key: wire_key(&key),
            required_by: required_by.as_ref().map(wire_key),
        },
        A::ObjectConflict { key } => W::ObjectConflict {
            key: wire_key(&key),
        },
        A::InvalidResolution { key, message } => W::InvalidResolution {
            key: wire_key(&key),
            message,
        },
        A::InvalidProfileResolution { key, message } => W::InvalidProfileResolution {
            key: wire_profile_key(key),
            message,
        },
        A::InvalidDescriptor { key, message } => W::InvalidDescriptor {
            key: wire_key(&key),
            message,
        },
        A::MissingProfile { key, required_by } => W::MissingProfile {
            key: wire_profile_key(key),
            required_by: wire_key(&required_by),
        },
        A::ProfileConflict { key } => W::ProfileConflict {
            key: wire_profile_key(key),
        },
        A::MissingManagedAsset { asset } => W::MissingManagedAsset {
            asset: wire_asset(asset),
        },
        A::ManagedAssetConflict { asset } => W::ManagedAssetConflict {
            asset: wire_asset(asset),
        },
        A::ReferenceRewrite { owner, message } => W::ReferenceRewrite {
            owner: wire_key(&owner),
            message,
        },
        A::CandidateInvalid { message } => W::CandidateInvalid { message },
    }
}

fn wire_key(value: &PortableShowObjectKey) -> wire::SelectiveImportObjectKey {
    wire::SelectiveImportObjectKey {
        kind: value.kind().into(),
        id: value.id().into(),
    }
}

fn wire_profile_key(value: app::ImportProfileKey) -> wire::SelectiveImportProfileKey {
    wire::SelectiveImportProfileKey {
        profile_id: value.profile_id.0,
        revision: value.revision,
    }
}

fn wire_asset(value: app::AssetReference) -> wire::SelectiveImportAssetReference {
    wire::SelectiveImportAssetReference {
        asset_id: value.id.0,
        revision: value.revision.0,
    }
}

fn display_name(key: &PortableShowObjectKey, body: &serde_json::Value) -> String {
    for field in ["name", "label", "title", "number"] {
        if let Some(value) = body.get(field) {
            if let Some(text) = value.as_str().filter(|text| !text.trim().is_empty()) {
                return text.into();
            }
            if value.is_number() {
                return value.to_string();
            }
        }
    }
    key.id().into()
}
