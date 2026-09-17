//! Speed Group reception for the running process.
//!
//! One UDP listener, bound once at startup when the network settings name a Speed Group listen
//! address. What it hears goes into one shared reception state; every output reads its tempo from
//! there each frame, and the API reads the same state for the operator. The Media Server only
//! receives Speed Groups — nothing here sends one.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use media_application::MediaConfiguration;
use media_domain::{OutputId, OutputTempo, Timestamp};
use media_net::speed_group_osc::SpeedGroupDatagram;
use media_net::{SpeedGroupListener, SpeedGroupReception, SpeedGroupReceptionStatus};

use crate::Shutdown;

/// The reception state every output and the API share.
pub type SharedSpeedGroups = Arc<Mutex<SpeedGroupReception>>;

pub fn shared() -> SharedSpeedGroups {
    Arc::new(Mutex::new(SpeedGroupReception::default()))
}

fn lock(reception: &SharedSpeedGroups) -> MutexGuard<'_, SpeedGroupReception> {
    reception
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn stamp(started: Instant) -> Timestamp {
    Timestamp::from_micros(started.elapsed().as_micros() as u64)
}

/// Starts listening, when the configuration asks for it.
///
/// A listener that cannot bind is reported to the operator and leaves the rest of the server
/// running: every output then simply keeps its tempo source without a live group.
pub fn spawn(
    configuration: &MediaConfiguration,
    reception: &SharedSpeedGroups,
    shutdown: &Shutdown,
    started: Instant,
) {
    let Some(address) = configuration.network.resolved().speed_group_endpoint else {
        return;
    };
    let mut listener = match SpeedGroupListener::bind(address) {
        Ok(listener) => listener,
        Err(error) => {
            tracing::warn!(%error, "Speed Groups will not be received");
            *lock(reception) = SpeedGroupReception::unavailable(error.to_string());
            return;
        }
    };
    let bound = listener.local_address().unwrap_or(address);
    tracing::info!(address = %bound, "listening for Speed Groups");
    *lock(reception) = SpeedGroupReception::listening(bound);
    let (reception, mut watcher) = (reception.clone(), shutdown.watcher());
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = watcher.wait() => break,
                datagram = listener.receive() => apply(&reception, datagram, stamp(started)),
            }
        }
    });
}

/// Applies one received datagram, recording every refusal for the operator.
pub fn apply(reception: &SharedSpeedGroups, (from, decoded): SpeedGroupDatagram, now: Timestamp) {
    let mut reception = lock(reception);
    match decoded {
        Ok(updates) => {
            for update in updates {
                if let Err(rejection) = reception.accept(update, Some(from), now) {
                    tracing::debug!(%from, %rejection, "Speed Group update refused");
                }
            }
        }
        Err(error) => {
            tracing::debug!(%from, %error, "Speed Group datagram refused");
            reception.reject(error.to_string(), Some(from), now);
        }
    }
}

/// The tempo one output's layers follow on this frame.
pub fn output_tempo(
    configuration: &MediaConfiguration,
    output: OutputId,
    reception: &SharedSpeedGroups,
) -> OutputTempo {
    let source = configuration
        .output(output)
        .map(|output| output.tempo_source)
        .unwrap_or_default();
    OutputTempo {
        source,
        speed_group: source
            .speed_group()
            .and_then(|group| lock(reception).snapshot(group)),
    }
}

/// What the API reports about reception.
pub fn diagnostics(
    reception: &SharedSpeedGroups,
    started: Instant,
) -> media_http::SpeedGroupSource {
    let reception = reception.clone();
    Arc::new(move || {
        let now = stamp(started);
        telemetry(&lock(&reception).status(now), now)
    })
}

fn telemetry(
    status: &SpeedGroupReceptionStatus,
    now: Timestamp,
) -> media_http::SpeedGroupTelemetry {
    media_http::SpeedGroupTelemetry {
        connection: status.connection.as_str().to_owned(),
        listening: status.listening.map(|address| address.to_string()),
        detail: status.detail.clone(),
        sender: status.sender.clone(),
        sender_address: status.sender_address.map(|address| address.to_string()),
        last_update_age_millis: status.last_update_age_millis,
        accepted: status.accepted,
        rejected: status.rejected,
        rejections: status
            .rejections
            .iter()
            .map(|record| media_http::SpeedGroupRejectionTelemetry {
                from: record.from.map(|address| address.to_string()),
                reason: record.reason.clone(),
                age_millis: now.since(record.at).as_millis() as u64,
            })
            .collect(),
        groups: status
            .groups
            .iter()
            .map(|reading| media_http::SpeedGroupReadingTelemetry {
                group: reading.group,
                bpm: reading.bpm,
                beat_phase: reading.beat_phase,
                running: reading.running,
                fresh: reading.fresh,
                age_millis: reading.age_millis,
            })
            .collect(),
    }
}

#[cfg(test)]
#[path = "speed_groups_tests.rs"]
mod tests;
