//! Installation-scoped Programmer lifecycle projection and lossless deltas.

use super::events::EventSnapshotCursor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingLifecycleSession {
    pub session_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingLifecycleProgrammer {
    pub programmer_id: Uuid,
    pub user_id: Uuid,
    pub connected: bool,
    #[ts(type = "number")]
    pub selected_fixture_count: u64,
    #[ts(type = "number")]
    pub normal_value_count: u64,
    /// Aggregate activity signal only; active Preload values and identities remain private.
    pub preload_active: bool,
    pub sessions: Vec<ProgrammingLifecycleSession>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingLifecycleProjection {
    #[ts(type = "number")]
    pub revision: u64,
    pub programmers: Vec<ProgrammingLifecycleProgrammer>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingLifecycleDelta {
    Upsert {
        programmer: ProgrammingLifecycleProgrammer,
    },
    Remove {
        programmer_id: Uuid,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingLifecycleChange {
    #[ts(type = "number")]
    pub revision: u64,
    pub delta: ProgrammingLifecycleDelta,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingLifecycleSnapshot {
    pub cursor: EventSnapshotCursor,
    pub projection: ProgrammingLifecycleProjection,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v2::events::EventPayload;

    #[test]
    fn lifecycle_projection_rejects_programmer_content_and_selection_ids() {
        let projection = serde_json::json!({
            "revision": 1,
            "programmers": [{
                "programmer_id": Uuid::from_u128(1),
                "user_id": Uuid::from_u128(2),
                "connected": true,
                "selected_fixture_count": 1,
                "normal_value_count": 1,
                "preload_active": true,
                "sessions": [{
                    "session_id": Uuid::from_u128(3),
                    "selected": [Uuid::from_u128(4)]
                }],
                "values": [{"fixture_id": Uuid::from_u128(4)}]
            }]
        });

        assert!(serde_json::from_value::<ProgrammingLifecycleProjection>(projection).is_err());
    }

    #[test]
    fn lifecycle_programmer_requires_preload_active_and_rejects_extra_details() {
        let exact = serde_json::json!({
            "programmer_id": Uuid::from_u128(1),
            "user_id": Uuid::from_u128(2),
            "connected": true,
            "selected_fixture_count": 0,
            "normal_value_count": 0,
            "preload_active": false,
            "sessions": []
        });
        assert!(serde_json::from_value::<ProgrammingLifecycleProgrammer>(exact.clone()).is_ok());

        let mut missing = exact.clone();
        missing.as_object_mut().unwrap().remove("preload_active");
        assert!(serde_json::from_value::<ProgrammingLifecycleProgrammer>(missing).is_err());

        let mut detailed = exact;
        detailed["preload_fixture_ids"] = serde_json::json!([Uuid::from_u128(3)]);
        assert!(serde_json::from_value::<ProgrammingLifecycleProgrammer>(detailed).is_err());
    }

    #[test]
    fn lifecycle_delta_round_trips_one_exact_transition() {
        let change = ProgrammingLifecycleChange {
            revision: 4,
            delta: ProgrammingLifecycleDelta::Remove {
                programmer_id: Uuid::from_u128(1),
            },
        };
        let json = serde_json::to_value(&change).unwrap();

        assert_eq!(json["revision"], 4);
        assert_eq!(json["delta"]["type"], "remove");
        assert_eq!(
            serde_json::from_value::<ProgrammingLifecycleChange>(json).unwrap(),
            change
        );
    }

    #[test]
    fn lifecycle_change_has_an_exact_event_payload_variant() {
        let change = ProgrammingLifecycleChange {
            revision: 4,
            delta: ProgrammingLifecycleDelta::Remove {
                programmer_id: Uuid::from_u128(1),
            },
        };
        let payload = EventPayload::ProgrammingLifecycleChanged {
            change: change.clone(),
        };
        let json = serde_json::to_value(&payload).unwrap();

        assert_eq!(json["type"], "programming_lifecycle_changed");
        assert_eq!(json["change"], serde_json::to_value(change).unwrap());
        assert_eq!(
            serde_json::from_value::<EventPayload>(json).unwrap(),
            payload
        );
    }
}
