use crate::{ActionContext, ApplicationCommand, CommandFamily};
use light_core::{FixtureId, Revision, ShowId};
use light_fixture::{
    FixtureSplit, PatchPolicy, PatchedFixturePatch, PatchedFixtureProfileReference,
};
use light_show::{PortablePatchRevision, PortableShowRevision};
use uuid::Uuid;

/// One requested fixture mutation containing only patch-owned state and an immutable profile
/// reference. Fixture definitions and catalog entries never cross this application boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchFixtureCandidate {
    pub profile: PatchedFixtureProfileReference,
    pub patch: PatchedFixturePatch,
}

/// Atomic, non-empty candidate batch for one active show.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchFixturesCommand {
    pub show_id: ShowId,
    pub fixtures: Vec<PatchFixtureCandidate>,
    /// Stable fixture identities removed by the same atomic patch transaction. Already-absent
    /// identities are ignored so retries and convergent desired-state updates remain idempotent.
    pub remove_fixture_ids: Vec<FixtureId>,
}

impl ApplicationCommand for PatchFixturesCommand {
    type Value = PatchFixturesResult;

    const FAMILY: CommandFamily = CommandFamily::Show;
}

/// Authoritative patch projection for one fixture, without an inline profile definition.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchFixtureProjection {
    pub fixture_revision: Revision,
    pub profile: PatchedFixtureProfileReference,
    pub patch: PatchedFixturePatch,
}

/// Selected-mode metadata needed by patch views without exposing the complete fixture catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchModeProjection {
    pub mode_id: Uuid,
    pub name: String,
    pub splits: Vec<FixtureSplit>,
}

/// Deduplicated metadata for one immutable profile revision referenced by a patch projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchProfileRevisionProjection {
    pub profile_id: FixtureId,
    pub profile_revision: Revision,
    pub content_digest: String,
    pub manufacturer: String,
    pub name: String,
    pub fixture_type: String,
    pub patch_policy: PatchPolicy,
    pub referenced_modes: Vec<PatchModeProjection>,
}

/// One committed semantic patch change. The event envelope owns its monotonic sequence.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchChange {
    pub show_id: ShowId,
    pub show_revision: PortableShowRevision,
    pub patch_revision: PortablePatchRevision,
    pub fixtures: Vec<PatchFixtureProjection>,
    pub removed_fixture_ids: Vec<FixtureId>,
    pub profile_revisions: Vec<PatchProfileRevisionProjection>,
}

/// Idempotent application result returned by both the first commit and exact retries.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchFixturesResult {
    pub context: ActionContext,
    pub request_id: String,
    pub replayed: bool,
    pub changed: bool,
    pub change: PatchChange,
    pub event_sequence: Option<u64>,
}

/// Authoritative patch snapshot paired with the event cursor captured under the show lock.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchSnapshot {
    pub show_id: ShowId,
    pub show_revision: PortableShowRevision,
    pub patch_revision: PortablePatchRevision,
    pub event_sequence: u64,
    pub fixtures: Vec<PatchFixtureProjection>,
    pub profile_revisions: Vec<PatchProfileRevisionProjection>,
}
