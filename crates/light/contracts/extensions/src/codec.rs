use serde_json::Error as JsonError;
use thiserror::Error;

use crate::{DRAFT_PROTOCOL_V1, Frame};

pub const LENGTH_PREFIX_BYTES: usize = size_of::<u32>();
/// Large enough for a full control-surface projection, while still bounding hostile peers.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum CodecError {
    #[error("frame payload is empty")]
    EmptyFrame,
    #[error("frame payload is {actual} bytes; the maximum is {maximum}")]
    FrameTooLarge { actual: usize, maximum: usize },
    #[error("malformed frame JSON: {0}")]
    MalformedJson(#[source] JsonError),
    #[error("unsupported protocol version {actual}; expected {expected}")]
    UnsupportedVersion { actual: u16, expected: u16 },
    #[error("received frame sequence {actual}; expected {expected}")]
    InvalidSequence { actual: u64, expected: u64 },
    #[error("frame sequence overflow after {0}")]
    SequenceOverflow(u64),
}

impl PartialEq for CodecError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::EmptyFrame, Self::EmptyFrame) => true,
            (
                Self::FrameTooLarge {
                    actual: left_actual,
                    maximum: left_maximum,
                },
                Self::FrameTooLarge {
                    actual: right_actual,
                    maximum: right_maximum,
                },
            ) => left_actual == right_actual && left_maximum == right_maximum,
            (
                Self::UnsupportedVersion {
                    actual: left_actual,
                    expected: left_expected,
                },
                Self::UnsupportedVersion {
                    actual: right_actual,
                    expected: right_expected,
                },
            ) => left_actual == right_actual && left_expected == right_expected,
            (
                Self::InvalidSequence {
                    actual: left_actual,
                    expected: left_expected,
                },
                Self::InvalidSequence {
                    actual: right_actual,
                    expected: right_expected,
                },
            ) => left_actual == right_actual && left_expected == right_expected,
            (Self::SequenceOverflow(left), Self::SequenceOverflow(right)) => left == right,
            (Self::MalformedJson(left), Self::MalformedJson(right)) => {
                left.classify() == right.classify()
            }
            _ => false,
        }
    }
}

/// Serializes one frame with a four-byte big-endian payload length.
pub fn encode_frame(frame: &Frame) -> Result<Vec<u8>, CodecError> {
    validate_version(frame)?;
    let payload = serde_json::to_vec(frame).map_err(CodecError::MalformedJson)?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(CodecError::FrameTooLarge {
            actual: payload.len(),
            maximum: MAX_FRAME_BYTES,
        });
    }

    let mut encoded = Vec::with_capacity(LENGTH_PREFIX_BYTES + payload.len());
    encoded.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    encoded.extend_from_slice(&payload);
    Ok(encoded)
}

/// Incremental decoder for fragmented or coalesced framed messages.
///
/// A decoder represents one transport direction and therefore owns one strict sequence. Its
/// pending allocation never exceeds `MAX_FRAME_BYTES`.
#[derive(Debug)]
pub struct FrameDecoder {
    expected_sequence: u64,
    announced_length: Option<usize>,
    pending: Vec<u8>,
}

impl FrameDecoder {
    pub fn new(first_expected_sequence: u64) -> Self {
        Self {
            expected_sequence: first_expected_sequence,
            announced_length: None,
            pending: Vec::new(),
        }
    }

