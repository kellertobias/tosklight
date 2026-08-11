//! Feeding the reducer from the network.
//!
//! Packets arrive on a background task, are decoded through the canonical personality, and become
//! typed commands. The reducer applies them and publishes an immutable snapshot; the render loop
//! reads that snapshot without ever taking a lock the network can hold.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use arc_swap::ArcSwap;
use media_application::configuration::{DmxProtocol, MediaConfiguration};
use media_domain::personality::decode;
use media_domain::{Command, CommandKind, MediaState, OutputId, Timestamp, apply};
use media_net::{ArtNetListener, IngressError, SacnListener, UniverseFrame};

use crate::shutdown::Shutdown;

/// The authoritative state, published for readers.
///
/// The reducer is the only writer. A reader swaps in a whole new `Arc` rather than mutating, so
/// the render loop never blocks on a packet arriving and never sees a half-applied frame.
pub type SharedState = Arc<ArcSwap<MediaState>>;

pub type SharedDiagnostics = Arc<Mutex<HashMap<OutputId, IngressSample>>>;

#[derive(Debug, Clone)]
pub struct IngressSample {
    protocol: DmxProtocol,
    universe: u16,
    start_address: u16,
    source: String,
    frames_per_second: f32,
    received_at: Timestamp,
    slots: Vec<u8>,
}

pub fn diagnostics() -> SharedDiagnostics {
    Arc::new(Mutex::new(HashMap::new()))
}

