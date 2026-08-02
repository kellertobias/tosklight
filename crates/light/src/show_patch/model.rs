use crate::{ActionContext, ApplicationCommand, CommandFamily};
use light_core::{FixtureId, Revision, ShowId};
use light_fixture::{
    FixtureSplit, InstalledFixtureAppearance, PatchPolicy, PatchedFixturePatch,
    PatchedFixtureProfileReference,
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
///
/// Optimistic concurrency is scoped to the portable Patch revision in the action context. The
/// result still reports both the whole-show and Patch revisions after a successful mutation.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchFixturesCommand {
    pub show_id: ShowId,
    pub fixtures: Vec<PatchFixtureCandidate>,
    /// Stable fixture identities removed by the same atomic patch transaction. Already-absent
    /// identities are ignored so retries and convergent desired-state updates remain idempotent.
    pub remove_fixture_ids: Vec<FixtureId>,
    /// Ordered placement intents whose final split assignments are resolved from authoritative
    /// selected-mode footprints before the candidate show is validated and committed.
    pub placements: Vec<PatchPlacementIntent>,
    /// Ordered coordinate spreads resolved against authoritative candidates on the server.
    pub vector_spreads: Vec<PatchVectorSpreadIntent>,
    /// Sparse, revision-guarded edits resolved from the authoritative stored fixture on the
    /// server. Keeping the raw intent in the command makes request replay independent of any
    /// read-modify-write projection.
    pub fixture_updates: Vec<PatchFixtureUpdateIntent>,
}

impl ApplicationCommand for PatchFixturesCommand {
    type Value = PatchFixturesResult;

    const FAMILY: CommandFamily = CommandFamily::Show;
}

#[derive(Clone, Debug, PartialEq)]
pub struct PatchFixtureUpdateIntent {
    pub fixture_id: FixtureId,
    pub expected_fixture_revision: Revision,
    pub expected_show_revision: PortableShowRevision,
    /// Absent targets the root physical fixture. Present must resolve to that exact copy.
    pub multipatch_instance_id: Option<Uuid>,
    pub action: PatchFixtureUpdateAction,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PatchFixtureUpdateAction {
    SetMasters {
        group_masters_enabled: bool,
        grand_master_enabled: bool,
    },
    SetPanTilt {
        invert_pan: bool,
        invert_tilt: bool,
    },
    SetMoveInBlack {
        enabled: bool,
        delay_millis: u64,
    },
    SetLocationAxis {
        axis: PatchFixtureAxis,
        millimetres: i32,
    },
    SetRotationAxis {
        axis: PatchFixtureAxis,
        degrees: f32,
    },
    SetBracketAngle {
        degrees: f32,
    },
    SetShaperModuleAngle {
        degrees: Option<f32>,
    },
    SetStaticShaperAngle {
        element: u8,
        degrees: f32,
    },
    SetInstalledAppearance {
        appearance: InstalledFixtureAppearance,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchFixtureAxis {
    X,
    Y,
    Z,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PatchPlacementIntent {
    pub fixture_ids: Vec<FixtureId>,
    pub splits: Vec<PatchSplitPlacementIntent>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PatchSplitPlacementIntent {
    pub split: u16,
    pub universe: Option<u16>,
    pub address: Option<u16>,
    pub mode: PatchSplitPlacementMode,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PatchSplitPlacementMode {
    Consecutive,
    OperatorOverrides(Vec<PatchOperatorAddressOverride>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatchOperatorAddressOverride {
    pub fixture_id: FixtureId,
    pub universe: u16,
    pub address: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PatchVectorSpreadIntent {
    pub fixture_ids: Vec<FixtureId>,
    pub kind: PatchVectorKind,
    pub axis: PatchVectorAxis,
    pub points: Vec<f32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchVectorKind {
    Location,
    Rotation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchVectorAxis {
    X,
    Y,
    Z,
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
#[derive(Clone, Debug, PartialEq)]
pub struct PatchProfileRevisionProjection {
    pub profile_id: FixtureId,
    pub profile_revision: Revision,
    pub content_digest: String,
    pub manufacturer: String,
    pub name: String,
    pub fixture_type: String,
    pub patch_policy: PatchPolicy,
    pub referenced_modes: Vec<PatchModeProjection>,
    /// Server-resolved parameterized profile snapshot for this revision, carried to patch
    /// consumers so they can build head parameters, channels, and control actions client-side.
    pub profile_snapshot: serde_json::Value,
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
