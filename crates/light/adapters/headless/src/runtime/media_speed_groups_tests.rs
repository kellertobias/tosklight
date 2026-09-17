use super::super::{SoundToLightConfig, SpeedGroupController};

use super::*;

/// The contract's reference datagram: source `desk`, sequence 1, group 1, 120 BPM, beat phase
/// 0.5, running. The Media Server's decoder asserts the same bytes, so the two sides cannot drift
/// apart unnoticed.
const REFERENCE_DATAGRAM: &str = "2f746f736b6c696768742f73706565642d67726f757000002c736969666669006465736b00000000000000010000000142f000003f00000000000001";

fn snapshot(bpm: f64) -> SpeedSnapshot {
    SpeedGroupController::new(bpm, SoundToLightConfig::default())
        .expect("a valid controller")
        .snapshot(0)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn the_desk_encodes_the_reference_datagram() {
    let mut publisher = SpeedGroupPublisher::new("desk".to_owned());
    let mut group = snapshot(120.0);
    group.beat_phase = 0.5;
    let packets = publisher.packets(&[group]);
    assert_eq!(hex(&packets[0]), REFERENCE_DATAGRAM);
}

#[test]
fn every_group_is_sent_numbered_from_one_with_a_rising_sequence() {
    let mut publisher = SpeedGroupPublisher::new("desk".to_owned());
    let groups: Vec<SpeedSnapshot> = (0..5)
        .map(|index| snapshot(100.0 + f64::from(index)))
        .collect();
    let first = publisher.packets(&groups);
    let second = publisher.packets(&groups);
    assert_eq!(first.len(), 5);

    let field = |packet: &[u8], offset: usize| {
        i32::from_be_bytes(packet[offset..offset + 4].try_into().unwrap())
    };
    // Address (24) + tags (8) + "desk" (8) put the sequence at 40 and the group at 44.
    let sequences: Vec<i32> = first
        .iter()
        .chain(&second)
        .map(|packet| field(packet, 40))
        .collect();
    assert_eq!(sequences, (1..=10).collect::<Vec<_>>());
    let numbers: Vec<i32> = first.iter().map(|packet| field(packet, 44)).collect();
    assert_eq!(numbers, vec![1, 2, 3, 4, 5]);
}

#[test]
fn a_paused_group_is_sent_as_not_running_with_its_rate() {
    let mut publisher = SpeedGroupPublisher::new("desk".to_owned());
    let mut paused = snapshot(128.0);
    paused.paused = true;
    let packet = &publisher.packets(&[paused])[0];
    let bpm = f32::from_be_bytes(packet[48..52].try_into().unwrap());
    let running = i32::from_be_bytes(packet[56..60].try_into().unwrap());
    assert_eq!(bpm, 128.0);
    assert_eq!(running, 0);
}

#[test]
fn values_are_kept_inside_the_contract() {
    let mut publisher = SpeedGroupPublisher::new("desk".to_owned());
    let mut wild = snapshot(120.0);
    wild.effective_bpm = f64::NAN;
    wild.beat_phase = 0.999_999_99;
    let packet = &publisher.packets(&[wild])[0];
    let bpm = f32::from_be_bytes(packet[48..52].try_into().unwrap());
    let phase = f32::from_be_bytes(packet[52..56].try_into().unwrap());
    assert_eq!(bpm, 0.0);
    assert!((0.0..1.0).contains(&phase), "{phase}");

    wild.effective_bpm = 5_000.0;
    wild.beat_phase = -0.25;
    let packet = &publisher.packets(&[wild])[0];
    assert_eq!(
        f32::from_be_bytes(packet[48..52].try_into().unwrap()),
        999.0
    );
    assert_eq!(f32::from_be_bytes(packet[52..56].try_into().unwrap()), 0.75);
}

#[tokio::test]
async fn datagrams_reach_a_listening_media_server() {
    let media = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let desk = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let mut publisher = SpeedGroupPublisher::new("desk".to_owned());
    for packet in publisher.packets(&[snapshot(120.0)]) {
        desk.send_to(&packet, media.local_addr().unwrap())
            .await
            .unwrap();
    }
    let mut buffer = [0u8; 512];
    let (length, _) = media.recv_from(&mut buffer).await.unwrap();
    assert!(buffer[..length].starts_with(b"/tosklight/speed-group\0\0,siiffi\0"));
}
