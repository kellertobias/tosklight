use light_output::{DMX_SLOTS, DeliveryMode, NetworkOutput, OutputRoute, Protocol};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio::net::UdpSocket;

fn unicast_route(protocol: Protocol, universe: u16, destination: SocketAddr) -> OutputRoute {
    OutputRoute {
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
