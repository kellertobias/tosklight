//! Portable live-DMX input intent authored by the Viz editor.
//!
//! These records say how logical show universes arrive on the wire. They deliberately do not
//! carry a network interface: choosing an interface belongs to the machine running the renderer,
//! not to a show file that moves between desks.

use crate::{DocumentError, PlanningDocument};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const LIVE_DMX_INPUT_KIND: &str = "visualizer_input";
const LIVE_DMX_INPUT_ID: &str = "main";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveDmxInputs {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub mappings: Vec<LiveDmxInputMapping>,
}

fn schema_version() -> u32 {
    1
}

impl Default for LiveDmxInputs {
    fn default() -> Self {
        Self {
            schema_version: schema_version(),
            mappings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveDmxInputMapping {
    pub id: String,
    pub logical_universe: u16,
    /// Stable values are `artnet` and `sacn`.
    pub protocol: String,
    pub destination_universe: u16,
    pub port: u16,
    pub enabled: bool,
    /// `broadcast` or `unicast` for Art-Net; `multicast` or `unicast` for sACN.
    pub delivery: String,
}

impl LiveDmxInputs {
    pub fn validate(&self) -> Result<(), DocumentError> {
        if self.schema_version != 1 {
            return Err(DocumentError::Store(format!(
                "unsupported live-DMX input schema version {}",
                self.schema_version
            )));
        }
        let mut ids = HashSet::new();
        let mut logical_universes = HashSet::new();
        for mapping in &self.mappings {
            if mapping.id.trim().is_empty() || !ids.insert(mapping.id.as_str()) {
                return Err(DocumentError::Store(
                    "every live-DMX input needs a unique non-empty ID".into(),
                ));
            }
            if mapping.logical_universe == 0 || !logical_universes.insert(mapping.logical_universe)
            {
                return Err(DocumentError::Store(format!(
                    "logical universe {} must be non-zero and configured only once",
                    mapping.logical_universe
                )));
            }
            if mapping.port == 0 {
                return Err(DocumentError::Store(format!(
                    "live-DMX input for universe {} needs a UDP port from 1 to 65535",
                    mapping.logical_universe
                )));
            }
            match mapping.protocol.as_str() {
                "artnet" => {
                    if mapping.destination_universe > 32_767 {
                        return Err(DocumentError::Store(
                            "Art-Net destination universes must be between 0 and 32767".into(),
                        ));
                    }
                    if !matches!(mapping.delivery.as_str(), "broadcast" | "unicast") {
                        return Err(DocumentError::Store(
                            "Art-Net inputs use Broadcast or Unicast delivery".into(),
                        ));
                    }
                }
                "sacn" => {
                    if !(1..=63_999).contains(&mapping.destination_universe) {
                        return Err(DocumentError::Store(
                            "sACN destination universes must be between 1 and 63999".into(),
                        ));
                    }
                    if !matches!(mapping.delivery.as_str(), "multicast" | "unicast") {
                        return Err(DocumentError::Store(
                            "sACN inputs use Multicast or Unicast delivery".into(),
                        ));
                    }
                }
                _ => {
                    return Err(DocumentError::Store(format!(
                        "unknown live-DMX protocol {}",
                        mapping.protocol
                    )));
                }
            }
        }
        Ok(())
    }
}

impl PlanningDocument {
    pub fn live_dmx_inputs(&self) -> Result<LiveDmxInputs, DocumentError> {
        let Some(object) = self
            .objects(LIVE_DMX_INPUT_KIND)?
            .into_iter()
            .find(|object| object.id == LIVE_DMX_INPUT_ID)
        else {
            return Ok(LiveDmxInputs {
                schema_version: 1,
                mappings: Vec::new(),
            });
        };
        let inputs: LiveDmxInputs = serde_json::from_value(object.body)
            .map_err(|error| DocumentError::Store(format!("invalid live-DMX inputs: {error}")))?;
        inputs.validate()?;
        Ok(inputs)
    }

    pub fn save_live_dmx_inputs(&self, inputs: &LiveDmxInputs) -> Result<(), DocumentError> {
        inputs.validate()?;
        let body = serde_json::to_value(inputs)
            .map_err(|error| DocumentError::Store(error.to_string()))?;
        self.put_object(LIVE_DMX_INPUT_KIND, LIVE_DMX_INPUT_ID, &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(protocol: &str) -> LiveDmxInputMapping {
        LiveDmxInputMapping {
            id: "u1".into(),
            logical_universe: 1,
            protocol: protocol.into(),
            destination_universe: 1,
            port: if protocol == "sacn" { 5568 } else { 6454 },
            enabled: true,
            delivery: if protocol == "sacn" {
                "multicast".into()
            } else {
                "broadcast".into()
            },
        }
    }

    #[test]
    fn validates_protocol_specific_universe_and_delivery_rules() {
        for protocol in ["artnet", "sacn"] {
            let inputs = LiveDmxInputs {
                schema_version: 1,
                mappings: vec![mapping(protocol)],
            };
            inputs.validate().expect("valid mapping");
        }

        let mut bad = mapping("sacn");
        bad.destination_universe = 0;
        assert!(
            LiveDmxInputs {
                schema_version: 1,
                mappings: vec![bad]
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn rejects_two_authorities_for_one_logical_universe() {
        let mut second = mapping("sacn");
        second.id = "also-u1".into();
        let inputs = LiveDmxInputs {
            schema_version: 1,
            mappings: vec![mapping("artnet"), second],
        };
        assert!(inputs.validate().is_err());
    }

    #[test]
    fn inputs_survive_document_reopen() {
        let base = std::env::var_os("LIGHT_TMP_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let _ = std::fs::create_dir_all(&base);
        let path = base.join(format!("viz-inputs-{}.show", uuid::Uuid::new_v4()));
        let document = PlanningDocument::create(&path, "Input show").expect("create show");
        let inputs = LiveDmxInputs {
            schema_version: 1,
            mappings: vec![mapping("artnet")],
        };
        document
            .save_live_dmx_inputs(&inputs)
            .expect("save live inputs");
        drop(document);

        let reopened = PlanningDocument::open(&path).expect("reopen show");
        assert_eq!(reopened.live_dmx_inputs().expect("read inputs"), inputs);
        drop(reopened);
        let _ = std::fs::remove_file(path);
    }
}
