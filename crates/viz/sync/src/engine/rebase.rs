//! Rebuilding the working document as the confirmed mirror plus the edits still awaiting the desk.

use super::{Answer, Shared};
use crate::intent::{HeldFields, PendingOperation, overlay};
use crate::journal::{EntryState, JournalEntry};
use crate::state::{ObjectKey, ProfileKey, SyncState, VersionedState, fixture_profile};
use light_application::PatchFixturesCommand;
use light_core::FixtureId;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use viz_document::PlanningDocument;

/// A rebuild that needs a profile revision the document does not hold yet; a snapshot of the desk
/// supplies it.
pub(crate) const MISSING_PROFILE: &str = "missing profile revision";

/// The edits of `entry` that still shape the document, and the fields the desk kept as its own.
fn overlaid(entry: &JournalEntry, mirrored: u64) -> Option<HeldFields> {
    let committed_after_mirror = entry
        .outcome
        .as_ref()
        .is_some_and(|outcome| outcome.show_revision > mirrored);
    match entry.state {
        EntryState::Pending => Some(HeldFields::new()),
        EntryState::Accepted if committed_after_mirror => Some(HeldFields::new()),
        EntryState::Conflict if committed_after_mirror => {
            let mut held = HeldFields::new();
            for conflict in entry.outcome.iter().flat_map(|outcome| &outcome.conflicts) {
                held.entry(conflict_key(&conflict.kind, &conflict.id))
                    .or_default()
                    .insert(conflict.path.clone());
            }
            Some(held)
        }
        _ => None,
    }
}

pub(crate) fn conflict_key(kind: &str, id: &str) -> ObjectKey {
    ObjectKey::new(kind, id)
}

/// The document the Architect should be showing: the mirror with every unconfirmed edit applied
/// over it, restricted to `scope` when one is given.
pub(crate) fn expected_state(
    mirror: &VersionedState,
    entries: &[JournalEntry],
    scope: Option<&BTreeSet<ObjectKey>>,
) -> SyncState {
    let mut state = SyncState {
        objects: match scope {
            None => mirror.state.objects.clone(),
            Some(keys) => keys
                .iter()
                .filter_map(|key| {
                    mirror
                        .state
                        .objects
                        .get(key)
                        .map(|body| (key.clone(), body.clone()))
                })
                .collect(),
        },
        metadata: mirror.state.metadata.clone(),
    };
    for entry in entries {
        let Some(held) = overlaid(entry, mirror.show_revision) else {
            continue;
        };
        let operations: Vec<PendingOperation> = entry
            .operations
            .iter()
            .filter(|operation| match (scope, operation.object()) {
                (Some(keys), Some(key)) => keys.contains(key),
                _ => true,
            })
            .cloned()
            .collect();
        overlay(&mut state, &operations, &held);
    }
    state
}

/// Every object an unresolved entry touches.
pub(crate) fn entry_keys(entries: &[JournalEntry]) -> BTreeSet<ObjectKey> {
    entries
        .iter()
        .flat_map(|entry| entry.operations.iter().filter_map(PendingOperation::object))
        .cloned()
        .collect()
}

impl Shared {
    /// Brings the document to the expected state for `scope` (everything when `None`). Returns
    /// whether the document changed.
    pub(crate) fn rebase(
        &self,
        document: &PlanningDocument,
        scope: Option<&BTreeSet<ObjectKey>>,
        profiles: &BTreeMap<ProfileKey, Value>,
    ) -> Answer<bool> {
        let entries = self.journal.lock().unresolved()?;
        let mut keys = scope.cloned();
        if let Some(keys) = keys.as_mut() {
            keys.extend(entry_keys(&entries));
        }
        let target = {
            let mirror = self.mirror.lock();
            expected_state(mirror.state(), &entries, keys.as_ref())
        };
        let mut shadow = self.shadow.lock();
        let current = &shadow.versioned.state;
        let candidates: BTreeSet<ObjectKey> = match &keys {
            Some(keys) => keys.clone(),
            None => current
                .objects
                .keys()
                .chain(target.objects.keys())
                .cloned()
                .collect(),
        };
        let objects: Vec<(ObjectKey, Option<Value>)> = candidates
            .into_iter()
            .filter(|key| current.objects.get(key) != target.objects.get(key))
            .map(|key| {
                let body = target.objects.get(&key).cloned();
                (key, body)
            })
            .collect();
        let metadata: Vec<(String, Option<String>)> = current
            .metadata
            .keys()
            .chain(target.metadata.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|key| current.metadata.get(*key) != target.metadata.get(*key))
            .map(|key| (key.clone(), target.metadata.get(key).cloned()))
            .collect();
        if objects.is_empty() && metadata.is_empty() {
            return Ok(false);
        }
        let applied = apply_changes(document, &objects, &metadata, profiles);
        // Whatever happened, the shadow follows the document, so nothing applied here is ever
        // mistaken for the Architect's own edit.
        shadow.versioned = shadow.reader.read(document)?;
        applied?;
        Ok(true)
    }
}

/// Writes desk-confirmed content into the working document: object puts, then fixtures through
/// the patch service (so the document stays a valid show), then object deletes and metadata.
pub(crate) fn apply_changes(
    document: &PlanningDocument,
    objects: &[(ObjectKey, Option<Value>)],
    metadata: &[(String, Option<String>)],
    profiles: &BTreeMap<ProfileKey, Value>,
) -> Answer<()> {
    for (key, body) in objects {
        if let (false, Some(body)) = (key.is_fixture(), body) {
            document
                .put_object(&key.kind, &key.id, body)
                .map_err(|e| e.to_string())?;
        }
    }
    let mut fixtures = Vec::new();
    let mut removed = Vec::new();
    for (key, body) in objects.iter().filter(|(key, _)| key.is_fixture()) {
        match body {
            Some(body) => {
                if let Some(profile) = fixture_profile(body) {
                    ensure_profile(document, profile, profiles)?;
                }
                let input = serde_json::from_value(body.clone()).map_err(|e| e.to_string())?;
                fixtures.push(light_patch_wire::application_fixture(input)?);
            }
            None => removed.push(FixtureId(
                key.id
                    .parse()
                    .map_err(|_| "patched fixture identity is not a UUID")?,
            )),
        }
    }
    if !fixtures.is_empty() || !removed.is_empty() {
        document
            .patch_fixtures(PatchFixturesCommand {
                show_id: document.show_id(),
                fixtures,
                remove_fixture_ids: removed,
                placements: Vec::new(),
                vector_spreads: Vec::new(),
                fixture_updates: Vec::new(),
            })
            .map_err(|e| format!("Control's patch change could not be applied here: {e}"))?;
    }
    for (key, body) in objects {
        if !key.is_fixture() && body.is_none() {
            document
                .delete_object(&key.kind, &key.id)
                .map_err(|e| e.to_string())?;
        }
    }
    if !metadata.is_empty() {
        let values: Vec<(&str, Option<&str>)> = metadata
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_deref()))
            .collect();
        document
            .replace_metadata_values(&values)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_profile(
    document: &PlanningDocument,
    profile: ProfileKey,
    profiles: &BTreeMap<ProfileKey, Value>,
) -> Answer<()> {
    if super::capture::document_profile(document, profile).is_some() {
        return Ok(());
    }
    let body = profiles.get(&profile).ok_or(MISSING_PROFILE)?;
    document
        .retain_fixture_profile(body.clone())
        .map_err(|e| e.to_string())
}
