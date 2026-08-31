//! Driving the pixel map from the render loops.
//!
//! Pixel mapping is output, not a preview: it runs whether or not anyone is watching, and it runs
//! at the rate a rig expects rather than the rate a thumbnail does. What it borrows from the
//! preview path is only the readback — the composited frame is already there to be read.

use std::collections::HashMap;

use media_application::configuration::{OutputConfiguration, PixelOutputMode, PixelOutputRoute};
use media_domain::OutputId;
use media_domain::pixel_map::{
    CanvasImage, UniverseFrames, ZoneMergeControls, map_pixels, merge_slot,
};
use media_pixel::PixelSender;

/// How often a mapped universe goes out.
///
/// Forty frames a second is what a lighting rig is used to receiving and what the desk's own output
/// scheduler runs at; sending faster buys nothing a fixture can show.
pub const PIXEL_OUTPUT_FPS: u64 = 40;
pub const DESK_SOURCE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(2_500);

/// Whether this output has anything to send at all.
///
/// In both modes Media Server owns the physical output. Desk-merge changes the values, not the
/// sender.
pub fn sends_pixels(configuration: &OutputConfiguration) -> bool {
    configuration.enabled
        && configuration
            .pixel_map
            .zones
            .iter()
            .any(|zone| zone.enabled)
        && configuration
            .pixel_map
            .routes
            .iter()
            .any(|route| route.enabled)
}

/// A stable sACN source identity for one server.
///
/// sACN receivers tell sources apart by their CID and expect it to survive a restart, so it is
/// derived from the configured instance identity rather than generated afresh each time.
pub fn instance_cid(instance_id: &str) -> [u8; 16] {
    use std::hash::{Hash, Hasher};
    let mut cid = [0_u8; 16];
    for (half, salt) in [(0_usize, 0x5a5a_5a5a_u64), (8, 0xa5a5_a5a5)] {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        salt.hash(&mut hasher);
        instance_id.hash(&mut hasher);
        cid[half..half + 8].copy_from_slice(&hasher.finish().to_be_bytes());
    }
    cid
}

/// Whether this instant should be sent, given when the last one was.
pub const fn due(last_send_millis: Option<u64>, now_millis: u64) -> bool {
    match last_send_millis {
        None => true,
        Some(last) => now_millis.saturating_sub(last) >= 1_000 / PIXEL_OUTPUT_FPS,
    }
}

/// The pixel senders of every output that has one.
#[derive(Default)]
pub struct PixelOutputs {
    senders: HashMap<OutputId, PixelSender>,
    last_send_millis: HashMap<OutputId, u64>,
    /// Failures already reported, so a rig that is simply unplugged does not fill the log at forty
    /// lines a second.
    reported: HashMap<OutputId, String>,
    input_live: HashMap<String, bool>,
    shutdown: HashMap<OutputId, (Vec<PixelOutputRoute>, Vec<u16>)>,
}

impl PixelOutputs {
    /// Whether a frame should be read back for this output now.
    ///
    /// The caller asks before reading rather than after, because the readback is the expensive
    /// part and there is no point paying for it on a frame that will not be sent.
    pub fn wants(&self, configuration: &OutputConfiguration, now_millis: u64) -> bool {
        sends_pixels(configuration)
            && due(
                self.last_send_millis.get(&configuration.id).copied(),
                now_millis,
            )
    }

    /// Maps the composited frame and sends it, if this output sends and is due.
    ///
    /// Returns whether anything was sent, so the caller knows whether the readback it did was used.
    pub fn send(
        &mut self,
        configuration: &OutputConfiguration,
        image: CanvasImage<'_>,
        now_millis: u64,
        instance: [u8; 16],
        inputs: &crate::dmx::SharedUniverseInputs,
    ) -> bool {
        if !sends_pixels(configuration) {
            return false;
        }
        if !due(
            self.last_send_millis.get(&configuration.id).copied(),
            now_millis,
        ) {
            return false;
        }
        self.last_send_millis.insert(configuration.id, now_millis);
        let mapped = map_pixels(&configuration.pixel_map.zones, image);
        let frames = self.output_frames(configuration, &mapped, inputs);
        self.deliver(configuration, &frames, instance);
        true
    }

