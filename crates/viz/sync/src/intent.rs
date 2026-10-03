//! Show-edit intents: the only thing the journal accepts.
//!
//! An intent is the difference between two states of the *persistent* document, captured around
//! one Architect gesture. It has no public constructor other than [`ShowEditIntent::capture`],
//! which takes two [`SyncState`]s read from the show file. Live control — preview values, DMX
//! input, Highlight, selection — never changes the show file, so it has no way to become an intent
//! and therefore no way into the journal or onto the wire for replay.

use crate::state::{ObjectKey, SyncState, VersionedState, fixture_profile};
use light_application::show_sync::PATCHED_FIXTURE_KIND;
use light_wire::v2::show_sync::{ShowSyncFieldEdit, ShowSyncOperation};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

type Answer<T> = Result<T, String>;

/// One edit of a journaled gesture. Deletes carry the desk revision the document last saw, or
/// none for an object created by an earlier entry that the desk has not yet numbered; that one
/// is resolved when the entry is sent.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PendingOperation {
    UpdateObject {
        key: ObjectKey,
        fields: Vec<ShowSyncFieldEdit>,
    },
    CreateObject {
        key: ObjectKey,
        body: Value,
    },
    DeleteObject {
        key: ObjectKey,
        base_revision: Option<u64>,
    },
    RetainProfile {
        profile_id: Uuid,
        revision: u64,
        profile: Value,
    },
    SetMetadata {
        key: String,
        base: Option<String>,
        value: Option<String>,
    },
}

impl PendingOperation {
    /// The object this operation changes, if any.
    pub fn object(&self) -> Option<&ObjectKey> {
        match self {
            Self::UpdateObject { key, .. }
            | Self::CreateObject { key, .. }
            | Self::DeleteObject { key, .. } => Some(key),
            _ => None,
        }
    }

    /// The wire operation, resolving an unnumbered delete through `revision_of`.
    pub fn to_wire(
        &self,
        revision_of: impl Fn(&ObjectKey) -> Option<u64>,
    ) -> Answer<ShowSyncOperation> {
        Ok(match self.clone() {
            Self::UpdateObject { key, fields } if key.is_fixture() => {
                ShowSyncOperation::UpdatePatchFixture {
                    fixture_id: fixture_id(&key)?,
                    fields,
                }
            }
            Self::UpdateObject { key, fields } => ShowSyncOperation::UpdateObject {
                kind: key.kind,
                id: key.id,
                fields,
            },
            Self::CreateObject { key, body } if key.is_fixture() => {
                ShowSyncOperation::CreatePatchFixture {
                    fixture: Box::new(serde_json::from_value(body).map_err(|e| e.to_string())?),
                }
            }
            Self::CreateObject { key, body } => ShowSyncOperation::CreateObject {
                kind: key.kind,
                id: key.id,
                body,
            },
            Self::DeleteObject { key, base_revision } => {
                let base = base_revision.or_else(|| revision_of(&key)).unwrap_or(0);
                if key.is_fixture() {
                    ShowSyncOperation::RemovePatchFixture {
                        fixture_id: fixture_id(&key)?,
                        base_fixture_revision: base,
                    }
                } else {
                    ShowSyncOperation::DeleteObject {
                        kind: key.kind,
                        id: key.id,
                        base_object_revision: base,
                    }
                }
            }
            Self::RetainProfile {
                profile_id,
                revision,
                profile,
            } => ShowSyncOperation::RetainProfileRevision {
                profile_id,
                revision,
                profile,
            },
            Self::SetMetadata { key, base, value } => {
                ShowSyncOperation::SetMetadata { key, base, value }
            }
        })
    }
}

fn fixture_id(key: &ObjectKey) -> Answer<Uuid> {
    key.id
        .parse()
        .map_err(|_| format!("patched fixture identity {:?} is not a UUID", key.id))
}

/// The most operations the desk takes in one sync transaction is 1 024; stay below it.
pub const MAX_TRANSACTION_OPERATIONS: usize = 1_000;

/// One Architect gesture's change to the persistent show, ready for the journal.
#[derive(Clone, Debug, PartialEq)]
pub struct ShowEditIntent {
    operations: Vec<PendingOperation>,
}

/// What [`ShowEditIntent::capture`] needs to know about the desk's copy.
pub trait DeskKnowledge {
    /// The desk revision of an object, when the desk has numbered it.
    fn revision(&self, key: &ObjectKey) -> Option<u64>;
    /// Whether the desk already holds a profile revision.
    fn has_profile(&self, profile: (Uuid, u64)) -> bool;
}

