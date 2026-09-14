use light_output::{DMX_SLOTS, DeliveryMode, NetworkOutput, OutputRoute, Protocol};
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

async fn local_receiver() -> UdpSocket {
    UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap()
}

#[tokio::test]
async fn route_scoped_failure_is_observable_without_stopping_healthy_output() {
    let output = NetworkOutput::bind(IpAddr::V4(Ipv4Addr::LOCALHOST), [7; 16], "Light")
        .await
        .unwrap();
    let healthy = local_receiver().await;
    let failing = local_receiver().await;
    let healthy_destination = healthy.local_addr().unwrap();
    let failing_destination = failing.local_addr().unwrap();
    let routes = [
        unicast_route(Protocol::ArtNet, 10, healthy_destination),
        unicast_route(Protocol::ArtNet, 11, failing_destination),
    ];
    let frames = HashMap::from([(1, [0x44; DMX_SLOTS])]);
    let mut sequences = HashMap::new();

    output.inject_failure(failing_destination, true);
    let sent = output
        .send_routes(&routes, &frames, &HashMap::from([(1, 512)]), &mut sequences)
        .await
        .unwrap();
    assert_eq!(sent, 1);
    assert_payload(&healthy, 18, 0x44).await;

    let errors = output.route_send_errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].protocol, Protocol::ArtNet);
    assert_eq!(errors[0].universe, 11);
    assert_eq!(errors[0].destination, failing_destination);
    assert_eq!(errors[0].errors, 1);
    assert_eq!(output.take_send_errors(), 1);

    output.inject_failure(failing_destination, false);
    let sent = output
        .send_routes(&routes, &frames, &HashMap::from([(1, 512)]), &mut sequences)
        .await
        .unwrap();
    assert_eq!(sent, 2);
    assert_payload(&failing, 18, 0x44).await;
    assert_eq!(output.route_send_errors()[0].errors, 1);
}

#[tokio::test]
async fn shutdown_sends_three_sacn_termination_packets_and_no_artnet_black_frame() {
    let output = NetworkOutput::bind(IpAddr::V4(Ipv4Addr::LOCALHOST), [8; 16], "Light")
        .await
        .unwrap();
    let artnet = local_receiver().await;
    let sacn = local_receiver().await;
    let routes = [
        unicast_route(Protocol::ArtNet, 10, artnet.local_addr().unwrap()),
        unicast_route(Protocol::Sacn, 20, sacn.local_addr().unwrap()),
    ];
    let mut sequences = HashMap::new();

    output
        .terminate_routes(&routes, &mut sequences)
        .await
        .unwrap();

    for _ in 0..3 {
        let packet = receive_packet(&sacn).await;
        assert_eq!(packet.len(), 126 + DMX_SLOTS);
        assert_eq!(packet[111], 1);
        assert_eq!(packet[112], 0x40);
        assert!(packet[126..].iter().all(|slot| *slot == 0));
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(30), receive_packet(&artnet))
            .await
            .is_err()
    );
    assert_eq!(sequences.get(&(Protocol::Sacn, 20)), Some(&1));
    assert!(!sequences.contains_key(&(Protocol::ArtNet, 10)));
}

#[tokio::test]
async fn an_art_poll_is_answered_with_every_art_net_universe_the_desk_sends() {
    let output = NetworkOutput::bind(IpAddr::V4(Ipv4Addr::LOCALHOST), [9; 16], "Light")
        .await
        .unwrap()
        .listen_for_art_polls(
            SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
            Ipv4Addr::LOCALHOST,
        )
        .unwrap();
    let poll_address = *output.art_poll_addresses().last().unwrap();
    let sink = local_receiver().await.local_addr().unwrap();
    let poller = local_receiver().await;
    let routes = [
        unicast_route(Protocol::ArtNet, 0x21, sink),
        unicast_route(Protocol::ArtNet, 3, sink),
        unicast_route(Protocol::Sacn, 9, sink),
    ];
    let mut poll = [0_u8; 14];
    poll[..8].copy_from_slice(b"Art-Net\0");
    poll[8..10].copy_from_slice(&0x2000_u16.to_le_bytes());
    poll[11] = 14;
    poller.send_to(&poll, poll_address).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    // The poll is answered on the next output frame.
    output
        .send_routes(
            &routes,
            &HashMap::new(),
            &HashMap::new(),
            &mut HashMap::new(),
        )
        .await
        .unwrap();

    let first = receive_packet(&poller).await;
    let second = receive_packet(&poller).await;
    for reply in [&first, &second] {
        assert_eq!(&reply[8..10], &0x2100_u16.to_le_bytes());
        assert_eq!(&reply[10..14], &[127, 0, 0, 1]);
        assert_eq!(&reply[26..32], b"Light\0");
        assert_eq!(reply[173], 1);
        assert_eq!(reply[174], 0x40);
    }
    // Universe 3 on sub-net 0, then 0x21 on sub-net 2: one reply each.
    assert_eq!((first[19], first[186], first[211]), (0, 3, 1));
    assert_eq!((second[19], second[186], second[211]), (2, 1, 2));
}

#[tokio::test]
async fn sacn_universes_are_announced_at_once_and_again_only_when_they_change() {
    let output = NetworkOutput::bind(IpAddr::V4(Ipv4Addr::LOCALHOST), [10; 16], "Light")
        .await
        .unwrap();
    let discovery = local_receiver().await;
    output.redirect_sacn_discovery(discovery.local_addr().unwrap());
    let sink = local_receiver().await.local_addr().unwrap();
    let mut routes = vec![
        unicast_route(Protocol::Sacn, 20, sink),
        unicast_route(Protocol::Sacn, 7, sink),
        unicast_route(Protocol::ArtNet, 5, sink),
    ];
    let mut sequences = HashMap::new();
    let frames = HashMap::new();
    let slots = HashMap::new();

    output
        .send_routes(&routes, &frames, &slots, &mut sequences)
        .await
        .unwrap();
    let packet = receive_packet(&discovery).await;
    assert_eq!(&packet[18..22], &8_u32.to_be_bytes());
    assert_eq!(&packet[22..38], &[10; 16]);
    assert_eq!(&packet[114..118], &1_u32.to_be_bytes());
    assert_eq!(&packet[120..], &[0, 7, 0, 20]);

    // Within the ten-second interval an unchanged set is not repeated.
    output
        .send_routes(&routes, &frames, &slots, &mut sequences)
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(30), receive_packet(&discovery))
            .await
            .is_err()
    );

    routes[1].enabled = false;
    output
        .send_routes(&routes, &frames, &slots, &mut sequences)
        .await
        .unwrap();
    assert_eq!(&receive_packet(&discovery).await[120..], &[0, 20]);
}

async fn assert_payload(socket: &UdpSocket, offset: usize, value: u8) {
    let packet = receive_packet(socket).await;
    assert_eq!(packet[offset], value);
}

async fn receive_packet(socket: &UdpSocket) -> Vec<u8> {
    let mut packet = vec![0_u8; 126 + DMX_SLOTS];
    let (length, _) = tokio::time::timeout(Duration::from_secs(1), socket.recv_from(&mut packet))
        .await
        .unwrap()
        .unwrap();
    packet.truncate(length);
    packet
}
