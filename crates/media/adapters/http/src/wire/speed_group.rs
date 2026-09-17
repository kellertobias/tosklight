//! Speed Group reception, as the operator sees it.
//!
//! Volatile, so it travels on the telemetry socket rather than being polled. The Media Server only
//! receives Speed Groups from a Light desk; nothing here describes anything it sends.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::diagnostics::{
    SpeedGroupReadingTelemetry, SpeedGroupRejectionTelemetry, SpeedGroupTelemetry,
};

/// One Speed Group as the desk last published it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpeedGroupReadingView {
    pub group: u32,
    pub bpm: f64,
    pub beat_phase: f64,
    /// False while the desk has the group paused; synchronized playback then holds its frame.
    pub running: bool,
    /// False once the group has not been refreshed within the freshness window. The output keeps
    /// the last tempo and warns.
    pub fresh: bool,
    #[ts(type = "number")]
    pub age_millis: u64,
}

/// A datagram that was refused, and why.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpeedGroupRejectionView {
    pub from: Option<String>,
    pub reason: String,
    #[ts(type = "number")]
    pub age_millis: u64,
}

/// The Speed Group stream this server follows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpeedGroupReceptionView {
    /// `disabled`, `unavailable`, `waiting`, `connected`, or `lost`.
    pub connection: String,
    pub listening: Option<String>,
    pub detail: Option<String>,
    pub sender: Option<String>,
    pub sender_address: Option<String>,
    #[ts(type = "number | null")]
    pub last_update_age_millis: Option<u64>,
    #[ts(type = "number")]
    pub accepted: u64,
    #[ts(type = "number")]
    pub rejected: u64,
    pub rejections: Vec<SpeedGroupRejectionView>,
    pub groups: Vec<SpeedGroupReadingView>,
}

impl SpeedGroupReceptionView {
    pub fn of(telemetry: &SpeedGroupTelemetry) -> Self {
        Self {
            connection: telemetry.connection.clone(),
            listening: telemetry.listening.clone(),
            detail: telemetry.detail.clone(),
            sender: telemetry.sender.clone(),
            sender_address: telemetry.sender_address.clone(),
            last_update_age_millis: telemetry.last_update_age_millis,
            accepted: telemetry.accepted,
            rejected: telemetry.rejected,
            rejections: telemetry.rejections.iter().map(rejection).collect(),
            groups: telemetry.groups.iter().map(reading).collect(),
        }
    }
}

fn rejection(record: &SpeedGroupRejectionTelemetry) -> SpeedGroupRejectionView {
    SpeedGroupRejectionView {
        from: record.from.clone(),
        reason: record.reason.clone(),
        age_millis: record.age_millis,
    }
}

fn reading(record: &SpeedGroupReadingTelemetry) -> SpeedGroupReadingView {
    SpeedGroupReadingView {
        group: record.group,
        bpm: record.bpm,
        beat_phase: record.beat_phase,
        running: record.running,
        fresh: record.fresh,
        age_millis: record.age_millis,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_carries_the_connection_the_groups_and_the_refusals() {
        let telemetry = SpeedGroupTelemetry {
            connection: "connected".to_owned(),
            listening: Some("0.0.0.0:4810".to_owned()),
            sender: Some("desk-1".to_owned()),
            sender_address: Some("192.168.1.10:50000".to_owned()),
            last_update_age_millis: Some(40),
            accepted: 12,
            rejected: 1,
            rejections: vec![SpeedGroupRejectionTelemetry {
                from: Some("192.168.1.11:1".to_owned()),
                reason: "invalid message: BPM -1 is outside 0–999".to_owned(),
                age_millis: 900,
            }],
            groups: vec![SpeedGroupReadingTelemetry {
                group: 1,
                bpm: 128.0,
                beat_phase: 0.5,
                running: true,
                fresh: true,
                age_millis: 40,
            }],
            ..SpeedGroupTelemetry::default()
        };
        let json = serde_json::to_value(SpeedGroupReceptionView::of(&telemetry)).unwrap();
        assert_eq!(json["connection"], "connected");
        assert_eq!(json["senderAddress"], "192.168.1.10:50000");
        assert_eq!(json["groups"][0]["bpm"], 128.0);
        assert_eq!(json["groups"][0]["beatPhase"], 0.5);
        assert_eq!(json["rejections"][0]["ageMillis"], 900);
        assert_eq!(json["rejected"], 1);
    }

    #[test]
    fn a_server_that_was_not_asked_to_listen_says_so() {
        let json =
            serde_json::to_value(SpeedGroupReceptionView::of(&SpeedGroupTelemetry::default()))
                .unwrap();
        assert_eq!(json["connection"], "disabled");
        assert!(json["groups"].as_array().unwrap().is_empty());
    }
}
