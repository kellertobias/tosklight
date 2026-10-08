//! The desk's commits and snapshots, applied to the mirror and then to the working document.

use super::rebase::{MISSING_PROFILE, entry_keys, expected_state};
use super::{Answer, Shared};
use crate::journal::{EntryOutcome, EntryState};
use crate::mirror::MirrorChange;
use crate::state::{DocumentReader, ObjectKey, ProfileKey, VersionedState, fixture_entry};
use light_application::show_sync::PATCHED_FIXTURE_KIND;
use light_wire::v2::show_sync::{ShowSyncCommit, ShowSyncConflict, ShowSyncConflictReason};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use viz_document::PlanningDocument;

/// What applying one desk commit found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommitResult {
    Applied,
    /// The mirror already holds this commit.
    AlreadyHeld,
    /// The mirror missed a commit, or the commit needs content only a snapshot carries.
    NeedsSnapshot,
}

/// A desk snapshot read from the show file it downloaded.
pub(crate) struct Snapshot {
    pub versioned: VersionedState,
    pub profiles: BTreeMap<ProfileKey, Value>,
}

impl Snapshot {
    pub(crate) fn read(path: &std::path::Path) -> Answer<Self> {
        let document = PlanningDocument::open(path).map_err(|e| e.to_string())?;
        let versioned = DocumentReader::default().read(&document)?;
        let profiles = versioned
            .profiles
            .iter()
            .filter_map(|key| {
                super::capture::document_profile(&document, *key).map(|profile| (*key, profile))
            })
            .collect();
        Ok(Self {
            versioned,
            profiles,
        })
    }
}

/// The mirror changes one commit describes. Omitted bodies come from `fetched`.
fn mirror_changes(
    change: &ShowSyncCommit,
    fetched: &BTreeMap<(String, String), (u64, Value)>,
) -> Answer<Option<Vec<MirrorChange>>> {
    let mut changes = Vec::new();
    for object in &change.objects {
        if object.deleted {
            changes.push(MirrorChange::Removed(ObjectKey::new(
                &object.kind,
                &object.id,
            )));
            continue;
        }
        let (revision, body) = match (&object.body, object.revision) {
            (Some(body), Some(revision)) => (revision, body.clone()),
            _ => match fetched.get(&(object.kind.clone(), object.id.clone())) {
                Some((revision, body)) => (*revision, body.clone()),
                None => return Ok(None),
            },
        };
        let (key, body) = if object.kind == PATCHED_FIXTURE_KIND {
            fixture_entry(body, revision)?
        } else {
            (ObjectKey::new(&object.kind, &object.id), body)
        };
        changes.push(MirrorChange::Object {
            key,
            revision,
            body,
        });
    }
    changes.extend(
        change
            .metadata
            .iter()
            .map(|metadata| MirrorChange::Metadata {
                key: metadata.key.clone(),
                value: metadata.value.clone(),
            }),
    );
    changes.extend(
        change
            .profile_revisions
            .iter()
            .map(|profile| MirrorChange::Profile((profile.profile_id, profile.revision))),
    );
    Ok(Some(changes))
}

impl Shared {
    /// Applies one desk commit to the mirror and the affected objects of the document.
    pub(crate) fn apply_commit(
        &self,
        document: &PlanningDocument,
        change: &ShowSyncCommit,
        fetched: &BTreeMap<(String, String), (u64, Value)>,
    ) -> Answer<(CommitResult, bool)> {
        let keys = {
            let mut mirror = self.mirror.lock();
            if change.show_revision <= mirror.show_revision() {
                return Ok((CommitResult::AlreadyHeld, false));
            }
            if change.previous_show_revision != mirror.show_revision() || !mirror.trusted() {
                return Ok((CommitResult::NeedsSnapshot, false));
            }
            let Some(changes) = mirror_changes(change, fetched)? else {
                return Ok((CommitResult::NeedsSnapshot, false));
            };
            let keys: BTreeSet<ObjectKey> = changes
                .iter()
                .filter_map(|change| match change {
                    MirrorChange::Object { key, .. } | MirrorChange::Removed(key) => {
                        Some(key.clone())
                    }
                    _ => None,
                })
                .collect();
            mirror.apply(change.show_revision, &changes)?;
            keys
        };
        if self.is_own_echo(change)? {
            // The desk committed exactly what this Architect sent, so the document already holds
            // it: the mirror advances and the document is left alone. (Rebuilding would find
            // nothing to write either — the edit is overlaid from the journal or now mirrored.)
            return Ok((CommitResult::Applied, false));
        }
        match self.rebase(document, Some(&keys), &BTreeMap::new()) {
            Ok(changed) => Ok((CommitResult::Applied, changed)),
            Err(error) if error == MISSING_PROFILE => Ok((CommitResult::NeedsSnapshot, false)),
            Err(error) => Err(error),
        }
    }