impl DeskKnowledge for VersionedState {
    fn revision(&self, key: &ObjectKey) -> Option<u64> {
        self.revisions.get(key).copied()
    }

    fn has_profile(&self, profile: (Uuid, u64)) -> bool {
        self.profiles.contains(&profile)
    }
}

impl ShowEditIntent {
    /// The change from `before` to `after`, or `None` when the gesture changed nothing that is
    /// synchronized. `profile` reads a profile revision from the document, for fixtures patched
    /// with one the desk does not hold.
    pub fn capture(
        before: &SyncState,
        after: &SyncState,
        desk: &dyn DeskKnowledge,
        profile: &dyn Fn((Uuid, u64)) -> Option<Value>,
    ) -> Option<Self> {
        let mut operations = Vec::new();
        let mut profiles = BTreeSet::new();
        let keys: BTreeSet<&ObjectKey> =
            before.objects.keys().chain(after.objects.keys()).collect();
        for key in keys {
            match (before.objects.get(key), after.objects.get(key)) {
                (Some(old), Some(new)) if old != new => {
                    let mut fields = Vec::new();
                    field_edits(old, new, &mut String::new(), &mut fields);
                    if key.is_fixture() && fixture_profile(old) != fixture_profile(new) {
                        profiles.extend(fixture_profile(new));
                    }
                    operations.push(PendingOperation::UpdateObject {
                        key: key.clone(),
                        fields,
                    });
                }
                (None, Some(new)) => {
                    if key.is_fixture() {
                        profiles.extend(fixture_profile(new));
                    }
                    operations.push(PendingOperation::CreateObject {
                        key: key.clone(),
                        body: new.clone(),
                    });
                }
                (Some(_), None) => operations.push(PendingOperation::DeleteObject {
                    key: key.clone(),
                    base_revision: desk.revision(key),
                }),
                _ => {}
            }
        }
        let retained = profiles
            .into_iter()
            .filter(|key| !desk.has_profile(*key))
            .filter_map(|key| {
                profile(key).map(|profile| PendingOperation::RetainProfile {
                    profile_id: key.0,
                    revision: key.1,
                    profile,
                })
            })
            .collect::<Vec<_>>();
        let metadata_keys: BTreeSet<&String> = before
            .metadata
            .keys()
            .chain(after.metadata.keys())
            .collect();
        for key in metadata_keys {
            let (base, value) = (before.metadata.get(key), after.metadata.get(key));
            if base != value {
                operations.push(PendingOperation::SetMetadata {
                    key: key.clone(),
                    base: base.cloned(),
                    value: value.cloned(),
                });
            }
        }
        if operations.is_empty() {
            return None;
        }
        // Profiles travel first, so the desk resolves the fixtures that use them.
        Some(Self {
            operations: retained.into_iter().chain(operations).collect(),
        })
    }

    /// An intent re-stating `operations`: a conflict resolved as "use mine", or a journal entry
    /// read back. Crate-private, so nothing outside the sync engine can journal arbitrary edits.
    pub(crate) fn from_operations(operations: Vec<PendingOperation>) -> Option<Self> {
        (!operations.is_empty()).then_some(Self { operations })
    }

    /// The intent as transactions the desk accepts: at most [`MAX_TRANSACTION_OPERATIONS`]
    /// operations each, in order, so profiles still precede the fixtures that use them. Only a
    /// bulk gesture such as an MVR import is ever split.
    pub fn into_transactions(self) -> Vec<ShowEditIntent> {
        self.operations
            .chunks(MAX_TRANSACTION_OPERATIONS)
            .map(|operations| Self {
                operations: operations.to_vec(),
            })
            .collect()
    }

    pub fn operations(&self) -> &[PendingOperation] {
        &self.operations
    }

    pub(crate) fn into_operations(self) -> Vec<PendingOperation> {
        self.operations
    }
}

/// Field edits turning `old` into `new`, with `old` as each field's base.
pub fn field_edits_between(old: &Value, new: &Value, out: &mut Vec<ShowSyncFieldEdit>) {
    field_edits(old, new, &mut String::new(), out);
}

