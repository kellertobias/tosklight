//! The pixel map and display regions of one output, as the web UI sees them.
//!
//! The domain types stay free of transport concerns, so these are their mirrors. Colour components
//! and orders travel as the same strings their configuration spells, which keeps the layout model
//! open: a fixture nobody has thought of yet is a new list of component names, not a new type here.

use media_application::configuration::{
    DmxProtocol, PixelMapConfiguration, PixelOutputMode, PixelOutputRoute, PixelZoneHandoff,
};
use media_domain::display_region::{CanvasRect, DisplayRegion, RegionFit, RegionRotation};
use media_domain::pixel_map::{CanvasPoint, PixelComponent, PixelLayout, PixelOrder, PixelZone};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Why a pixel-map edit could not describe a usable map.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PixelMapEditError {
    #[error("mode must be 'direct' or 'desk-merge'")]
    Mode,
    #[error("'{value}' is not a colour component this understands")]
    Component { value: String },
    #[error(
        "order must be 'row-major', 'column-major', 'serpentine-rows', or 'serpentine-columns'"
    )]
    Order,
    #[error("rotation must be 'none', 'clockwise-90', 'half', or 'counter-clockwise-90'")]
    Rotation,
    #[error("fit must be 'fill', 'contain', or 'stretch'")]
    Fit,
    #[error("protocol must be 'art-net' or 'sacn'")]
    Protocol,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct CanvasPointView {
    pub x: f32,
    pub y: f32,
}

impl CanvasPointView {
    fn of(point: CanvasPoint) -> Self {
        Self {
            x: point.x,
            y: point.y,
        }
    }

    fn into_domain(self) -> CanvasPoint {
        CanvasPoint::new(self.x, self.y)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PixelLayoutView {
    pub name: String,
    /// `red`, `green`, `blue`, `white`, `amber`, `ultra-violet`, or `dimmer`, in wire order.
    pub components: Vec<String>,
}

fn component_name(component: PixelComponent) -> &'static str {
    match component {
        PixelComponent::Red => "red",
        PixelComponent::Green => "green",
        PixelComponent::Blue => "blue",
        PixelComponent::White => "white",
        PixelComponent::Amber => "amber",
        PixelComponent::UltraViolet => "ultra-violet",
        PixelComponent::Dimmer => "dimmer",
    }
}

fn component_of(value: &str) -> Result<PixelComponent, PixelMapEditError> {
    Ok(match value {
        "red" => PixelComponent::Red,
        "green" => PixelComponent::Green,
        "blue" => PixelComponent::Blue,
        "white" => PixelComponent::White,
        "amber" => PixelComponent::Amber,
        "ultra-violet" => PixelComponent::UltraViolet,
        "dimmer" => PixelComponent::Dimmer,
        other => {
            return Err(PixelMapEditError::Component {
                value: other.to_owned(),
            });
        }
    })
}

impl PixelLayoutView {
    fn of(layout: &PixelLayout) -> Self {
        Self {
            name: layout.name.clone(),
            components: layout
                .components
                .iter()
                .map(|c| component_name(*c).to_owned())
                .collect(),
        }
    }

    fn into_domain(self) -> Result<PixelLayout, PixelMapEditError> {
        let components = self
            .components
            .iter()
            .map(|value| component_of(value))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PixelLayout::new(self.name, components))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PixelZoneView {
    pub id: String,
    pub name: String,
    pub start: CanvasPointView,
    pub end: CanvasPointView,
    pub columns: u32,
    pub rows: u32,
    pub layout: PixelLayoutView,
    /// `row-major`, `column-major`, `serpentine-rows`, or `serpentine-columns`.
    pub order: String,
    pub universe: u16,
    pub start_address: u16,
    pub enabled: bool,
    /// How many DMX slots this zone occupies, so the UI can show it without recomputing it.
    pub footprint: u32,
}

fn order_name(order: PixelOrder) -> &'static str {
    match order {
        PixelOrder::RowMajor => "row-major",
        PixelOrder::ColumnMajor => "column-major",
        PixelOrder::SerpentineRows => "serpentine-rows",
        PixelOrder::SerpentineColumns => "serpentine-columns",
    }
}

impl PixelZoneView {
    fn of(zone: &PixelZone) -> Self {
        Self {
            id: zone.id.clone(),
            name: zone.name.clone(),
            start: CanvasPointView::of(zone.start),
            end: CanvasPointView::of(zone.end),
            columns: zone.columns,
            rows: zone.rows,
            layout: PixelLayoutView::of(&zone.layout),
            order: order_name(zone.order).to_owned(),
            universe: zone.universe,
            start_address: zone.start_address,
            enabled: zone.enabled,
            footprint: zone.footprint() as u32,
        }
    }

