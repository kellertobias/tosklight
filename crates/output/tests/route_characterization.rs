use light_output::{
    DMX_SLOTS, DeliveryMode, OutputRoute, Protocol, artnet_broadcast_destination, encode_routes,
    sacn_multicast_destination,
};
use std::{collections::HashMap, net::SocketAddr};

fn route(protocol: Protocol, logical: u16, destination: u16) -> OutputRoute {
    OutputRoute {
        protocol,
        logical_universe: logical,
        destination_universe: destination,
        delivery_mode: Some(match protocol {
            Protocol::ArtNet => DeliveryMode::Broadcast,
            Protocol::Sacn => DeliveryMode::Multicast,
        }),
        destination: None,
        enabled: true,
        minimum_slots: 512,
    }
}

fn unicast_route(protocol: Protocol, logical: u16, destination: u16) -> OutputRoute {
    let port = match protocol {
        Protocol::ArtNet => 6454,
        Protocol::Sacn => 5568,
    };
    OutputRoute {
        delivery_mode: Some(DeliveryMode::Unicast),
        destination: Some(SocketAddr::from(([127, 0, 0, 1], port))),
        ..route(protocol, logical, destination)
    }
}

#[test]
fn route_encoding_maps_logical_to_protocol_universes() {
    let mut disabled = route(Protocol::Sacn, 2, 21);
    disabled.enabled = false;
    let routes = [
        unicast_route(Protocol::ArtNet, 1, 10),
        route(Protocol::Sacn, 1, 20),
        disabled,
    ];
    let packets = encode_routes(
        &routes,
        &HashMap::from([(1, [0x33; DMX_SLOTS])]),
        &HashMap::from([(1, 512)]),
        &mut HashMap::new(),
        [1; 16],
        "Light",
        100,
    )
    .unwrap();
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0].universe, 10);
    assert_eq!(packets[0].bytes[18], 0x33);
    assert_eq!(packets[1].destination, sacn_multicast_destination(20));
    assert_eq!(packets[1].bytes[126], 0x33);
}

#[test]
fn disabled_route_suppresses_a_nonzero_highlight_frame() {
    let mut disabled = unicast_route(Protocol::ArtNet, 1, 1);
    disabled.enabled = false;
    disabled.minimum_slots = 1;
    let packets = encode_routes(
        &[disabled],
        &HashMap::from([(1, [200; DMX_SLOTS])]),
        &HashMap::from([(1, 1)]),
        &mut HashMap::new(),
        [1; 16],
        "Highlight safety test",
        100,
    )
    .unwrap();
    assert!(packets.is_empty());
}

#[test]
fn artnet_route_without_an_explicit_destination_uses_standard_broadcast() {
    let packets = encode_routes(
        &[route(Protocol::ArtNet, 1, 1)],
        &HashMap::from([(1, [0; DMX_SLOTS])]),
        &HashMap::new(),
        &mut HashMap::new(),
        [0; 16],
        "Light",
        100,
    )
    .unwrap();
    assert_eq!(packets[0].destination, artnet_broadcast_destination());
}

#[test]
fn explicit_delivery_modes_resolve_protocol_correct_destinations_with_equal_payloads() {
    let unicast_artnet: SocketAddr = "127.0.0.1:6454".parse().unwrap();
    let unicast_sacn: SocketAddr = "127.0.0.1:5568".parse().unwrap();
    let mut artnet_broadcast = route(Protocol::ArtNet, 1, 10);
    let mut artnet_unicast = unicast_route(Protocol::ArtNet, 1, 10);
    let mut sacn_multicast = route(Protocol::Sacn, 1, 110);
    let mut sacn_unicast = unicast_route(Protocol::Sacn, 1, 110);
    for route in [
        &mut artnet_broadcast,
        &mut artnet_unicast,
        &mut sacn_multicast,
        &mut sacn_unicast,
    ] {
        route.minimum_slots = 128;
    }
    let packets = encode_routes(
        &[
            artnet_broadcast,
            artnet_unicast,
            sacn_multicast,
            sacn_unicast,
        ],
        &HashMap::from([(1, [0x5a; DMX_SLOTS])]),
        &HashMap::new(),
        &mut HashMap::new(),
        [9; 16],
        "Delivery test",
        100,
    )
    .unwrap();

    assert_eq!(packets[0].destination, artnet_broadcast_destination());
    assert_eq!(packets[1].destination, unicast_artnet);
    assert_eq!(packets[2].destination, sacn_multicast_destination(110));
    assert_eq!(packets[3].destination, unicast_sacn);
    assert_eq!(&packets[0].bytes[18..], &packets[1].bytes[18..]);
    assert_eq!(&packets[2].bytes[126..], &packets[3].bytes[126..]);
    assert!(
        packets
            .iter()
            .all(|packet| packet.bytes.ends_with(&[0x5a; 128]))
    );
}

