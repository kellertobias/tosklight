#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{Read, Write};
use std::time::Duration;

use light_extensions_contract::{
    ControlInput, ControlInputEvent, DRAFT_PROTOCOL_V1, DeviceActionResult, DeviceActionStatus,
    ExtensionCapability, ExtensionHello, Frame, FrameDecoder, HealthReport, HealthStatus, Message,
    ProtocolVersion, TelemetryQuality, TelemetrySample, TelemetryValue, TimecodeRate,
    TimecodeSample, encode_frame,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("synthetic extension failed: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mode = environment("TL_EXTENSION_TEST_MODE").unwrap_or_else(|| "normal".into());
    if mode == "silent" {
        std::thread::sleep(Duration::from_secs(10));
        return Ok(());
    }

    let input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut input = WireReader::new(input);
    let hello = match input.next_message()? {
        Message::HostHello(hello) => hello,
        other => return Err(format!("expected host hello, received {other:?}")),
    };
    let attempt = environment("TOSKLIGHT_EXTENSION_LAUNCH_ATTEMPT")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let token = required_environment("TOSKLIGHT_EXTENSION_CHANNEL_CREDENTIAL")?;
    let mut extension_id = required_environment("TOSKLIGHT_EXTENSION_ID")?;
    let mut instance_id = required_environment("TOSKLIGHT_EXTENSION_INSTANCE_ID")?;
    let mut digest = required_environment("TOSKLIGHT_EXTENSION_PACKAGE_DIGEST")?;
    let mut response = light_extensions_host::channel_response(&token, &hello.channel_challenge);
    let mut selected_version = DRAFT_PROTOCOL_V1;
    let mut capabilities = BTreeSet::from([
        ExtensionCapability::ControlSurface,
        ExtensionCapability::TelemetrySource,
        ExtensionCapability::TimecodeSource,
    ]);
    match mode.as_str() {
        "bad_version" => selected_version = ProtocolVersion(2),
        "bad_identity" => extension_id = "wrong.extension".into(),
        "bad_instance" => instance_id = "wrong-instance".into(),
        "bad_digest" => digest = "sha256:wrong".into(),
        "bad_auth" => response = "wrong-response".into(),
        "bad_capabilities" => capabilities.clear(),
        _ => {}
    }
    let hello = ExtensionHello {
        extension_id,
        extension_instance_id: instance_id,
        extension_version: "0.0.0-test".into(),
        package_digest: digest,
        selected_version,
        capabilities,
        channel_response: response,
    };
    if mode == "bad_version" {
        write_unchecked_frame(
            &mut output,
            &Frame {
                version: ProtocolVersion(2),
                sequence: 0,
                message: Message::ExtensionHello(hello),
            },
        )?;
        std::thread::sleep(Duration::from_secs(1));
        return Ok(());
    }
    send(&mut output, 0, Message::ExtensionHello(hello))?;
    let configure = match input.next_message()? {
        Message::Configure(configure) => configure,
        other => return Err(format!("expected configure, received {other:?}")),
    };
    let snapshot_revision = configure
        .feedback
        .as_ref()
        .map_or(0, |snapshot| snapshot.revision);
    send_health(
        &mut output,
        1,
        BTreeMap::from([
            ("snapshot_revision".into(), snapshot_revision),
            ("launch_attempt".into(), attempt),
        ]),
    )?;

    match mode.as_str() {
        "malformed_after_config" => {
            output.write_all(&5_u32.to_be_bytes()).map_err(io_error)?;
            output.write_all(b"nope!").map_err(io_error)?;
            output.flush().map_err(io_error)?;
            std::thread::sleep(Duration::from_secs(1));
            return Ok(());
        }
        "oversized_after_config" => {
            output
                .write_all(&((light_extensions_contract::MAX_FRAME_BYTES + 1) as u32).to_be_bytes())
                .map_err(io_error)?;
            output.flush().map_err(io_error)?;
            std::thread::sleep(Duration::from_secs(1));
            return Ok(());
        }
        "crash_once" if attempt == 1 => std::process::exit(17),
        "always_crash" => std::process::exit(19),
        "flood" => {
            // No newline until after a payload much larger than the host's test log cap. The host
            // must drain this without ever constructing an unbounded line.
            eprint!("{}", "unbroken-log".repeat(16 * 1024));
            for index in 0..2_000_u64 {
                eprintln!("synthetic-log-{index:04}-{}", "x".repeat(64));
                send(
                    &mut output,
                    index + 2,
                    Message::TelemetrySample(telemetry(index + 1)),
                )?;
            }
        }
        "stall_after_config" => std::thread::sleep(Duration::from_millis(400)),
        "undeclared_telemetry" => send(
            &mut output,
            2,
            Message::TelemetrySample(TelemetrySample {
                sample_id: 1,
                observed_at_micros: 1_000,
                channel_id: "not-declared".into(),
                value: TelemetryValue::Number(42.0),
                quality: TelemetryQuality::Good,
            }),
        )?,
        "telemetry_health" => send_telemetry_health_stream(&mut output)?,
        "timecode" => send(&mut output, 2, Message::TimecodeSample(timecode(1, 12)))?,
        "invalid_timecode" => send(&mut output, 2, Message::TimecodeSample(timecode(1, 25)))?,
        "duplicate_timecode" => {
            send(&mut output, 2, Message::TimecodeSample(timecode(1, 12)))?;
            send(&mut output, 3, Message::TimecodeSample(timecode(1, 13)))?;
        }
        "duplicate_press" => {
            send_button(&mut output, 2, 1, true)?;
            send_button(&mut output, 3, 2, true)?;
        }
        "release_without_press" => send_button(&mut output, 2, 1, false)?,
        _ => send_round_trip_inputs(&mut output)?,
    }

    let mut next_sequence = match mode.as_str() {
        "flood" => 2_002,
        "stall_after_config" => 2,
        "undeclared_telemetry" => 3,
        "telemetry_health" => 8,
        "timecode" | "invalid_timecode" => 3,
        "duplicate_timecode" | "duplicate_press" => 4,
        "release_without_press" => 3,
        _ => 6,
    };
    loop {
        match input.next_message()? {
            Message::FeedbackDelta(delta) => {
                send_health(
                    &mut output,
                    next_sequence,
                    BTreeMap::from([("delta_revision".into(), delta.revision)]),
                )?;
                next_sequence += 1;
            }
            Message::FeedbackSnapshot(snapshot) => {
                send_health(
                    &mut output,
                    next_sequence,
                    BTreeMap::from([("snapshot_revision".into(), snapshot.revision)]),
                )?;
                next_sequence += 1;
            }
            Message::DeviceActionRequest(request) => {
                let accepted = if mode == "bad_device_action_result" {
                    TelemetryValue::Text("not-a-boolean".into())
                } else {
                    TelemetryValue::Boolean(true)
                };
                send(
                    &mut output,
                    next_sequence,
                    Message::DeviceActionResult(DeviceActionResult {
                        request_id: request.request_id,
                        action_id: request.action_id,
                        status: DeviceActionStatus::Completed,
                        detail: None,
                        values: BTreeMap::from([("accepted".into(), accepted)]),
                    }),
                )?;
                next_sequence += 1;
            }
            Message::Shutdown(_) if mode == "hang_shutdown" => {
                std::thread::sleep(Duration::from_secs(10));
                return Ok(());
            }
            Message::Shutdown(_) => return Ok(()),
            other => return Err(format!("unexpected host message: {other:?}")),
        }
    }
}

fn send_button(
    output: &mut impl Write,
    sequence: u64,
    input_id: u64,
    pressed: bool,
) -> Result<(), String> {
    send(
        output,
        sequence,
        Message::ControlInput(ControlInputEvent {
            input_id,
            occurred_at_micros: 100 + input_id,
            control: ControlInput::Button {
                control_id: "go".into(),
                pressed,
            },
        }),
    )
}

fn timecode(sample_id: u64, frames: u8) -> TimecodeSample {
    TimecodeSample {
        sample_id,
        observed_at_micros: sample_id * 1_000,
        hours: 1,
        minutes: 2,
        seconds: 3,
        frames,
        rate: TimecodeRate::Fps25,
        drop_frame: false,
    }
}

fn send_telemetry_health_stream(output: &mut impl Write) -> Result<(), String> {
    send(output, 2, Message::TelemetrySample(telemetry(1)))?;
    send(output, 3, Message::TelemetrySample(telemetry(3)))?;
    send(output, 4, Message::TelemetrySample(telemetry(2)))?;
    send(
        output,
        5,
        Message::TelemetrySample(TelemetrySample {
            sample_id: 4,
            observed_at_micros: 4_000,
            channel_id: "temperature".into(),
            value: TelemetryValue::Number(24.0),
            quality: TelemetryQuality::Stale,
        }),
    )?;
    send(
        output,
        6,
        Message::TelemetrySample(TelemetrySample {
            sample_id: 5,
            observed_at_micros: 5_000,
            channel_id: "temperature".into(),
            value: TelemetryValue::Number(999.0),
            quality: TelemetryQuality::Good,
        }),
    )?;
    send(output, 7, Message::TelemetrySample(telemetry(6)))
}

fn send_round_trip_inputs(output: &mut impl Write) -> Result<(), String> {
    send(
        output,
        2,
        Message::ControlInput(ControlInputEvent {
            input_id: 1,
            occurred_at_micros: 100,
            control: ControlInput::Button {
                control_id: "go".into(),
                pressed: true,
            },
        }),
    )?;
    send(
        output,
        3,
        Message::ControlInput(ControlInputEvent {
            input_id: 2,
            occurred_at_micros: 101,
            control: ControlInput::Button {
                control_id: "go".into(),
                pressed: false,
            },
        }),
    )?;
    send(output, 4, Message::TelemetrySample(telemetry(1)))?;
    send(output, 5, Message::TelemetrySample(telemetry(2)))
}

fn telemetry(sample_id: u64) -> TelemetrySample {
    TelemetrySample {
        sample_id,
        observed_at_micros: sample_id * 1_000,
        channel_id: "temperature".into(),
        value: TelemetryValue::Number(20.0 + sample_id as f64),
        quality: TelemetryQuality::Good,
    }
}

fn send_health(
    output: &mut impl Write,
    sequence: u64,
    counters: BTreeMap<String, u64>,
) -> Result<(), String> {
    send(
        output,
        sequence,
        Message::Health(HealthReport {
            status: HealthStatus::Ready,
            detail: None,
            counters,
        }),
    )
}

fn send(output: &mut impl Write, sequence: u64, message: Message) -> Result<(), String> {
    let bytes = encode_frame(&Frame::v1(sequence, message)).map_err(|error| error.to_string())?;
    output.write_all(&bytes).map_err(io_error)?;
    output.flush().map_err(io_error)
}

fn write_unchecked_frame(output: &mut impl Write, frame: &Frame) -> Result<(), String> {
    let payload = serde_json::to_vec(frame).map_err(|error| error.to_string())?;
    output
        .write_all(&(payload.len() as u32).to_be_bytes())
        .map_err(io_error)?;
    output.write_all(&payload).map_err(io_error)?;
    output.flush().map_err(io_error)
}

struct WireReader<R> {
    input: R,
    decoder: FrameDecoder,
    pending: VecDeque<Frame>,
}

impl<R: Read> WireReader<R> {
    fn new(input: R) -> Self {
        Self {
            input,
            decoder: FrameDecoder::new(0),
            pending: VecDeque::new(),
        }
    }

    fn next_message(&mut self) -> Result<Message, String> {
        let mut bytes = [0_u8; 8 * 1024];
        loop {
            if let Some(frame) = self.pending.pop_front() {
                return Ok(frame.message);
            }
            let count = self.input.read(&mut bytes).map_err(io_error)?;
            if count == 0 {
                return Err("host channel closed".into());
            }
            self.pending.extend(
                self.decoder
                    .push(&bytes[..count])
                    .map_err(|error| error.to_string())?,
            );
        }
    }
}

fn required_environment(name: &str) -> Result<String, String> {
    environment(name).ok_or_else(|| format!("missing {name}"))
}

fn environment(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}
