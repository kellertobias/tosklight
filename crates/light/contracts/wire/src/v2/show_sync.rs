//! Control ↔ Architect automatic show synchronization.
//!
//! `POST /api/v2/show-sync/transactions` carries one Architect gesture as one atomic transaction.
//! Each field names the value the client last saw (`base`) and the value it wants; the server
//! applies untouched fields and reports fields another user changed as conflicts, keeping their
//! value. The committed changes of every show commit — from either application — are published
//! on the event socket as `show_sync_committed`, or as `show_sync_gap` when a commit is too large
//! or bypassed the incremental path and the client must re-read snapshots.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::patch::PatchFixtureInput;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowSyncTransactionRequest {
    /// Client-generated identity, kept by the client's journal across retries and restarts.
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    /// The Architect binding this transaction belongs to. Request identities are scoped to it.
    pub association_id: Uuid,
    /// The show the binding names. A transaction for any other show is refused, never applied.
    pub show_id: Uuid,
    /// The show revision the client's mirror held when it built the transaction. Informational:
    /// compare-and-set is per field, not per show.
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub base_show_revision: u64,
    pub origin: ShowSyncOrigin,
    #[schemars(length(min = 1, max = 1024))]
    pub operations: Vec<ShowSyncOperation>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowSyncOrigin {
    pub app: ShowSyncApp,
    /// The desk installation the client believes it is bound to. A mismatch is refused, so a
    /// binding never writes into a different desk that answered on the same address.
    #[serde(default)]
    pub desk_identity: Option<Uuid>,
    /// Free-form identity of the client process, for diagnostics.
    #[serde(default)]
    pub client_instance: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ShowSyncApp {
    Architect,
    Control,
}

/// One edit inside a sync transaction.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ShowSyncOperation {
    /// Field edits against a synchronized object's JSON body.
    UpdateObject {
        kind: String,
        id: String,
        fields: Vec<ShowSyncFieldEdit>,
    },
    /// A new object under a client-minted identity. Re-creating identical content is a no-op.
    CreateObject {
        kind: String,
        id: String,
        #[ts(type = "Record<string, unknown>")]
        body: serde_json::Value,
    },
    /// Deletes an object unless it changed since the client's base revision.
    DeleteObject {
        kind: String,
        id: String,
        #[ts(type = "number")]
        base_object_revision: u64,
    },
    /// Field edits against the fixture's `PatchFixtureInput` document.
    UpdatePatchFixture {
        fixture_id: Uuid,
        fields: Vec<ShowSyncFieldEdit>,
    },
    /// A new patched fixture under a client-minted identity.
    CreatePatchFixture { fixture: Box<PatchFixtureInput> },
    /// Removes a patched fixture unless it changed since the client's base revision.
    RemovePatchFixture {
        fixture_id: Uuid,
        #[ts(type = "number")]
        base_fixture_revision: u64,
    },
    /// An immutable fixture profile revision referenced by this transaction's fixtures, retained
    /// in the show when the desk's library has no copy.
    RetainProfileRevision {
        profile_id: Uuid,
        #[ts(type = "number")]
        revision: u64,
        #[ts(type = "Record<string, unknown>")]
        profile: serde_json::Value,
    },
    /// One `previs.*` or `architect.*` metadata value. `None` removes it.
    SetMetadata {
        key: String,
        #[serde(default)]
        base: Option<String>,
        #[serde(default)]
        value: Option<String>,
    },
}

