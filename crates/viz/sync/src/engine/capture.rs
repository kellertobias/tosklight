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
    pub known: &'a BTreeMap<ObjectKey, u64>,
}

impl DeskKnowledge for Knowledge<'_> {
    fn revision(&self, key: &ObjectKey) -> Option<u64> {
        self.known
            .get(key)
            .copied()
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
