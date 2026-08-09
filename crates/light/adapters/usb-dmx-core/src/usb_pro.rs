use crate::{TransportError, UniverseFrame};

pub const PRO_MAX_PAYLOAD: usize = 600;
const START: u8 = 0x7e;
const END: u8 = 0xe7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProLabel {
    GetParameters = 3,
    SetParameters = 4,
    SendDmx = 6,
    GetSerial = 10,
}

impl TryFrom<u8> for ProLabel {
    type Error = ProProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            3 => Ok(Self::GetParameters),
            4 => Ok(Self::SetParameters),
            6 => Ok(Self::SendDmx),
            10 => Ok(Self::GetSerial),
            _ => Err(ProProtocolError::UnsupportedLabel(value)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProProtocolError {
    PayloadTooLarge(usize),
    MalformedParameterReply(usize),
    UnsupportedLabel(u8),
    InvalidRequestLabel(ProLabel),
    RequestWouldStopOutput(ProLabel),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProFrame {
    pub label: ProLabel,
    pub payload: Vec<u8>,
}

pub fn encode_pro_frame(label: ProLabel, payload: &[u8]) -> Result<Vec<u8>, ProProtocolError> {
    if payload.len() > PRO_MAX_PAYLOAD {
        return Err(ProProtocolError::PayloadTooLarge(payload.len()));
    }
    let length = payload.len() as u16;
    let mut encoded = Vec::with_capacity(payload.len() + 5);
    encoded.extend_from_slice(&[START, label as u8]);
    encoded.extend_from_slice(&length.to_le_bytes());
    encoded.extend_from_slice(payload);
    encoded.push(END);
    Ok(encoded)
}

/// Incremental parser which tolerates fragmented reads and resynchronizes after corrupt input.
#[derive(Clone, Debug, Default)]
pub struct ProFrameParser {
    buffered: Vec<u8>,
}

impl ProFrameParser {
    pub fn reset_for_reconnect(&mut self) {
        self.buffered.clear();
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<ProFrame> {
        self.buffered.extend_from_slice(bytes);
        let mut frames = Vec::new();
        loop {
            let Some(start) = self.buffered.iter().position(|byte| *byte == START) else {
                self.buffered.clear();
                break;
            };
            if start > 0 {
                self.buffered.drain(..start);
            }
            if self.buffered.len() < 5 {
                break;
            }
            let length = u16::from_le_bytes([self.buffered[2], self.buffered[3]]) as usize;
            if length > PRO_MAX_PAYLOAD {
                self.buffered.drain(..1);
                continue;
            }
            let total = length + 5;
            if self.buffered.len() < total {
                break;
            }
            if self.buffered[total - 1] != END {
                self.buffered.drain(..1);
                continue;
            }
            let label = ProLabel::try_from(self.buffered[1]);
            if let Ok(label) = label {
                frames.push(ProFrame {
                    label,
                    payload: self.buffered[4..4 + length].to_vec(),
                });
            }
            self.buffered.drain(..total);
        }
        frames
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WidgetParameters {
    pub break_units: u8,
    pub mab_units: u8,
    pub refresh_hz: u8,
}

impl Default for WidgetParameters {
    fn default() -> Self {
        Self {
            break_units: 9,
            mab_units: 2,
            refresh_hz: 40,
        }
    }
}

impl WidgetParameters {
    pub fn payload(self) -> [u8; 5] {
        [0, 0, self.break_units, self.mab_units, self.refresh_hz]
    }

    pub fn decode_reply(payload: &[u8]) -> Result<WidgetParameterReply, ProProtocolError> {
        if payload.len() < 5 {
            return Err(ProProtocolError::MalformedParameterReply(payload.len()));
        }
        Ok(WidgetParameterReply {
            firmware_version: u16::from_le_bytes([payload[0], payload[1]]),
            parameters: Self {
                break_units: payload[2],
                mab_units: payload[3],
                refresh_hz: payload[4],
            },
            user_configuration: payload[5..].to_vec(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WidgetParameterReply {
    pub firmware_version: u16,
    pub parameters: WidgetParameters,
    pub user_configuration: Vec<u8>,
}

pub trait ProSerial {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
}

/// Buffered-widget sender. Pending host updates coalesce; the widget repeats its newest buffer.
#[derive(Clone, Debug, Default)]
pub struct ProDriver {
    latest: Option<UniverseFrame>,
    pending_generation: u64,
    sent_generation: u64,
    output_active: bool,
}

impl ProDriver {
    pub fn enqueue(&mut self, frame: UniverseFrame) {
        self.latest = Some(frame);
        self.pending_generation = self.pending_generation.wrapping_add(1);
    }

    pub fn has_pending_frame(&self) -> bool {
        self.latest.is_some() && self.pending_generation != self.sent_generation
    }

    pub fn connect(&mut self, serial: &mut impl ProSerial) -> Result<(), TransportError> {
        self.output_active = false;
        self.sent_generation = self.pending_generation.wrapping_sub(1);
        let configuration = encode_pro_frame(
            ProLabel::SetParameters,
            &WidgetParameters::default().payload(),
        )
        .expect("fixed widget-parameter payload is valid");
        serial.write_all(&configuration)
    }

    pub fn request(
        &mut self,
        serial: &mut impl ProSerial,
        label: ProLabel,
    ) -> Result<(), RequestError> {
        if matches!(label, ProLabel::SetParameters | ProLabel::SendDmx) {
            return Err(RequestError::Protocol(
                ProProtocolError::InvalidRequestLabel(label),
            ));
        }
        if self.output_active && label != ProLabel::GetParameters {
            return Err(RequestError::Protocol(
                ProProtocolError::RequestWouldStopOutput(label),
            ));
        }
        let request = encode_pro_frame(label, &[]).map_err(RequestError::Protocol)?;
        serial.write_all(&request).map_err(RequestError::Transport)
    }

    pub fn transmit_pending(
        &mut self,
        serial: &mut impl ProSerial,
    ) -> Result<bool, TransportError> {
        let Some(frame) = self.latest.as_ref() else {
            return Ok(false);
        };
        if self.pending_generation == self.sent_generation {
            return Ok(false);
        }
        let generation = self.pending_generation;
        let encoded = encode_pro_frame(ProLabel::SendDmx, &frame.wire_payload())
            .expect("full DMX payload is below protocol cap");
        serial.write_all(&encoded)?;
        self.sent_generation = generation;
        self.output_active = true;
        Ok(true)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestError {
    Protocol(ProProtocolError),
    Transport(TransportError),
}

/// Label-10 serial is a four-byte little-endian integer. All-ones is unprogrammed.
pub fn decode_widget_serial(payload: &[u8]) -> Option<String> {
    let bytes: [u8; 4] = payload.try_into().ok()?;
    if bytes == [0xff; 4] {
        return None;
    }
    Some(u32::from_le_bytes(bytes).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeSerial {
        writes: Vec<Vec<u8>>,
        fail_next: bool,
    }

    impl ProSerial for FakeSerial {
        fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
            if self.fail_next {
                self.fail_next = false;
                return Err(TransportError("unplugged".into()));
            }
            self.writes.push(bytes.to_vec());
            Ok(())
        }
    }

    fn frame(value: u8) -> UniverseFrame {
        UniverseFrame::new([value; 512])
    }

    #[test]
    fn golden_frames_cover_exactly_the_accepted_labels() {
        assert_eq!(
            encode_pro_frame(ProLabel::GetParameters, &[]).unwrap(),
            [0x7e, 3, 0, 0, 0xe7]
        );
        assert_eq!(
            encode_pro_frame(ProLabel::SetParameters, &[0, 0, 9, 2, 40]).unwrap(),
            [0x7e, 4, 5, 0, 0, 0, 9, 2, 40, 0xe7]
        );
        assert_eq!(
            encode_pro_frame(ProLabel::GetSerial, &[]).unwrap(),
            [0x7e, 10, 0, 0, 0xe7]
        );
        let output = encode_pro_frame(ProLabel::SendDmx, &[0; 513]).unwrap();
        assert_eq!(&output[..4], &[0x7e, 6, 1, 2]);
        assert_eq!(output.len(), 518);
        assert_eq!(output[517], 0xe7);
        assert_eq!(
            encode_pro_frame(ProLabel::SendDmx, &[0; 601]),
            Err(ProProtocolError::PayloadTooLarge(601))
        );
    }

    #[test]
    fn parser_accepts_every_fragmentation_boundary_and_multiple_messages() {
        let first = encode_pro_frame(ProLabel::GetParameters, &[1, 2, 3]).unwrap();
        let second = encode_pro_frame(ProLabel::GetSerial, &[0x12, 0x34, 0x56, 0x78]).unwrap();
        let mut combined = first.clone();
        combined.extend_from_slice(&second);
        for boundary in 0..=combined.len() {
            let mut parser = ProFrameParser::default();
            let mut frames = parser.push(&combined[..boundary]);
            frames.extend(parser.push(&combined[boundary..]));
            assert_eq!(frames.len(), 2, "boundary {boundary}");
            assert_eq!(frames[0].label, ProLabel::GetParameters);
            assert_eq!(frames[1].label, ProLabel::GetSerial);
        }
    }

    #[test]
    fn parser_recovers_from_garbage_oversize_bad_terminator_and_reconnect_mid_frame() {
        let good = encode_pro_frame(ProLabel::GetSerial, &[1, 2, 3, 4]).unwrap();
        let mut parser = ProFrameParser::default();
        let mut corrupt = vec![9, 8, 0x7e, 3, 0x59, 0x02]; // 601-byte claim
        corrupt.extend_from_slice(&[0x7e, 3, 0, 0, 0]); // bad terminator
        corrupt.extend_from_slice(&good);
        let frames = parser.push(&corrupt);
        assert_eq!(
            frames,
            vec![ProFrame {
                label: ProLabel::GetSerial,
                payload: vec![1, 2, 3, 4]
            }]
        );

        let mut parser = ProFrameParser::default();
        assert!(parser.push(&good[..3]).is_empty());
        parser.reset_for_reconnect();
        assert_eq!(parser.push(&good).len(), 1);
    }

    #[test]
    fn driver_configures_defaults_sends_full_frame_and_latest_update_wins() {
        let mut driver = ProDriver::default();
        let mut serial = FakeSerial::default();
        driver.connect(&mut serial).unwrap();
        assert_eq!(serial.writes[0], [0x7e, 4, 5, 0, 0, 0, 9, 2, 40, 0xe7]);
        driver.enqueue(frame(1));
        driver.enqueue(frame(2));
        driver.enqueue(frame(3));
        assert!(driver.transmit_pending(&mut serial).unwrap());
        assert_eq!(serial.writes.len(), 2);
        assert_eq!(serial.writes[1][4], 0); // DMX start code
        assert!(serial.writes[1][5..517].iter().all(|byte| *byte == 3));
    }

    #[test]
    fn failed_write_and_reconnect_resend_the_newest_frame() {
        let mut driver = ProDriver::default();
        let mut serial = FakeSerial::default();
        driver.connect(&mut serial).unwrap();
        driver.enqueue(frame(7));
        serial.fail_next = true;
        assert!(driver.transmit_pending(&mut serial).is_err());
        assert!(driver.has_pending_frame());
        driver.enqueue(frame(8));

        let mut reconnected = FakeSerial::default();
        driver.connect(&mut reconnected).unwrap();
        driver.transmit_pending(&mut reconnected).unwrap();
        assert!(reconnected.writes[1][5..517].iter().all(|byte| *byte == 8));
    }

    #[test]
    fn output_mode_rejects_unrelated_probes_but_allows_parameter_read() {
        let mut driver = ProDriver::default();
        let mut serial = FakeSerial::default();
        driver.connect(&mut serial).unwrap();
        driver.enqueue(frame(1));
        driver.transmit_pending(&mut serial).unwrap();
        driver
            .request(&mut serial, ProLabel::GetParameters)
            .unwrap();
        assert_eq!(
            driver.request(&mut serial, ProLabel::GetSerial),
            Err(RequestError::Protocol(
                ProProtocolError::RequestWouldStopOutput(ProLabel::GetSerial)
            ))
        );
    }

    #[test]
    fn serial_decoder_handles_little_endian_binary_and_unprogrammed_value() {
        assert_eq!(
            decode_widget_serial(&[0x78, 0x56, 0x34, 0x12]),
            Some("305419896".into())
        );
        assert_eq!(decode_widget_serial(&[0xff; 4]), None);
        assert_eq!(decode_widget_serial(&[0xfa, 0, 0, 0]), Some("250".into()));
    }

    #[test]
    fn parameter_reply_decodes_firmware_timing_and_user_configuration() {
        assert_eq!(
            WidgetParameters::decode_reply(&[0x44, 0x01, 9, 2, 40, 7, 8]),
            Ok(WidgetParameterReply {
                firmware_version: 0x0144,
                parameters: WidgetParameters::default(),
                user_configuration: vec![7, 8],
            })
        );
        assert_eq!(
            WidgetParameters::decode_reply(&[1, 2, 3]),
            Err(ProProtocolError::MalformedParameterReply(3))
        );
    }
}