/// One field of an object body, addressed by an RFC 6901 JSON pointer. `null` and an absent
/// field are the same state; arrays are best edited as one field holding the whole array.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowSyncFieldEdit {
    pub path: String,
    #[serde(default)]
    #[ts(type = "unknown")]
    pub base: Option<serde_json::Value>,
    #[serde(default)]
    #[ts(type = "unknown")]
    pub value: Option<serde_json::Value>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ShowSyncStatus {
    /// Every edit applied or already held its value.
    Accepted,
    /// At least one field conflicted; every other field applied.
    Conflicted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ShowSyncConflictReason {
    FieldChanged,
    ObjectDeleted,
    ObjectModified,
    ObjectExists,
}

/// A field left as Control holds it, with both sides so the client keeps a recoverable draft.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowSyncConflict {
    /// Object kind, `patched_fixture`, or `metadata`.
    pub kind: String,
    pub id: String,
    pub path: String,
    #[ts(type = "unknown")]
    pub base: Option<serde_json::Value>,
    #[ts(type = "unknown")]
    pub mine: Option<serde_json::Value>,
    #[ts(type = "unknown")]
    pub theirs: Option<serde_json::Value>,
    #[ts(as = "Option<f64>")]
    pub theirs_revision: Option<u64>,
    pub reason: ShowSyncConflictReason,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowSyncAppliedObject {
    pub kind: String,
    pub id: String,
    #[ts(as = "Option<f64>")]
    pub revision: Option<u64>,
    pub deleted: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowSyncTransactionOutcome {
    pub request_id: String,
    pub association_id: Uuid,
    pub show_id: Uuid,
    pub status: ShowSyncStatus,
    /// `true` when this answers a retry of a request the show already applied.
    pub replayed: bool,
    #[ts(type = "number")]
    pub show_revision: u64,
    #[ts(type = "number")]
    pub patch_revision: u64,
    pub applied: Vec<ShowSyncAppliedObject>,
    pub metadata: Vec<String>,
    pub conflicts: Vec<ShowSyncConflict>,
    #[ts(as = "Option<f64>")]
    pub event_sequence: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ShowSyncErrorKind {
    /// The named show is not the desk's active show. The client holds its edits Offline.
    ShowNotActive,
    /// The client is bound to a different desk installation.
    DeskMismatch,
    /// A request identity was reused for different content.
    RequestReused,
    Invalid,
    /// The show store is busy or unavailable; retry the same request.
    Unavailable,
    Internal,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowSyncErrorResponse {
    pub error: String,
    pub kind: ShowSyncErrorKind,
    pub active_show_id: Option<Uuid>,
    pub retryable: bool,
}

/// The committed state of one show commit, published for every accepted commit of the active
/// show, from any Control path or a sync transaction.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowSyncCommit {
    pub show_id: Uuid,
    /// The revision this commit produced: the client's durable reconnect cursor.
    #[ts(type = "number")]
    pub show_revision: u64,
    /// The revision before it. A client whose mirror is not here missed a change and must re-read.
    #[ts(type = "number")]
    pub previous_show_revision: u64,
    #[ts(type = "number")]
    pub patch_revision: u64,
    /// Present when a sync transaction produced the commit, so its sender recognises the echo.
    pub request_id: Option<String>,
    pub association_id: Option<Uuid>,
    /// Synchronized objects written or deleted.
    pub objects: Vec<ShowSyncCommittedObject>,
    pub metadata: Vec<ShowSyncMetadataChange>,
    pub profile_revisions: Vec<ShowSyncProfileReference>,
    /// Desk-only objects the commit also changed. They are not mirrored and carry no bodies.
    pub desk_only_changes: u32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowSyncCommittedObject {
    pub kind: String,
    pub id: String,
    #[ts(as = "Option<f64>")]
    pub revision: Option<u64>,
    pub deleted: bool,
    /// The committed body, omitted when larger than the per-object event budget.
    #[ts(type = "unknown")]
    pub body: Option<serde_json::Value>,
    /// `true` when `body` was withheld for size; read it with `GET /api/v2/objects/{kind}/{id}`.
    pub body_omitted: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowSyncMetadataChange {
    pub key: String,
    pub value: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowSyncProfileReference {
    pub profile_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    pub content_digest: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ShowSyncGapReason {
    /// One commit changed more than an event may carry.
    BulkCommit,
    /// The show changed outside the incremental commit path.
    OutOfBandWrite,
    /// The show file was replaced or reloaded as a whole.
    ShowReplaced,
}

/// The show moved to `show_revision` in a way no `show_sync_committed` describes. A client
/// re-reads its synchronized kinds from snapshots before trusting its mirror again.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowSyncGap {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub reason: ShowSyncGapReason,
}
