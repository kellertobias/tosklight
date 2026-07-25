use light_output::{DMX_SLOTS, Protocol, artdmx_packet, next_sequence, sacn_data_packet};
use std::collections::HashMap;

#[test]
fn artdmx_packet_matches_protocol_fields() {
    let mut frame = [0; DMX_SLOTS];
    frame[0] = 0xaa;
    let packet = artdmx_packet(0x1234, 7, &frame);
    assert_eq!(packet.len(), 530);
    assert_eq!(&packet[..8], b"Art-Net\0");
    assert_eq!(&packet[8..10], &[0x00, 0x50]);
    assert_eq!(&packet[10..12], &[0x00, 0x0e]);
    assert_eq!(packet[12], 7);
    assert_eq!(&packet[14..16], &[0x34, 0x12]);
    assert_eq!(&packet[16..18], &[0x02, 0x00]);
    assert_eq!(packet[18], 0xaa);
}

#[test]
fn sacn_packet_matches_e131_layers() {
    let packet = sacn_data_packet(0x1234, 9, &[0x55; DMX_SLOTS], [1; 16], "Light", 100, false);
    assert_eq!(packet.len(), 638);
    assert_eq!(&packet[4..16], b"ASC-E1.17\0\0\0");
    assert_eq!(&packet[18..22], &[0, 0, 0, 4]);
    assert_eq!(&packet[40..44], &[0, 0, 0, 2]);
    assert_eq!(packet[108], 100);
    assert_eq!(packet[111], 9);
    assert_eq!(&packet[113..115], &[0x12, 0x34]);
    assert_eq!(packet[117], 0x02);
    assert_eq!(packet[118], 0xa1);
    assert_eq!(&packet[123..125], &[0x02, 0x01]);
    assert_eq!(packet[125], 0);
    assert_eq!(packet[126], 0x55);
}

#[test]
fn sacn_termination_sets_option() {
    let packet = sacn_data_packet(1, 1, &[0; DMX_SLOTS], [0; 16], "Light", 100, true);
    assert_eq!(packet[112], 0x40);
}

#[test]
fn sequence_never_emits_zero_after_wrap() {
    let mut values = HashMap::from([((Protocol::ArtNet, 1), 255)]);
    assert_eq!(next_sequence(&mut values, (Protocol::ArtNet, 1)), 1);
}

#[test]
fn sequences_are_independent_per_protocol_and_universe() {
    let mut values = HashMap::new();
    assert_eq!(next_sequence(&mut values, (Protocol::ArtNet, 1)), 1);
    assert_eq!(next_sequence(&mut values, (Protocol::ArtNet, 1)), 2);
    assert_eq!(next_sequence(&mut values, (Protocol::Sacn, 1)), 1);
    assert_eq!(next_sequence(&mut values, (Protocol::ArtNet, 2)), 1);
}
