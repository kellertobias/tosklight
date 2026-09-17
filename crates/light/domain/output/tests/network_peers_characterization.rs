//! The Nodes tab's view of the network: what each route sent, and which Art-Net and sACN peers
//! the desk heard.

use light_output::{DMX_SLOTS, DeliveryMode, NetworkOutput, OutputRoute, Protocol, artdmx_packet};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio::net::UdpSocket;

fn unicast_route(protocol: Protocol, universe: u16, destination: SocketAddr) -> OutputRoute {
    OutputRoute {
        target: Default::default(),
        protocol,
        logical_universe: 1,
        destination_universe: universe,
        delivery_mode: Some(DeliveryMode::Unicast),
        destination: Some(destination),
        enabled: true,
        minimum_slots: 512,
    }
}

async fn local_socket() -> UdpSocket {
    UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap()
}

async fn send_frame(output: &NetworkOutput, routes: &[OutputRoute]) {
    let frames = HashMap::from([(1, [0x10; DMX_SLOTS])]);
    let _ = output
        .send_routes(
            routes,
            &frames,
            &HashMap::from([(1, 512)]),
            &mut HashMap::new(),
        )
        .await;
}

#[tokio::test]
async fn each_art_net_and_sacn_destination_reports_what_it_sent_and_what_failed() {
    let output = NetworkOutput::bind(IpAddr::V4(Ipv4Addr::LOCALHOST), [11; 16], "Light")
        .await
        .unwrap();
    let art_net = local_socket().await.local_addr().unwrap();
    let sacn = local_socket().await.local_addr().unwrap();
    let routes = [
        unicast_route(Protocol::ArtNet, 4, art_net),
        unicast_route(Protocol::Sacn, 9, sacn),
    ];
    output.inject_failure(sacn, true);
    send_frame(&output, &routes).await;

    let activity = output.network_activity();
    assert_eq!(activity.routes.len(), 2);
    let sent = &activity.routes[0];
    assert_eq!(
        (sent.protocol, sent.universe, sent.destination),
        (Protocol::ArtNet, 4, art_net)
    );
    assert!(sent.last_sent_millis_ago.is_some());
    assert_eq!((sent.errors, sent.last_error.as_deref()), (0, None));
    let failed = &activity.routes[1];
    assert_eq!(
        (failed.protocol, failed.universe, failed.destination),
        (Protocol::Sacn, 9, sacn)
    );
    assert_eq!(failed.last_sent_millis_ago, None);
    assert_eq!(failed.errors, 1);
    assert!(
        failed
            .last_error
            .as_deref()
            .is_some_and(|error| error.contains("injected output failure"))
    );
    assert_eq!(output.route_send_errors().len(), 1);
}

#[tokio::test]
async fn art_net_pollers_and_other_art_dmx_senders_are_heard_but_the_desks_own_frames_are_not() {
    // The desk's own address on this "network" is not loopback, so a loopback peer is foreign.
    let own = Ipv4Addr::new(192, 0, 2, 1);
    let output = NetworkOutput::bind(IpAddr::V4(Ipv4Addr::LOCALHOST), [12; 16], "Light")
        .await
        .unwrap()
        .listen_for_art_polls(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), own)
        .unwrap();
    let listener = *output.art_poll_addresses().last().unwrap();
    let peer = local_socket().await;
    let mut poll = [0_u8; 14];
    poll[..8].copy_from_slice(b"Art-Net\0");
    poll[8..10].copy_from_slice(&0x2000_u16.to_le_bytes());
    poll[11] = 14;
    peer.send_to(&poll, listener).await.unwrap();
    peer.send_to(&artdmx_packet(0x0102, 1, &[0; 24]), listener)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    send_frame(&output, &[]).await;

    let activity = output.network_activity();
    assert!(activity.art_poll_listeners.contains(&listener));
    assert_eq!(activity.art_pollers.len(), 1);
    assert_eq!(activity.art_pollers[0].address, peer.local_addr().unwrap());
    assert_eq!(activity.art_pollers[0].polls, 1);
    assert_eq!(activity.art_net_senders.len(), 1);
    assert_eq!(
        activity.art_net_senders[0].address,
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    );
    assert_eq!(activity.art_net_senders[0].universes, [0x0102]);

    // A listener whose own address is the sender ignores the frames as the desk's own.
    let echo = NetworkOutput::bind(IpAddr::V4(Ipv4Addr::LOCALHOST), [13; 16], "Light")
        .await
        .unwrap()
        .listen_for_art_polls(
            SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
            Ipv4Addr::LOCALHOST,
        )
        .unwrap();
    let echo_listener = *echo.art_poll_addresses().last().unwrap();
    peer.send_to(&artdmx_packet(3, 1, &[0; 24]), echo_listener)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    send_frame(&echo, &[]).await;
    assert!(echo.network_activity().art_net_senders.is_empty());
}

#[tokio::test]
async fn other_sacn_sources_are_heard_by_announcement_and_data_but_the_desk_itself_is_not() {
    let sacn_bind = local_socket().await;
    let sacn_address = sacn_bind.local_addr().unwrap();
    drop(sacn_bind);
    let output = NetworkOutput::bind(IpAddr::V4(Ipv4Addr::LOCALHOST), [14; 16], "Light")
        .await
        .unwrap()
        .listen_for_sacn(sacn_address)
        .unwrap();
    let peer = local_socket().await;
    let announcement = light_dmx_wire::sacn_discovery_packets([15; 16], "Other console", &[9, 12]);
    peer.send_to(&announcement[0], sacn_address).await.unwrap();
    let data =
        light_output::sacn_data_packet(30, 1, &[0; 8], [15; 16], "Other console", 100, false);
    peer.send_to(&data, sacn_address).await.unwrap();
    // The desk's own CID is skipped, as its multicast loopback would be.
    let own = light_output::sacn_data_packet(9, 1, &[0; 8], [14; 16], "Light", 100, false);
    peer.send_to(&own, sacn_address).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    send_frame(&output, &[]).await;

    let activity = output.network_activity();
    assert_eq!(activity.sacn_discovery_listener, Some(sacn_address));
    assert_eq!(activity.sacn_sources.len(), 1);
    let source = &activity.sacn_sources[0];
    assert_eq!(source.name, "Other console");
    assert_eq!(source.cid, "0f".repeat(16));
    assert_eq!(source.universes, [9, 12, 30]);
}