    fn output_frames(
        &mut self,
        configuration: &OutputConfiguration,
        mapped: &UniverseFrames,
        inputs: &crate::dmx::SharedUniverseInputs,
    ) -> UniverseFrames {
        if configuration.pixel_map.mode == PixelOutputMode::Direct {
            return mapped.clone();
        }
        let mut output = UniverseFrames::default();
        for zone in configuration
            .pixel_map
            .zones
            .iter()
            .filter(|zone| zone.enabled)
        {
            output.open(zone.universe);
            let Some(handoff) = configuration
                .pixel_map
                .handoffs
                .iter()
                .find(|handoff| handoff.zone_id == zone.id)
            else {
                continue;
            };
            let desk = crate::dmx::fresh_universe(
                inputs,
                handoff.protocol,
                handoff.input_universe,
                DESK_SOURCE_TIMEOUT,
            );
            self.report_source_transition(&zone.id, &zone.name, desk.is_some());
            let Some(desk) = desk else {
                continue; // fail closed: the opened zone stays black until one fresh full frame
            };
            let dimmer = desk[usize::from(handoff.dimmer_address - 1)];
            let mix = desk[usize::from(handoff.mix_address - 1)];
            let controls = ZoneMergeControls { dimmer, mix };
            let input = usize::from(handoff.input_start_address - 1);
            let output_start = zone.start_address;
            let pixel_footprint = zone.footprint();
            for offset in 0..usize::from(handoff.fixture_footprint) {
                let desk_value = desk[input + offset];
                let value = if offset < pixel_footprint {
                    let media = mapped
                        .get(zone.universe)
                        .map_or(0, |frame| frame[usize::from(output_start - 1) + offset]);
                    merge_slot(media, desk_value, controls)
                } else {
                    desk_value
                };
                output.write(
                    zone.universe,
                    output_start.saturating_add(offset as u16),
                    value,
                );
            }
        }
        output
    }

    fn report_source_transition(&mut self, id: &str, name: &str, live: bool) {
        let previous = self.input_live.insert(id.to_owned(), live);
        if previous == Some(live) {
            return;
        }
        if live {
            tracing::info!(zone = name, "desk input recovered; pixel handoff resumed");
        } else {
            tracing::warn!(
                zone = name,
                "desk input unavailable; pixel handoff is blacked out"
            );
        }
    }

    fn deliver(
        &mut self,
        configuration: &OutputConfiguration,
        frames: &UniverseFrames,
        instance: [u8; 16],
    ) {
        self.shutdown.insert(
            configuration.id,
            (
                configuration.pixel_map.routes.clone(),
                frames.universes().collect(),
            ),
        );
        let sender = match self.senders.entry(configuration.id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                match PixelSender::bind(instance, configuration.name.to_string()) {
                    Ok(sender) => entry.insert(sender),
                    Err(error) => {
                        self.report(configuration.id, &error.to_string());
                        return;
                    }
                }
            }
        };
        let failures = sender.send(&configuration.pixel_map.routes, frames);
        match failures.first() {
            Some(failure) => self.report(configuration.id, &failure.to_string()),
            None => {
                self.reported.remove(&configuration.id);
            }
        }
    }

    /// Logs a failure the first time it is seen, and stays quiet while it persists.
    fn report(&mut self, id: OutputId, message: &str) {
        if self.reported.get(&id).is_some_and(|seen| seen == message) {
            return;
        }
        tracing::warn!(output = %id, "pixel output: {message}");
        self.reported.insert(id, message.to_owned());
    }

    /// Forgets an output that has gone away, closing its socket with it.
    pub fn forget(&mut self, id: OutputId) {
        if let (Some(mut sender), Some((routes, universes))) =
            (self.senders.remove(&id), self.shutdown.remove(&id))
        {
            send_blackout(&mut sender, &routes, &universes);
        }
        self.last_send_millis.remove(&id);
        self.reported.remove(&id);
    }
}

