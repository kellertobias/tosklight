use crate::{ApplicationCommand, CommandFamily};
use light_core::{FixtureId, ShowId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Object kinds a sync transaction may write directly. Everything else is desk-only — cue lists,
/// playbacks, presets, layouts, routes — and is refused, never silently dropped.
///
/// Patched fixtures are not listed: they travel as patch operations, so the patch capability
/// keeps owning their validation, profile resolution and runtime installation.
pub const SHOW_SYNC_OBJECT_KINDS: &[&str] = &[
    "patch_layer",
    "rig_attachment",
    "cad_annotation",
    "cad_drawing_tree",
    "cad_underlay",
    "cad_venue_groups",
    "fixture_note",
    "fixture_visibility",
    "media_server",
    "media_source",
    "media_surface",
    "media_projector",
    "media_fallback_asset",
    "visualizer_input",
    "venue",
];

/// Metadata key prefixes a sync transaction may write. The show name and identity are library
/// state and change only through the library.
pub const SHOW_SYNC_METADATA_PREFIXES: &[&str] = &["previs.", "architect."];

/// Object kind used for patched fixtures in sync outcomes and events.
pub const PATCHED_FIXTURE_KIND: &str = "patched_fixture";

/// The refusal of a request identity reused for different content.
pub const REQUEST_REUSED_MESSAGE: &str =
    "request_id was already used for a different sync transaction";

/// Pseudo-kind naming a metadata key in conflicts and outcomes.
pub const METADATA_KIND: &str = "metadata";

pub fn is_sync_object_kind(kind: &str) -> bool {
    SHOW_SYNC_OBJECT_KINDS.contains(&kind)
}

pub fn is_sync_metadata_key(key: &str) -> bool {
    SHOW_SYNC_METADATA_PREFIXES
        .iter()
        .any(|prefix| key.len() > prefix.len() && key.starts_with(prefix))
}

/// One atomic Architect gesture against the active show.
///
/// Fields carry the value the client last saw (`base`) next to the value it wants. Each field
/// commits independently: a field nobody else touched applies, a field another user changed is
/// reported as a conflict and left exactly as that user set it.
#[derive(Clone, Debug, PartialEq)]
pub struct ShowSyncCommand {
    pub show_id: ShowId,
    pub association_id: Uuid,
    pub request_id: String,
    /// Digest of the request content. A request identity reused with different content is
    /// refused instead of replayed.
    pub signature: String,
    pub operations: Vec<ShowSyncOperation>,
}

impl ApplicationCommand for ShowSyncCommand {
    type Value = ShowSyncResult;

    const FAMILY: CommandFamily = CommandFamily::Show;
}

#[derive(Clone, Debug, PartialEq)]
pub enum ShowSyncOperation {
    UpdateObject {
        kind: String,
        id: String,
        fields: Vec<ShowSyncFieldEdit>,
    },
    CreateObject {
        kind: String,
        id: String,
        body: Value,
    },
    DeleteObject {
        kind: String,
        id: String,
        base_object_revision: u64,
    },
    /// Field edits against the fixture's patch document, as the adapter projects it.
    UpdatePatchFixture {
        fixture_id: FixtureId,
        fields: Vec<ShowSyncFieldEdit>,
    },
    /// A new fixture, as the adapter's complete patch document.
    CreatePatchFixture {
        fixture_id: FixtureId,
        document: Value,
    },
    RemovePatchFixture {
        fixture_id: FixtureId,
        base_fixture_revision: u64,
    },
    /// An immutable profile revision the client's fixtures reference, retained in the show when
    /// Control has no copy of its own.
    RetainProfileRevision {
        profile_id: FixtureId,
        revision: u64,
        profile: Value,
    },
    SetMetadata {
        key: String,
        base: Option<String>,
        value: Option<String>,
    },
}

/// One field: a JSON pointer, the value the client saw, and the value it wants. `None` means the
/// field is absent; JSON `null` reads as absent too.
#[derive(Clone, Debug, PartialEq)]
pub struct ShowSyncFieldEdit {
    pub path: String,
    pub base: Option<Value>,
    pub value: Option<Value>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShowSyncConflictReason {
    /// Another user changed this field since the client's base.
    FieldChanged,
    /// The object was deleted while the client edited it.
    ObjectDeleted,
    /// The client deleted an object another user changed since.
    ObjectModified,
    /// The client created an identity that already holds different content.
    ObjectExists,
}

/// A field whose value is kept as Control has it, reported with both sides so the client can
/// keep its draft and offer a resolution.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ShowSyncConflict {
    pub kind: String,
    pub id: String,
    pub path: String,
    pub base: Option<Value>,
    pub mine: Option<Value>,
    pub theirs: Option<Value>,
    pub theirs_revision: Option<u64>,
    pub reason: ShowSyncConflictReason,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ShowSyncAppliedObject {
    pub kind: String,
    pub id: String,
    /// Revision after the commit; absent for a deletion.
    pub revision: Option<u64>,
    pub deleted: bool,
}

/// The outcome of a sync transaction, stored with its commit and returned again to every retry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ShowSyncResult {
    pub request_id: String,
    pub association_id: Uuid,
    pub show_id: Uuid,
    /// `true` when this answers a retry of an already-applied request.
    #[serde(default)]
    pub replayed: bool,
    pub show_revision: u64,
    pub patch_revision: u64,
    pub applied: Vec<ShowSyncAppliedObject>,
    /// Metadata keys this request wrote.
    pub metadata: Vec<String>,
    pub conflicts: Vec<ShowSyncConflict>,
    /// Sequence of the last event published for the commit; absent when nothing changed or when
    /// replayed after the desk restarted.
    #[serde(default)]
    pub event_sequence: Option<u64>,
}

impl ShowSyncResult {
    pub fn is_conflicted(&self) -> bool {
        !self.conflicts.is_empty()
    }
}
