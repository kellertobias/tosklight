//! Configuration validation.
//!
//! Everything here runs before a subsystem starts. A configuration that cannot be honored stops
//! startup with a message that names the output and the field, rather than letting the process
//! come up in a state the operator did not ask for.

use std::collections::HashSet;

use media_domain::PresentationMode;
use media_domain::personality::StartAddressError;

use super::{
    DmxProtocol, MediaConfiguration, OutputConfiguration, PixelOutputMode,
    migration::MigrationError, zone_last_address,
};

/// Why a configuration cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigurationError {
    #[error("the configuration document is not readable: {detail}")]
    Malformed { detail: String },
    #[error(transparent)]
    Migration(#[from] MigrationError),
    #[error("the configuration defines no outputs; a Media Server needs at least one")]
    NoOutputs,
    #[error("two outputs share the identity {id}")]
    DuplicateOutputId { id: String },
    #[error("output '{output}' has an invalid DMX start address: {source}")]
    StartAddress {
        output: String,
        #[source]
        source: StartAddressError,
    },
    #[error("output '{output}' has a zero {axis} resolution")]
    EmptyResolution { output: String, axis: &'static str },
    #[error(
        "output '{output}' requests an invalid fixed frame rate; it must be between 1 and 65535"
    )]
    InvalidFixedRate { output: String },
    #[error(
        "output '{output}' requests {protocol} universe {universe}; {protocol} universes are {range}"
    )]
    UniverseOutOfRange {
        output: String,
        protocol: &'static str,
        universe: u16,
        range: &'static str,
    },
    #[error("the UTC offset {minutes} minutes is outside the range any timezone uses")]
    InvalidUtcOffset { minutes: i16 },
    #[error(
        "outputs '{first}' and '{second}' both consume universe {universe} at address {start_address}"
    )]
    OverlappingPatch {
        first: String,
        second: String,
        universe: u16,
        start_address: u16,
    },
    #[error("output '{output}' has two pixel zones with the identity {id}")]
    DuplicateZoneId { output: String, id: String },
    #[error("output '{output}' has two pixel output routes with the identity {id}")]
    DuplicatePixelRouteId { output: String, id: String },
    #[error("outputs '{first}' and '{second}' both send complete {protocol} universe {universe}")]
    PixelRouteCollision {
        first: String,
        second: String,
        protocol: &'static str,
        universe: u16,
    },
    #[error(
        "pixel zone '{zone}' on output '{output}' has no pixels; give it at least one column and one row"
    )]
    EmptyZone { output: String, zone: String },
    #[error("pixel zone '{zone}' on output '{output}' has no colour channels to send")]
    EmptyLayout { output: String, zone: String },
    #[error(
        "pixel zone '{zone}' on output '{output}' needs {footprint} slots from address {start_address}, which runs past the end of universe {universe}"
    )]
    ZoneOverrunsUniverse {
        output: String,
        zone: String,
        universe: u16,
        start_address: u16,
        footprint: usize,
    },
    #[error(
        "pixel zones '{first}' and '{second}' on output '{output}' both use universe {universe} around address {address}"
    )]
    OverlappingZones {
        output: String,
        first: String,
        second: String,
        universe: u16,
        address: u16,
    },
    #[error(
        "pixel zone '{zone}' on output '{output}' sends universe {universe}, which no enabled output route carries"
    )]
    UnroutedZone {
        output: String,
        zone: String,
        universe: u16,
    },
    #[error("pixel zone '{zone}' on output '{output}' has no desk input patch")]
    MissingZoneHandoff { output: String, zone: String },
    #[error("output '{output}' has more than one desk input patch for pixel zone {zone_id}")]
    DuplicateZoneHandoff { output: String, zone_id: String },
    #[error("output '{output}' has a desk input patch for unknown pixel zone {zone_id}")]
    UnknownZoneHandoff { output: String, zone_id: String },
    #[error("output '{output}' is in direct mode but still contains desk input patches")]
    DirectModeHasHandoffs { output: String },
    #[error(
        "pixel zone '{zone}' on output '{output}' has an invalid desk {field} address {address}"
    )]
    InvalidHandoffAddress {
        output: String,
        zone: String,
        field: &'static str,
        address: u16,
    },
    #[error(
        "pixel zone '{zone}' on output '{output}' has fixture footprint {fixture_footprint}, smaller than its {pixel_footprint} pixel slots"
    )]
    HandoffFootprintTooSmall {
        output: String,
        zone: String,
        fixture_footprint: u16,
        pixel_footprint: usize,
    },
    #[error("output '{output}' has two display regions with the identity {id}")]
    DuplicateRegionId { output: String, id: String },
    #[error("display region '{region}' on output '{output}' covers none of the canvas")]
    EmptyRegion { output: String, region: String },
}

