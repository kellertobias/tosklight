//! Speed Groups published to every patched ToskLight Media Server.
//!
//! The desk is authoritative for Speed Group tempo; a Media Server only receives it. Each tick
//! sends every group as one OSC message of the ToskLight Speed Group contract
//! (`docs/help/90-Protocols/02-media-speed-groups.md`) to UDP port 4810 on the host of each
//! patched ToskLight Media fixture's CITP endpoint. Sending continuously rather than on change is
//! what lets a Media Server that restarts, or a network that drops packets, recover within one
//! tick without any handshake.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Duration;

use light_fixture::DirectControlProtocol;
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

use super::{AppState, OscArgument, SpeedSnapshot, encode_osc_message};

/// The OSC address of one Speed Group update.
pub(super) const SPEED_GROUP_ADDRESS: &str = "/tosklight/speed-group";
/// The UDP port a ToskLight Media Server listens on for Speed Groups by default.
pub(super) const MEDIA_SPEED_GROUP_PORT: u16 = 4810;
/// Well inside the Media Server's 1.5 s freshness window, and quick enough that a tap-tempo
/// change reaches the picture within a beat.
const PUBLISH_INTERVAL: Duration = Duration::from_millis(100);
const MAX_BPM: f64 = 999.0;

pub(super) async fn run(state: AppState, cancellation: CancellationToken) -> anyhow::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|error| {
        anyhow::anyhow!("Light Desk could not open its Speed Group socket: {error}")
    })?;
    // A new identity per run tells a Media Server that the sequence starts over.
    let mut publisher = SpeedGroupPublisher::new(format!("tosklight-{}", uuid::Uuid::new_v4()));
    let mut ticker = tokio::time::interval(PUBLISH_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => return Ok(()),
            _ = ticker.tick() => publish(&socket, &state, &mut publisher).await,
        }
    }
}

async fn publish(socket: &UdpSocket, state: &AppState, publisher: &mut SpeedGroupPublisher) {
    if state.active_show.current().is_none() {
        return;
    }
    let targets = media_server_targets(&state.output.snapshot().fixtures);
    if targets.is_empty() {
        return;
    }
    let snapshots = state
        .output
        .speed_group_snapshots(super::speed_groups::application_millis(state));
    for packet in publisher.packets(&snapshots) {
        for target in &targets {
            if let Err(error) = socket.send_to(&packet, target).await {
                tracing::debug!(%target, %error, "Speed Group update could not be sent");
            }
        }
    }
}

/// Where Speed Groups go: every patched ToskLight Media Server with a CITP endpoint.
pub(super) fn media_server_targets(
    fixtures: &[light_fixture::PatchedFixture],
) -> HashSet<SocketAddr> {
    fixtures
        .iter()
        .filter(|fixture| super::media_api::native_media_action(fixture).is_some())
        .filter_map(|fixture| fixture.direct_control.as_ref())
        .filter(|endpoint| endpoint.protocol == DirectControlProtocol::Citp)
        .map(|endpoint| SocketAddr::new(endpoint.ip_address, MEDIA_SPEED_GROUP_PORT))
        .collect()
}

/// The sender side of the contract: one identity, and a sequence that only moves forward.
pub(super) struct SpeedGroupPublisher {
    source: String,
    sequence: u32,
}

impl SpeedGroupPublisher {
    pub(super) fn new(source: String) -> Self {
        Self {
            source,
            sequence: 0,
        }
    }

    /// One encoded message per Speed Group, groups numbered from one.
    pub(super) fn packets(&mut self, snapshots: &[SpeedSnapshot]) -> Vec<Vec<u8>> {
        snapshots
            .iter()
            .enumerate()
            .filter_map(|(index, snapshot)| {
                self.sequence = self.sequence.wrapping_add(1) & i32::MAX as u32;
                let arguments = self.arguments(index as i32 + 1, snapshot);
                encode_osc_message(SPEED_GROUP_ADDRESS, &arguments).ok()
            })
            .collect()
    }

    fn arguments(&self, group: i32, snapshot: &SpeedSnapshot) -> Vec<OscArgument> {
        let bpm = if snapshot.effective_bpm.is_finite() {
            snapshot.effective_bpm.clamp(0.0, MAX_BPM)
        } else {
            0.0
        };
        let phase = if snapshot.beat_phase.is_finite() {
            snapshot.beat_phase.rem_euclid(1.0)
        } else {
            0.0
        };
        vec![
            OscArgument::String(self.source.clone()),
            OscArgument::Int(self.sequence as i32),
            OscArgument::Int(group),
            OscArgument::Float(bpm as f32),
            // A phase that rounds up to exactly one in single precision wraps to the next beat.
            OscArgument::Float(if (phase as f32) < 1.0 {
                phase as f32
            } else {
                0.0
            }),
            OscArgument::Int(i32::from(!snapshot.paused)),
        ]
    }
}

#[cfg(test)]
#[path = "media_speed_groups_tests.rs"]
mod tests;
