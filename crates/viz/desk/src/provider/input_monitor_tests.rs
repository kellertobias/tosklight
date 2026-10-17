use super::*;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use viz_dmx::{Delivery, InputMapping, Protocol};

fn mapping(port: u16) -> InputMapping {
    InputMapping {
        id: "monitor-test".into(),
        protocol: Protocol::ArtNet,
        logical_universe: 1,
        destination_universe: 1,
        delivery: Delivery::Unicast,
        bind: SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        priority: 100,
        enabled: true,
    }
}

#[test]
fn passive_monitor_observes_real_slots_and_held_receipts_without_second_drain() {
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let (mut provider, _outbox) = DeskProvider::detached(DeskConnection::default(), Instant::now());
    provider.set_input_monitoring(true);
    let scene = viz_scene::Scene {
        show_id: Some(uuid::Uuid::nil()),
        source_identity: "http://127.0.0.1:1234".into(),
        source_show_revision: 7,
        ..Default::default()
    };
    provider.adopt_scene(
        scene,
        vec![],
        None,
        vec![],
        vec![mapping(port)],
        ProviderDiagnostics::default(),
    );
    let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    for sequence in 1..=2 {
        let mut packet = b"Art-Net\0".to_vec();
        packet.extend_from_slice(&0x5000_u16.to_le_bytes());
        packet.extend_from_slice(&[0, 14, sequence, 0, 1, 0, 0, 2, 123, 42]);
        sender
            .send_to(&packet, (Ipv4Addr::LOCALHOST, port))
            .unwrap();
        std::thread::sleep(Duration::from_millis(25));
        provider.poll();
    }
    let report = provider.input_monitor_snapshot().unwrap();
    assert!(report.validate());
    assert_eq!(report.context.show_revision, 7);
    assert_eq!(
        report.received.universes[0].slots.as_ref().unwrap()[..2],
        [123, 42]
    );
    assert_eq!(report.received.inputs[0].accepted_packets, 2);
    assert!(
        report.received.inputs[0]
            .source
            .as_ref()
            .unwrap()
            .contains(&sender.local_addr().unwrap().to_string())
    );
    assert!(
        provider
            .receivers
            .as_ref()
            .unwrap()
            .drain_changed()
            .is_empty(),
        "monitor did not steal or refresh decoder's drain"
    );
    let newest = provider.values.newest_input_micros;
    for _ in 0..3 {
        provider.poll();
        assert_eq!(
            provider.input_monitor_snapshot().unwrap().received.inputs[0].accepted_packets,
            2
        );
        assert_eq!(
            provider.values.newest_input_micros, newest,
            "monitor polling is not another receipt"
        );
    }
    std::thread::sleep(Duration::from_millis(2600));
    provider.poll();
    let held = provider.input_monitor_snapshot().unwrap();
    assert!(!held.received.universes[0].live);
    assert_eq!(
        held.received.universes[0].slots,
        report.received.universes[0].slots
    );
    assert_eq!(held.received.inputs[0].accepted_packets, 2);
    provider.shutdown();
}

#[test]
fn monitor_is_opt_in_and_context_tracks_actual_receiver_rebinding() {
    let (mut provider, _outbox) = DeskProvider::detached(DeskConnection::default(), Instant::now());
    assert!(provider.monitor_frames.is_none());
    assert!(provider.input_monitor_snapshot().is_none());
    provider.set_input_monitoring(true);
    let scene = viz_scene::Scene {
        show_id: Some(uuid::Uuid::nil()),
        source_identity: "first".into(),
        source_show_revision: 1,
        ..Default::default()
    };
    provider.adopt_scene(
        scene.clone(),
        vec![],
        None,
        vec![],
        vec![],
        ProviderDiagnostics::default(),
    );
    provider
        .monitor_frames
        .as_mut()
        .unwrap()
        .insert(1, vec![9; 512]);
    let mut changed = scene;
    changed.source_show_revision = 2;
    provider.apply_delta(
        changed,
        vec![],
        None,
        vec![],
        vec![mapping(0)],
        ProviderDiagnostics::default(),
    );
    let report = provider.input_monitor_snapshot().unwrap();
    assert_eq!(report.context.show_revision, 2);
    assert_eq!(report.context.mappings, vec![format!("{:?}", mapping(0))]);
    assert!(
        report
            .received
            .universes
            .iter()
            .all(|row| row.slots.is_none())
    );
    provider.set_input_monitoring(false);
    assert!(provider.input_monitor_snapshot().is_none());
    provider.shutdown();
}