pub(super) fn validate(configuration: &MediaConfiguration) -> Result<(), ConfigurationError> {
    if configuration.outputs.is_empty() {
        return Err(ConfigurationError::NoOutputs);
    }
    if !configuration.time.is_valid() {
        return Err(ConfigurationError::InvalidUtcOffset {
            minutes: configuration.time.utc_offset_minutes,
        });
    }

    let mut seen = HashSet::new();
    for output in &configuration.outputs {
        if !seen.insert(output.id) {
            return Err(ConfigurationError::DuplicateOutputId {
                id: output.id.to_string(),
            });
        }
        validate_output(output)?;
        validate_pixel_map(output)?;
    }

    validate_patch_overlap(&configuration.outputs)?;
    validate_pixel_route_collisions(&configuration.outputs)
}

/// Everything a pixel map has to satisfy before the sampler is allowed to run against it.
fn validate_pixel_map(output: &OutputConfiguration) -> Result<(), ConfigurationError> {
    let name = output.name.to_string();
    let map = &output.pixel_map;

    let mut seen = HashSet::new();
    for zone in &map.zones {
        if !seen.insert(zone.id.clone()) {
            return Err(ConfigurationError::DuplicateZoneId {
                output: name,
                id: zone.id.clone(),
            });
        }
        if zone.pixel_count() == 0 {
            return Err(ConfigurationError::EmptyZone {
                output: name,
                zone: zone.name.clone(),
            });
        }
        if zone.layout.footprint() == 0 {
            return Err(ConfigurationError::EmptyLayout {
                output: name,
                zone: zone.name.clone(),
            });
        }
        if zone_last_address(zone).is_none() {
            return Err(ConfigurationError::ZoneOverrunsUniverse {
                output: name,
                zone: zone.name.clone(),
                universe: zone.universe,
                start_address: zone.start_address,
                footprint: zone.footprint(),
            });
        }
    }

    validate_zone_overlap(&name, output)?;
    validate_pixel_routes(&name, output)?;
    validate_zone_routes(&name, output)?;
    validate_handoffs(&name, output)?;

    let mut regions = HashSet::new();
    for region in &map.regions {
        if !regions.insert(region.id.clone()) {
            return Err(ConfigurationError::DuplicateRegionId {
                output: name,
                id: region.id.clone(),
            });
        }
        if region.source.width() <= 0.0 || region.source.height() <= 0.0 {
            return Err(ConfigurationError::EmptyRegion {
                output: name,
                region: region.name.clone(),
            });
        }
    }
    Ok(())
}

fn validate_pixel_routes(
    output_name: &str,
    output: &OutputConfiguration,
) -> Result<(), ConfigurationError> {
    let mut ids = HashSet::new();
    for route in &output.pixel_map.routes {
        if !ids.insert(route.id.as_str()) {
            return Err(ConfigurationError::DuplicatePixelRouteId {
                output: output_name.to_owned(),
                id: route.id.clone(),
            });
        }
        match route.protocol {
            DmxProtocol::ArtNet if route.universe > 32_767 => {
                return Err(ConfigurationError::UniverseOutOfRange {
                    output: format!("{output_name} pixel output route"),
                    protocol: "Art-Net",
                    universe: route.universe,
                    range: "0 through 32767",
                });
            }
            DmxProtocol::Sacn if !(1..=63_999).contains(&route.universe) => {
                return Err(ConfigurationError::UniverseOutOfRange {
                    output: format!("{output_name} pixel output route"),
                    protocol: "sACN",
                    universe: route.universe,
                    range: "1 through 63999",
                });
            }
            DmxProtocol::ArtNet | DmxProtocol::Sacn => {}
        }
    }
    Ok(())
}