    /// Whether `change` is the desk's commit of one of this association's own journal entries.
    fn is_own_echo(&self, change: &ShowSyncCommit) -> Answer<bool> {
        let association = self.binding.lock().association_id;
        match (&change.request_id, change.association_id) {
            (Some(request), Some(sender)) if sender == association => {
                self.journal.lock().holds(request)
            }
            _ => Ok(false),
        }
    }

    /// Adopts a fresh desk snapshot as the mirror and rebuilds the document from it.
    pub(crate) fn adopt_snapshot(
        &self,
        document: &PlanningDocument,
        snapshot: &Snapshot,
    ) -> Answer<bool> {
        if self
            .rebuild_mirror
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            self.recover_drafts(snapshot)?;
        }
        self.mirror.lock().replace(snapshot.versioned.clone())?;
        self.known_revisions.lock().clear();
        {
            let mut binding = self.binding.lock();
            binding.acknowledged_show_revision = snapshot.versioned.show_revision;
            self.store.update(&binding)?;
        }
        self.rebase(document, None, &snapshot.profiles)
    }

    /// With no trustworthy mirror, a difference between the document and the desk cannot be told
    /// apart from an edit on either side. Each one becomes a recoverable conflict: the document
    /// shows the desk's version and the Architect's version is kept as the draft.
    fn recover_drafts(&self, snapshot: &Snapshot) -> Answer<()> {
        let working = self.shadow.lock().versioned.state.clone();
        let journal = self.journal.lock();
        let entries = journal.unresolved()?;
        let expected = expected_state(&snapshot.versioned, &entries, None);
        let mut conflicts = Vec::new();
        let mut drafts = BTreeMap::new();
        let keys: BTreeSet<&ObjectKey> = working
            .objects
            .keys()
            .chain(expected.objects.keys())
            .collect();
        for key in keys {
            let (mine, theirs) = (working.objects.get(key), expected.objects.get(key));
            if mine == theirs {
                continue;
            }
            if let Some(mine) = mine {
                drafts.insert(format!("{}/{}", key.kind, key.id), mine.clone());
            }
            conflicts.push(ShowSyncConflict {
                kind: key.kind.clone(),
                id: key.id.clone(),
                path: String::new(),
                base: None,
                mine: mine.cloned(),
                theirs: theirs.cloned(),
                theirs_revision: snapshot.versioned.revisions.get(key).copied(),
                reason: ShowSyncConflictReason::ObjectModified,
            });
        }
        let metadata: BTreeSet<&String> = working
            .metadata
            .keys()
            .chain(expected.metadata.keys())
            .collect();
        for key in metadata {
            let (mine, theirs) = (working.metadata.get(key), expected.metadata.get(key));
            if mine != theirs {
                conflicts.push(ShowSyncConflict {
                    kind: crate::intent::METADATA_KIND.into(),
                    id: key.clone(),
                    path: String::new(),
                    base: None,
                    mine: mine.map(|value| Value::String(value.clone())),
                    theirs: theirs.map(|value| Value::String(value.clone())),
                    theirs_revision: None,
                    reason: ShowSyncConflictReason::FieldChanged,
                });
            }
        }
        if conflicts.is_empty() {
            return Ok(());
        }
        let entry = journal.append_recovered(snapshot.versioned.show_revision)?;
        journal.record(
            entry,
            EntryState::Conflict,
            &EntryOutcome {
                show_revision: snapshot.versioned.show_revision,
                conflicts,
                drafts,
                error: None,
                recovered: true,
            },
        )
    }

    /// Objects an entry touched, for rebuilding after its outcome.
    pub(crate) fn keys_of(&self, seq: i64) -> Answer<BTreeSet<ObjectKey>> {
        Ok(self
            .journal
            .lock()
            .entry(seq)?
            .map(|entry| entry_keys(std::slice::from_ref(&entry)))
            .unwrap_or_default())
    }
}

/// Runs `edit` on the host's document from the async task.
pub(crate) async fn on_document<T: Send + 'static>(
    shared: &Arc<Shared>,
    edit: impl FnMut(&Shared, &PlanningDocument) -> Answer<(T, bool)> + Send + 'static,
) -> Answer<T> {
    let shared = shared.clone();
    tokio::task::spawn_blocking(move || {
        let mut edit = edit;
        let mut answer = None;
        let host = shared.host.clone();
        host.apply_remote(&mut |document| {
            let (value, changed) = edit(&shared, document)?;
            answer = Some(value);
            Ok(changed)
        })?;
        answer.ok_or_else(|| "the bound document is not open".to_owned())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Runs `edit` on the host's document from the caller's thread, outside any gesture.
pub(crate) fn on_document_blocking(
    shared: &Shared,
    edit: &mut dyn FnMut(&Shared, &PlanningDocument) -> Answer<bool>,
) -> Answer<()> {
    let host = shared.host.clone();
    host.apply_remote(&mut |document| edit(shared, document))
}
