//! Real network input through the provider, including a held look and source rejection.
use super::*;
use std::net::{Ipv4Addr, UdpSocket};

fn packet(sequence: u8, red: u8) -> Vec<u8> {
    let mut packet = b"Art-Net\0".to_vec();
    packet.extend_from_slice(&0x5000_u16.to_le_bytes());
    packet.extend_from_slice(&14_u16.to_be_bytes());
    packet.extend_from_slice(&[sequence, 0]);
    packet.extend_from_slice(&1_u16.to_le_bytes());
    packet.extend_from_slice(&8_u16.to_be_bytes());
    packet.extend_from_slice(&[red, 255, 0, 0, 0, 0, 0, 0]);
    packet
}

fn rig() -> (DeskProvider, u16) {
    let port = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let (mut provider, _) = DeskProvider::detached(DeskConnection::default(), Instant::now());
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library/cameo--root-par-6.toskfixture");
    let profile =
        Arc::new(light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap());
    let mode_id = profile
        .modes
        .iter()
        .find(|mode| mode.channels.len() == 7)
        .unwrap()
        .id;
    let plan = viz_project::compile(&[viz_project::PatchedFixture {
        fixture_id: uuid::Uuid::new_v4(),
        name: "Freshness".into(),
        number: Some(1),
        profile,
        mode_id,
        instances: vec![viz_project::PhysicalInstance {
            instance_id: uuid::Uuid::new_v4(),
            name: "Freshness".into(),
            split_patches: vec![(1, Some((1, 1)))],
            position: glam::Vec3::ZERO,
            rotation_degrees: glam::Vec3::ZERO,
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 0.,
            shaper_angle: None,
            installed_appearance: Default::default(),
            scenery_size_metres: None,
            scenery_options: Default::default(),
            model_scale: 1.,
            color_calibration: None,
            position_calibration: None,
        }],
    }]);
    let mut mapping = viz_dmx::InputMapping::loopback(viz_dmx::Protocol::ArtNet, 1, 1);
    mapping.bind.set_port(port);
    provider.adopt_scene(
        plan.scene,
        plan.bindings,
        plan.external_camera,
        plan.position_points,
        vec![mapping],
        ProviderDiagnostics::default(),
    );
    provider.poll();
    (provider, port)
}

fn receive(provider: &mut DeskProvider, accepted: u64, rejected: bool) -> Vec<ProviderEvent> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut all = Vec::new();
    while Instant::now() < deadline {
        let events = provider.poll();
        let complete = events.iter().any(|event| match event {
            ProviderEvent::Diagnostics(diagnostics) => diagnostics.inputs.iter().any(|input| {
                if rejected {
                    input.duplicate_packets > 0
                } else {
                    input.accepted_packets >= accepted
                }
            }),
            _ => false,
        });
        all.extend(events);
        if complete {
            return all;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("network receipt did not reach provider diagnostics");
}

fn freshness(events: &[ProviderEvent]) -> u64 {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            ProviderEvent::Diagnostics(diagnostics) => serde_json::to_value(diagnostics)
                .unwrap()
                .get("newest_input_micros")
                .and_then(serde_json::Value::as_u64),
            _ => None,
        })
        .unwrap_or(0)
}

#[test]
fn held_winning_input_freshness_reaches_consumer_without_a_value_revision() {
    let (mut provider, port) = rig();
    let winner = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let loser = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    winner
        .send_to(&packet(1, 40), (Ipv4Addr::LOCALHOST, port))
        .unwrap();
    let initial = receive(&mut provider, 1, false);
    let first = freshness(&initial);
    assert!(first > 0, "winning receipt timing must reach the consumer");
    let values = initial
        .iter()
        .find_map(|event| match event {
            ProviderEvent::Values(values) => Some(values.clone()),
            _ => None,
        })
        .expect("initial live values");
    let initial_frame = values.frame;
    assert!(values.emitters.iter().any(|emitter| emitter.intensity > 0.));
    for sequence in 2..=4 {
        std::thread::sleep(Duration::from_millis(15));
        winner
            .send_to(&packet(sequence, 40), (Ipv4Addr::LOCALHOST, port))
            .unwrap();
        let held = receive(&mut provider, u64::from(sequence), false);
        assert!(freshness(&held) > first);
        assert!(
            held.iter()
                .all(|event| !matches!(event, ProviderEvent::Values(_)))
        );
        assert_eq!(provider.values.frame, values.frame);
        assert_eq!(
            provider.values.emitters[0].intensity,
            values.emitters[0].intensity
        );
    }
    let held_stamp = freshness(&provider.poll());
    loser
        .send_to(&packet(5, 200), (Ipv4Addr::LOCALHOST, port))
        .unwrap();
    let rejected = receive(&mut provider, 0, true);
    assert_eq!(
        freshness(&rejected),
        held_stamp,
        "loser must not refresh winning timing"
    );
    assert!(
        rejected
            .iter()
            .all(|event| !matches!(event, ProviderEvent::Values(_)))
    );
    winner
        .send_to(&packet(6, 200), (Ipv4Addr::LOCALHOST, port))
        .unwrap();
    let changed = receive(&mut provider, 5, false);
    assert!(freshness(&changed) > held_stamp);
    assert!(changed.iter().any(
        |event| matches!(event, ProviderEvent::Values(values) if values.frame > initial_frame)
    ));
    assert!(
        provider.values.emitters[0].source_primaries[0] > values.emitters[0].source_primaries[0]
    );
}