fn validate_pixel_route_collisions(
    outputs: &[OutputConfiguration],
) -> Result<(), ConfigurationError> {
    let mut occupied: HashSet<(DmxProtocol, u16)> = HashSet::new();
    let mut owners = std::collections::HashMap::new();
    for output in outputs
        .iter()
        .filter(|output| output.enabled && output.pixel_map.zones.iter().any(|zone| zone.enabled))
    {
        for route in output.pixel_map.routes.iter().filter(|route| route.enabled) {
            let key = (route.protocol, route.universe);
            if !occupied.insert(key) {
                return Err(ConfigurationError::PixelRouteCollision {
                    first: owners
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| "another output".to_owned()),
                    second: output.name.to_string(),
                    protocol: match route.protocol {
                        DmxProtocol::ArtNet => "Art-Net",
                        DmxProtocol::Sacn => "sACN",
                    },
                    universe: route.universe,
                });
            }
            owners.insert(key, output.name.to_string());
        }
    }
    Ok(())
}

/// Two zones may share a universe, but not a slot within it.
fn validate_zone_overlap(
    output_name: &str,
    output: &OutputConfiguration,
) -> Result<(), ConfigurationError> {
    let enabled: Vec<_> = output
        .pixel_map
        .zones
        .iter()
        .filter(|zone| zone.enabled)
        .collect();
    for (index, zone) in enabled.iter().enumerate() {
        let Some(last) = output_zone_last_address(output, zone) else {
            continue;
        };
        for other in enabled.iter().skip(index + 1) {
            let Some(other_last) = output_zone_last_address(output, other) else {
                continue;
            };
            if zone.universe != other.universe {
                continue;
            }
            let overlaps = zone.start_address <= other_last && other.start_address <= last;
            if overlaps {
                return Err(ConfigurationError::OverlappingZones {
                    output: output_name.to_string(),
                    first: zone.name.clone(),
                    second: other.name.clone(),
                    universe: zone.universe,
                    address: zone.start_address.max(other.start_address),
                });
            }
        }
    }
    Ok(())
}

fn output_zone_last_address(
    output: &OutputConfiguration,
    zone: &media_domain::pixel_map::PixelZone,
) -> Option<u16> {
    let footprint = if output.pixel_map.mode == PixelOutputMode::DeskMerge {
        output
            .pixel_map
            .handoffs
            .iter()
            .find(|handoff| handoff.zone_id == zone.id)
            .map_or(zone.footprint(), |handoff| {
                usize::from(handoff.fixture_footprint)
            })
    } else {
        zone.footprint()
    };
    let footprint = u16::try_from(footprint).ok()?;
    let last = zone.start_address.checked_add(footprint.checked_sub(1)?)?;
    (zone.start_address >= 1 && last <= 512).then_some(last)
}

/// Every enabled zone needs an output route. In desk-merge mode Media sends the merged result.
fn validate_zone_routes(
    output_name: &str,
    output: &OutputConfiguration,
) -> Result<(), ConfigurationError> {
    let carried: HashSet<u16> = output
        .pixel_map
        .routes
        .iter()
        .filter(|route| route.enabled)
        .map(|route| route.universe)
        .collect();
    for zone in output.pixel_map.zones.iter().filter(|zone| zone.enabled) {
        if !carried.contains(&zone.universe) {
            return Err(ConfigurationError::UnroutedZone {
                output: output_name.to_string(),
                zone: zone.name.clone(),
                universe: zone.universe,
            });
        }
    }
    Ok(())
}

