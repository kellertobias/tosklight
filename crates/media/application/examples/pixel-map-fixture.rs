//! Writes a configuration with one off-screen output and one pixel zone, for smoke-testing the
//! pixel-output path against a real server.

use media_application::configuration::{
    DmxProtocol, MediaConfiguration, OutputTarget, PixelMapConfiguration, PixelOutputRoute,
    Resolution, save,
};
use media_domain::pixel_map::{CanvasPoint, PixelLayout, PixelOrder, PixelZone};

fn main() {
    let mut configuration = MediaConfiguration::default();
    let output = configuration.outputs.first_mut().expect("one output");
    output.target = OutputTarget::OffScreen;
    output.resolution = Resolution {
        width: 64,
        height: 36,
    };
    output.pixel_map = PixelMapConfiguration {
        zones: vec![PixelZone {
            id: "strip".into(),
            name: "Strip".into(),
            start: CanvasPoint::new(0.0, 0.0),
            end: CanvasPoint::new(1.0, 1.0),
            columns: 4,
            rows: 1,
            layout: PixelLayout::rgb(),
            order: PixelOrder::RowMajor,
            universe: 7,
            start_address: 1,
            enabled: true,
        }],
        routes: vec![PixelOutputRoute {
            id: "route".into(),
            name: "Universe 7".into(),
            protocol: DmxProtocol::ArtNet,
            universe: 7,
            destination: Some("127.0.0.1:6455".into()),
            enabled: true,
        }],
        ..PixelMapConfiguration::default()
    };
    print!("{}", save(&configuration));
}
