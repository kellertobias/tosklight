//! Preserve the existing flat scalar document; typed bodies have an explicit exclusive key.
//! Inspect key presence before parsing so null/mixed bodies cannot silently select one branch.
use super::*;
use serde::{Deserializer, Serializer, de::Error, ser::SerializeMap};
use serde_json::{Map, Value};
use uuid::Uuid;

#[derive(Deserialize)]
struct StoredLane {
    id: Uuid,
    speed_multiplier: Rational,
    width: f32,
    #[serde(default)]
    phase: Option<PhaseDistribution>,
    #[serde(default)]
    random_group_id: Option<Uuid>,
    #[serde(flatten)]
    body: Map<String, Value>,
}

impl<'de> Deserialize<'de> for DynamicLane {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut stored = StoredLane::deserialize(deserializer)?;
        let body = if let Some(programming) = stored.body.remove("programming") {
            if [
                "attribute",
                "mode",
                "keyframes",
                "max_min",
                "middle_amplitude",
            ]
            .iter()
            .any(|key| stored.body.contains_key(*key))
            {
                return Err(D::Error::custom(
                    "a typed Dynamic lane cannot contain scalar fields",
                ));
            }
            DynamicLaneBody::Programming(
                serde_json::from_value(programming).map_err(D::Error::custom)?,
            )
        } else {
            DynamicLaneBody::LegacyScalar(
                serde_json::from_value(Value::Object(stored.body)).map_err(D::Error::custom)?,
            )
        };
        Ok(Self {
            id: stored.id,
            body,
            speed_multiplier: stored.speed_multiplier,
            width: stored.width,
            phase: stored.phase,
            random_group_id: stored.random_group_id,
        })
    }
}

impl Serialize for DynamicLaneBody {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::LegacyScalar(body) => body.serialize(serializer),
            Self::Programming(body) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("programming", body)?;
                map.end()
            }
        }
    }
}

#[derive(Deserialize)]
struct StoredRandomGroup {
    id: Uuid,
    seed: u64,
    decision_interval_millis: u64,
    start_probability: f32,
    mean_duration_millis: u64,
    duration_spread_millis: u64,
    attack_ratio: f32,
    decay_ratio: f32,
    #[serde(flatten)]
    range: Map<String, Value>,
}

#[derive(Serialize, Deserialize)]
struct Range<S> {
    low: S,
    high: S,
}

impl<'de> Deserialize<'de> for DynamicRandomGroup {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut stored = StoredRandomGroup::deserialize(deserializer)?;
        let range = if let Some(programming) = stored.range.remove("programming_range") {
            if ["low", "high"]
                .iter()
                .any(|key| stored.range.contains_key(*key))
            {
                return Err(D::Error::custom(
                    "a typed Random group cannot contain scalar fields",
                ));
            }
            let Range { low, high } =
                serde_json::from_value(programming).map_err(D::Error::custom)?;
            DynamicRandomRange::Programming { low, high }
        } else {
            let Range { low, high } =
                serde_json::from_value(Value::Object(stored.range)).map_err(D::Error::custom)?;
            DynamicRandomRange::LegacyScalar { low, high }
        };
        Ok(Self {
            id: stored.id,
            seed: stored.seed,
            range,
            decision_interval_millis: stored.decision_interval_millis,
            start_probability: stored.start_probability,
            mean_duration_millis: stored.mean_duration_millis,
            duration_spread_millis: stored.duration_spread_millis,
            attack_ratio: stored.attack_ratio,
            decay_ratio: stored.decay_ratio,
        })
    }
}

impl Serialize for DynamicRandomRange {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::LegacyScalar { low, high } => Range { low, high }.serialize(serializer),
            Self::Programming { low, high } => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("programming_range", &Range { low, high })?;
                map.end()
            }
        }
    }
}
