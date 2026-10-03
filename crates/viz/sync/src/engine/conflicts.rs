//! Conflicts and refused drafts, and the operator's deliberate choice between the two versions.

use super::remote::on_document_blocking;
use super::{Answer, Shared};
use crate::intent::{METADATA_KIND, PendingOperation, ShowEditIntent};
use crate::journal::{EntryState, JournalEntry};
use crate::state::ObjectKey;
use light_wire::v2::show_sync::{ShowSyncConflict, ShowSyncConflictReason, ShowSyncFieldEdit};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// One decision the operator owes: a field both sides changed, or a change the desk refused.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictView {
    /// The journal entry; a resolution decides every conflict of one entry together.
    pub entry: i64,
    pub kind: String,
    pub id: String,
    /// The field, as a JSON pointer; empty for the whole object or a metadata value.
    pub path: String,
    /// What this computer had before either change.
    pub base: Option<Value>,
    /// The Architect's version: the recoverable draft.
    pub mine: Option<Value>,
    /// Control's version, which the document shows until the operator decides.
    pub theirs: Option<Value>,
    pub reason: ConflictReason,
    /// A short description of what conflicts, for the panel.
    pub label: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictReason {
    FieldChanged,
    ObjectDeleted,
    ObjectModified,
    ObjectExists,
    /// The desk refused the change outright.
    Refused,
    /// Found while rebuilding a damaged confirmed copy: either side may hold the newer version.
    Recovered,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// Control's version stays; the Architect's draft is kept in the journal as history.
    KeepControl,
    /// The Architect's version is sent again, this time over Control's current value.
    UseMine,
}

fn label(kind: &str, id: &str, path: &str) -> String {
    let what = match kind {
        METADATA_KIND => format!("Show detail {id}"),
        "patched_fixture" => format!("Fixture {}", short(id)),
        other => format!("{} {}", other.replace('_', " "), short(id)),
    };
    if path.is_empty() {
        what
    } else {
        format!(
            "{what} · {}",
            path.trim_start_matches('/').replace('/', " › ")
        )
    }
}

