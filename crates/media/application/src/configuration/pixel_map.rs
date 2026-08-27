//! The persisted pixel-map and display-region configuration of one output.

use media_domain::display_region::DisplayRegion;
use media_domain::pixel_map::{DMX_SLOTS, PixelZone};
use serde::{Deserialize, Serialize};

/// Where a zone's mapped values go once they leave the sampler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelOutputMode {
    /// The Media Server sends the mapped values itself. No desk is involved.
    #[default]
    Direct,
    /// The values are handed to the lighting desk, which merges them with the zone's own Dimmer
    /// and Mix channels before anything reaches the rig.
    DeskMerge,
}

/// One route the mapped universes are sent on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PixelOutputRoute {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub protocol: super::DmxProtocol,
    /// The universe this route carries, matching the universe a zone addresses.
    pub universe: u16,
    /// Where the packets go. Absent means the protocol's own default — broadcast for Art-Net,
    /// the universe's multicast group for sACN.
    #[serde(default)]
    pub destination: Option<String>,
    #[serde(default = "enabled")]
    pub enabled: bool,
}

const fn enabled() -> bool {
    true
}

/// Everything one output maps and shows.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PixelMapConfiguration {
    #[serde(default)]
    pub mode: PixelOutputMode,
    #[serde(default)]
    pub zones: Vec<PixelZone>,
    #[serde(default)]
    pub routes: Vec<PixelOutputRoute>,
    /// The screens this output's canvas is divided across. Empty means the whole canvas on the one
    /// screen, which is what every output did before regions existed.
    #[serde(default)]
    pub regions: Vec<DisplayRegion>,
}

impl PixelMapConfiguration {
    pub fn is_empty(&self) -> bool {
        self.zones.is_empty() && self.regions.is_empty()
    }

    /// The universes the enabled zones address.
    pub fn universes(&self) -> Vec<u16> {
        let mut universes: Vec<u16> = self
            .zones
            .iter()
            .filter(|zone| zone.enabled)
            .map(|zone| zone.universe)
            .collect();
        universes.sort_unstable();
        universes.dedup();
        universes
    }
}

/// The last slot a zone occupies, one-based, or `None` when it runs past the end of a universe.
pub fn zone_last_address(zone: &PixelZone) -> Option<u16> {
    let footprint = u16::try_from(zone.footprint()).ok()?;
    let last = zone.start_address.checked_add(footprint.checked_sub(1)?)?;
    (zone.start_address >= 1 && usize::from(last) <= DMX_SLOTS).then_some(last)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use media_domain::pixel_map::{CanvasPoint, PixelLayout, PixelOrder};

    pub(crate) fn zone(id: &str, universe: u16, start_address: u16, pixels: u32) -> PixelZone {
        PixelZone {
            id: id.into(),
            name: id.into(),
            start: CanvasPoint::new(0.0, 0.0),
            end: CanvasPoint::new(1.0, 1.0),
            columns: pixels,
            rows: 1,
            layout: PixelLayout::rgb(),
            order: PixelOrder::RowMajor,
            universe,
            start_address,
            enabled: true,
        }
    }

    #[test]
    fn a_zone_reports_the_last_slot_it_occupies() {
        // Ten RGB pixels from address one end at thirty.
        assert_eq!(zone_last_address(&zone("strip", 1, 1, 10)), Some(30));
    }

    #[test]
    fn a_zone_that_runs_past_the_universe_has_no_last_slot() {
        assert_eq!(zone_last_address(&zone("strip", 1, 500, 10)), None);
        assert_eq!(zone_last_address(&zone("strip", 1, 0, 1)), None);
    }

    #[test]
    fn a_map_lists_the_universes_its_enabled_zones_address() {
        let mut off = zone("off", 9, 1, 4);
        off.enabled = false;
        let map = PixelMapConfiguration {
            zones: vec![
                zone("a", 3, 1, 4),
                zone("b", 1, 1, 4),
                zone("c", 3, 100, 4),
                off,
            ],
            ..PixelMapConfiguration::default()
        };
        assert_eq!(map.universes(), vec![1, 3]);
    }

    #[test]
    fn an_output_maps_nothing_until_it_is_given_a_zone_or_a_region() {
        assert!(PixelMapConfiguration::default().is_empty());
        assert_eq!(
            PixelMapConfiguration::default().mode,
            PixelOutputMode::Direct
        );
    }
}

#[cfg(test)]
mod compatibility_tests {
    use crate::configuration::{load, save};

    /// The same document with every output's pixel map taken back out of it.
    fn strip_pixel_map(mut document: serde_json::Value) -> String {
        if let Some(outputs) = document
            .get_mut("configuration")
            .and_then(|configuration| configuration.get_mut("outputs"))
            .and_then(serde_json::Value::as_array_mut)
        {
            for output in outputs {
                if let Some(output) = output.as_object_mut() {
                    output.remove("pixelMap");
                }
            }
        }
        document.to_string()
    }

    /// A document written before pixel mapping existed still loads, and comes back with nothing
    /// mapped rather than failing on an absent field.
    #[test]
    fn a_configuration_from_before_pixel_mapping_still_loads() {
        let before = save(&crate::configuration::MediaConfiguration::default());
        let document: serde_json::Value =
            serde_json::from_str(&before).expect("the document parses");
        let stripped = strip_pixel_map(document);
        assert!(
            !stripped.contains("pixelMap"),
            "the fixture still mentions the new field"
        );
        let loaded = load(&stripped).expect("an older document loads");
        let output = loaded.outputs.first().expect("its one output");
        assert!(output.pixel_map.is_empty());
        assert!(output.pixel_map.routes.is_empty());
    }

    /// What is written comes back the same, zones and regions included.
    #[test]
    fn a_pixel_map_survives_being_written_and_read() {
        use super::tests::zone;
        use media_domain::display_region::DisplayRegion;
        let mut configuration = crate::configuration::MediaConfiguration::default();
        let output = configuration.outputs.first_mut().expect("one output");
        output.pixel_map.zones = vec![zone("strip", 1, 1, 12)];
        output.pixel_map.routes = vec![super::PixelOutputRoute {
            id: "route-1".into(),
            name: "Universe 1".into(),
            protocol: crate::configuration::DmxProtocol::default(),
            universe: 1,
            destination: None,
            enabled: true,
        }];
        output.pixel_map.regions = vec![DisplayRegion::whole("main", "HDMI 1")];
        let restored = load(&save(&configuration)).expect("the document round-trips");
        assert_eq!(restored, configuration);
    }
}