fn validate_handoffs(
    output_name: &str,
    output: &OutputConfiguration,
) -> Result<(), ConfigurationError> {
    let zone_ids: HashSet<&str> = output
        .pixel_map
        .zones
        .iter()
        .map(|zone| zone.id.as_str())
        .collect();
    if output.pixel_map.mode == PixelOutputMode::Direct && !output.pixel_map.handoffs.is_empty() {
        return Err(ConfigurationError::DirectModeHasHandoffs {
            output: output_name.to_owned(),
        });
    }
    let mut seen = HashSet::new();
    for handoff in &output.pixel_map.handoffs {
        if !zone_ids.contains(handoff.zone_id.as_str()) {
            return Err(ConfigurationError::UnknownZoneHandoff {
                output: output_name.to_owned(),
                zone_id: handoff.zone_id.clone(),
            });
        }
        if !seen.insert(handoff.zone_id.as_str()) {
            return Err(ConfigurationError::DuplicateZoneHandoff {
                output: output_name.to_owned(),
                zone_id: handoff.zone_id.clone(),
            });
        }
    }
    if output.pixel_map.mode != PixelOutputMode::DeskMerge {
        return Ok(());
    }
    for zone in output.pixel_map.zones.iter().filter(|zone| zone.enabled) {
        let Some(handoff) = output
            .pixel_map
            .handoffs
            .iter()
            .find(|handoff| handoff.zone_id == zone.id)
        else {
            return Err(ConfigurationError::MissingZoneHandoff {
                output: output_name.to_owned(),
                zone: zone.name.clone(),
            });
        };
        for (field, address) in [
            ("input start", handoff.input_start_address),
            ("Dimmer", handoff.dimmer_address),
            ("Mix", handoff.mix_address),
        ] {
            if !(1..=512).contains(&address) {
                return Err(ConfigurationError::InvalidHandoffAddress {
                    output: output_name.to_owned(),
                    zone: zone.name.clone(),
                    field,
                    address,
                });
            }
        }
        if handoff.dimmer_address == handoff.mix_address {
            return Err(ConfigurationError::InvalidHandoffAddress {
                output: output_name.to_owned(),
                zone: zone.name.clone(),
                field: "Mix (same as Dimmer)",
                address: handoff.mix_address,
            });
        }
        match handoff.protocol {
            DmxProtocol::ArtNet if handoff.input_universe > 32_767 => {
                return Err(ConfigurationError::UniverseOutOfRange {
                    output: format!("{output_name} / {} desk input", zone.name),
                    protocol: "Art-Net",
                    universe: handoff.input_universe,
                    range: "0 through 32767",
                });
            }
            DmxProtocol::Sacn if !(1..=63_999).contains(&handoff.input_universe) => {
                return Err(ConfigurationError::UniverseOutOfRange {
                    output: format!("{output_name} / {} desk input", zone.name),
                    protocol: "sACN",
                    universe: handoff.input_universe,
                    range: "1 through 63999",
                });
            }
            DmxProtocol::ArtNet | DmxProtocol::Sacn => {}
        }
        if usize::from(handoff.fixture_footprint) < zone.footprint() {
            return Err(ConfigurationError::HandoffFootprintTooSmall {
                output: output_name.to_owned(),
                zone: zone.name.clone(),
                fixture_footprint: handoff.fixture_footprint,
                pixel_footprint: zone.footprint(),
            });
        }
        let last_input = usize::from(handoff.input_start_address)
            .saturating_add(usize::from(handoff.fixture_footprint))
            .saturating_sub(1);
        if last_input > 512 {
            return Err(ConfigurationError::InvalidHandoffAddress {
                output: output_name.to_owned(),
                zone: zone.name.clone(),
                field: "fixture footprint",
                address: last_input.min(usize::from(u16::MAX)) as u16,
            });
        }
        let physical = usize::from(handoff.input_start_address)..=last_input;
        for (field, address) in [
            ("Dimmer (inside pixel fixture span)", handoff.dimmer_address),
            ("Mix (inside pixel fixture span)", handoff.mix_address),
        ] {
            if physical.contains(&usize::from(address)) {
                return Err(ConfigurationError::InvalidHandoffAddress {
                    output: output_name.to_owned(),
                    zone: zone.name.clone(),
                    field,
                    address,
                });
            }
        }
        let last_output = usize::from(zone.start_address)
            .saturating_add(usize::from(handoff.fixture_footprint))
            .saturating_sub(1);
        if last_output > 512 {
            return Err(ConfigurationError::InvalidHandoffAddress {
                output: output_name.to_owned(),
                zone: zone.name.clone(),
                field: "Media Server output footprint",
                address: last_output.min(usize::from(u16::MAX)) as u16,
            });
        }
    }
    Ok(())
}

