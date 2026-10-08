//! The committed-change feed a bound Architect follows.
//!
//! Every accepted commit of the active show becomes one `ShowSyncCommitted` — whichever desk
//! path, sync transaction or import produced it — carrying the synchronized objects it wrote with
//! bounded bodies. A commit too large for one event, or a change that bypassed the incremental
//! commit path, becomes a `ShowSyncGap` instead: the client re-reads snapshots. The show revision
//! is the durable cursor, because event sequences restart with the desk.

use super::{PATCHED_FIXTURE_KIND, is_sync_metadata_key, is_sync_object_kind};
use crate::{
    ApplicationEvent, DeliveryPolicy, EventCapability, EventClass, EventDraft, EventObject,
    EventSource, ShowEvent,
};
use light_core::ShowId;
use light_show::{PortableShowCommit, PortableShowRevision};
use serde_json::Value;
use uuid::Uuid;

/// Largest body carried inline; a larger one is announced and read by snapshot.
pub const SHOW_SYNC_EVENT_BODY_BYTES: usize = 32 * 1024;
/// Most synchronized objects one event describes before the commit becomes a gap.
pub const SHOW_SYNC_EVENT_OBJECTS: usize = 256;
/// Most body bytes one event carries; later bodies are announced without content.
pub const SHOW_SYNC_EVENT_BYTES: usize = 256 * 1024;
/// Most announced-but-withheld bodies before a commit is cheaper to repair as a gap.
pub const SHOW_SYNC_EVENT_OMITTED_BODIES: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub struct ShowSyncCommittedObject {
    pub kind: String,
    pub id: String,
    pub revision: Option<u64>,
    pub deleted: bool,
    pub body: Option<Value>,
    pub body_omitted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShowSyncProfileReference {
    pub profile_id: Uuid,
    pub revision: u64,
    pub content_digest: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShowSyncCommitChange {
    pub show_id: ShowId,
    pub show_revision: u64,
    pub previous_show_revision: u64,
    pub patch_revision: u64,
    pub request_id: Option<String>,
    pub association_id: Option<Uuid>,
    pub objects: Vec<ShowSyncCommittedObject>,
    pub metadata: Vec<(String, Option<String>)>,
    pub profile_revisions: Vec<ShowSyncProfileReference>,
    pub desk_only_changes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShowSyncGapReason {
    BulkCommit,
    OutOfBandWrite,
    ShowReplaced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShowSyncGapChange {
    pub show_id: ShowId,
    pub show_revision: u64,
    pub reason: ShowSyncGapReason,
}

/// What one committed transaction publishes on the sync feed.
#[derive(Clone, Debug, PartialEq)]
pub enum ShowSyncPublication {
    Committed(Box<ShowSyncCommitChange>),
    Gap(ShowSyncGapChange),
}

impl ShowSyncPublication {
    /// Describes one commit, within the event budget, or as a gap when it exceeds it.
    pub fn for_commit(
        show_id: ShowId,
        previous: PortableShowRevision,
        commit: &PortableShowCommit,
    ) -> Self {
        let mirrored = commit
            .written_objects()
            .iter()
            .map(|object| object.key().kind())
            .chain(commit.deleted_objects().iter().map(|key| key.kind()))
            .filter(|kind| is_mirrored_kind(kind))
            .count();
        if mirrored > SHOW_SYNC_EVENT_OBJECTS {
            return Self::bulk(show_id, commit);
        }
        let mut budget = SHOW_SYNC_EVENT_BYTES;
        let mut objects = Vec::new();
        let mut desk_only_changes = 0_u32;
        for object in commit.written_objects() {
            let kind = object.key().kind();
            if !is_mirrored_kind(kind) {
                desk_only_changes = desk_only_changes.saturating_add(1);
                continue;
            }
            let size = serde_json::to_vec(object.body()).map_or(usize::MAX, |bytes| bytes.len());
            let inline = size <= SHOW_SYNC_EVENT_BODY_BYTES && size <= budget;
            if inline {
                budget -= size;
            }
            objects.push(ShowSyncCommittedObject {
                kind: kind.to_owned(),
                id: object.key().id().to_owned(),
                revision: Some(object.revision()),
                deleted: false,
                body: inline.then(|| object.body().clone()),
                body_omitted: !inline,
            });
        }
        for key in commit.deleted_objects() {
            if !is_mirrored_kind(key.kind()) {
                desk_only_changes = desk_only_changes.saturating_add(1);
                continue;
            }
            objects.push(ShowSyncCommittedObject {
                kind: key.kind().to_owned(),
                id: key.id().to_owned(),
                revision: None,
                deleted: true,
                body: None,
                body_omitted: false,
            });
        }
        let omitted = objects.iter().filter(|object| object.body_omitted).count();
        if omitted > SHOW_SYNC_EVENT_OMITTED_BODIES {
            return Self::bulk(show_id, commit);
        }
        Self::Committed(Box::new(ShowSyncCommitChange {
            show_id,
            show_revision: commit.revision().value(),
            previous_show_revision: previous.value(),
            patch_revision: commit.patch_revision().value(),
            request_id: commit.sync_request().map(|(_, request)| request.to_owned()),
            association_id: commit.sync_request().map(|(association, _)| association),
            objects,
            metadata: commit
                .metadata_changes()
                .iter()
                .filter(|(key, _)| is_sync_metadata_key(key))
                .cloned()
                .collect(),
            profile_revisions: commit
                .fixture_profile_revisions()
                .iter()
                .map(|profile| ShowSyncProfileReference {
                    profile_id: profile.id().profile_id().0,
                    revision: profile.id().revision(),
                    content_digest: profile.digest().as_str().to_owned(),
                })
                .collect(),
            desk_only_changes,
        }))
    }

    fn bulk(show_id: ShowId, commit: &PortableShowCommit) -> Self {
        Self::Gap(ShowSyncGapChange {
            show_id,
            show_revision: commit.revision().value(),
            reason: ShowSyncGapReason::BulkCommit,
        })
    }

    pub fn show_revision(&self) -> u64 {
        match self {
            Self::Committed(change) => change.show_revision,
            Self::Gap(gap) => gap.show_revision,
        }
    }
}

/// Kinds a bound Architect mirrors: the synchronized object kinds and patched fixtures.
pub fn is_mirrored_kind(kind: &str) -> bool {
    kind == PATCHED_FIXTURE_KIND || is_sync_object_kind(kind)
}

fn sync_route(show_id: ShowId) -> EventObject {
    EventObject::new(EventCapability::Show, format!("show-sync:{}", show_id.0))
}

impl EventDraft {
    pub fn show_sync_published(publication: ShowSyncPublication) -> Self {
        let (show_id, payload) = match publication {
            ShowSyncPublication::Committed(change) => {
                (change.show_id, ShowEvent::SyncCommitted(change))
            }
            ShowSyncPublication::Gap(gap) => (gap.show_id, ShowEvent::SyncGap(gap)),
        };
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(sync_route(show_id)),
            related_objects: Vec::new(),
            source: EventSource::Runtime,
            correlation_id: None,
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Show(payload),
        }
    }
}