#[test]
fn route_validation_rejects_protocol_invalid_modes_and_destinations() {
    let mut invalid_mode = route(Protocol::ArtNet, 1, 1);
    invalid_mode.delivery_mode = Some(DeliveryMode::Multicast);
    assert_eq!(
        invalid_mode.validate().unwrap_err(),
        "Art-Net supports Broadcast or Unicast delivery, not Multicast"
    );
    let invalid_unicast = OutputRoute {
        protocol: Protocol::Sacn,
        delivery_mode: Some(DeliveryMode::Unicast),
        ..invalid_mode
    };
    assert!(
        invalid_unicast
            .validate()
            .unwrap_err()
            .contains("requires a destination")
    );
}

#[test]
fn enabled_empty_routes_emit_their_minimum_and_patched_zero_slots_extend_payloads() {
    let mut artnet = unicast_route(Protocol::ArtNet, 32, 10);
    let mut sacn = unicast_route(Protocol::Sacn, 33, 101);
    artnet.minimum_slots = 128;
    sacn.minimum_slots = 128;
    let packets = encode_routes(
        &[artnet, sacn],
        &HashMap::from([(33, [0; DMX_SLOTS])]),
        &HashMap::from([(33, 201)]),
        &mut HashMap::new(),
        [0; 16],
        "Light",
        100,
    )
    .unwrap();

    assert_eq!(packets[0].bytes.len(), 18 + 128);
    assert!(packets[0].bytes[18..].iter().all(|slot| *slot == 0));
    assert_eq!(packets[1].bytes.len(), 126 + 201);
    assert_eq!(&packets[1].bytes[123..125], &202_u16.to_be_bytes());
    assert!(packets[1].bytes[126..].iter().all(|slot| *slot == 0));
}

#[test]
fn artnet_payload_rounds_an_odd_slot_count_up_to_an_even_length() {
    let mut artnet = unicast_route(Protocol::ArtNet, 1, 10);
    artnet.minimum_slots = 1;
    let packets = encode_routes(
        &[artnet],
        &HashMap::from([(1, [0x7f; DMX_SLOTS])]),
        &HashMap::from([(1, 201)]),
        &mut HashMap::new(),
        [0; 16],
        "Light",
        100,
    )
    .unwrap();
    assert_eq!(&packets[0].bytes[16..18], &202_u16.to_be_bytes());
    assert_eq!(packets[0].bytes.len(), 18 + 202);
}

#[test]
fn legacy_route_without_minimum_slots_keeps_full_universe_payloads() {
    let route: OutputRoute = serde_json::from_value(serde_json::json!({
        "protocol": "art_net",
        "logical_universe": 1,
        "destination_universe": 1,
        "destination": null,
        "enabled": true
    }))
    .unwrap();
    assert_eq!(route.minimum_slots, 512);
    assert_eq!(route.resolved_delivery_mode(), DeliveryMode::Broadcast);

    let legacy_unicast: OutputRoute = serde_json::from_value(serde_json::json!({
        "protocol": "sacn",
        "logical_universe": 1,
        "destination_universe": 101,
        "destination": "127.0.0.1:5568",
        "enabled": true
    }))
    .unwrap();
    assert_eq!(
        legacy_unicast.resolved_delivery_mode(),
        DeliveryMode::Unicast
    );
}