fn validate_output(output: &OutputConfiguration) -> Result<(), ConfigurationError> {
    let name = output.name.to_string();

    match output.protocol {
        DmxProtocol::ArtNet if output.universe > 32_767 => {
            return Err(ConfigurationError::UniverseOutOfRange {
                output: name,
                protocol: "Art-Net",
                universe: output.universe,
                range: "0 through 32767",
            });
        }
        DmxProtocol::Sacn if !(1..=63_999).contains(&output.universe) => {
            return Err(ConfigurationError::UniverseOutOfRange {
                output: name,
                protocol: "sACN",
                universe: output.universe,
                range: "1 through 63999",
            });
        }
        DmxProtocol::ArtNet | DmxProtocol::Sacn => {}
    }

    output
        .personality
        .footprint()
        .validate_start_address(output.start_address)
        .map_err(|source| ConfigurationError::StartAddress {
            output: name.clone(),
            source,
        })?;

    if output.resolution.width == 0 {
        return Err(ConfigurationError::EmptyResolution {
            output: name,
            axis: "width",
        });
    }
    if output.resolution.height == 0 {
        return Err(ConfigurationError::EmptyResolution {
            output: name,
            axis: "height",
        });
    }

    if let PresentationMode::FixedFps { frames_per_second } = output.presentation
        && (!frames_per_second.is_finite() || !(1.0..=65_535.0).contains(&frames_per_second))
    {
        return Err(ConfigurationError::InvalidFixedRate { output: name });
    }

    Ok(())
}

/// Two enabled outputs on the same protocol and universe must not claim the same slots. Silently
/// letting them overlap would make one desk fader drive two outputs by accident.
fn validate_patch_overlap(outputs: &[OutputConfiguration]) -> Result<(), ConfigurationError> {
    let patched: Vec<&OutputConfiguration> =
        outputs.iter().filter(|output| output.enabled).collect();

    for (index, first) in patched.iter().enumerate() {
        for second in &patched[index + 1..] {
            if first.protocol != second.protocol || first.universe != second.universe {
                continue;
            }
            let first_span = span(first);
            let second_span = span(second);
            if first_span.0 <= second_span.1 && second_span.0 <= first_span.1 {
                return Err(ConfigurationError::OverlappingPatch {
                    first: first.name.to_string(),
                    second: second.name.to_string(),
                    universe: first.universe,
                    start_address: second.start_address,
                });
            }
        }
    }
    Ok(())
}

fn span(output: &OutputConfiguration) -> (u16, u16) {
    let total = output
        .personality
        .footprint_for(output.personality_layout)
        .total();
    (output.start_address, output.start_address + total - 1)
}

#[cfg(test)]
mod tests {
    use media_domain::LayerPersonality;

    use super::super::{MediaConfiguration, OutputConfiguration};
    use super::*;

    fn configuration(outputs: Vec<OutputConfiguration>) -> MediaConfiguration {
        MediaConfiguration {
            outputs,
            ..Default::default()
        }
    }

    #[test]
    fn the_default_configuration_validates() {
        assert_eq!(validate(&MediaConfiguration::default()), Ok(()));
    }

    #[test]
    fn a_configuration_without_outputs_is_rejected() {
        assert_eq!(
            validate(&configuration(vec![])),
            Err(ConfigurationError::NoOutputs)
        );
    }

    #[test]
    fn an_eight_layer_output_may_not_span_universes() {
        let mut output = OutputConfiguration::new("Main");
        output.start_address = 300;
        let error = validate(&configuration(vec![output])).unwrap_err();
        assert!(
            matches!(error, ConfigurationError::StartAddress { .. }),
            "{error}"
        );
        assert!(error.to_string().contains("Main"), "{error}");
    }