    pub fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }

    pub fn push(&mut self, mut bytes: &[u8]) -> Result<Vec<Frame>, CodecError> {
        let mut frames = Vec::new();
        while !bytes.is_empty() {
            if self.announced_length.is_none() {
                let needed = LENGTH_PREFIX_BYTES - self.pending.len();
                let take = needed.min(bytes.len());
                self.pending.extend_from_slice(&bytes[..take]);
                bytes = &bytes[take..];
                if self.pending.len() < LENGTH_PREFIX_BYTES {
                    continue;
                }

                let length =
                    u32::from_be_bytes(self.pending[..LENGTH_PREFIX_BYTES].try_into().unwrap())
                        as usize;
                self.pending.clear();
                if length == 0 {
                    return Err(CodecError::EmptyFrame);
                }
                if length > MAX_FRAME_BYTES {
                    return Err(CodecError::FrameTooLarge {
                        actual: length,
                        maximum: MAX_FRAME_BYTES,
                    });
                }
                self.pending.reserve(length);
                self.announced_length = Some(length);
            }

            let announced_length = self.announced_length.expect("length was set above");
            let needed = announced_length - self.pending.len();
            let take = needed.min(bytes.len());
            self.pending.extend_from_slice(&bytes[..take]);
            bytes = &bytes[take..];
            if self.pending.len() < announced_length {
                continue;
            }

            let frame: Frame =
                serde_json::from_slice(&self.pending).map_err(CodecError::MalformedJson)?;
            validate_version(&frame)?;
            if frame.sequence != self.expected_sequence {
                return Err(CodecError::InvalidSequence {
                    actual: frame.sequence,
                    expected: self.expected_sequence,
                });
            }
            self.expected_sequence = self
                .expected_sequence
                .checked_add(1)
                .ok_or(CodecError::SequenceOverflow(self.expected_sequence))?;
            self.pending.clear();
            self.announced_length = None;
            frames.push(frame);
        }
        Ok(frames)
    }
}