/// Field edits turning `old` into `new`: one per changed leaf, where objects are descended and
/// arrays and scalars are leaves, so two users editing different fields of one object never meet.
fn field_edits(old: &Value, new: &Value, path: &mut String, out: &mut Vec<ShowSyncFieldEdit>) {
    match (old, new) {
        (Value::Object(old), Value::Object(new)) => {
            let keys: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
            for key in keys {
                let (before, after) = (present(old.get(key)), present(new.get(key)));
                if before == after {
                    continue;
                }
                let length = path.len();
                path.push('/');
                path.push_str(&key.replace('~', "~0").replace('/', "~1"));
                match (before, after) {
                    (Some(before @ Value::Object(_)), Some(after @ Value::Object(_))) => {
                        field_edits(before, after, path, out);
                    }
                    _ => out.push(ShowSyncFieldEdit {
                        path: path.clone(),
                        base: before.cloned(),
                        value: after.cloned(),
                    }),
                }
                path.truncate(length);
            }
        }
        _ if old != new => out.push(ShowSyncFieldEdit {
            path: path.clone(),
            base: Some(old.clone()),
            value: Some(new.clone()),
        }),
        _ => {}
    }
}

fn present(value: Option<&Value>) -> Option<&Value> {
    value.filter(|value| !value.is_null())
}

/// The fields of one object that a conflict left as Control holds them.
pub type HeldFields = BTreeMap<ObjectKey, BTreeSet<String>>;

/// Applies `operations` to `state` without comparing bases: the optimistic view of the document
/// while they await the desk. Fields in `held` are skipped — the desk kept its value there.
pub fn overlay(state: &mut SyncState, operations: &[PendingOperation], held: &HeldFields) {
    for operation in operations {
        let skip_object = |key: &ObjectKey| held.get(key).is_some_and(|paths| paths.contains(""));
        match operation {
            PendingOperation::UpdateObject { key, fields } => {
                if skip_object(key) {
                    continue;
                }
                let Some(body) = state.objects.get_mut(key) else {
                    continue;
                };
                for field in fields {
                    if held
                        .get(key)
                        .is_some_and(|paths| paths.contains(&field.path))
                    {
                        continue;
                    }
                    set_field(body, &field.path, present(field.value.as_ref()).cloned());
                }
            }
            PendingOperation::CreateObject { key, body } => {
                if !skip_object(key) {
                    state.objects.insert(key.clone(), body.clone());
                }
            }
            PendingOperation::DeleteObject { key, .. } => {
                if !skip_object(key) {
                    state.objects.remove(key);
                }
            }
            PendingOperation::SetMetadata { key, value, .. } => {
                let metadata = ObjectKey::new("metadata", key.clone());
                if skip_object(&metadata) {
                    continue;
                }
                match value {
                    Some(value) if !value.is_empty() => {
                        state.metadata.insert(key.clone(), value.clone());
                    }
                    _ => {
                        state.metadata.remove(key);
                    }
                }
            }
            PendingOperation::RetainProfile { .. } => {}
        }
    }
}

/// Sets or removes one RFC 6901 field, creating absent object levels on the way.
pub fn set_field(body: &mut Value, path: &str, value: Option<Value>) {
    let tokens: Vec<String> = path
        .split('/')
        .skip(1)
        .map(|token| token.replace("~1", "/").replace("~0", "~"))
        .collect();
    let Some((last, parents)) = tokens.split_last() else {
        if let Some(value) = value {
            *body = value;
        }
        return;
    };
    let mut parent = body;
    for token in parents {
        if value.is_none() {
            // Removing below an absent level removes nothing; never create the level.
            let next = match parent {
                Value::Object(map) => map.get_mut(token),
                Value::Array(items) => token
                    .parse::<usize>()
                    .ok()
                    .and_then(move |index| items.get_mut(index)),
                _ => None,
            };
            let Some(next) = next else { return };
            parent = next;
            continue;
        }
        if !parent.is_object() {
            *parent = Value::Object(Map::new());
        }
        let Value::Object(map) = parent else { return };
        parent = map
            .entry(token.clone())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    match (parent, value) {
        (Value::Object(map), Some(value)) => {
            map.insert(last.clone(), value);
        }
        (Value::Object(map), None) => {
            map.remove(last);
        }
        (Value::Array(items), value) => {
            if let Ok(index) = last.parse::<usize>() {
                match value {
                    Some(value) if index < items.len() => items[index] = value,
                    Some(value) if index == items.len() => items.push(value),
                    None if index < items.len() => {
                        items.remove(index);
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

/// The metadata pseudo-kind conflicts and outcomes use.
pub const METADATA_KIND: &str = "metadata";

/// Whether `kind` is a patched fixture.
pub fn is_fixture_kind(kind: &str) -> bool {
    kind == PATCHED_FIXTURE_KIND
}

#[cfg(test)]
#[path = "intent_tests.rs"]
mod tests;
