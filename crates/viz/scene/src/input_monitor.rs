//! Passive observations of a provider's network receiver, never a source of renderer values.

use crate::{InputHealth, InputMappingStatus, SourceProtocol, UniverseHealth};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MONITOR_ENV: &str = "TOSKLIGHT_VIZ_INPUT_MONITOR";
pub const MAX_MONITOR_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_MONITOR_UNIVERSES: usize = 1024;
pub const MAX_MONITOR_INPUTS: usize = 4096;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonitorContext {
    pub producer: String,
    pub endpoint: String,
    pub show_id: Option<crate::uuid::Uuid>,
    pub show_revision: u64,
    /// Complete effective mapping descriptors, including disabled inputs and priorities.
    pub mappings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonitorSnapshot {
    pub version: u8,
    pub generated_millis: u64,
    pub context: MonitorContext,
    pub received: ReceivedDmx,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceivedDmx {
    pub universes: Vec<ReceivedUniverse>,
    pub inputs: Vec<ReceivingInput>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceivedUniverse {
    pub universe: u16,
    pub slots: Option<Vec<u8>>,
    pub live: bool,
    pub rate_hz: f32,
    pub protocol: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceivingInput {
    pub id: String,
    pub protocol: String,
    pub logical_universe: u16,
    pub destination_universe: u16,
    pub delivery: String,
    pub bind: String,
    pub health: String,
    pub source: Option<String>,
    pub accepted_packets: u64,
    pub detail: String,
}

impl ReceivedDmx {
    pub fn from_receiver(
        frames: &BTreeMap<u16, Vec<u8>>,
        configured: impl Iterator<Item = u16>,
        health: Vec<UniverseHealth>,
        inputs: Vec<InputMappingStatus>,
        warnings: Vec<String>,
    ) -> Self {
        let health: BTreeMap<_, _> = health.into_iter().map(|row| (row.universe, row)).collect();
        let universes: BTreeSet<_> = configured.chain(frames.keys().copied()).collect();
        Self {
            universes: universes
                .into_iter()
                .map(|universe| {
                    let row = health.get(&universe);
                    ReceivedUniverse {
                        universe,
                        slots: frames.get(&universe).cloned(),
                        live: row.is_some_and(|row| !row.stale),
                        rate_hz: row.map(|row| row.rate_hz).unwrap_or_default(),
                        protocol: row.and_then(|row| row.protocol).map(protocol),
                    }
                })
                .collect(),
            inputs: inputs
                .into_iter()
                .map(|row| ReceivingInput {
                    id: row.mapping_id,
                    protocol: protocol(row.protocol),
                    logical_universe: row.logical_universe,
                    destination_universe: row.destination_universe,
                    delivery: row.delivery,
                    bind: row.bind,
                    health: row.health.label().into(),
                    source: source(row.source_name, row.source_address),
                    accepted_packets: row.accepted_packets,
                    detail: row.detail,
                })
                .collect(),
            warnings,
        }
    }

    /// Transport loss preserves held slots, but cannot assert a healthy current input or rate.
    pub fn unavailable(mut self, warning: &str) -> Self {
        for row in &mut self.universes {
            row.live = false;
            row.rate_hz = 0.0;
        }
        for row in &mut self.inputs {
            row.health = InputHealth::Stale.label().into();
            row.detail = warning.into();
        }
        self.warnings.push(warning.into());
        self
    }
}

fn protocol(value: SourceProtocol) -> String {
    match value {
        SourceProtocol::ArtNet => "artnet",
        SourceProtocol::Sacn => "sacn",
    }
    .into()
}

impl MonitorSnapshot {
    pub fn validate(&self) -> bool {
        self.version == 1
            && self.context.producer == "network_receiver"
            && self.context.endpoint.len() <= 1024
            && self.context.mappings.len() <= MAX_MONITOR_INPUTS
            && self
                .context
                .mappings
                .iter()
                .all(|value| value.len() <= 1024)
            && self.received.universes.len() <= MAX_MONITOR_UNIVERSES
            && self.received.inputs.len() <= MAX_MONITOR_INPUTS
            && self.received.warnings.len() <= 256
            && self
                .received
                .warnings
                .iter()
                .all(|value| value.len() <= 4096)
            && self.received.universes.iter().all(|row| {
                row.universe > 0
                    && row.rate_hz.is_finite()
                    && row.rate_hz >= 0.0
                    && row.slots.as_ref().is_none_or(|slots| slots.len() == 512)
                    && row.protocol.as_deref().is_none_or(valid_protocol)
            })
            && self.received.inputs.iter().all(|row| {
                valid_protocol(&row.protocol)
                    && row.id.len() <= 1024
                    && row.delivery.len() <= 128
                    && row.bind.len() <= 128
                    && row.detail.len() <= 4096
                    && row
                        .source
                        .as_ref()
                        .is_none_or(|source| source.len() <= 1024)
                    && [
                        "Waiting for DMX",
                        "Healthy",
                        "Stale",
                        "Superseded",
                        "Failed",
                    ]
                    .contains(&row.health.as_str())
            })
    }
}

fn valid_protocol(value: &str) -> bool {
    matches!(value, "artnet" | "sacn")
}

/// Same-machine wall clock used only to reject delayed/future monitor messages, never for DMX timing.
pub fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

pub fn source(name: String, address: Option<String>) -> Option<String> {
    match (name.trim(), address) {
        ("", address) => address,
        (name, None) => Some(name.into()),
        (name, Some(address)) => Some(format!("{name} · {address}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> MonitorSnapshot {
        MonitorSnapshot {
            version: 1,
            generated_millis: unix_millis(),
            context: MonitorContext {
                producer: "network_receiver".into(),
                endpoint: "http://127.0.0.1:1234".into(),
                show_id: None,
                show_revision: 7,
                mappings: vec![],
            },
            received: ReceivedDmx {
                universes: vec![ReceivedUniverse {
                    universe: 1,
                    slots: Some(vec![7; 512]),
                    live: true,
                    rate_hz: 44.0,
                    protocol: Some("artnet".into()),
                }],
                inputs: vec![],
                warnings: vec![],
            },
        }
    }

    #[test]
    fn passive_dto_keeps_existing_values_json_shape_and_strict_bounded_envelope() {
        let original = snapshot();
        assert!(original.validate());
        let json = serde_json::to_value(&original.received).unwrap();
        assert_eq!(
            json.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["inputs", "universes", "warnings"]
        );
        let universe = json["universes"][0].as_object().unwrap();
        assert_eq!(
            universe.keys().map(String::as_str).collect::<Vec<_>>(),
            ["live", "protocol", "rateHz", "slots", "universe"]
        );
        let mut unknown = serde_json::to_value(&original).unwrap();
        unknown["arbitrary"] = serde_json::json!("rejected");
        assert!(serde_json::from_value::<MonitorSnapshot>(unknown).is_err());
        for mutation in 0..6 {
            let mut changed = original.clone();
            match mutation {
                0 => changed.version = 2,
                1 => changed.context.producer = "preview".into(),
                2 => changed.received.universes[0].slots = Some(vec![0; 511]),
                3 => changed.received.universes[0].rate_hz = f32::NAN,
                4 => changed.context.mappings = vec!["x".into(); MAX_MONITOR_INPUTS + 1],
                _ => {
                    changed.received.universes =
                        vec![changed.received.universes[0].clone(); MAX_MONITOR_UNIVERSES + 1]
                }
            }
            assert!(!changed.validate(), "reject mutation {mutation}");
        }
    }
}