    #[test]
    fn duplicate_output_identities_are_rejected() {
        let first = OutputConfiguration::new("Main");
        let mut second = OutputConfiguration::new("Second");
        second.id = first.id;
        second.start_address = 280;
        second.personality = LayerPersonality::TwoLayers;
        let error = validate(&configuration(vec![first, second])).unwrap_err();
        assert!(
            matches!(error, ConfigurationError::DuplicateOutputId { .. }),
            "{error}"
        );
    }

    #[test]
    fn two_outputs_may_not_claim_the_same_slots() {
        let first = OutputConfiguration::new("Main");
        let mut second = OutputConfiguration::new("Second");
        second.start_address = 100;
        let error = validate(&configuration(vec![first, second])).unwrap_err();
        assert!(
            matches!(error, ConfigurationError::OverlappingPatch { .. }),
            "{error}"
        );
    }

    #[test]
    fn disabled_outputs_do_not_conflict() {
        let first = OutputConfiguration::new("Main");
        let mut second = OutputConfiguration::new("Second");
        second.start_address = 100;
        second.enabled = false;
        assert_eq!(validate(&configuration(vec![first, second])), Ok(()));
    }

    #[test]
    fn outputs_on_different_universes_coexist() {
        let first = OutputConfiguration::new("Main");
        let mut second = OutputConfiguration::new("Second");
        second.universe = 1;
        assert_eq!(validate(&configuration(vec![first, second])), Ok(()));
    }

    #[test]
    fn two_outputs_cannot_each_send_a_complete_pixel_universe() {
        let pixel_zone = |id: &str| media_domain::pixel_map::PixelZone {
            id: id.into(),
            name: id.into(),
            start: media_domain::pixel_map::CanvasPoint::new(0.0, 0.0),
            end: media_domain::pixel_map::CanvasPoint::new(1.0, 1.0),
            columns: 1,
            rows: 1,
            layout: media_domain::pixel_map::PixelLayout::rgb(),
            order: media_domain::pixel_map::PixelOrder::RowMajor,
            universe: 7,
            start_address: 1,
            enabled: true,
        };
        let mut first = OutputConfiguration::new("Main");
        first.pixel_map.zones = vec![pixel_zone("first")];
        first.pixel_map.routes = vec![crate::configuration::PixelOutputRoute {
            id: "first-route".into(),
            name: "Wall".into(),
            protocol: DmxProtocol::ArtNet,
            universe: 7,
            destination: None,
            enabled: true,
        }];
        let mut second = OutputConfiguration::new("Second");
        second.universe = 2;
        second.pixel_map.zones = vec![pixel_zone("second")];
        second.pixel_map.routes = vec![crate::configuration::PixelOutputRoute {
            id: "second-route".into(),
            name: "Same wall".into(),
            protocol: DmxProtocol::ArtNet,
            universe: 7,
            destination: None,
            enabled: true,
        }];
        assert!(matches!(
            validate(&configuration(vec![first, second])),
            Err(ConfigurationError::PixelRouteCollision { universe: 7, .. })
        ));
    }

    #[test]
    fn an_empty_resolution_is_rejected() {
        let mut output = OutputConfiguration::new("Main");
        output.resolution.height = 0;
        let error = validate(&configuration(vec![output])).unwrap_err();
        assert_eq!(
            error,
            ConfigurationError::EmptyResolution {
                output: "Main".to_owned(),
                axis: "height"
            }
        );
    }

    #[test]
    fn a_fixed_rate_of_zero_is_rejected() {
        let mut output = OutputConfiguration::new("Main");
        output.presentation = PresentationMode::FixedFps {
            frames_per_second: 0.0,
        };
        let error = validate(&configuration(vec![output])).unwrap_err();
        assert_eq!(
            error,
            ConfigurationError::InvalidFixedRate {
                output: "Main".to_owned()
            }
        );
    }

