//! The socket, and the loop that turns what arrives into what the desk holds.
//!
//! PosiStageNet is multicast, so the desk joins a group and listens; there is nothing to connect
//! to and nothing to ask for. The address is shared with whatever else on this machine wants to
//! hear the same tracking system, which is why the socket is opened with the address reusable —
//! a visualizer running beside the desk must not stop the desk from receiving.
//!
//! The loop does two things at once: it reads datagrams as fast as they arrive, and every 20
//! milliseconds it decides what that means. Those are separate because a tracking system sends at
//! 60 Hz and may burst; recomputing the whole show's bindings per datagram would do the same work
//! several times for one frame.
//!
//! Rebinding is driven by a generation counter rather than by watching the show: when an operator
//! changes the group, the port, the interface, or the enabled switch, the number moves and the
//! socket is opened again. Editing a binding or a zone does not touch the socket.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

use super::super::AppState;
use super::config::PsnConfiguration;

/// How often the bindings and zones are worked out. Half a tracking frame at 60 Hz, so a held
/// point is never more than one frame behind the marker.
const TICK: Duration = Duration::from_millis(20);
/// A PSN datagram is capped at 1500 bytes by the protocol; this leaves room for a sender that
/// ignores the cap, so an oversized packet is read and rejected rather than silently truncated.
const DATAGRAM_BUFFER: usize = 2_048;

pub(in crate::runtime) async fn run(
    state: AppState,
    cancellation: CancellationToken,
) -> anyhow::Result<()> {
    let mut socket: Option<UdpSocket> = None;
    let mut bound_generation = u64::MAX;
    let mut buffer = vec![0_u8; DATAGRAM_BUFFER];
    let mut ticker = tokio::time::interval(TICK);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let generation = state.psn.generation();
        if generation != bound_generation {
            bound_generation = generation;
            socket = rebind(&state).await;
        }
        // A desk with no socket still ticks: the operator has to keep seeing why, and a
        // configuration change has to be noticed.
        match socket.as_ref() {
            Some(listening) => {
                tokio::select! {
                    _ = cancellation.cancelled() => return Ok(()),
                    _ = ticker.tick() => tick(&state),
                    received = listening.recv_from(&mut buffer) => match received {
                        Ok((length, source)) => {
                            state.psn.observe(source, &buffer[..length], now_millis());
                        }
                        Err(error) => {
                            // The socket is gone rather than the packet being bad: drop it and
                            // let the next pass open a new one.
                            state.psn.set_error(Some(format!(
                                "the PSN socket stopped receiving: {error}"
                            )));
                            socket = None;
                            bound_generation = u64::MAX;
                        }
                    },
                }
            }
            None => {
                tokio::select! {
                    _ = cancellation.cancelled() => return Ok(()),
                    _ = ticker.tick() => tick(&state),
                }
            }
        }
    }
}

/// Open the group the show asks for, or say why that was not possible.
async fn rebind(state: &AppState) -> Option<UdpSocket> {
    let configuration = state.psn.configuration();
    if !configuration.enabled {
        state.psn.set_error(None);
        return None;
    }
    match open(&configuration) {
        Ok(socket) => {
            state.psn.set_error(None);
            tracing::info!(
                group = %configuration.group,
                port = configuration.port,
                "listening for PosiStageNet"
            );
            Some(socket)
        }
        Err(error) => {
            // Actionable rather than a stack trace: the two things that go wrong here are a port
            // already in use and an interface that cannot join the group.
            let message = format!(
                "the desk could not listen for PSN on {}:{} — {error}",
                configuration.group, configuration.port
            );
            tracing::warn!("{message}");
            state.psn.set_error(Some(message));
            None
        }
    }
}

fn open(configuration: &PsnConfiguration) -> std::io::Result<UdpSocket> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    // Several programs on one machine may listen to the same tracking system, and the desk is not
    // entitled to be the only one.
    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;
    socket.set_nonblocking(true)?;
    let interface = configuration.interface.unwrap_or(Ipv4Addr::UNSPECIFIED);
    socket.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, configuration.port).into())?;
    socket.join_multicast_v4(&configuration.group, &interface)?;
    UdpSocket::from_std(socket.into())
}

fn tick(state: &AppState) {
    state
        .psn
        .install_point_locations(point_locations(&state.output.snapshot()));
    let outcome = state.psn.tick(now_millis());
    state
        .output
        .engine()
        .set_tracked_overrides(outcome.overrides);
    for (zone_id, transition) in outcome.zone_transitions {
        super::zone_macros::run(state, zone_id, transition);
    }
}

/// Where every 3D Point in the show was patched, in metres.
fn point_locations(snapshot: &light_engine::EngineSnapshot) -> HashMap<uuid::Uuid, [f32; 3]> {
    snapshot
        .fixtures
        .iter()
        .filter(|fixture| {
            fixture.definition.heads.iter().any(|head| {
                head.parameters.iter().any(|parameter| {
                    parameter.attribute.0.as_ref() == super::bindings::POINT_AXIS_ATTRIBUTES[0]
                })
            })
        })
        .map(|fixture| {
            (
                fixture.fixture_id.0,
                [
                    fixture.location.x as f32 / 1000.0,
                    fixture.location.y as f32 / 1000.0,
                    fixture.location.z as f32 / 1000.0,
                ],
            )
        })
        .collect()
}

/// The desk's own clock, in milliseconds. Nothing in PSN is synchronised to it — the sender's
/// timestamps are on the sender's clock — so freshness is measured against arrival here.
fn now_millis() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as u64
}