fn send_blackout(sender: &mut PixelSender, routes: &[PixelOutputRoute], universes: &[u16]) {
    let mut black = UniverseFrames::default();
    for universe in universes {
        black.open(*universe);
    }
    let _ = sender.send(routes, &black);
}

impl Drop for PixelOutputs {
    fn drop(&mut self) {
        for (id, mut sender) in self.senders.drain() {
            if let Some((routes, universes)) = self.shutdown.remove(&id) {
                send_blackout(&mut sender, &routes, &universes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_application::configuration::{
        DmxProtocol, PixelMapConfiguration, PixelOutputRoute, PixelZoneHandoff,
    };
    use media_domain::pixel_map::{CanvasPoint, PixelLayout, PixelOrder, PixelZone};

    fn zone() -> PixelZone {
        PixelZone {
            id: "strip".into(),
            name: "Strip".into(),
            start: CanvasPoint::new(0.0, 0.0),
            end: CanvasPoint::new(1.0, 1.0),
            columns: 2,
            rows: 1,
            layout: PixelLayout::rgb(),
            order: PixelOrder::RowMajor,
            universe: 1,
            start_address: 1,
            enabled: true,
        }
    }

    fn route() -> PixelOutputRoute {
        PixelOutputRoute {
            id: "route".into(),
            name: "Universe 1".into(),
            protocol: media_application::configuration::DmxProtocol::default(),
            universe: 1,
            destination: None,
            enabled: true,
        }
    }

    fn mapping_output() -> OutputConfiguration {
        OutputConfiguration {
            pixel_map: PixelMapConfiguration {
                zones: vec![zone()],
                routes: vec![route()],
                ..PixelMapConfiguration::default()
            },
            ..OutputConfiguration::new("Main")
        }
    }

    #[test]
    fn an_output_with_a_direct_zone_and_a_route_sends() {
        assert!(sends_pixels(&mapping_output()));
    }

    #[test]
    fn an_output_that_maps_nothing_sends_nothing() {
        assert!(!sends_pixels(&OutputConfiguration::new("Main")));
    }

    #[test]
    fn a_desk_merge_map_is_still_sent_by_media_server() {
        let mut handed = mapping_output();
        handed.pixel_map.mode = PixelOutputMode::DeskMerge;
        assert!(sends_pixels(&handed));
    }

    #[test]
    fn a_zone_with_no_route_sends_nothing() {
        let mut unrouted = mapping_output();
        unrouted.pixel_map.routes.clear();
        assert!(!sends_pixels(&unrouted));
    }

    #[test]
    fn a_disabled_output_sends_nothing() {
        let mut dark = mapping_output();
        dark.enabled = false;
        assert!(!sends_pixels(&dark));
    }

    #[test]
    fn a_server_keeps_the_same_sacn_identity_across_restarts() {
        assert_eq!(instance_cid("media"), instance_cid("media"));
        assert_ne!(instance_cid("media"), instance_cid("media-2"));
        // Both halves are filled, rather than eight bytes of zero.
        assert!(instance_cid("media")[8..].iter().any(|byte| *byte != 0));
    }

    #[test]
    fn the_first_frame_is_always_due_and_the_next_waits_its_turn() {
        assert!(due(None, 0));
        assert!(!due(Some(1_000), 1_010));
        assert!(due(Some(1_000), 1_025));
    }

    #[test]
    fn sending_is_rate_limited_per_output() {
        let mut outputs = PixelOutputs::default();
        let configuration = mapping_output();
        let rgba = vec![255; 4 * 4];
        let image = CanvasImage {
            width: 2,
            height: 2,
            rgba: &rgba,
        };
        let inputs = crate::dmx::universe_inputs();
        assert!(outputs.send(&configuration, image, 0, [0; 16], &inputs));
        // Ten milliseconds later is inside the same frame.
        assert!(!outputs.send(&configuration, image, 10, [0; 16], &inputs));
        assert!(outputs.send(&configuration, image, 25, [0; 16], &inputs));
    }

    #[test]
    fn an_output_that_maps_nothing_is_never_sampled() {
        let mut outputs = PixelOutputs::default();
        let rgba = vec![255; 4 * 4];
        let image = CanvasImage {
            width: 2,
            height: 2,
            rgba: &rgba,
        };
        assert!(!outputs.send(
            &OutputConfiguration::new("Main"),
            image,
            0,
            [0; 16],
            &crate::dmx::universe_inputs(),
        ));
    }

    #[test]
    fn forgetting_an_output_lets_its_next_frame_send_at_once() {
        let mut outputs = PixelOutputs::default();
        let configuration = mapping_output();
        let rgba = vec![255; 4 * 4];
        let image = CanvasImage {
            width: 2,
            height: 2,
            rgba: &rgba,
        };
        let inputs = crate::dmx::universe_inputs();
        assert!(outputs.send(&configuration, image, 0, [0; 16], &inputs));
        outputs.forget(configuration.id);
        assert!(outputs.send(&configuration, image, 1, [0; 16], &inputs));
    }

    fn merge_output() -> OutputConfiguration {
        let mut output = mapping_output();
        output.pixel_map.mode = PixelOutputMode::DeskMerge;
        output.pixel_map.zones[0].start_address = 20;
        output.pixel_map.handoffs = vec![PixelZoneHandoff {
            zone_id: "strip".into(),
            fixture_name: "Strip fixture".into(),
            protocol: DmxProtocol::ArtNet,
            input_universe: 7,
            input_start_address: 3,
            dimmer_address: 1,
            mix_address: 2,
            fixture_footprint: 8,
            automatic_patch: false,
        }];
        output
    }

    fn mapped_red(output: &OutputConfiguration) -> UniverseFrames {
        map_pixels(
            &output.pixel_map.zones,
            CanvasImage {
                width: 2,
                height: 1,
                rgba: &[200, 0, 0, 255, 100, 0, 0, 255],
            },
        )
    }

    #[test]
    fn merge_remaps_desk_pixels_and_passes_extra_fixture_channels_through() {
        let output = merge_output();
        let inputs = crate::dmx::universe_inputs();
        let mut desk = [0; 512];
        desk[0] = 255; // Dimmer
        desk[1] = 254; // desk endpoint
        desk[2..8].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
        desk[8] = 77;
        desk[9] = 88;
        crate::dmx::remember_universe_for_test(
            &inputs,
            DmxProtocol::ArtNet,
            7,
            desk,
            std::time::Duration::ZERO,
        );
        let frames = PixelOutputs::default().output_frames(&output, &mapped_red(&output), &inputs);
        assert_eq!(&frames.get(1).unwrap()[19..27], &[1, 2, 3, 4, 5, 6, 77, 88]);
    }

    #[test]
    fn lost_or_stale_desk_input_fails_closed_and_a_fresh_frame_recovers() {
        let output = merge_output();
        let inputs = crate::dmx::universe_inputs();
        let mapped = mapped_red(&output);
        let black = PixelOutputs::default().output_frames(&output, &mapped, &inputs);
        assert!(black.get(1).unwrap()[19..27].iter().all(|slot| *slot == 0));

        let mut desk = [0; 512];
        desk[0] = 255;
        desk[1] = 0; // Media endpoint
        crate::dmx::remember_universe_for_test(
            &inputs,
            DmxProtocol::ArtNet,
            7,
            desk,
            DESK_SOURCE_TIMEOUT + std::time::Duration::from_millis(1),
        );
        let stale = PixelOutputs::default().output_frames(&output, &mapped, &inputs);
        assert!(stale.get(1).unwrap()[19..27].iter().all(|slot| *slot == 0));

        crate::dmx::remember_universe_for_test(
            &inputs,
            DmxProtocol::ArtNet,
            7,
            desk,
            std::time::Duration::ZERO,
        );
        let recovered = PixelOutputs::default().output_frames(&output, &mapped, &inputs);
        assert_eq!(&recovered.get(1).unwrap()[19..25], &[200, 0, 0, 100, 0, 0]);
    }
}