    #[test]
    fn a_protocols_reserved_universes_are_rejected() {
        let mut art_net = OutputConfiguration::new("Art-Net wall");
        art_net.universe = 32_768;
        let error = validate(&configuration(vec![art_net])).unwrap_err();
        assert_eq!(
            error,
            ConfigurationError::UniverseOutOfRange {
                output: "Art-Net wall".to_owned(),
                protocol: "Art-Net",
                universe: 32_768,
                range: "0 through 32767",
            }
        );

        let mut sacn = OutputConfiguration::new("sACN wall");
        sacn.protocol = DmxProtocol::Sacn;
        sacn.universe = 0;
        let error = validate(&configuration(vec![sacn])).unwrap_err();
        assert_eq!(
            error,
            ConfigurationError::UniverseOutOfRange {
                output: "sACN wall".to_owned(),
                protocol: "sACN",
                universe: 0,
                range: "1 through 63999",
            }
        );
    }
}

#[cfg(test)]
mod pixel_map_tests {
    use super::*;
    use crate::configuration::pixel_map::tests::zone;
    use crate::configuration::{PixelMapConfiguration, PixelOutputRoute, PixelZoneHandoff};
    use media_domain::display_region::DisplayRegion;
    use media_domain::pixel_map::CanvasPoint;

    fn route(universe: u16) -> PixelOutputRoute {
        PixelOutputRoute {
            id: format!("route-{universe}"),
            name: format!("Universe {universe}"),
            protocol: DmxProtocol::default(),
            universe,
            destination: None,
            enabled: true,
        }
    }

    fn handoff(zone_id: &str, footprint: u16) -> PixelZoneHandoff {
        PixelZoneHandoff {
            zone_id: zone_id.into(),
            fixture_name: "Zone fixture".into(),
            protocol: DmxProtocol::ArtNet,
            input_universe: 9,
            input_start_address: 3,
            dimmer_address: 1,
            mix_address: 2,
            fixture_footprint: footprint,
            automatic_patch: false,
        }
    }

    fn output_with(map: PixelMapConfiguration) -> OutputConfiguration {
        OutputConfiguration {
            pixel_map: map,
            ..OutputConfiguration::new("Main")
        }
    }

    #[test]
    fn a_sound_pixel_map_passes() {
        let map = PixelMapConfiguration {
            zones: vec![zone("left", 1, 1, 10), zone("right", 1, 31, 10)],
            routes: vec![route(1)],
            ..PixelMapConfiguration::default()
        };
        assert_eq!(validate_pixel_map(&output_with(map)), Ok(()));
    }

    #[test]
    fn two_zones_may_share_a_universe_but_not_a_slot() {
        let map = PixelMapConfiguration {
            // The first zone ends at slot 30; the second starts inside it.
            zones: vec![zone("left", 1, 1, 10), zone("right", 1, 30, 10)],
            routes: vec![route(1)],
            ..PixelMapConfiguration::default()
        };
        assert!(matches!(
            validate_pixel_map(&output_with(map)),
            Err(ConfigurationError::OverlappingZones { universe: 1, .. })
        ));
    }

    #[test]
    fn a_zone_may_not_run_past_the_end_of_its_universe() {
        let map = PixelMapConfiguration {
            zones: vec![zone("strip", 1, 500, 10)],
            routes: vec![route(1)],
            ..PixelMapConfiguration::default()
        };
        assert!(matches!(
            validate_pixel_map(&output_with(map)),
            Err(ConfigurationError::ZoneOverrunsUniverse { footprint: 30, .. })
        ));
    }

    #[test]
    fn a_zone_with_no_pixels_is_refused() {
        let mut empty = zone("strip", 1, 1, 0);
        empty.rows = 0;
        let map = PixelMapConfiguration {
            zones: vec![empty],
            routes: vec![route(1)],
            ..PixelMapConfiguration::default()
        };
        assert!(matches!(
            validate_pixel_map(&output_with(map)),
            Err(ConfigurationError::EmptyZone { .. })
        ));
    }

    #[test]
    fn a_zone_sending_directly_needs_a_route_for_its_universe() {
        let map = PixelMapConfiguration {
            zones: vec![zone("strip", 4, 1, 10)],
            routes: vec![route(1)],
            ..PixelMapConfiguration::default()
        };
        assert!(matches!(
            validate_pixel_map(&output_with(map)),
            Err(ConfigurationError::UnroutedZone { universe: 4, .. })
        ));
    }

