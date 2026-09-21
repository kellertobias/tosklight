use super::*;
use light_output::{ObservedArtNetSender, ObservedArtPoller, ObservedSacnSource};
use std::net::Ipv4Addr;

fn route(protocol: Protocol, universe: u16, destination: Option<SocketAddr>) -> OutputRoute {
    OutputRoute {
        target: Default::default(),
        protocol,
        logical_universe: 1,
        destination_universe: universe,
        delivery_mode: None,
        destination,
        enabled: true,
        minimum_slots: 512,
    }
}

fn sent(protocol: Protocol, universe: u16, destination: SocketAddr) -> RouteActivity {
    RouteActivity {
        protocol,
        universe,
        destination,
        last_sent_millis_ago: Some(20),
        last_error: None,
        last_error_millis_ago: None,
        errors: 0,
    }
}

fn peer_ip() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(10, 0, 0, 50))
}

fn by_id<'a>(snapshot: &'a NetworkEndpointsSnapshot, id: &str) -> &'a NetworkEndpoint {
    snapshot
        .endpoints
        .iter()
        .find(|endpoint| endpoint.id == id)
        .unwrap_or_else(|| panic!("missing {id} in {:#?}", snapshot.endpoints))
}

fn inputs<'a>(routes: &'a [OutputRoute], activity: Option<NetworkActivity>) -> EndpointInputs<'a> {
    EndpointInputs {
        routes,
        activity,
        output_bind_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        art_timecode_bind: None,
        timecode: None,
    }
}

#[test]
fn art_net_and_sacn_sends_report_destination_universe_and_live_status() {
    let unicast = SocketAddr::from((Ipv4Addr::new(10, 0, 0, 7), 6454));
    let routes = [
        route(Protocol::ArtNet, 4, None),
        route(Protocol::ArtNet, 5, Some(unicast)),
        route(Protocol::Sacn, 9, None),
    ];
    let broadcast = routes[0].resolved_destination().unwrap();
    let multicast = routes[2].resolved_destination().unwrap();
    let mut failing = sent(Protocol::ArtNet, 5, unicast);
    failing.last_error = Some("No route to host".into());
    failing.last_error_millis_ago = Some(5);
    failing.errors = 3;
    let activity = NetworkActivity {
        routes: vec![
            sent(Protocol::ArtNet, 4, broadcast),
            failing,
            sent(Protocol::Sacn, 9, multicast),
        ],
        sacn_discovery_listener: Some(SocketAddr::from((Ipv4Addr::new(239, 255, 250, 214), 5568))),
        ..NetworkActivity::default()
    };
    let snapshot = project(&inputs(&routes, Some(activity)));
    assert!(snapshot.network_output_available);

    let art_net = by_id(&snapshot, "send:artnet:1:4:255.255.255.255:6454");
    assert_eq!(art_net.protocol, OutputProtocol::ArtNet);
    assert_eq!(art_net.direction, Direction::Send);
    assert_eq!(art_net.origin, Origin::Configured);
    assert_eq!(art_net.delivery_mode, Some(OutputDeliveryMode::Broadcast));
    assert_eq!(
        (art_net.logical_universe, art_net.universes.as_slice()),
        (Some(1), &[4][..])
    );
    assert_eq!(art_net.status, Status::Active);

    let error = by_id(&snapshot, "send:artnet:1:5:10.0.0.7:6454");
    assert_eq!(error.status, Status::Error);
    assert_eq!(error.errors, 3);
    assert!(
        error.detail.contains("No route to host"),
        "{}",
        error.detail
    );

    let sacn = by_id(&snapshot, "send:sacn:1:9:239.255.0.9:5568");
    assert_eq!(sacn.protocol, OutputProtocol::Sacn);
    assert_eq!(sacn.delivery_mode, Some(OutputDeliveryMode::Multicast));
    assert_eq!(sacn.status, Status::Active);

    let discovery = by_id(&snapshot, "send:sacn:announce");
    assert_eq!(discovery.endpoint, "239.255.250.214:5568");
    assert_eq!(discovery.universes, [9]);
    let listener = by_id(&snapshot, "receive:sacn:discovery");
    assert_eq!(
        (listener.direction, listener.status),
        (Direction::Receive, Status::Listening)
    );
    // No broadcast network: the Art-Net poll listener is reported as unavailable, not omitted.
    let poll = by_id(&snapshot, "receive:artnet:poll");
    assert_eq!(poll.status, Status::Unavailable);
}

#[test]
fn disabled_invalid_and_unstarted_sends_explain_what_to_do() {
    let mut disabled = route(Protocol::Sacn, 2, None);
    disabled.enabled = false;
    let mut invalid = route(Protocol::ArtNet, 3, None);
    invalid.delivery_mode = Some(DeliveryMode::Multicast);
    let routes = [disabled, invalid, route(Protocol::ArtNet, 6, None)];
    let snapshot = project(&inputs(&routes, Some(NetworkActivity::default())));
    let disabled = by_id(&snapshot, "send:sacn:1:2:239.255.0.2:5568");
    assert_eq!(disabled.status, Status::Disabled);
    assert!(disabled.detail.contains("Desk Setup > Outputs > Routes"));
    let invalid = by_id(&snapshot, "send:artnet:1:3:—");
    assert_eq!(invalid.status, Status::Error);
    assert!(invalid.detail.contains("Multicast"), "{}", invalid.detail);
    assert_eq!(
        by_id(&snapshot, "send:artnet:1:6:255.255.255.255:6454").status,
        Status::Idle
    );
    // A disabled sACN route announces nothing.
    assert!(
        snapshot
            .endpoints
            .iter()
            .all(|endpoint| endpoint.id != "send:sacn:announce")
    );

    let offline = project(&inputs(&routes[2..], None));
    assert!(!offline.network_output_available);
    assert_eq!(offline.endpoints[0].status, Status::Unavailable);
    assert_eq!(
        by_id(&offline, "receive:sacn:discovery").status,
        Status::Unavailable
    );
}