pub fn diagnostic_snapshot(
    diagnostics: &SharedDiagnostics,
    now: Timestamp,
) -> Vec<media_http::DmxTelemetry> {
    diagnostics
        .lock()
        .map(|samples| {
            samples
                .iter()
                .map(|(output, sample)| {
                    let age_millis = now.since(sample.received_at).as_millis() as u64;
                    media_http::DmxTelemetry {
                        output: *output,
                        protocol: match sample.protocol {
                            DmxProtocol::ArtNet => "art-net",
                            DmxProtocol::Sacn => "sacn",
                        }
                        .to_owned(),
                        universe: sample.universe,
                        start_address: sample.start_address,
                        source: sample.source.clone(),
                        frames_per_second: sample.frames_per_second,
                        age_millis,
                        active: age_millis <= 2_500,
                        slots: sample.slots.clone(),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Which outputs a universe feeds, and how to address them.
#[derive(Debug, Clone)]
struct Route {
    output: OutputId,
    universe: u16,
    protocol: DmxProtocol,
    start_address: u16,
    personality: media_domain::LayerPersonality,
}

/// Builds the routing table from configuration.
fn routes(configuration: &MediaConfiguration) -> Vec<Route> {
    configuration
        .outputs
        .iter()
        .filter(|output| output.enabled)
        .map(|output| Route {
            output: output.id,
            universe: output.universe,
            protocol: output.protocol,
            start_address: output.start_address,
            personality: output.personality,
        })
        .collect()
}

/// Applies one received universe to every output patched to it.
///
/// A frame that does not reach an output's footprint is skipped with a reason rather than applied
/// half-decoded.
fn apply_frame_with_diagnostics(
    state: &SharedState,
    routes: &[Route],
    frame: &UniverseFrame,
    diagnostics: &SharedDiagnostics,
) {
    let matching: Vec<&Route> = routes
        .iter()
        .filter(|route| {
            route.universe == frame.universe
                && matches!(
                    (route.protocol, frame.source),
                    (DmxProtocol::ArtNet, media_domain::CommandSource::ArtNet)
                        | (DmxProtocol::Sacn, media_domain::CommandSource::Sacn)
                )
        })
        .collect();
    if matching.is_empty() {
        return;
    }

    // One mutation of a cloned snapshot, then one publish, so readers never observe a partially
    // updated state.
    let mut next = MediaState::clone(&state.load());
    let mut changed = false;

    for route in matching {
        let start = usize::from(route.start_address.saturating_sub(1));
        let end = start.saturating_add(usize::from(route.personality.footprint().total()));
        if let Some(slots) = frame.slots.get(start..end)
            && let Ok(mut samples) = diagnostics.lock()
        {
            let frames_per_second = samples
                .get(&route.output)
                .map(|previous| frame.received_at.since(previous.received_at).as_micros())
                .filter(|micros| *micros > 0)
                .map(|micros| 1_000_000.0 / micros as f32)
                .unwrap_or(0.0);
            samples.insert(
                route.output,
                IngressSample {
                    protocol: route.protocol,
                    universe: route.universe,
                    start_address: route.start_address,
                    source: frame.source_label.clone(),
                    frames_per_second,
                    received_at: frame.received_at,
                    slots: slots.to_vec(),
                },
            );
        }
        match decode::frame(route.personality, route.start_address, &frame.slots) {
            Ok(decoded) => {
                let command = Command::new(
                    CommandKind::SetDmxFrame {
                        output: route.output,
                        frame: Box::new(decoded),
                    },
                    frame.source,
                    frame.received_at,
                );
                changed |= apply(&mut next, &command).is_accepted();
            }
            Err(error) => {
                tracing::debug!(
                    universe = frame.universe,
                    output = %route.output,
                    %error,
                    "the frame does not cover this output's footprint"
                );
            }
        }
    }

    if changed {
        state.store(Arc::new(next));
    }
}

#[cfg(test)]
fn apply_frame(state: &SharedState, routes: &[Route], frame: &UniverseFrame) {
    apply_frame_with_diagnostics(state, routes, frame, &diagnostics());
}

/// Starts the listeners the configuration calls for.
///
/// Each protocol is bound only if some enabled output actually uses it, so a show that speaks only
/// Art-Net never holds the sACN port and cannot collide with something else that wants it.
pub fn spawn(
    configuration: &MediaConfiguration,
    state: SharedState,
    shutdown: Shutdown,
    started: std::time::Instant,
    diagnostics: SharedDiagnostics,
) -> Result<(), IngressError> {
    let routes = routes(configuration);
    let resolved = configuration.network.resolved();
    let now = move || Timestamp::from_micros(started.elapsed().as_micros() as u64);

    if routes
        .iter()
        .any(|route| route.protocol == DmxProtocol::ArtNet)
    {
        let mut listener = ArtNetListener::bind(resolved.art_net_listen)?;
        tracing::info!(address = %resolved.art_net_listen, "listening for Art-Net");
        let (routes, state, mut watcher, now, diagnostics) = (
            routes.clone(),
            state.clone(),
            shutdown.watcher(),
            now,
            diagnostics.clone(),
        );
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = watcher.wait() => break,
                    frame = listener.receive(&now) => apply_frame_with_diagnostics(&state, &routes, &frame, &diagnostics),
                }
            }
        });
    }

    if routes
        .iter()
        .any(|route| route.protocol == DmxProtocol::Sacn)
    {
        let universes: Vec<u16> = routes
            .iter()
            .filter(|route| route.protocol == DmxProtocol::Sacn)
            .map(|route| route.universe)
            .collect();
        let mut listener = SacnListener::bind(resolved.sacn_listen, &universes)?;
        tracing::info!(address = %resolved.sacn_listen, ?universes, "listening for sACN");
        let (routes, state, mut watcher, diagnostics) = (
            routes.clone(),
            state.clone(),
            shutdown.watcher(),
            diagnostics.clone(),
        );
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = watcher.wait() => break,
                    frame = listener.receive(&now) => apply_frame_with_diagnostics(&state, &routes, &frame, &diagnostics),
                }
            }
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use media_application::configuration::OutputConfiguration;
    use media_domain::{LayerPersonality, MediaAddress, OutputState};

    use super::*;

    fn configuration(protocol: DmxProtocol, universe: u16, start: u16) -> MediaConfiguration {
        let mut output = OutputConfiguration::new("Main");
        output.protocol = protocol;
        output.universe = universe;
        output.start_address = start;
        output.personality = LayerPersonality::TwoLayers;
        MediaConfiguration {
            outputs: vec![output],
            ..Default::default()
        }
    }

    fn state_for(configuration: &MediaConfiguration) -> SharedState {
        Arc::new(ArcSwap::from_pointee(MediaState::with_outputs(
            configuration
                .outputs
                .iter()
                .map(|output| OutputState::new(output.id, output.personality))
                .collect(),
        )))
    }

    /// A universe where the first layer selects folder 1, file 4 at full dimmer.
    fn slots(start: u16) -> Vec<u8> {
        let mut slots = vec![0u8; 512];
        let base = usize::from(start - 1);
        slots[base] = 1; // folder
        slots[base + 1] = 4; // file
        slots[base + 14] = 255; // dimmer
        slots
    }

    #[test]
    fn a_frame_reaches_the_output_patched_to_its_universe() {
        let configuration = configuration(DmxProtocol::ArtNet, 3, 1);
        let state = state_for(&configuration);
        let routes = routes(&configuration);
        let id = configuration.outputs[0].id;

        apply_frame(
            &state,
            &routes,
            &UniverseFrame {
                universe: 3,
                source: media_domain::CommandSource::ArtNet,
                source_label: "desk".to_owned(),
                slots: slots(1),
                received_at: Timestamp::from_millis(0),
            },
        );

        let loaded = state.load();
        let layer = &loaded.output(id).unwrap().layers[0];
        assert_eq!(layer.address, MediaAddress::new(1, 4));
        assert_eq!(layer.dimmer, 1.0);
        assert_eq!(
            loaded.output(id).unwrap().ownership.dmx.unwrap().source,
            media_domain::CommandSource::ArtNet,
            "the first valid frame records the permanent standby transition"
        );
    }

    #[test]
    fn a_frame_on_another_universe_is_ignored() {
        let configuration = configuration(DmxProtocol::ArtNet, 3, 1);
        let state = state_for(&configuration);
        let before = state.load_full();

        apply_frame(
            &state,
            &routes(&configuration),
            &UniverseFrame {
                universe: 9,
                source: media_domain::CommandSource::ArtNet,
                source_label: "desk".to_owned(),
                slots: slots(1),
                received_at: Timestamp::from_millis(0),
            },
        );
        assert!(
            Arc::ptr_eq(&before, &state.load_full()),
            "nothing was published"
        );
    }

    #[test]
    fn an_output_patched_to_sacn_ignores_art_net_on_the_same_universe() {
        let configuration = configuration(DmxProtocol::Sacn, 3, 1);
        let state = state_for(&configuration);
        let before = state.load_full();

        apply_frame(
            &state,
            &routes(&configuration),
            &UniverseFrame {
                universe: 3,
                source: media_domain::CommandSource::ArtNet,
                source_label: "desk".to_owned(),
                slots: slots(1),
                received_at: Timestamp::from_millis(0),
            },
        );
        assert!(Arc::ptr_eq(&before, &state.load_full()));
    }

    #[test]
    fn the_start_address_is_honoured() {
        let configuration = configuration(DmxProtocol::ArtNet, 0, 100);
        let state = state_for(&configuration);
        let id = configuration.outputs[0].id;

        apply_frame(
            &state,
            &routes(&configuration),
            &UniverseFrame {
                universe: 0,
                source: media_domain::CommandSource::ArtNet,
                source_label: "desk".to_owned(),
                slots: slots(100),
                received_at: Timestamp::from_millis(0),
            },
        );
        assert_eq!(
            state.load().output(id).unwrap().layers[0].address,
            MediaAddress::new(1, 4)
        );
    }

    #[test]
    fn a_frame_too_short_for_the_footprint_publishes_nothing() {
        let configuration = configuration(DmxProtocol::ArtNet, 0, 500);
        let state = state_for(&configuration);
        let before = state.load_full();

        apply_frame(
            &state,
            &routes(&configuration),
            &UniverseFrame {
                universe: 0,
                source: media_domain::CommandSource::ArtNet,
                source_label: "desk".to_owned(),
                slots: vec![0u8; 512],
                received_at: Timestamp::from_millis(0),
            },
        );
        assert!(
            Arc::ptr_eq(&before, &state.load_full()),
            "a half-decoded frame is not applied"
        );
    }

    #[test]
    fn both_protocols_produce_the_same_state_from_the_same_slots() {
        let art_net = configuration(DmxProtocol::ArtNet, 3, 1);
        let mut sacn = art_net.clone();
        sacn.outputs[0].protocol = DmxProtocol::Sacn;

        let make = |configuration: &MediaConfiguration, source| {
            let state = state_for(configuration);
            apply_frame(
                &state,
                &routes(configuration),
                &UniverseFrame {
                    universe: 3,
                    source,
                    source_label: "desk".to_owned(),
                    slots: slots(1),
                    received_at: Timestamp::from_millis(0),
                },
            );
            state
                .load()
                .output(configuration.outputs[0].id)
                .unwrap()
                .layers
                .clone()
        };

        assert_eq!(
            make(&art_net, media_domain::CommandSource::ArtNet),
            make(&sacn, media_domain::CommandSource::Sacn),
            "identical payloads reach identical state whichever protocol carried them"
        );
    }

    #[test]
    fn diagnostics_keep_the_exact_footprint_source_rate_and_staleness() {
        let configuration = configuration(DmxProtocol::ArtNet, 3, 100);
        let state = state_for(&configuration);
        let diagnostics = diagnostics();
        let id = configuration.outputs[0].id;
        let mut frame = UniverseFrame {
            universe: 3,
            source: media_domain::CommandSource::ArtNet,
            source_label: "10.0.0.8".to_owned(),
            slots: slots(100),
            received_at: Timestamp::from_millis(100),
        };
        apply_frame_with_diagnostics(&state, &routes(&configuration), &frame, &diagnostics);
        frame.received_at = Timestamp::from_millis(140);
        frame.slots[99] = 7;
        apply_frame_with_diagnostics(&state, &routes(&configuration), &frame, &diagnostics);

        let live = diagnostic_snapshot(&diagnostics, Timestamp::from_millis(200));
        assert_eq!(live[0].output, id);
        assert_eq!(live[0].source, "10.0.0.8");
        assert_eq!(live[0].slots[0], 7);
        assert_eq!(live[0].slots.len(), 75);
        assert!((live[0].frames_per_second - 25.0).abs() < f32::EPSILON);
        assert!(live[0].active);

        let stale = diagnostic_snapshot(&diagnostics, Timestamp::from_millis(3_000));
        assert!(!stale[0].active);
        assert_eq!(stale[0].age_millis, 2_860);
    }
}
