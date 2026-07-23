//! Request and outcome DTOs for the v2 stage-layout intent API.
//!
//! Multi-fixture stage-position edits are server-side fan-outs: the client sends the ordered
//! selection plus one typed operation and the server computes and persists each fixture's new
//! position. Request bodies deliberately tolerate unknown fields (api-rules §5).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Body of the idempotent stage-layout action POST operation.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct StageLayoutActionRequest {
    /// Client-generated idempotency identity, scoped to the authenticated desk session.
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub action: StageLayoutAction,
}

/// One stage-layout intent, using the shared fan-out vocabulary: an explicitly ordered
/// selection plus a typed operation payload (see `ProgrammingValueMutation::SetSelection`).
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StageLayoutAction {
    /// Applies one uniform axis delta to every selected fixture. The server resolves each base
    /// position exactly like the stage views: the stored 3D entry, else the migrated legacy 2D
    /// entry, else the patch-order default grid slot; selected ids outside the patch are skipped.
    MoveSelection {
        #[schemars(length(min = 1, max = 10_000))]
        fixture_ids: Vec<Uuid>,
        axis: StagePositionAxis,
        /// Meters for translation axes, degrees for rotation axes. Must be finite.
        delta: f64,
    },
}

/// One editable component of a fixture's 3D stage position.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum StagePositionAxis {
    X,
    Y,
    Z,
    RotationX,
    RotationY,
    RotationZ,
}

/// Successful result of a stage-layout action.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct StageLayoutActionOutcome {
    pub request_id: String,
    /// Stage-layout object revision after the action (unchanged for a no-op).
    #[ts(type = "number")]
    pub revision: u64,
    /// Selected fixtures whose stored position the action changed, in selection order.
    pub moved_fixture_ids: Vec<Uuid>,
    /// `true` when idempotency replay returned the already committed authoritative result.
    pub replayed: bool,
    /// `false` when no selected fixture had a resolvable position and nothing was written.
    pub changed: bool,
}

/// Transport error for stage-layout actions.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct StageLayoutErrorResponse {
    pub error: String,
    pub retryable: bool,
}