fn validate_version(frame: &Frame) -> Result<(), CodecError> {
    if frame.version != DRAFT_PROTOCOL_V1 {
        return Err(CodecError::UnsupportedVersion {
            actual: frame.version.0,
            expected: DRAFT_PROTOCOL_V1.0,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::{
        Configure, ControlInput, ControlInputEvent, DRAFT_PROTOCOL_V1, DeviceActionRequest,
        DeviceActionResult, DeviceActionStatus, ExtensionCapability, ExtensionHello,
        FeedbackChange, FeedbackContext, FeedbackDelta, FeedbackSnapshot, FeedbackValue,
        HealthReport, HealthStatus, HostHello, Message, ProtocolErrorCode, ProtocolErrorMessage,
        ProtocolVersion, Shutdown, ShutdownReason, TelemetryChannelDeclaration, TelemetryQuality,
        TelemetrySample, TelemetryValue, TelemetryValueKind, TimecodeRate, TimecodeSample,
    };

    fn test_frame(sequence: u64) -> Frame {
        Frame::v1(
            sequence,
            Message::ControlInput(ControlInputEvent {
                input_id: 17,
                occurred_at_micros: 1_234,
                control: ControlInput::Absolute {
                    control_id: "executor-1".into(),
                    value: 0.75,
                },
            }),
        )
    }

    #[test]
    fn deterministic_round_trip_and_fragmentation() {
        let frame = test_frame(7);
        let encoded = encode_frame(&frame).unwrap();
        assert_eq!(encoded, encode_frame(&frame).unwrap());

        let mut decoder = FrameDecoder::new(7);
        let mut decoded = Vec::new();
        for byte in encoded {
            decoded.extend(decoder.push(&[byte]).unwrap());
        }
        assert_eq!(decoded, vec![frame]);
        assert_eq!(decoder.expected_sequence(), 8);
    }

    #[test]
    fn decodes_multiple_frames_from_one_read() {
        let first = test_frame(20);
        let second = Frame::v1(
            21,
            Message::Health(HealthReport {
                status: HealthStatus::Ready,
                detail: None,
                counters: BTreeMap::new(),
            }),
        );
        let mut bytes = encode_frame(&first).unwrap();
        bytes.extend(encode_frame(&second).unwrap());

        assert_eq!(
            FrameDecoder::new(20).push(&bytes).unwrap(),
            vec![first, second]
        );
    }

    #[test]
    fn round_trips_every_message_family() {
        let capabilities = BTreeSet::from([
            ExtensionCapability::ControlSurface,
            ExtensionCapability::TelemetrySource,
            ExtensionCapability::TimecodeSource,
        ]);
        let feedback = FeedbackSnapshot {
            context: FeedbackContext {
                desk_id: "desk-main".into(),
                show_id: Some("show-main".into()),
                show_generation: 3,
            },
            revision: 4,
            controls: BTreeMap::from([("go".into(), FeedbackValue::Boolean(true))]),
        };
        let messages = vec![
            Message::HostHello(HostHello {
                host_name: "ToskLight".into(),
                host_instance_id: "desk-main".into(),
                supported_versions: vec![DRAFT_PROTOCOL_V1],
                requested_capabilities: capabilities.clone(),
                channel_challenge: "challenge".into(),
            }),
            Message::ExtensionHello(ExtensionHello {
                extension_id: "example.extension".into(),
                extension_instance_id: "spawn-1".into(),
                extension_version: "0.1.0".into(),
                package_digest: "sha256:approved".into(),
                selected_version: DRAFT_PROTOCOL_V1,
                capabilities: capabilities.clone(),
                channel_response: "response".into(),
            }),
            Message::Configure(Configure {
                enabled_capabilities: capabilities,
                feedback: Some(feedback.clone()),
                telemetry_channels: vec![TelemetryChannelDeclaration {
                    channel_id: "temperature".into(),
                    label: "Temperature".into(),
                    quantity: "temperature".into(),
                    unit: "degC".into(),
                    value_kind: TelemetryValueKind::Number,
                    minimum: Some(-40.0),
                    maximum: Some(125.0),
                    precision: Some(1),
                    expected_interval_micros: Some(1_000_000),
                    quality_flags: BTreeSet::from([TelemetryQuality::Good]),
                }],
                device_actions: Vec::new(),
                control_bindings: BTreeMap::new(),
                settings: BTreeMap::from([("port".into(), serde_json::json!("virtual"))]),
            }),
            Message::ControlInput(ControlInputEvent {
                input_id: 1,
                occurred_at_micros: 2,
                control: ControlInput::Relative {
                    control_id: "wheel".into(),
                    delta: -1,
                },
            }),
            Message::FeedbackSnapshot(feedback),
            Message::FeedbackDelta(FeedbackDelta {
                context: feedback_context(),
                base_revision: 4,
                revision: 5,
                changes: vec![FeedbackChange {
                    control_id: "go".into(),
                    value: Some(FeedbackValue::Rgb {
                        red: 1,
                        green: 2,
                        blue: 3,
                    }),
                }],
            }),
            Message::TelemetrySample(TelemetrySample {
                sample_id: 6,
                observed_at_micros: 7,
                channel_id: "temperature".into(),
                value: TelemetryValue::Number(21.5),
                quality: TelemetryQuality::Good,
            }),
            Message::DeviceActionRequest(DeviceActionRequest {
                request_id: 7,
                action_id: "identify".into(),
                parameters: BTreeMap::from([("seconds".into(), TelemetryValue::Integer(2))]),
            }),
            Message::DeviceActionResult(DeviceActionResult {
                request_id: 7,
                action_id: "identify".into(),
                status: DeviceActionStatus::Completed,
                detail: None,
                values: BTreeMap::new(),
            }),
            Message::TimecodeSample(TimecodeSample {
                sample_id: 8,
                observed_at_micros: 9,
                hours: 1,
                minutes: 2,
                seconds: 3,
                frames: 4,
                rate: TimecodeRate::Fps25,
                drop_frame: false,
            }),
            Message::Health(HealthReport {
                status: HealthStatus::Degraded,
                detail: Some("test".into()),
                counters: BTreeMap::from([("dropped".into(), 1)]),
            }),
            Message::Shutdown(Shutdown {
                reason: ShutdownReason::HostRequested,
                detail: None,
            }),
            Message::ProtocolError(ProtocolErrorMessage {
                code: ProtocolErrorCode::InvalidPayload,
                detail: "test rejection".into(),
                rejected_sequence: Some(3),
            }),
        ];
        let expected: Vec<_> = messages
            .into_iter()
            .enumerate()
            .map(|(sequence, message)| Frame::v1(sequence as u64, message))
            .collect();
        let bytes: Vec<_> = expected
            .iter()
            .flat_map(|frame| encode_frame(frame).unwrap())
            .collect();

        assert_eq!(FrameDecoder::new(0).push(&bytes).unwrap(), expected);
    }

    fn feedback_context() -> FeedbackContext {
        FeedbackContext {
            desk_id: "desk-main".into(),
            show_id: Some("show-main".into()),
            show_generation: 3,
        }
    }

    #[test]
    fn rejects_announced_and_encoded_oversized_frames() {
        let mut decoder = FrameDecoder::new(0);
        let announced = (MAX_FRAME_BYTES as u32 + 1).to_be_bytes();
        assert_eq!(
            decoder.push(&announced).unwrap_err(),
            CodecError::FrameTooLarge {
                actual: MAX_FRAME_BYTES + 1,
                maximum: MAX_FRAME_BYTES,
            }
        );

        let oversized = Frame::v1(
            0,
            Message::Health(HealthReport {
                status: HealthStatus::Failed,
                detail: Some("x".repeat(MAX_FRAME_BYTES)),
                counters: BTreeMap::new(),
            }),
        );
        assert!(matches!(
            encode_frame(&oversized),
            Err(CodecError::FrameTooLarge { .. })
        ));
    }

    #[test]
    fn rejects_empty_malformed_wrong_version_and_out_of_sequence_frames() {
        assert_eq!(
            FrameDecoder::new(0).push(&0_u32.to_be_bytes()).unwrap_err(),
            CodecError::EmptyFrame
        );

        let malformed = br#"{"version":1,"#;
        let mut bytes = (malformed.len() as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(malformed);
        assert!(matches!(
            FrameDecoder::new(0).push(&bytes),
            Err(CodecError::MalformedJson(_))
        ));

        let wrong_version = Frame {
            version: ProtocolVersion(2),
            sequence: 0,
            message: Message::Health(HealthReport {
                status: HealthStatus::Ready,
                detail: None,
                counters: BTreeMap::new(),
            }),
        };
        assert_eq!(
            encode_frame(&wrong_version).unwrap_err(),
            CodecError::UnsupportedVersion {
                actual: 2,
                expected: 1,
            }
        );

        let encoded = encode_frame(&test_frame(9)).unwrap();
        assert_eq!(
            FrameDecoder::new(8).push(&encoded).unwrap_err(),
            CodecError::InvalidSequence {
                actual: 9,
                expected: 8,
            }
        );
    }

    #[test]
    fn ignores_unknown_fields_for_forward_compatible_v1_objects() {
        let frame = Frame::v1(
            1,
            Message::HostHello(HostHello {
                host_name: "ToskLight".into(),
                host_instance_id: "main".into(),
                supported_versions: vec![DRAFT_PROTOCOL_V1],
                requested_capabilities: BTreeSet::from([ExtensionCapability::ControlSurface]),
                channel_challenge: "fresh-challenge".into(),
            }),
        );
        let mut value = serde_json::to_value(&frame).unwrap();
        value.as_object_mut().unwrap().insert(
            "future_envelope_field".into(),
            serde_json::json!({"ignored": true}),
        );
        value["message"]["body"]["future_hello_field"] = serde_json::json!(42);
        let payload = serde_json::to_vec(&value).unwrap();
        let mut encoded = (payload.len() as u32).to_be_bytes().to_vec();
        encoded.extend(payload);

        assert_eq!(FrameDecoder::new(1).push(&encoded).unwrap(), vec![frame]);
    }
}
