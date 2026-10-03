//! Resolves a sync command against the open document into one staged transaction.

use super::fields::{FieldResolution, apply_field_edits, present};
use super::ports::RetainedProfilePorts;
use super::{
    METADATA_KIND, PATCHED_FIXTURE_KIND, ShowSyncCommand, ShowSyncConflict, ShowSyncConflictReason,
    ShowSyncFieldEdit, ShowSyncOperation, ShowSyncPorts, is_sync_metadata_key, is_sync_object_kind,
};
use crate::show_patch::{StagedPatch, fixture_projection, stage_patch_command};
use crate::{
    ActionError, ActionErrorKind, ActiveShowObjectBody, ActiveShowObjectKind, PatchFixturesCommand,
};
use light_core::{FixtureId, Revision};
use light_show::{
    FixtureProfileRevision, PortableShowDocument, PortableShowObjectKey, PortableShowTransaction,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

/// One sync command resolved field by field against the document it will commit to.
pub(super) struct SyncStage {
    pub(super) transaction: PortableShowTransaction,
    pub(super) patch: Option<StagedPatch>,
    /// Generic objects this command writes, and whether each is deleted.
    pub(super) objects: BTreeMap<PortableShowObjectKey, bool>,
    pub(super) patched_fixtures: Vec<(FixtureId, bool)>,
    pub(super) metadata: Vec<String>,
    pub(super) conflicts: Vec<ShowSyncConflict>,
}

impl SyncStage {
    pub(super) fn extra_keys(&self) -> BTreeSet<PortableShowObjectKey> {
        self.objects.keys().cloned().collect()
    }
}

struct WorkingObject {
    body: Option<Value>,
    revision: Option<u64>,
    dirty: bool,
}

#[derive(Default)]
struct Staging {
    objects: BTreeMap<PortableShowObjectKey, WorkingObject>,
    /// Staged patch documents by fixture UUID.
    fixtures: BTreeMap<Uuid, Value>,
    removed_fixtures: Vec<FixtureId>,
    profiles: BTreeMap<(Uuid, Revision), FixtureProfileRevision>,
    metadata: BTreeMap<String, Option<String>>,
    conflicts: Vec<ShowSyncConflict>,
}

pub(super) fn validate_command(command: &ShowSyncCommand) -> Result<(), ActionError> {
    if command.operations.is_empty() {
        return Err(invalid("a sync transaction needs at least one operation"));
    }
    for operation in &command.operations {
        match operation {
            ShowSyncOperation::UpdateObject { kind, .. }
            | ShowSyncOperation::CreateObject { kind, .. }
            | ShowSyncOperation::DeleteObject { kind, .. }
                if !is_sync_object_kind(kind) =>
            {
                return Err(invalid(format!(
                    "object kind {kind:?} is not synchronized between Control and Architect"
                )));
            }
            ShowSyncOperation::SetMetadata { key, .. } if !is_sync_metadata_key(key) => {
                return Err(invalid(format!(
                    "metadata key {key:?} is not synchronized between Control and Architect"
                )));
            }
            ShowSyncOperation::CreateObject { body, .. } if !body.is_object() => {
                return Err(invalid("a created object body must be a JSON object"));
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn stage_sync<P: ShowSyncPorts>(
    document: &PortableShowDocument,
    command: &ShowSyncCommand,
    ports: &P,
) -> Result<SyncStage, ActionError> {
    let mut staging = Staging::default();
    for operation in &command.operations {
        staging.apply(document, operation, ports)?;
    }
    staging.finish(document, command, ports)
}

impl Staging {
    fn apply<P: ShowSyncPorts>(
        &mut self,
        document: &PortableShowDocument,
        operation: &ShowSyncOperation,
        ports: &P,
    ) -> Result<(), ActionError> {
        match operation {
            ShowSyncOperation::UpdateObject { kind, id, fields } => {
                self.update_object(document, kind, id, fields)
            }
            ShowSyncOperation::CreateObject { kind, id, body } => {
                self.create_object(document, kind, id, body);
                Ok(())
            }
            ShowSyncOperation::DeleteObject {
                kind,
                id,
                base_object_revision,
            } => {
                self.delete_object(document, kind, id, *base_object_revision);
                Ok(())
            }
            ShowSyncOperation::UpdatePatchFixture { fixture_id, fields } => {
                self.update_fixture(document, *fixture_id, fields, ports)
            }
            ShowSyncOperation::CreatePatchFixture {
                fixture_id,
                document: fixture,
            } => self.create_fixture(document, *fixture_id, fixture, ports),
            ShowSyncOperation::RemovePatchFixture {
                fixture_id,
                base_fixture_revision,
            } => self.remove_fixture(document, *fixture_id, *base_fixture_revision),
            ShowSyncOperation::RetainProfileRevision {
                profile_id,
                revision,
                profile,
            } => self.retain_profile(document, *profile_id, *revision, profile),
            ShowSyncOperation::SetMetadata { key, base, value } => {
                self.set_metadata(document, key, base.as_deref(), value.as_deref());
                Ok(())
            }
        }
    }

    fn working(
        &mut self,
        document: &PortableShowDocument,
        kind: &str,
        id: &str,
    ) -> &mut WorkingObject {
        self.objects
            .entry(PortableShowObjectKey::new(kind, id))
            .or_insert_with(|| {
                let stored = document.object(kind, id);
                WorkingObject {
                    body: stored.map(|object| object.body().clone()),
                    revision: stored.map(light_show::PortableShowObject::revision),
                    dirty: false,
                }
            })
    }

    fn update_object(
        &mut self,
        document: &PortableShowDocument,
        kind: &str,
        id: &str,
        fields: &[ShowSyncFieldEdit],
    ) -> Result<(), ActionError> {
        let working = self.working(document, kind, id);
        let revision = working.revision;
        let Some(body) = working.body.as_mut() else {
            let conflicts = deleted_conflicts(kind, id, fields);
            self.conflicts.extend(conflicts);
            return Ok(());
        };
        let resolutions = apply_field_edits(body, fields)?;
        let applied = resolutions.contains(&FieldResolution::Applied);
        working.dirty |= applied;
        self.conflicts
            .extend(field_conflicts(kind, id, fields, resolutions, revision));
        Ok(())
    }

    fn create_object(
        &mut self,
        document: &PortableShowDocument,
        kind: &str,
        id: &str,
        body: &Value,
    ) {
        let working = self.working(document, kind, id);
        match &working.body {
            None => {
                working.body = Some(body.clone());
                working.dirty = true;
            }
            Some(existing) if existing == body => {}
            Some(existing) => {
                let conflict = ShowSyncConflict {
                    kind: kind.to_owned(),
                    id: id.to_owned(),
                    path: String::new(),
                    base: None,
                    mine: Some(body.clone()),
                    theirs: Some(existing.clone()),
                    theirs_revision: working.revision,
                    reason: ShowSyncConflictReason::ObjectExists,
                };
                self.conflicts.push(conflict);
            }
        }
    }

    fn delete_object(
        &mut self,
        document: &PortableShowDocument,
        kind: &str,
        id: &str,
        base_object_revision: u64,
    ) {
        let working = self.working(document, kind, id);
        let Some(existing) = &working.body else {
            return;
        };
        if working.revision == Some(base_object_revision) && !working.dirty {
            working.body = None;
            working.dirty = true;
            return;
        }
        let conflict = ShowSyncConflict {
            kind: kind.to_owned(),
            id: id.to_owned(),
            path: String::new(),
            base: None,
            mine: None,
            theirs: Some(existing.clone()),
            theirs_revision: working.revision,
            reason: ShowSyncConflictReason::ObjectModified,
        };
        self.conflicts.push(conflict);
    }

    fn fixture_document<P: ShowSyncPorts>(
        &mut self,
        document: &PortableShowDocument,
        fixture_id: FixtureId,
        ports: &P,
    ) -> Result<Option<(Value, u64)>, ActionError> {
        let Some(projection) = fixture_projection(document, fixture_id)? else {
            return Ok(None);
        };
        let revision = projection.fixture_revision;
        if let Some(staged) = self.fixtures.get(&fixture_id.0) {
            return Ok(Some((staged.clone(), revision)));
        }
        Ok(Some((ports.patch_fixture_document(&projection)?, revision)))
    }

    fn update_fixture<P: ShowSyncPorts>(
        &mut self,
        document: &PortableShowDocument,
        fixture_id: FixtureId,
        fields: &[ShowSyncFieldEdit],
        ports: &P,
    ) -> Result<(), ActionError> {
        let id = fixture_id.0.to_string();
        let Some((mut body, revision)) = self.fixture_document(document, fixture_id, ports)? else {
            let staged = self.fixtures.get(&fixture_id.0).cloned();
            let Some(mut body) = staged else {
                self.conflicts
                    .extend(deleted_conflicts(PATCHED_FIXTURE_KIND, &id, fields));
                return Ok(());
            };
            let resolutions = apply_field_edits(&mut body, fields)?;
            self.conflicts.extend(field_conflicts(
                PATCHED_FIXTURE_KIND,
                &id,
                fields,
                resolutions,
                None,
            ));
            self.fixtures.insert(fixture_id.0, body);
            return Ok(());
        };
        let resolutions = apply_field_edits(&mut body, fields)?;
        if resolutions.contains(&FieldResolution::Applied) {
            self.fixtures.insert(fixture_id.0, body);
        }
        self.conflicts.extend(field_conflicts(
            PATCHED_FIXTURE_KIND,
            &id,
            fields,
            resolutions,
            Some(revision),
        ));
        Ok(())
    }

    fn create_fixture<P: ShowSyncPorts>(
        &mut self,
        document: &PortableShowDocument,
        fixture_id: FixtureId,
        fixture: &Value,
        ports: &P,
    ) -> Result<(), ActionError> {
        match self.fixture_document(document, fixture_id, ports)? {
            None => {
                self.fixtures.insert(fixture_id.0, fixture.clone());
            }
            Some((existing, _)) if &existing == fixture => {}
            Some((existing, revision)) => self.conflicts.push(ShowSyncConflict {
                kind: PATCHED_FIXTURE_KIND.into(),
                id: fixture_id.0.to_string(),
                path: String::new(),
                base: None,
                mine: Some(fixture.clone()),
                theirs: Some(existing),
                theirs_revision: Some(revision),
                reason: ShowSyncConflictReason::ObjectExists,
            }),
        }
        Ok(())
    }

    fn remove_fixture(
        &mut self,
        document: &PortableShowDocument,
        fixture_id: FixtureId,
        base_fixture_revision: u64,
    ) -> Result<(), ActionError> {
        let Some(projection) = fixture_projection(document, fixture_id)? else {
            self.fixtures.remove(&fixture_id.0);
            return Ok(());
        };
        if projection.fixture_revision == base_fixture_revision {
            self.fixtures.remove(&fixture_id.0);
            self.removed_fixtures.push(fixture_id);
            return Ok(());
        }
        self.conflicts.push(ShowSyncConflict {
            kind: PATCHED_FIXTURE_KIND.into(),
            id: fixture_id.0.to_string(),
            path: String::new(),
            base: None,
            mine: None,
            theirs: None,
            theirs_revision: Some(projection.fixture_revision),
            reason: ShowSyncConflictReason::ObjectModified,
        });
        Ok(())
    }

    fn retain_profile(
        &mut self,
        document: &PortableShowDocument,
        profile_id: FixtureId,
        revision: Revision,
        profile: &Value,
    ) -> Result<(), ActionError> {
        let candidate = FixtureProfileRevision::new(profile_id, revision, profile.clone())
            .map_err(|error| invalid(format!("retained fixture profile is invalid: {error}")))?;
        if let Some(existing) = document.fixture_profile_revision(profile_id, revision) {
            if existing.digest() != candidate.digest() {
                return Err(ActionError::new(
                    ActionErrorKind::Conflict,
                    "this show already holds a different profile under the same revision",
                ));
            }
            return Ok(());
        }
        self.profiles.insert((profile_id.0, revision), candidate);
        Ok(())
    }

    fn set_metadata(
        &mut self,
        document: &PortableShowDocument,
        key: &str,
        base: Option<&str>,
        value: Option<&str>,
    ) {
        let current = match self.metadata.get(key) {
            Some(staged) => staged.clone(),
            None => document.metadata().get(key).cloned(),
        };
        let current = current.filter(|value| !value.is_empty());
        if current.as_deref() == non_empty(value) {
            return;
        }
        if current.as_deref() == non_empty(base) {
            self.metadata
                .insert(key.to_owned(), non_empty(value).map(str::to_owned));
            return;
        }
        self.conflicts.push(ShowSyncConflict {
            kind: METADATA_KIND.into(),
            id: key.to_owned(),
            path: String::new(),
            base: base.map(|value| Value::String(value.to_owned())),
            mine: value.map(|value| Value::String(value.to_owned())),
            theirs: current.map(Value::String),
            theirs_revision: None,
            reason: ShowSyncConflictReason::FieldChanged,
        });
    }

    fn finish<P: ShowSyncPorts>(
        self,
        document: &PortableShowDocument,
        command: &ShowSyncCommand,
        ports: &P,
    ) -> Result<SyncStage, ActionError> {
        let Self {
            objects,
            fixtures,
            removed_fixtures,
            profiles,
            metadata,
            conflicts,
        } = self;
        let mut patched_fixtures = Vec::new();
        let patch = if fixtures.is_empty() && removed_fixtures.is_empty() {
            None
        } else {
            let mut patch_command = PatchFixturesCommand {
                show_id: command.show_id,
                fixtures: Vec::with_capacity(fixtures.len()),
                remove_fixture_ids: removed_fixtures.clone(),
                placements: Vec::new(),
                vector_spreads: Vec::new(),
                fixture_updates: Vec::new(),
            };
            for (fixture_id, body) in fixtures {
                patch_command
                    .fixtures
                    .push(ports.patch_fixture_candidate(body)?);
                patched_fixtures.push((FixtureId(fixture_id), false));
            }
            patched_fixtures.extend(removed_fixtures.iter().map(|id| (*id, true)));
            let retained = RetainedProfilePorts {
                inner: ports,
                retained: &profiles,
            };
            Some(stage_patch_command(document, &patch_command, &retained)?)
        };
        let mut patch = patch.filter(|patch| !patch.is_empty());
        if patch.is_none() {
            patched_fixtures.clear();
        }
        let mut transaction = patch
            .as_mut()
            .map_or_else(|| document.transaction(), StagedPatch::take_transaction);
        let mut written = BTreeMap::new();
        for (key, object) in objects.into_iter().filter(|(_, object)| object.dirty) {
            match object.body {
                Some(body) => {
                    validate_typed_body(key.kind(), &body)?;
                    transaction.put(key.kind(), key.id(), body);
                    written.insert(key, false);
                }
                None => {
                    transaction.delete(key.kind(), key.id());
                    written.insert(key, true);
                }
            }
        }
        let metadata_keys = metadata.keys().cloned().collect();
        for (key, value) in metadata {
            transaction.set_metadata(key, value);
        }
        Ok(SyncStage {
            transaction,
            patch,
            objects: written,
            patched_fixtures,
            metadata: metadata_keys,
            conflicts,
        })
    }
}

/// A synchronized kind Control also owns is checked against its typed family before it can
/// reach the desk's runtime.
fn validate_typed_body(kind: &str, body: &Value) -> Result<(), ActionError> {
    if let Some(family) = ActiveShowObjectKind::from_storage_kind(kind) {
        ActiveShowObjectBody::decode(family, body.clone())
            .map_err(|error| invalid(format!("invalid {kind} body: {error}")))?;
    }
    Ok(())
}

fn field_conflicts(
    kind: &str,
    id: &str,
    fields: &[ShowSyncFieldEdit],
    resolutions: Vec<FieldResolution>,
    revision: Option<u64>,
) -> Vec<ShowSyncConflict> {
    fields
        .iter()
        .zip(resolutions)
        .filter_map(|(field, resolution)| match resolution {
            FieldResolution::Conflict { theirs } => Some(ShowSyncConflict {
                kind: kind.to_owned(),
                id: id.to_owned(),
                path: field.path.clone(),
                base: present(field.base.as_ref()).cloned(),
                mine: present(field.value.as_ref()).cloned(),
                theirs,
                theirs_revision: revision,
                reason: ShowSyncConflictReason::FieldChanged,
            }),
            _ => None,
        })
        .collect()
}

fn deleted_conflicts(kind: &str, id: &str, fields: &[ShowSyncFieldEdit]) -> Vec<ShowSyncConflict> {
    fields
        .iter()
        .map(|field| ShowSyncConflict {
            kind: kind.to_owned(),
            id: id.to_owned(),
            path: field.path.clone(),
            base: present(field.base.as_ref()).cloned(),
            mine: present(field.value.as_ref()).cloned(),
            theirs: None,
            theirs_revision: None,
            reason: ShowSyncConflictReason::ObjectDeleted,
        })
        .collect()
}

/// An empty metadata value reads as absent, as the paperwork editor writes it.
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