    #[test]
    fn a_zone_handed_to_the_desk_still_needs_a_physical_output_route() {
        let map = PixelMapConfiguration {
            mode: PixelOutputMode::DeskMerge,
            zones: vec![zone("strip", 4, 1, 10)],
            routes: vec![route(4)],
            handoffs: vec![PixelZoneHandoff {
                zone_id: "strip".into(),
                fixture_name: "Strip fixture".into(),
                protocol: DmxProtocol::ArtNet,
                input_universe: 9,
                input_start_address: 3,
                dimmer_address: 1,
                mix_address: 2,
                fixture_footprint: 30,
                automatic_patch: false,
            }],
            ..PixelMapConfiguration::default()
        };
        assert_eq!(validate_pixel_map(&output_with(map)), Ok(()));
    }

    #[test]
    fn merge_mode_requires_one_desk_patch_per_enabled_zone() {
        let map = PixelMapConfiguration {
            mode: PixelOutputMode::DeskMerge,
            zones: vec![zone("strip", 4, 1, 10)],
            routes: vec![route(4)],
            ..PixelMapConfiguration::default()
        };
        assert!(matches!(
            validate_pixel_map(&output_with(map)),
            Err(ConfigurationError::MissingZoneHandoff { .. })
        ));
    }

    #[test]
    fn direct_mode_has_no_hidden_desk_input_patch() {
        let map = PixelMapConfiguration {
            zones: vec![zone("strip", 1, 1, 10)],
            routes: vec![route(1)],
            handoffs: vec![handoff("strip", 30)],
            ..PixelMapConfiguration::default()
        };
        assert!(matches!(
            validate_pixel_map(&output_with(map)),
            Err(ConfigurationError::DirectModeHasHandoffs { .. })
        ));
    }

    #[test]
    fn merge_mode_validates_the_complete_physical_output_footprint() {
        let map = PixelMapConfiguration {
            mode: PixelOutputMode::DeskMerge,
            zones: vec![zone("strip", 1, 480, 10)],
            routes: vec![route(1)],
            handoffs: vec![handoff("strip", 40)],
            ..PixelMapConfiguration::default()
        };
        assert!(matches!(
            validate_pixel_map(&output_with(map)),
            Err(ConfigurationError::InvalidHandoffAddress {
                field: "Media Server output footprint",
                ..
            })
        ));
    }

    #[test]
    fn a_disabled_route_carries_nothing() {
        let mut dark = route(1);
        dark.enabled = false;
        let map = PixelMapConfiguration {
            zones: vec![zone("strip", 1, 1, 10)],
            routes: vec![dark],
            ..PixelMapConfiguration::default()
        };
        assert!(matches!(
            validate_pixel_map(&output_with(map)),
            Err(ConfigurationError::UnroutedZone { universe: 1, .. })
        ));
    }

    #[test]
    fn two_zones_may_not_share_an_identity() {
        let map = PixelMapConfiguration {
            zones: vec![zone("strip", 1, 1, 10), zone("strip", 1, 100, 10)],
            routes: vec![route(1)],
            ..PixelMapConfiguration::default()
        };
        assert!(matches!(
            validate_pixel_map(&output_with(map)),
            Err(ConfigurationError::DuplicateZoneId { .. })
        ));
    }

    #[test]
    fn a_region_covering_none_of_the_canvas_is_refused() {
        let flat = DisplayRegion {
            source: media_domain::display_region::CanvasRect::new(
                CanvasPoint::new(0.5, 0.0),
                CanvasPoint::new(0.5, 1.0),
            ),
            ..DisplayRegion::whole("flat", "Flat")
        };
        let map = PixelMapConfiguration {
            regions: vec![flat],
            ..PixelMapConfiguration::default()
        };
        assert!(matches!(
            validate_pixel_map(&output_with(map)),
            Err(ConfigurationError::EmptyRegion { .. })
        ));
    }

    #[test]
    fn an_output_that_maps_nothing_is_valid() {
        assert_eq!(
            validate_pixel_map(&OutputConfiguration::new("Main")),
            Ok(())
        );
    }
}
