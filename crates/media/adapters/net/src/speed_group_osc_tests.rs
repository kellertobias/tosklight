use super::*;

fn update(group: u32, bpm: f64) -> SpeedGroupUpdate {
    SpeedGroupUpdate {
        source: "desk-a".to_owned(),
        sequence: 7,
        group,
        bpm,
        beat_phase: 0.25,
        running: true,
    }
}

fn message(tags: &str, arguments: &[&[u8]]) -> Vec<u8> {
    let mut packet = Vec::new();
    push_string(&mut packet, SPEED_GROUP_OSC_ADDRESS);
    push_string(&mut packet, tags);
    for argument in arguments {
        packet.extend_from_slice(argument);
    }
    packet
}

fn string(value: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_string(&mut bytes, value);
    bytes
}

#[test]
fn an_encoded_update_decodes_to_itself() {
    let sent = update(3, 128.0);
    assert_eq!(decode(&encode(&sent)).unwrap(), vec![sent]);
}

#[test]
fn every_padding_length_of_the_source_round_trips() {
    for source in ["a", "ab", "abc", "abcd", "desk-7f3a"] {
        let sent = SpeedGroupUpdate {
            source: source.to_owned(),
            ..update(1, 90.0)
        };
        assert_eq!(decode(&encode(&sent)).unwrap(), vec![sent], "{source}");
    }
}

#[test]
fn doubles_and_boolean_tags_are_accepted() {
    let packet = message(
        ",siiddF",
        &[
            &string("desk-a"),
            &1i32.to_be_bytes(),
            &2i32.to_be_bytes(),
            &140.5f64.to_be_bytes(),
            &0.5f64.to_be_bytes(),
        ],
    );
    let decoded = decode(&packet).unwrap();
    assert_eq!(decoded[0].bpm, 140.5);
    assert_eq!(decoded[0].beat_phase, 0.5);
    assert!(!decoded[0].running);
}

#[test]
fn a_bundle_carries_every_group_in_order() {
    let mut packet = b"#bundle\0".to_vec();
    packet.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    for group in 1..=5 {
        let element = encode(&update(group, 100.0 + f64::from(group)));
        packet.extend_from_slice(&(element.len() as i32).to_be_bytes());
        packet.extend_from_slice(&element);
    }
    let groups: Vec<u32> = decode(&packet)
        .unwrap()
        .iter()
        .map(|update| update.group)
        .collect();
    assert_eq!(groups, vec![1, 2, 3, 4, 5]);
}

#[test]
fn invalid_datagrams_say_what_is_wrong() {
    let mut other = Vec::new();
    push_string(&mut other, "/light/desk/feedback/speed-group/1");
    push_string(&mut other, ",i");
    other.extend_from_slice(&120i32.to_be_bytes());
    assert!(matches!(
        decode(&other),
        Err(SpeedGroupDecodeError::Address(_))
    ));

    let wrong_tags = message(",sii", &[&string("x"), &[0; 8]]);
    assert!(matches!(
        decode(&wrong_tags),
        Err(SpeedGroupDecodeError::TypeTags(_))
    ));

    let full = encode(&update(1, 120.0));
    assert!(matches!(
        decode(&full[..full.len() - 2]),
        Err(SpeedGroupDecodeError::Malformed(_))
    ));
    assert!(matches!(
        decode(b"garbage"),
        Err(SpeedGroupDecodeError::Malformed(_))
    ));
    assert!(matches!(
        decode(&[]),
        Err(SpeedGroupDecodeError::Malformed(_))
    ));

    let mut trailing = full.clone();
    trailing.extend_from_slice(&[0; 4]);
    assert!(matches!(
        decode(&trailing),
        Err(SpeedGroupDecodeError::Malformed(_))
    ));

    let negative_group = message(
        ",siiffi",
        &[
            &string("x"),
            &0i32.to_be_bytes(),
            &0i32.to_be_bytes(),
            &120f32.to_be_bytes(),
            &0f32.to_be_bytes(),
            &1i32.to_be_bytes(),
        ],
    );
    assert_eq!(
        decode(&negative_group),
        Err(SpeedGroupDecodeError::Group(0))
    );

    let negative_sequence = message(
        ",siiffi",
        &[
            &string("x"),
            &(-1i32).to_be_bytes(),
            &1i32.to_be_bytes(),
            &120f32.to_be_bytes(),
            &0f32.to_be_bytes(),
            &1i32.to_be_bytes(),
        ],
    );
    assert_eq!(
        decode(&negative_sequence),
        Err(SpeedGroupDecodeError::Sequence(-1))
    );
}

#[test]
fn a_bundle_with_a_lying_element_size_is_refused() {
    let mut packet = b"#bundle\0".to_vec();
    packet.extend_from_slice(&[0; 8]);
    packet.extend_from_slice(&1000i32.to_be_bytes());
    packet.extend_from_slice(&encode(&update(1, 120.0)));
    assert!(matches!(
        decode(&packet),
        Err(SpeedGroupDecodeError::Malformed(_))
    ));
}

#[tokio::test]
async fn the_listener_receives_what_a_desk_sends() {
    let mut listener = SpeedGroupListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = listener.local_address().unwrap();
    let desk = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    desk.send_to(&encode(&update(2, 126.0)), address).unwrap();
    desk.send_to(b"nonsense", address).unwrap();

    let (from, first) = listener.receive().await;
    assert_eq!(from, desk.local_addr().unwrap());
    assert_eq!(first.unwrap(), vec![update(2, 126.0)]);
    let (_, second) = listener.receive().await;
    assert!(second.is_err());
}

#[test]
fn a_taken_port_is_reported_rather_than_shared() {
    let taken = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let error = SpeedGroupListener::bind(taken.local_addr().unwrap()).unwrap_err();
    assert!(error.to_string().contains("Speed Groups"), "{error}");
}

/// The contract's reference datagram, byte for byte as Tos Light Control encodes it (asserted on
/// the desk side in `media_speed_groups_tests.rs`).
const REFERENCE_DATAGRAM: &str = "2f746f736b6c696768742f73706565642d67726f757000002c736969666669006465736b00000000000000010000000142f000003f00000000000001";

#[test]
fn the_desks_reference_datagram_decodes() {
    let bytes: Vec<u8> = (0..REFERENCE_DATAGRAM.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&REFERENCE_DATAGRAM[index..index + 2], 16).unwrap())
        .collect();
    let expected = SpeedGroupUpdate {
        source: "desk".to_owned(),
        sequence: 1,
        group: 1,
        bpm: 120.0,
        beat_phase: 0.5,
        running: true,
    };
    assert_eq!(decode(&bytes).unwrap(), vec![expected.clone()]);
    assert_eq!(encode(&expected), bytes);
}