fn short(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

fn views(entry: &JournalEntry) -> Vec<ConflictView> {
    let outcome = entry.outcome.clone().unwrap_or_default();
    if entry.state == EntryState::Rejected {
        let keys: BTreeSet<&ObjectKey> = entry
            .operations
            .iter()
            .filter_map(PendingOperation::object)
            .collect();
        let error = outcome.error.unwrap_or_default();
        let mut list: Vec<ConflictView> = keys
            .into_iter()
            .map(|key| ConflictView {
                entry: entry.seq,
                kind: key.kind.clone(),
                id: key.id.clone(),
                path: String::new(),
                base: None,
                mine: None,
                theirs: None,
                reason: ConflictReason::Refused,
                label: format!("{} — {error}", label(&key.kind, &key.id, "")),
            })
            .collect();
        if list.is_empty() {
            list.push(ConflictView {
                entry: entry.seq,
                kind: METADATA_KIND.into(),
                id: String::new(),
                path: String::new(),
                base: None,
                mine: None,
                theirs: None,
                reason: ConflictReason::Refused,
                label: error,
            });
        }
        return list;
    }
    outcome
        .conflicts
        .iter()
        .map(|conflict| ConflictView {
            entry: entry.seq,
            kind: conflict.kind.clone(),
            id: conflict.id.clone(),
            path: conflict.path.clone(),
            base: conflict.base.clone(),
            mine: conflict.mine.clone(),
            theirs: conflict.theirs.clone(),
            reason: if outcome.recovered {
                ConflictReason::Recovered
            } else {
                match conflict.reason {
                    ShowSyncConflictReason::FieldChanged => ConflictReason::FieldChanged,
                    ShowSyncConflictReason::ObjectDeleted => ConflictReason::ObjectDeleted,
                    ShowSyncConflictReason::ObjectModified => ConflictReason::ObjectModified,
                    ShowSyncConflictReason::ObjectExists => ConflictReason::ObjectExists,
                }
            },
            label: label(&conflict.kind, &conflict.id, &conflict.path),
        })
        .collect()
}

/// The edits that restate the Architect's version of every conflict over Control's current one.
fn use_mine(
    conflicts: &[ShowSyncConflict],
    drafts: &BTreeMap<String, Value>,
) -> Vec<PendingOperation> {
    let mut fields: BTreeMap<ObjectKey, Vec<ShowSyncFieldEdit>> = BTreeMap::new();
    let mut whole = Vec::new();
    for conflict in conflicts {
        let key = ObjectKey::new(&conflict.kind, &conflict.id);
        if conflict.kind == METADATA_KIND {
            whole.push(PendingOperation::SetMetadata {
                key: conflict.id.clone(),
                base: conflict.theirs.as_ref().and_then(as_text),
                value: conflict.mine.as_ref().and_then(as_text),
            });
            continue;
        }
        if conflict.reason == ShowSyncConflictReason::ObjectDeleted {
            if let Some(body) = drafts.get(&format!("{}/{}", key.kind, key.id))
                && !whole
                    .iter()
                    .any(|operation| operation.object() == Some(&key))
            {
                whole.push(PendingOperation::CreateObject {
                    key,
                    body: body.clone(),
                });
            }
            continue;
        }
        if !conflict.path.is_empty() {
            fields.entry(key).or_default().push(ShowSyncFieldEdit {
                path: conflict.path.clone(),
                base: conflict.theirs.clone(),
                value: conflict.mine.clone(),
            });
            continue;
        }
        match (&conflict.mine, &conflict.theirs) {
            (Some(mine), Some(theirs)) => {
                let mut edits = Vec::new();
                crate::intent::field_edits_between(theirs, mine, &mut edits);
                fields.entry(key).or_default().extend(edits);
            }
            (Some(mine), None) => whole.push(PendingOperation::CreateObject {
                key,
                body: mine.clone(),
            }),
            (None, Some(_)) => whole.push(PendingOperation::DeleteObject {
                key,
                base_revision: conflict.theirs_revision,
            }),
            (None, None) => {}
        }
    }
    whole
        .into_iter()
        .chain(
            fields
                .into_iter()
                .filter(|(_, fields)| !fields.is_empty())
                .map(|(key, fields)| PendingOperation::UpdateObject { key, fields }),
        )
        .collect()
}

fn as_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

impl Shared {
    pub(crate) fn conflict_views(&self) -> Answer<Vec<ConflictView>> {
        Ok(self
            .journal
            .lock()
            .unresolved()?
            .iter()
            .filter(|entry| matches!(entry.state, EntryState::Conflict | EntryState::Rejected))
            .flat_map(views)
            .collect())
    }

    pub(crate) fn resolve(&self, seq: i64, resolution: Resolution) -> Answer<()> {
        let entry = self
            .journal
            .lock()
            .entry(seq)?
            .filter(|entry| matches!(entry.state, EntryState::Conflict | EntryState::Rejected))
            .ok_or("That conflict is already resolved")?;
        let restated = match (resolution, entry.state) {
            (Resolution::KeepControl, _) => None,
            (Resolution::UseMine, EntryState::Rejected) => {
                ShowEditIntent::from_operations(entry.operations.clone())
            }
            (Resolution::UseMine, _) => {
                let outcome = entry.outcome.clone().unwrap_or_default();
                ShowEditIntent::from_operations(use_mine(&outcome.conflicts, &outcome.drafts))
            }
        };
        let keys: BTreeSet<ObjectKey> = entry
            .operations
            .iter()
            .chain(restated.iter().flat_map(ShowEditIntent::operations))
            .filter_map(PendingOperation::object)
            .cloned()
            .chain(entry.outcome.iter().flat_map(|outcome| {
                outcome
                    .conflicts
                    .iter()
                    .map(|conflict| ObjectKey::new(&conflict.kind, &conflict.id))
            }))
            .collect();
        on_document_blocking(self, &mut |shared, document| {
            let journal = shared.journal.lock();
            if let Some(intent) = restated.clone() {
                let base = shared.mirror.lock().show_revision();
                for transaction in intent.into_transactions() {
                    journal.append(transaction, base)?;
                }
            }
            journal.set_state(seq, EntryState::Superseded)?;
            drop(journal);
            shared.rebase(document, Some(&keys), &BTreeMap::new())
        })
    }
}
