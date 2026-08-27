//! Driving the pixel map from the render loops.
//!
//! Pixel mapping is output, not a preview: it runs whether or not anyone is watching, and it runs
//! at the rate a rig expects rather than the rate a thumbnail does. What it borrows from the
//! preview path is only the readback — the composited frame is already there to be read.

use std::collections::HashMap;

use media_application::configuration::{OutputConfiguration, PixelOutputMode};
use media_domain::OutputId;
use media_domain::pixel_map::{CanvasImage, UniverseFrames, map_pixels};
use media_pixel::PixelSender;

/// How often a mapped universe goes out.
///
/// Forty frames a second is what a lighting rig is used to receiving and what the desk's own output
/// scheduler runs at; sending faster buys nothing a fixture can show.
pub const PIXEL_OUTPUT_FPS: u64 = 40;

/// Whether this output has anything to send at all.
///
/// A map handed to the desk sends nothing from here: the desk owns the sending, and two senders on
/// one universe would fight.
pub fn sends_directly(configuration: &OutputConfiguration) -> bool {
    configuration.enabled
        && configuration.pixel_map.mode == PixelOutputMode::Direct
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
}

impl PixelOutputs {
    /// Whether a frame should be read back for this output now.
    ///
    /// The caller asks before reading rather than after, because the readback is the expensive
    /// part and there is no point paying for it on a frame that will not be sent.
    pub fn wants(&self, configuration: &OutputConfiguration, now_millis: u64) -> bool {
        sends_directly(configuration)
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
    ) -> bool {
        if !sends_directly(configuration) {
            return false;
        }
        if !due(
            self.last_send_millis.get(&configuration.id).copied(),
            now_millis,
        ) {
            return false;
        }
        self.last_send_millis.insert(configuration.id, now_millis);
        let frames = map_pixels(&configuration.pixel_map.zones, image);
        self.deliver(configuration, &frames, instance);
        true
    }

    fn deliver(
        &mut self,
        configuration: &OutputConfiguration,
        frames: &UniverseFrames,
        instance: [u8; 16],
    ) {
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
        self.senders.remove(&id);
        self.last_send_millis.remove(&id);
        self.reported.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_application::configuration::{PixelMapConfiguration, PixelOutputRoute};
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
        assert!(sends_directly(&mapping_output()));
    }

    #[test]
    fn an_output_that_maps_nothing_sends_nothing() {
        assert!(!sends_directly(&OutputConfiguration::new("Main")));
    }

    #[test]
    fn a_map_handed_to_the_desk_sends_nothing_from_here() {
        let mut handed = mapping_output();
        handed.pixel_map.mode = PixelOutputMode::DeskMerge;
        assert!(!sends_directly(&handed));
    }

    #[test]
    fn a_zone_with_no_route_sends_nothing() {
        let mut unrouted = mapping_output();
        unrouted.pixel_map.routes.clear();
        assert!(!sends_directly(&unrouted));
    }

    #[test]
    fn a_disabled_output_sends_nothing() {
        let mut dark = mapping_output();
        dark.enabled = false;
        assert!(!sends_directly(&dark));
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
        assert!(outputs.send(&configuration, image, 0, [0; 16]));
        // Ten milliseconds later is inside the same frame.
        assert!(!outputs.send(&configuration, image, 10, [0; 16]));
        assert!(outputs.send(&configuration, image, 25, [0; 16]));
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
        assert!(!outputs.send(&OutputConfiguration::new("Main"), image, 0, [0; 16]));
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
        assert!(outputs.send(&configuration, image, 0, [0; 16]));
        outputs.forget(configuration.id);
        assert!(outputs.send(&configuration, image, 1, [0; 16]));
    }
}