#[test]
fn heard_art_net_and_sacn_peers_are_received_endpoints_and_flag_universe_conflicts() {
    let routes = [
        route(Protocol::ArtNet, 4, None),
        route(Protocol::Sacn, 9, None),
    ];
    let poller = SocketAddr::from((Ipv4Addr::new(10, 0, 0, 60), 6454));
    let listener = SocketAddr::from((Ipv4Addr::new(10, 0, 0, 255), 6454));
    let activity = NetworkActivity {
        art_poll_listeners: vec![listener],
        art_pollers: vec![ObservedArtPoller {
            address: poller,
            polls: 2,
            last_seen_millis_ago: 100,
            announced_name: None,
        }],
        art_net_senders: vec![ObservedArtNetSender {
            address: peer_ip(),
            universes: vec![4, 8],
            last_seen_millis_ago: 30,
            announced_name: None,
        }],
        sacn_sources: vec![
            ObservedSacnSource {
                cid: "ab".repeat(16),
                name: "Backup console".into(),
                address: peer_ip(),
                universes: vec![9],
                last_seen_millis_ago: 4_000,
            },
            ObservedSacnSource {
                cid: "cd".repeat(16),
                name: "Media server".into(),
                address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 51)),
                universes: vec![20],
                last_seen_millis_ago: 4_000,
            },
        ],
        ..NetworkActivity::default()
    };
    let mut timecode_inputs = inputs(&routes, Some(activity));
    timecode_inputs.art_timecode_bind = Some(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 6454)));
    timecode_inputs.timecode = Some(("artnet:10.0.0.70:0".into(), 200));
    let snapshot = project(&timecode_inputs);

    let listening = by_id(&snapshot, "receive:artnet:poll:10.0.0.255:6454");
    assert_eq!(listening.status, Status::Listening);
    let polling = by_id(&snapshot, "receive:artnet:poller:10.0.0.60:6454");
    assert_eq!(
        (polling.origin, polling.status),
        (Origin::Observed, Status::Active)
    );
    assert_eq!(
        by_id(&snapshot, "send:artnet:announce").status,
        Status::Active
    );

    let art_net = by_id(&snapshot, "receive:artnet:sender:10.0.0.50");
    assert_eq!(art_net.direction, Direction::Receive);
    assert_eq!(art_net.universes, [4, 8]);
    assert_eq!(art_net.status, Status::Conflict);
    let sent_art_net = by_id(&snapshot, "send:artnet:1:4:255.255.255.255:6454");
    assert_eq!(sent_art_net.status, Status::Conflict);
    assert!(
        sent_art_net.detail.contains("10.0.0.50"),
        "{}",
        sent_art_net.detail
    );

    let conflicting = by_id(
        &snapshot,
        &format!("receive:sacn:source:{}", "ab".repeat(16)),
    );
    assert_eq!(conflicting.name.as_deref(), Some("Backup console"));
    assert_eq!(conflicting.status, Status::Conflict);
    let sent_sacn = by_id(&snapshot, "send:sacn:1:9:239.255.0.9:5568");
    assert_eq!(sent_sacn.status, Status::Conflict);
    assert!(sent_sacn.detail.contains("Backup console (10.0.0.50)"));
    let other = by_id(
        &snapshot,
        &format!("receive:sacn:source:{}", "cd".repeat(16)),
    );
    assert_eq!(
        (other.status, other.universes.as_slice()),
        (Status::Active, &[20][..])
    );

    let timecode = by_id(&snapshot, "receive:artnet:timecode");
    assert_eq!(timecode.endpoint, "0.0.0.0:6454");
    assert_eq!(timecode.status, Status::Active);
    assert_eq!(timecode.name.as_deref(), Some("10.0.0.70"));
}

#[test]
fn the_desks_own_software_is_named_and_third_party_nodes_are_not() {
    let activity = NetworkActivity {
        art_net_senders: vec![
            ObservedArtNetSender {
                address: peer_ip(),
                universes: vec![1],
                last_seen_millis_ago: 30,
                announced_name: Some("ToskLight Media Server".into()),
            },
            ObservedArtNetSender {
                address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 61)),
                universes: vec![2],
                last_seen_millis_ago: 30,
                announced_name: Some("MA Lighting grandMA3".into()),
            },
        ],
        sacn_sources: vec![ObservedSacnSource {
            cid: "ef".repeat(16),
            // An sACN source keeps the operator's own label after the identity.
            name: "ToskLight Media Server — Stage Left".into(),
            address: peer_ip(),
            universes: vec![3],
            last_seen_millis_ago: 30,
        }],
        ..NetworkActivity::default()
    };
    let endpoints = project(&inputs(&[], Some(activity))).endpoints;
    let software = |endpoint: &str| {
        endpoints
            .iter()
            .find(|item| item.endpoint.starts_with(endpoint))
            .and_then(|item| item.software.clone())
    };
    assert_eq!(software("10.0.0.50"), Some("Media Server".into()));
    assert_eq!(
        software("10.0.0.61"),
        None,
        "a third-party node is not ours"
    );
    assert!(
        endpoints
            .iter()
            .any(|item| item.software.as_deref() == Some("Media Server")
                && item.name.as_deref() == Some("ToskLight Media Server — Stage Left")),
        "the sACN source keeps its announced name as well as its marking"
    );
}
