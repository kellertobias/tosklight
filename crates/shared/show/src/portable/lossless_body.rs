use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

/// A typed portable-show body with its exact persisted representation retained by the codec.
///
/// Application services use `typed()` and never need to carry generic JSON. The portable-show
/// codec keeps `raw` so fields unknown to this build survive typed read/mutate/write cycles.
#[derive(Clone, Debug)]
pub struct LosslessBody<T> {
    typed: T,
    raw: Value,
}

impl<T> PartialEq for LosslessBody<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<T> LosslessBody<T> {
    pub const fn typed(&self) -> &T {
        &self.typed
    }

    /// Encodes the retained lossless body at a persistence or transport adapter boundary.
    pub fn encode(&self) -> Value {
        self.raw.clone()
    }
}

impl<T: DeserializeOwned> LosslessBody<T> {
    pub fn decode(raw: Value) -> serde_json::Result<Self> {
        let typed = serde_json::from_value(raw.clone())?;
        Ok(Self { typed, raw })
    }
}

impl<T: Serialize> LosslessBody<T> {
    pub fn from_typed(typed: T) -> serde_json::Result<Self> {
        let raw = serde_json::to_value(&typed)?;
        Ok(Self { typed, raw })
    }
}

impl<T> LosslessBody<T>
where
    T: Clone + DeserializeOwned + Serialize,
{
    pub fn merge_normalized_body(
        existing: Option<&Self>,
        request: &Self,
        normalized: T,
    ) -> serde_json::Result<Self> {
        let raw = super::lossless_json::merge_typed_request(
            existing.map(|body| &body.raw),
            existing.map(|body| &body.typed),
            &request.raw,
            &request.typed,
            &normalized,
        )?;
        Ok(Self {
            typed: normalized,
            raw,
        })
    }

    pub fn strip_zero_u64_echo(&mut self, key: &str) {
        super::lossless_json::strip_zero_u64_echo(&mut self.raw, key);
    }

    /// Removes one retired typed key from every object inside a named array while preserving all
    /// unrelated unknown extensions.
    pub fn strip_nested_array_object_key(&mut self, array: &str, key: &str) {
        if let Some(items) = self.raw.get_mut(array).and_then(Value::as_array_mut) {
            for item in items {
                if let Some(item) = item.as_object_mut() {
                    item.remove(key);
                }
            }
        }
    }
}

/// Codec-owned JSON retained for application fields whose schema is deliberately extensible and
/// interpreted only by their owning client capability.
#[derive(Clone, Debug, PartialEq)]
pub struct PortableJson(Value);

impl PortableJson {
    pub fn new(value: Value) -> Self {
        Self(value)
    }

    pub const fn as_raw(&self) -> &Value {
        &self.0
    }

    pub fn into_raw(self) -> Value {
        self.0
    }
}

impl Serialize for PortableJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for PortableJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct KnownBody {
        name: String,
    }

    #[test]
    fn typed_body_retains_unknown_raw_fields() {
        let body = LosslessBody::<KnownBody>::decode(serde_json::json!({
            "name": "Front",
            "future": {"kept": true}
        }))
        .unwrap();

        assert_eq!(body.typed().name, "Front");
        assert_eq!(body.encode()["future"]["kept"], true);
    }

    #[test]
    fn targeted_nested_scrub_preserves_unrelated_extensions() {
        #[derive(Clone, Debug, Deserialize, Serialize)]
        struct Body {
            cues: Vec<Cue>,
        }
        #[derive(Clone, Debug, Deserialize, Serialize)]
        struct Cue {
            name: String,
        }
        let mut body = LosslessBody::<Body>::decode(serde_json::json!({
            "cues": [{
                "name": "One",
                "phasers": [{"prototype": true}],
                "future": {"kept": true}
            }],
            "top_level_future": true
        }))
        .unwrap();
        body.strip_nested_array_object_key("cues", "phasers");
        let encoded = body.encode();
        assert!(encoded["cues"][0].get("phasers").is_none());
        assert_eq!(encoded["cues"][0]["future"]["kept"], true);
        assert_eq!(encoded["top_level_future"], true);
    }
}
