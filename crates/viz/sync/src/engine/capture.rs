//! Turning a finished gesture into one journal entry.

use super::{Answer, Shared};
use crate::intent::{DeskKnowledge, ShowEditIntent};
use crate::state::{ObjectKey, VersionedState};
use light_core::FixtureId;
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;
use viz_document::PlanningDocument;

/// What the desk is known to hold: the confirmed mirror, plus revisions the desk reported for
/// commits the mirror has not caught up with yet.
pub(crate) struct Knowledge<'a> {
    pub mirror: &'a VersionedState,
    pub known: &'a BTreeMap<ObjectKey, (u64, u64)>,
}

impl DeskKnowledge for Knowledge<'_> {
    fn revision(&self, key: &ObjectKey) -> Option<u64> {
        // Object counters may restart after delete/recreate. Show revisions order the sources:
        // an acknowledgement is useful only until its commit has reached the confirmed mirror.
        self.known
            .get(key)
            .filter(|(_, show_revision)| *show_revision > self.mirror.show_revision)
            .map(|(revision, _)| *revision)
            .or_else(|| self.mirror.revisions.get(key).copied())
    }

    fn has_profile(&self, profile: (Uuid, u64)) -> bool {
        self.mirror.profiles.contains(&profile)
    }
}

/// The stored profile revision a document holds, as the desk retains it.
pub(crate) fn document_profile(document: &PlanningDocument, key: (Uuid, u64)) -> Option<Value> {
    document
        .fixture_profile_revisions_for(FixtureId(key.0))
        .ok()?
        .into_iter()
        .find(|revision| revision.id().revision() == key.1)
        .map(|revision| revision.profile().clone())
}

impl Shared {
    pub(crate) fn capture(&self, document: &PlanningDocument) -> Answer<()> {
        let mut shadow = self.shadow.lock();
        let after = shadow.reader.read(document)?;
        let intent = {
            let mirror = self.mirror.lock();
            let known = self.known_revisions.lock();
            let knowledge = Knowledge {
                mirror: mirror.state(),
                known: &known,
            };
            ShowEditIntent::capture(&shadow.versioned.state, &after.state, &knowledge, &|key| {
                document_profile(document, key)
            })
        };
        if let Some(intent) = intent {
            let base = self.mirror.lock().show_revision();
            let journal = self.journal.lock();
            for transaction in intent.into_transactions() {
                journal.append(transaction, base)?;
            }
        }
        shadow.versioned = after;
        Ok(())
    }

    /// Journals any difference between the document and the confirmed mirror plus the journal:
    /// an edit whose gesture finished in the document but whose journal entry was lost to a crash
    /// or a damaged journal.
    pub(crate) fn recover_unjournaled(&self, document: &PlanningDocument) -> Answer<()> {
        let shadow = self.shadow.lock();
        let expected = {
            let journal = self.journal.lock();
            let mirror = self.mirror.lock();
            let entries = journal.unresolved()?;
            super::rebase::expected_state(mirror.state(), &entries, None)
        };
        let intent = {
            let mirror = self.mirror.lock();
            let known = self.known_revisions.lock();
            let knowledge = Knowledge {
                mirror: mirror.state(),
                known: &known,
            };
            ShowEditIntent::capture(&expected, &shadow.versioned.state, &knowledge, &|key| {
                document_profile(document, key)
            })
        };
        if let Some(intent) = intent {
            let base = self.mirror.lock().show_revision();
            let journal = self.journal.lock();
            for transaction in intent.into_transactions() {
                journal.append(transaction, base)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod revision_tests {
    use super::*;
    use crate::intent::PendingOperation;
    use crate::state::SyncState;
    use serde_json::json;

    #[test]
    fn fresh_delete_after_remote_update_uses_newer_mirror_revision() {
        let key = ObjectKey::new("patched_fixture", "fixture");
        let mut mirror = VersionedState::default();
        mirror.revisions.insert(key.clone(), 6);
        mirror.show_revision = 811;
        let known = BTreeMap::from([(key.clone(), (5, 810))]);
        let before = SyncState {
            objects: BTreeMap::from([(key.clone(), json!({"name":"remote"}))]),
            ..SyncState::default()
        };
        let intent = ShowEditIntent::capture(
            &before,
            &SyncState::default(),
            &Knowledge {
                mirror: &mirror,
                known: &known,
            },
            &|_| None,
        )
        .unwrap();
        assert_eq!(
            intent.operations(),
            &[PendingOperation::DeleteObject {
                key,
                base_revision: Some(6)
            }]
        );
    }
}

#[cfg(test)]
mod acknowledgement_tests {
    use super::*;
    #[test]
    fn acknowledgement_only_precedes_its_own_mirror_commit() {
        let key = ObjectKey::new("cad_annotation", "recreated");
        let mut mirror = VersionedState::default();
        mirror.show_revision = 10;
        mirror.revisions.insert(key.clone(), 1);
        let known = BTreeMap::from([(key.clone(), (7, 9))]);
        assert_eq!(
            Knowledge {
                mirror: &mirror,
                known: &known
            }
            .revision(&key),
            Some(1)
        );
        let known = BTreeMap::from([(key.clone(), (2, 11))]);
        assert_eq!(
            Knowledge {
                mirror: &mirror,
                known: &known
            }
            .revision(&key),
            Some(2)
        );
        mirror.show_revision = 11;
        mirror.revisions.insert(key.clone(), 2);
        assert_eq!(
            Knowledge {
                mirror: &mirror,
                known: &known
            }
            .revision(&key),
            Some(2)
        );
        mirror.show_revision = 12;
        mirror.revisions.remove(&key);
        assert_eq!(
            Knowledge {
                mirror: &mirror,
                known: &known
            }
            .revision(&key),
            None
        );
    }
}
