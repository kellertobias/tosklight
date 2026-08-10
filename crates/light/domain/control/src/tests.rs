use crate::*;
use chrono::Utc;
use std::time::{Duration, Instant};

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
fn timecode_router_uses_only_the_explicit_source_and_relocks_after_loss() {
    let mut router = TimecodeRouter::default();
    router.configure(TimecodeRouterConfig {
        selected_source: TimecodeSourceSelection::External {
            source: "extension:primary:frame".into(),
        },
        desk_rate: FrameRate::Fps25,
        external_loss_policy: ExternalTimecodeLossPolicy::Pause,
        loss_timeout_millis: 10,
    });
    let start = Instant::now();
    assert!(router.ingest_at(tc("osc:backup"), start).is_none());
    assert_eq!(router.active_source(), None);

    router.ingest_at(tc("extension:primary:frame"), start);
    assert_eq!(router.active_source(), Some("extension:primary:frame"));
    assert_eq!(
        router.take_transition(),
        Some(TimecodeSourceTransition::ExternalLocked {
            source: "extension:primary:frame".into()
        })
    );

    router.poll_loss_at(start + Duration::from_millis(11));
    assert_eq!(router.active_source(), None);
    assert!(!router.uses_internal_clock());
    assert_eq!(
        router.take_transition(),
        Some(TimecodeSourceTransition::ExternalLost {
            policy: ExternalTimecodeLossPolicy::Pause
        })
    );

    router.ingest_at(
        tc("extension:primary:frame"),
        start + Duration::from_millis(12),
    );
    assert_eq!(router.active_source(), Some("extension:primary:frame"));
    assert!(matches!(
        router.take_transition(),
        Some(TimecodeSourceTransition::ExternalLocked { .. })
    ));
}

#[test]
fn timecode_router_converts_known_rates_and_exposes_a_warning() {
    let mut router = TimecodeRouter::default();
    router.configure(TimecodeRouterConfig {
        selected_source: TimecodeSourceSelection::External {
            source: "artnet:10.0.0.1:2".into(),
        },
        desk_rate: FrameRate::Fps30,
        ..TimecodeRouterConfig::default()
    });
    let mut incoming = tc("artnet:10.0.0.1:2");
    incoming.frames = 24;
    let converted = router.ingest(incoming).unwrap();
    assert_eq!(converted.rate, FrameRate::Fps30);
    assert_eq!(converted.frames, 28);
    assert_eq!(
        router.rate_warning(),
        Some(&TimecodeRateWarning {
            source_rate: FrameRate::Fps25,
            desk_rate: FrameRate::Fps30
        })
    );
}

#[test]
fn internal_generator_selection_rejects_external_frames() {
    let mut router = TimecodeRouter::default();
    router.configure(TimecodeRouterConfig {
        selected_source: TimecodeSourceSelection::Internal,
        ..TimecodeRouterConfig::default()
    });
    assert!(router.ingest(tc("artnet:10.0.0.1:2")).is_none());
    assert_eq!(router.active_source(), Some("internal"));
    assert!(router.current().is_none());
    assert!(router.uses_internal_clock());
}