    fn into_domain(self) -> Result<PixelZone, PixelMapEditError> {
        let order = match self.order.as_str() {
            "row-major" => PixelOrder::RowMajor,
            "column-major" => PixelOrder::ColumnMajor,
            "serpentine-rows" => PixelOrder::SerpentineRows,
            "serpentine-columns" => PixelOrder::SerpentineColumns,
            _ => return Err(PixelMapEditError::Order),
        };
        Ok(PixelZone {
            id: self.id,
            name: self.name,
            start: self.start.into_domain(),
            end: self.end.into_domain(),
            columns: self.columns,
            rows: self.rows,
            layout: self.layout.into_domain()?,
            order,
            universe: self.universe,
            start_address: self.start_address,
            enabled: self.enabled,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PixelRouteView {
    pub id: String,
    pub name: String,
    /// `art-net` or `sacn`.
    pub protocol: String,
    pub universe: u16,
    /// A host, or a host and port. Absent means the protocol's own convention.
    pub destination: Option<String>,
    pub enabled: bool,
}

fn protocol_name(protocol: DmxProtocol) -> &'static str {
    match protocol {
        DmxProtocol::ArtNet => "art-net",
        DmxProtocol::Sacn => "sacn",
    }
}

impl PixelRouteView {
    fn of(route: &PixelOutputRoute) -> Self {
        Self {
            id: route.id.clone(),
            name: route.name.clone(),
            protocol: protocol_name(route.protocol).to_owned(),
            universe: route.universe,
            destination: route.destination.clone(),
            enabled: route.enabled,
        }
    }

    fn into_domain(self) -> Result<PixelOutputRoute, PixelMapEditError> {
        let protocol = match self.protocol.as_str() {
            "art-net" => DmxProtocol::ArtNet,
            "sacn" => DmxProtocol::Sacn,
            _ => return Err(PixelMapEditError::Protocol),
        };
        Ok(PixelOutputRoute {
            id: self.id,
            name: self.name,
            protocol,
            universe: self.universe,
            destination: self.destination,
            enabled: self.enabled,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PixelZoneHandoffView {
    pub zone_id: String,
    pub fixture_name: String,
    /// `art-net` or `sacn`.
    pub protocol: String,
    pub input_universe: u16,
    pub input_start_address: u16,
    pub dimmer_address: u16,
    pub mix_address: u16,
    pub fixture_footprint: u16,
    pub automatic_patch: bool,
}

impl PixelZoneHandoffView {
    fn of(handoff: &PixelZoneHandoff) -> Self {
        Self {
            zone_id: handoff.zone_id.clone(),
            fixture_name: handoff.fixture_name.clone(),
            protocol: protocol_name(handoff.protocol).to_owned(),
            input_universe: handoff.input_universe,
            input_start_address: handoff.input_start_address,
            dimmer_address: handoff.dimmer_address,
            mix_address: handoff.mix_address,
            fixture_footprint: handoff.fixture_footprint,
            automatic_patch: handoff.automatic_patch,
        }
    }

    fn into_domain(self) -> Result<PixelZoneHandoff, PixelMapEditError> {
        let protocol = match self.protocol.as_str() {
            "art-net" => DmxProtocol::ArtNet,
            "sacn" => DmxProtocol::Sacn,
            _ => return Err(PixelMapEditError::Protocol),
        };
        Ok(PixelZoneHandoff {
            zone_id: self.zone_id,
            fixture_name: self.fixture_name,
            protocol,
            input_universe: self.input_universe,
            input_start_address: self.input_start_address,
            dimmer_address: self.dimmer_address,
            mix_address: self.mix_address,
            fixture_footprint: self.fixture_footprint,
            automatic_patch: self.automatic_patch,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct DisplayRegionView {
    pub id: String,
    pub name: String,
    pub start: CanvasPointView,
    pub end: CanvasPointView,
    /// `none`, `clockwise-90`, `half`, or `counter-clockwise-90`.
    pub rotation: String,
    /// `fill`, `contain`, or `stretch`.
    pub fit: String,
    pub enabled: bool,
}

fn rotation_name(rotation: RegionRotation) -> &'static str {
    match rotation {
        RegionRotation::None => "none",
        RegionRotation::Clockwise90 => "clockwise-90",
        RegionRotation::Half => "half",
        RegionRotation::CounterClockwise90 => "counter-clockwise-90",
    }
}

fn fit_name(fit: RegionFit) -> &'static str {
    match fit {
        RegionFit::Fill => "fill",
        RegionFit::Contain => "contain",
        RegionFit::Stretch => "stretch",
    }
}

impl DisplayRegionView {
    fn of(region: &DisplayRegion) -> Self {
        Self {
            id: region.id.clone(),
            name: region.name.clone(),
            start: CanvasPointView::of(region.source.start),
            end: CanvasPointView::of(region.source.end),
            rotation: rotation_name(region.rotation).to_owned(),
            fit: fit_name(region.fit).to_owned(),
            enabled: region.enabled,
        }
    }

    fn into_domain(self) -> Result<DisplayRegion, PixelMapEditError> {
        let rotation = match self.rotation.as_str() {
            "none" => RegionRotation::None,
            "clockwise-90" => RegionRotation::Clockwise90,
            "half" => RegionRotation::Half,
            "counter-clockwise-90" => RegionRotation::CounterClockwise90,
            _ => return Err(PixelMapEditError::Rotation),
        };
        let fit = match self.fit.as_str() {
            "fill" => RegionFit::Fill,
            "contain" => RegionFit::Contain,
            "stretch" => RegionFit::Stretch,
            _ => return Err(PixelMapEditError::Fit),
        };
        Ok(DisplayRegion {
            id: self.id,
            name: self.name,
            source: CanvasRect::new(self.start.into_domain(), self.end.into_domain()),
            rotation,
            fit,
            enabled: self.enabled,
        })
    }
}

/// One output's whole pixel map.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PixelMapView {
    /// `direct` or `desk-merge`.
    pub mode: String,
    pub zones: Vec<PixelZoneView>,
    pub routes: Vec<PixelRouteView>,
    pub handoffs: Vec<PixelZoneHandoffView>,
    pub regions: Vec<DisplayRegionView>,
}

impl PixelMapView {
    pub fn of(map: &PixelMapConfiguration) -> Self {
        Self {
            mode: match map.mode {
                PixelOutputMode::Direct => "direct".to_owned(),
                PixelOutputMode::DeskMerge => "desk-merge".to_owned(),
            },
            zones: map.zones.iter().map(PixelZoneView::of).collect(),
            routes: map.routes.iter().map(PixelRouteView::of).collect(),
            handoffs: map.handoffs.iter().map(PixelZoneHandoffView::of).collect(),
            regions: map.regions.iter().map(DisplayRegionView::of).collect(),
        }
    }

    pub fn into_domain(self) -> Result<PixelMapConfiguration, PixelMapEditError> {
        let mode = match self.mode.as_str() {
            "direct" => PixelOutputMode::Direct,
            "desk-merge" => PixelOutputMode::DeskMerge,
            _ => return Err(PixelMapEditError::Mode),
        };
        Ok(PixelMapConfiguration {
            mode,
            zones: self
                .zones
                .into_iter()
                .map(PixelZoneView::into_domain)
                .collect::<Result<_, _>>()?,
            routes: self
                .routes
                .into_iter()
                .map(PixelRouteView::into_domain)
                .collect::<Result<_, _>>()?,
            handoffs: self
                .handoffs
                .into_iter()
                .map(PixelZoneHandoffView::into_domain)
                .collect::<Result<_, _>>()?,
            regions: self
                .regions
                .into_iter()
                .map(DisplayRegionView::into_domain)
                .collect::<Result<_, _>>()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone() -> PixelZone {
        PixelZone {
            id: "strip".into(),
            name: "Upstage strip".into(),
            start: CanvasPoint::new(0.1, 0.2),
            end: CanvasPoint::new(0.9, 0.3),
            columns: 12,
            rows: 1,
            layout: PixelLayout::rgbw(),
            order: PixelOrder::SerpentineRows,
            universe: 3,
            start_address: 5,
            enabled: true,
        }
    }

    fn map() -> PixelMapConfiguration {
        PixelMapConfiguration {
            mode: PixelOutputMode::Direct,
            zones: vec![zone()],
            routes: vec![PixelOutputRoute {
                id: "route".into(),
                name: "Universe 3".into(),
                protocol: DmxProtocol::Sacn,
                universe: 3,
                destination: Some("10.0.0.4".into()),
                enabled: true,
            }],
            handoffs: Vec::new(),
            regions: vec![DisplayRegion {
                rotation: RegionRotation::Clockwise90,
                fit: RegionFit::Contain,
                ..DisplayRegion::whole("centre", "HDMI 1")
            }],
        }
    }

    #[test]
    fn a_map_survives_the_round_trip_through_the_wire() {
        let view = PixelMapView::of(&map());
        assert_eq!(view.into_domain().expect("it reads back"), map());
    }

    #[test]
    fn a_zone_reports_the_slots_it_occupies_so_the_ui_need_not_work_it_out() {
        // Twelve RGBW pixels are forty-eight slots.
        assert_eq!(PixelZoneView::of(&zone()).footprint, 48);
    }

    #[test]
    fn an_unknown_colour_component_is_refused_by_name() {
        let mut view = PixelMapView::of(&map());
        view.zones[0].layout.components[0] = "octarine".into();
        assert_eq!(
            view.into_domain(),
            Err(PixelMapEditError::Component {
                value: "octarine".into()
            })
        );
    }

    #[test]
    fn an_unknown_mode_order_rotation_fit_or_protocol_is_refused() {
        let broken = |edit: fn(&mut PixelMapView)| {
            let mut view = PixelMapView::of(&map());
            edit(&mut view);
            view.into_domain().expect_err("a refusal")
        };
        assert_eq!(
            broken(|v| v.mode = "sideways".into()),
            PixelMapEditError::Mode
        );
        assert_eq!(
            broken(|v| v.zones[0].order = "spiral".into()),
            PixelMapEditError::Order
        );
        assert_eq!(
            broken(|v| v.regions[0].rotation = "upside".into()),
            PixelMapEditError::Rotation
        );
        assert_eq!(
            broken(|v| v.regions[0].fit = "squash".into()),
            PixelMapEditError::Fit
        );
        assert_eq!(
            broken(|v| v.routes[0].protocol = "smoke".into()),
            PixelMapEditError::Protocol
        );
    }

    #[test]
    fn every_colour_component_survives_the_round_trip() {
        for component in [
            PixelComponent::Red,
            PixelComponent::Green,
            PixelComponent::Blue,
            PixelComponent::White,
            PixelComponent::Amber,
            PixelComponent::UltraViolet,
            PixelComponent::Dimmer,
        ] {
            assert_eq!(component_of(component_name(component)), Ok(component));
        }
    }
}
