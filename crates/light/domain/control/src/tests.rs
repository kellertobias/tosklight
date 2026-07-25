use crate::*;
use chrono::Utc;
use std::time::Duration;

#[test]
fn parses_art_timecode_with_stream_identity() {
    let packet = [
        b'A', b'r', b't', b'-', b'N', b'e', b't', 0, 0x00, 0x97, 0, 14, 0, 2, 12, 34, 56, 7, 1,
    ];
    let result = parse_art_timecode(&packet, "10.0.0.1").unwrap();
    assert_eq!(
        (result.hours, result.minutes, result.seconds, result.frames),
        (7, 56, 34, 12)
    );
    assert_eq!(result.rate, FrameRate::Fps25);
    assert_eq!(result.source, "artnet:10.0.0.1:2");
}

#[test]
fn rejects_out_of_range_art_timecode() {
    let mut packet = [
        b'A', b'r', b't', b'-', b'N', b'e', b't', 0, 0x00, 0x97, 0, 14, 0, 0, 24, 0, 0, 0, 0,
    ];
    assert!(parse_art_timecode(&packet, "source").is_err());
    packet[14] = 23;
    assert!(parse_art_timecode(&packet, "source").is_ok());
}

#[test]
fn decodes_complete_mtc_quarter_frame_cycle() {
    let mut decoder = MidiTimecodeDecoder::default();
    let messages = [0x02, 0x11, 0x23, 0x30, 0x44, 0x50, 0x66, 0x74];
    let mut result = None;
    for message in messages {
        result = decoder
            .push_quarter_frame(message, "port-1")
            .unwrap()
            .or(result);
    }
    let result = result.unwrap();
    assert_eq!(
        (result.hours, result.minutes, result.seconds, result.frames),
        (6, 4, 3, 18)
    );
    assert_eq!(result.rate, FrameRate::Fps2997Drop);
}

#[test]
fn parses_typed_osc_message() {
    let packet = b"/light/go\0\0\0,ifsT\0\0\0\0\0\0*?\xc0\0\0main\0\0\0\0";
    let result = parse_osc_message(packet).unwrap();
    assert_eq!(
        result,
        ControlEvent::Osc {
            address: "/light/go".into(),
            arguments: vec![
                OscArgument::Int(42),
                OscArgument::Float(1.5),
                OscArgument::String("main".into()),
                OscArgument::Bool(true)
            ],
            source: None,
        }
    );
}

#[test]
fn encoded_osc_message_round_trips_supported_arguments() {
    let arguments = vec![
        OscArgument::Int(7),
        OscArgument::Float(0.5),
        OscArgument::String("slow".into()),
        OscArgument::Bool(false),
    ];
    let packet = encode_osc_message("/light/test", &arguments).unwrap();
    assert_eq!(
        parse_osc_message(&packet).unwrap(),
        ControlEvent::Osc {
            address: "/light/test".into(),
            arguments,
            source: None
        }
    );
}

fn tc(source: &str) -> SmpteTimecode {
    SmpteTimecode {
        hours: 0,
        minutes: 0,
        seconds: 0,
        frames: 0,
        rate: FrameRate::Fps25,
        source: source.into(),
        received_at: Utc::now(),
    }
}

#[test]
fn timecode_router_obeys_priority_and_explicit_fallback() {
    let mut router = TimecodeRouter::default();
    router.configure(vec![
        TimecodeSourceConfig {
            source_prefix: "osc:".into(),
            priority: 10,
            fallback: true,
            loss_timeout_millis: 1000,
        },
        TimecodeSourceConfig {
            source_prefix: "midi:".into(),
            priority: 20,
            fallback: false,
            loss_timeout_millis: 0,
        },
    ]);
    router.ingest(tc("osc:backup"));
    assert_eq!(router.active_source(), Some("osc:backup"));
    router.ingest(tc("midi:primary"));
    assert_eq!(router.active_source(), Some("midi:primary"));
    std::thread::sleep(Duration::from_millis(1));
    router.poll_loss();
    assert_eq!(router.active_source(), Some("osc:backup"));
}

#[test]
fn parses_rtp_midi_commands_and_running_status() {
    let mut packet = vec![
        0x80, 0x61, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0x06, 0x90, 60, 127, 0, 61, 0,
    ];
    let messages = parse_rtp_midi(&packet).unwrap();
    assert_eq!(messages, vec![vec![0x90, 60, 127], vec![0x90, 61, 0]]);
    packet[12] = 0x20 | 0x07;
    packet.insert(13, 0);
    assert_eq!(parse_rtp_midi(&packet).unwrap().len(), 2);
}

#[test]
fn rejects_truncated_rtp_midi_command_section() {
    let packet = [0x80, 0x61, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0x0f, 0x90, 60];
    assert!(parse_rtp_midi(&packet).is_err());
}
