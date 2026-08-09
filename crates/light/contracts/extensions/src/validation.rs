use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{
    CanonicalControlIntent, Configure, ControlInput, ControlInputEvent, DRAFT_PROTOCOL_V1,
    DeviceActionDeclaration, DeviceActionRequest, DeviceActionResult, ExtensionCapability,
    ExtensionHello, HostHello, Message, PlaybackControl, ProtocolVersion,
    TelemetryChannelDeclaration, TelemetrySample, TelemetryValue, TelemetryValueKind, TimecodeRate,
    TimecodeSample,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NegotiatedHandshake {
    pub version: ProtocolVersion,
    pub capabilities: BTreeSet<ExtensionCapability>,
}

/// Host-owned facts established while validating the package and spawning this one process.
/// These values are not accepted from the extension as authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandshakeExpectations {
    pub extension_id: String,
    pub extension_instance_id: String,
    pub approved_package_digest: String,
    /// Expected opaque response for the challenge sent in [`HostHello`].
    pub channel_response: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum HandshakeError {
    #[error("the host does not offer extension protocol v1")]
    HostDoesNotOfferV1,
    #[error("the extension selected unsupported protocol version {0}")]
    UnsupportedSelectedVersion(u16),
    #[error("handshake identity field {0} must not be empty")]
    EmptyIdentity(&'static str),
    #[error("extension identity does not match the spawned package")]
    ExtensionIdentityMismatch,
    #[error("extension instance does not match the supervised process")]
    ExtensionInstanceMismatch,
    #[error("extension package digest does not match the approved package")]
    PackageDigestMismatch,
    #[error("extension did not prove possession of the private channel credential")]
    ChannelAuthenticationFailed,
    #[error("the extension does not declare requested capability {0:?}")]
    MissingCapability(ExtensionCapability),
    #[error("configuration enables capability {0:?} outside the negotiated set")]
    UnnegotiatedCapability(ExtensionCapability),
    #[error("control-surface configuration requires a full feedback snapshot")]
    MissingFeedbackSnapshot,
    #[error("configuration contains feedback without enabling the control-surface capability")]
    UnexpectedFeedbackSnapshot,
    #[error("telemetry-source configuration requires at least one declared channel")]
    MissingTelemetryChannels,
    #[error("configuration declares telemetry channels without enabling telemetry-source")]
    UnexpectedTelemetryChannels,
    #[error("configuration declares device actions without enabling telemetry-source")]
    UnexpectedDeviceActions,
    #[error("telemetry channel ID {0} is declared more than once")]
    DuplicateTelemetryChannel(String),
    #[error("telemetry declaration field {field} must not be empty for channel {channel_id}")]
    InvalidTelemetryDeclaration {
        channel_id: String,
        field: &'static str,
    },
    #[error("telemetry sample references undeclared channel {0}")]
    UndeclaredTelemetryChannel(String),
    #[error("telemetry sample value kind does not match channel {0}")]
    TelemetryValueKindMismatch(String),
    #[error("telemetry sample for channel {0} has a non-finite or out-of-range value")]
    TelemetryValueOutOfRange(String),
    #[error("telemetry sample for channel {0} has unsupported quality")]
    UnsupportedTelemetryQuality(String),
    #[error("telemetry sample for channel {0} has no source timestamp")]
    MissingTelemetryTimestamp(String),
    #[error("control input references unbound control {0}")]
    UnboundControl(String),
    #[error("control input value is invalid for binding {0}")]
    ControlValueMismatch(String),
    #[error("timecode sample contains an invalid clock or frame value")]
    InvalidTimecodeSample,
    #[error("device action {0} is not declared or permitted for this session")]
    UndeclaredDeviceAction(String),
    #[error("device action {action_id} has invalid parameters")]
    InvalidDeviceActionParameters { action_id: String },
}

pub fn validate_device_action_request(
    request: &DeviceActionRequest,
    declarations: &[DeviceActionDeclaration],
) -> Result<(), HandshakeError> {
    let Some(declaration) = declarations
        .iter()
        .find(|declaration| declaration.action_id == request.action_id)
    else {
        return Err(HandshakeError::UndeclaredDeviceAction(
            request.action_id.clone(),
        ));
    };
    if !typed_values_match(&request.parameters, &declaration.parameters) {
        return Err(HandshakeError::InvalidDeviceActionParameters {
            action_id: request.action_id.clone(),
        });
    }
    Ok(())
}

pub fn validate_device_action_result(
    result: &DeviceActionResult,
    declaration: &DeviceActionDeclaration,
) -> Result<(), HandshakeError> {
    if result.action_id != declaration.action_id
        || !typed_values_match(&result.values, &declaration.result_values)
    {
        Err(HandshakeError::InvalidDeviceActionParameters {
            action_id: result.action_id.clone(),
        })
    } else {
        Ok(())
    }
}

fn typed_values_match(
    values: &BTreeMap<String, TelemetryValue>,
    schema: &BTreeMap<String, TelemetryValueKind>,
) -> bool {
    values.len() == schema.len()
        && schema.iter().all(|(name, expected)| {
            values.get(name).is_some_and(|value| {
                matches!(
                    (value, expected),
                    (TelemetryValue::Number(_), TelemetryValueKind::Number)
                        | (TelemetryValue::Integer(_), TelemetryValueKind::Integer)
                        | (TelemetryValue::Boolean(_), TelemetryValueKind::Boolean)
                        | (TelemetryValue::Text(_), TelemetryValueKind::Text)
                )
            })
        })
}

pub fn validate_timecode_sample(sample: &TimecodeSample) -> Result<(), HandshakeError> {
    let frames_per_second = match sample.rate {
        TimecodeRate::Fps24 => 24,
        TimecodeRate::Fps25 => 25,
        TimecodeRate::Fps2997 | TimecodeRate::Fps30 => 30,
    };
    if sample.observed_at_micros == 0
        || sample.hours > 23
        || sample.minutes > 59
        || sample.seconds > 59
        || sample.frames >= frames_per_second
        || (sample.drop_frame != (sample.rate == TimecodeRate::Fps2997))
    {
        Err(HandshakeError::InvalidTimecodeSample)
    } else {
        Ok(())
    }
}

pub fn validate_control_input(
    input: &ControlInputEvent,
    bindings: &BTreeMap<String, CanonicalControlIntent>,
) -> Result<(), HandshakeError> {
    let control_id = match &input.control {
        ControlInput::Button { control_id, .. }
        | ControlInput::Absolute { control_id, .. }
        | ControlInput::Relative { control_id, .. } => control_id,
    };
    let Some(binding) = bindings.get(control_id) else {
        return Err(HandshakeError::UnboundControl(control_id.clone()));
    };
    let valid = match (&input.control, binding) {
        (ControlInput::Absolute { value, .. }, _)
            if !value.is_finite() || !(0.0..=1.0).contains(value) =>
        {
            false
        }
        (ControlInput::Relative { delta, .. }, _) if *delta == 0 => false,
        (ControlInput::Absolute { .. }, CanonicalControlIntent::GrandMaster) => true,
        (ControlInput::Absolute { .. }, CanonicalControlIntent::Encoder { .. }) => true,
        (
            ControlInput::Absolute { .. },
            CanonicalControlIntent::PlaybackCurrent {
                control: PlaybackControl::Master,
                ..
            }
            | CanonicalControlIntent::PlaybackExplicit {
                control: PlaybackControl::Master,
                ..
            },
        ) => true,
        (ControlInput::Absolute { .. }, CanonicalControlIntent::SpeedGroup { .. }) => true,
        (ControlInput::Relative { .. }, CanonicalControlIntent::Encoder { .. }) => true,
        (ControlInput::Button { .. }, CanonicalControlIntent::GrandMaster) => false,
        (
            ControlInput::Button { .. },
            CanonicalControlIntent::PlaybackCurrent {
                control: PlaybackControl::Master,
                ..
            }
            | CanonicalControlIntent::PlaybackExplicit {
                control: PlaybackControl::Master,
                ..
            },
        ) => false,
        (ControlInput::Button { .. }, _) => true,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(HandshakeError::ControlValueMismatch(control_id.clone()))
    }
}

/// Validates both handshake halves and returns the exact v1 capability grant.
pub fn negotiate(
    host: &HostHello,
    extension: &ExtensionHello,
    expected: &HandshakeExpectations,
) -> Result<NegotiatedHandshake, HandshakeError> {
    require_non_empty("host_name", &host.host_name)?;
    require_non_empty("host_instance_id", &host.host_instance_id)?;
    require_non_empty("channel_challenge", &host.channel_challenge)?;
    require_non_empty("extension_id", &extension.extension_id)?;
    require_non_empty("extension_instance_id", &extension.extension_instance_id)?;
    require_non_empty("extension_version", &extension.extension_version)?;
    require_non_empty("package_digest", &extension.package_digest)?;
    require_non_empty("channel_response", &extension.channel_response)?;

    if extension.extension_id != expected.extension_id {
        return Err(HandshakeError::ExtensionIdentityMismatch);
    }
    if extension.extension_instance_id != expected.extension_instance_id {
        return Err(HandshakeError::ExtensionInstanceMismatch);
    }
    if extension.package_digest != expected.approved_package_digest {
        return Err(HandshakeError::PackageDigestMismatch);
    }
    if extension.channel_response != expected.channel_response {
        return Err(HandshakeError::ChannelAuthenticationFailed);
    }

    if !host.supported_versions.contains(&DRAFT_PROTOCOL_V1) {
        return Err(HandshakeError::HostDoesNotOfferV1);
    }
    if extension.selected_version != DRAFT_PROTOCOL_V1 {
        return Err(HandshakeError::UnsupportedSelectedVersion(
            extension.selected_version.0,
        ));
    }
    for requested in &host.requested_capabilities {
        if !extension.capabilities.contains(requested) {
            return Err(HandshakeError::MissingCapability(*requested));
        }
    }

    Ok(NegotiatedHandshake {
        version: DRAFT_PROTOCOL_V1,
        capabilities: host.requested_capabilities.clone(),
    })
}

impl NegotiatedHandshake {
    pub fn validate_configure(&self, configure: &Configure) -> Result<(), HandshakeError> {
        for enabled in &configure.enabled_capabilities {
            if !self.capabilities.contains(enabled) {
                return Err(HandshakeError::UnnegotiatedCapability(*enabled));
            }
        }
        if configure
            .enabled_capabilities
            .contains(&ExtensionCapability::ControlSurface)
            && configure.feedback.is_none()
        {
            return Err(HandshakeError::MissingFeedbackSnapshot);
        }
        if !configure
            .enabled_capabilities
            .contains(&ExtensionCapability::ControlSurface)
            && configure.feedback.is_some()
        {
            return Err(HandshakeError::UnexpectedFeedbackSnapshot);
        }

        let telemetry_enabled = configure
            .enabled_capabilities
            .contains(&ExtensionCapability::TelemetrySource);
        if telemetry_enabled && configure.telemetry_channels.is_empty() {
            return Err(HandshakeError::MissingTelemetryChannels);
        }
        if !telemetry_enabled && !configure.telemetry_channels.is_empty() {
            return Err(HandshakeError::UnexpectedTelemetryChannels);
        }
        if !telemetry_enabled && !configure.device_actions.is_empty() {
            return Err(HandshakeError::UnexpectedDeviceActions);
        }
        let mut channel_ids = BTreeSet::new();
        for channel in &configure.telemetry_channels {
            validate_telemetry_declaration(channel)?;
            if !channel_ids.insert(&channel.channel_id) {
                return Err(HandshakeError::DuplicateTelemetryChannel(
                    channel.channel_id.clone(),
                ));
            }
        }
        let mut action_ids = BTreeSet::new();
        for action in &configure.device_actions {
            if action.action_id.trim().is_empty()
                || action.label.trim().is_empty()
                || action.required_permission.trim().is_empty()
                || !action_ids.insert(&action.action_id)
            {
                return Err(HandshakeError::UndeclaredDeviceAction(
                    action.action_id.clone(),
                ));
            }
        }
        Ok(())
    }
}

/// Validates a sample against the complete channel declarations accepted in `Configure`.
pub fn validate_telemetry_sample(
    sample: &TelemetrySample,
    channels: &[TelemetryChannelDeclaration],
) -> Result<(), HandshakeError> {
    let Some(channel) = channels
        .iter()
        .find(|channel| channel.channel_id == sample.channel_id)
    else {
        return Err(HandshakeError::UndeclaredTelemetryChannel(
            sample.channel_id.clone(),
        ));
    };
    let actual = match &sample.value {
        TelemetryValue::Number(_) => TelemetryValueKind::Number,
        TelemetryValue::Integer(_) => TelemetryValueKind::Integer,
        TelemetryValue::Boolean(_) => TelemetryValueKind::Boolean,
        TelemetryValue::Text(_) => TelemetryValueKind::Text,
    };
    if actual != channel.value_kind {
        return Err(HandshakeError::TelemetryValueKindMismatch(
            sample.channel_id.clone(),
        ));
    }
    if sample.observed_at_micros == 0 {
        return Err(HandshakeError::MissingTelemetryTimestamp(
            sample.channel_id.clone(),
        ));
    }
    let numeric = match &sample.value {
        TelemetryValue::Number(value) => Some(*value),
        TelemetryValue::Integer(value) => Some(*value as f64),
        _ => None,
    };
    if numeric.is_some_and(|value| {
        !value.is_finite()
            || channel.minimum.is_some_and(|minimum| value < minimum)
            || channel.maximum.is_some_and(|maximum| value > maximum)
    }) {
        return Err(HandshakeError::TelemetryValueOutOfRange(
            sample.channel_id.clone(),
        ));
    }
    let accepted_qualities = if channel.quality_flags.is_empty() {
        BTreeSet::from([crate::TelemetryQuality::Good])
    } else {
        channel.quality_flags.clone()
    };
    if !accepted_qualities.contains(&sample.quality) {
        return Err(HandshakeError::UnsupportedTelemetryQuality(
            sample.channel_id.clone(),
        ));
    }
    Ok(())
}

fn validate_telemetry_declaration(
    channel: &TelemetryChannelDeclaration,
) -> Result<(), HandshakeError> {
    for (field, value) in [
        ("channel_id", channel.channel_id.as_str()),
        ("label", channel.label.as_str()),
        ("quantity", channel.quantity.as_str()),
        ("unit", channel.unit.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(HandshakeError::InvalidTelemetryDeclaration {
                channel_id: channel.channel_id.clone(),
                field,
            });
        }
    }
    let numeric = matches!(
        channel.value_kind,
        TelemetryValueKind::Number | TelemetryValueKind::Integer
    );
    if channel.minimum.is_some_and(|value| !value.is_finite())
        || channel.maximum.is_some_and(|value| !value.is_finite())
        || channel
            .minimum
            .zip(channel.maximum)
            .is_some_and(|(minimum, maximum)| minimum > maximum)
        || (!numeric
            && (channel.minimum.is_some()
                || channel.maximum.is_some()
                || channel.precision.is_some()))
        || channel.expected_interval_micros == Some(0)
    {
        return Err(HandshakeError::InvalidTelemetryDeclaration {
            channel_id: channel.channel_id.clone(),
            field: "metadata",
        });
    }
    Ok(())
}

/// Rejects a typed payload when its message family was not negotiated.
pub fn validate_capability(
    message: &Message,
    enabled: &BTreeSet<ExtensionCapability>,
) -> Result<(), HandshakeError> {
    if let Some(required) = message.required_capability()
        && !enabled.contains(&required)
    {
        return Err(HandshakeError::UnnegotiatedCapability(required));
    }
    Ok(())
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), HandshakeError> {
    if value.trim().is_empty() {
        Err(HandshakeError::EmptyIdentity(field))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::{
        ControlInput, ControlInputEvent, FeedbackContext, FeedbackSnapshot, HealthReport,
        HealthStatus, TelemetryChannelDeclaration, TelemetryQuality, TelemetrySample,
        TelemetryValue, TelemetryValueKind,
    };

    fn capabilities(
        values: impl IntoIterator<Item = ExtensionCapability>,
    ) -> BTreeSet<ExtensionCapability> {
        values.into_iter().collect()
    }

    fn host() -> HostHello {
        HostHello {
            host_name: "ToskLight".into(),
            host_instance_id: "desk-main".into(),
            supported_versions: vec![DRAFT_PROTOCOL_V1],
            requested_capabilities: capabilities([ExtensionCapability::ControlSurface]),
            channel_challenge: "fresh-challenge".into(),
        }
    }

    fn extension() -> ExtensionHello {
        ExtensionHello {
            extension_id: "example.surface".into(),
            extension_instance_id: "spawn-42".into(),
            extension_version: "1.2.3".into(),
            package_digest: "sha256:approved".into(),
            selected_version: DRAFT_PROTOCOL_V1,
            capabilities: capabilities([
                ExtensionCapability::ControlSurface,
                ExtensionCapability::TelemetrySource,
            ]),
            channel_response: "opaque-response".into(),
        }
    }

    fn expectations() -> HandshakeExpectations {
        HandshakeExpectations {
            extension_id: "example.surface".into(),
            extension_instance_id: "spawn-42".into(),
            approved_package_digest: "sha256:approved".into(),
            channel_response: "opaque-response".into(),
        }
    }

    #[test]
    fn negotiates_exact_requested_capabilities() {
        let negotiated = negotiate(&host(), &extension(), &expectations()).unwrap();
        assert_eq!(negotiated.version, DRAFT_PROTOCOL_V1);
        assert_eq!(
            negotiated.capabilities,
            capabilities([ExtensionCapability::ControlSurface])
        );
    }

    #[test]
    fn rejects_unsupported_versions_and_missing_capabilities() {
        let mut unsupported_host = host();
        unsupported_host.supported_versions = vec![ProtocolVersion(2)];
        assert_eq!(
            negotiate(&unsupported_host, &extension(), &expectations()),
            Err(HandshakeError::HostDoesNotOfferV1)
        );

        let mut unsupported_extension = extension();
        unsupported_extension.selected_version = ProtocolVersion(2);
        assert_eq!(
            negotiate(&host(), &unsupported_extension, &expectations()),
            Err(HandshakeError::UnsupportedSelectedVersion(2))
        );

        let mut missing = extension();
        missing.capabilities.clear();
        assert_eq!(
            negotiate(&host(), &missing, &expectations()),
            Err(HandshakeError::MissingCapability(
                ExtensionCapability::ControlSurface
            ))
        );
    }

    #[test]
    fn rejects_identity_digest_instance_and_channel_authentication_mismatches() {
        let cases = [
            (
                HandshakeExpectations {
                    extension_id: "other.extension".into(),
                    ..expectations()
                },
                HandshakeError::ExtensionIdentityMismatch,
            ),
            (
                HandshakeExpectations {
                    extension_instance_id: "different-spawn".into(),
                    ..expectations()
                },
                HandshakeError::ExtensionInstanceMismatch,
            ),
            (
                HandshakeExpectations {
                    approved_package_digest: "sha256:different".into(),
                    ..expectations()
                },
                HandshakeError::PackageDigestMismatch,
            ),
            (
                HandshakeExpectations {
                    channel_response: "wrong-response".into(),
                    ..expectations()
                },
                HandshakeError::ChannelAuthenticationFailed,
            ),
        ];
        for (expected, error) in cases {
            assert_eq!(negotiate(&host(), &extension(), &expected), Err(error));
        }
    }

    #[test]
    fn validates_configure_and_message_capabilities() {
        let negotiated = negotiate(&host(), &extension(), &expectations()).unwrap();
        let mut configure = Configure {
            enabled_capabilities: capabilities([ExtensionCapability::ControlSurface]),
            feedback: Some(FeedbackSnapshot {
                context: FeedbackContext::default(),
                revision: 0,
                controls: BTreeMap::new(),
            }),
            telemetry_channels: Vec::new(),
            device_actions: Vec::new(),
            control_bindings: BTreeMap::new(),
            settings: BTreeMap::new(),
        };
        negotiated.validate_configure(&configure).unwrap();

        configure.feedback = None;
        assert_eq!(
            negotiated.validate_configure(&configure),
            Err(HandshakeError::MissingFeedbackSnapshot)
        );

        let input = Message::ControlInput(ControlInputEvent {
            input_id: 1,
            occurred_at_micros: 2,
            control: ControlInput::Button {
                control_id: "go".into(),
                pressed: true,
            },
        });
        validate_capability(&input, &negotiated.capabilities).unwrap();
        assert_eq!(
            validate_capability(&input, &BTreeSet::new()),
            Err(HandshakeError::UnnegotiatedCapability(
                ExtensionCapability::ControlSurface
            ))
        );

        let health = Message::Health(HealthReport {
            status: HealthStatus::Ready,
            detail: None,
            counters: BTreeMap::new(),
        });
        validate_capability(&health, &BTreeSet::new()).unwrap();
    }

    #[test]
    fn highlight_controls_accept_buttons_and_reject_continuous_inputs() {
        use crate::HighlightControlAction;

        for action in [
            HighlightControlAction::Toggle,
            HighlightControlAction::Previous,
            HighlightControlAction::Next,
            HighlightControlAction::All,
        ] {
            let control_id = format!("highlight-{action:?}").to_ascii_lowercase();
            let bindings = BTreeMap::from([(
                control_id.clone(),
                CanonicalControlIntent::Highlight { action },
            )]);
            validate_control_input(
                &ControlInputEvent {
                    input_id: 1,
                    occurred_at_micros: 2,
                    control: ControlInput::Button {
                        control_id: control_id.clone(),
                        pressed: true,
                    },
                },
                &bindings,
            )
            .unwrap();
            assert_eq!(
                validate_control_input(
                    &ControlInputEvent {
                        input_id: 2,
                        occurred_at_micros: 3,
                        control: ControlInput::Absolute {
                            control_id: control_id.clone(),
                            value: 1.0,
                        },
                    },
                    &bindings,
                ),
                Err(HandshakeError::ControlValueMismatch(control_id))
            );
        }
    }

    #[test]
    fn validates_declared_telemetry_unit_quality_and_value_kind() {
        let channel = TelemetryChannelDeclaration {
            channel_id: "device-temperature".into(),
            label: "Device temperature".into(),
            quantity: "temperature".into(),
            unit: "degC".into(),
            value_kind: TelemetryValueKind::Number,
            minimum: Some(-40.0),
            maximum: Some(125.0),
            precision: Some(1),
            expected_interval_micros: Some(1_000_000),
            quality_flags: BTreeSet::from([TelemetryQuality::Good, TelemetryQuality::Stale]),
        };
        let telemetry_negotiated = NegotiatedHandshake {
            version: DRAFT_PROTOCOL_V1,
            capabilities: capabilities([ExtensionCapability::TelemetrySource]),
        };
        let configure = Configure {
            enabled_capabilities: capabilities([ExtensionCapability::TelemetrySource]),
            feedback: None,
            telemetry_channels: vec![channel.clone()],
            device_actions: Vec::new(),
            control_bindings: BTreeMap::new(),
            settings: BTreeMap::new(),
        };
        telemetry_negotiated.validate_configure(&configure).unwrap();

        let sample = TelemetrySample {
            sample_id: 9,
            observed_at_micros: 10,
            channel_id: channel.channel_id.clone(),
            value: TelemetryValue::Number(42.5),
            quality: TelemetryQuality::Good,
        };
        validate_telemetry_sample(&sample, &configure.telemetry_channels).unwrap();

        let wrong_kind = TelemetrySample {
            value: TelemetryValue::Text("hot".into()),
            ..sample.clone()
        };
        assert_eq!(
            validate_telemetry_sample(&wrong_kind, &configure.telemetry_channels),
            Err(HandshakeError::TelemetryValueKindMismatch(
                channel.channel_id.clone()
            ))
        );
        assert_eq!(
            validate_telemetry_sample(&sample, &[]),
            Err(HandshakeError::UndeclaredTelemetryChannel(
                channel.channel_id.clone()
            ))
        );
        let out_of_range = TelemetrySample {
            value: TelemetryValue::Number(126.0),
            ..sample.clone()
        };
        assert_eq!(
            validate_telemetry_sample(&out_of_range, &configure.telemetry_channels),
            Err(HandshakeError::TelemetryValueOutOfRange(
                channel.channel_id.clone()
            ))
        );
        let unsupported_quality = TelemetrySample {
            quality: TelemetryQuality::Invalid,
            ..sample
        };
        assert_eq!(
            validate_telemetry_sample(&unsupported_quality, &configure.telemetry_channels),
            Err(HandshakeError::UnsupportedTelemetryQuality(
                channel.channel_id
            ))
        );
    }

    #[test]
    fn validates_timecode_clock_frame_and_drop_frame_contract() {
        let valid = TimecodeSample {
            sample_id: 1,
            observed_at_micros: 1_000,
            hours: 23,
            minutes: 59,
            seconds: 59,
            frames: 29,
            rate: TimecodeRate::Fps2997,
            drop_frame: true,
        };
        validate_timecode_sample(&valid).unwrap();
        assert_eq!(
            validate_timecode_sample(&TimecodeSample {
                frames: 30,
                ..valid.clone()
            }),
            Err(HandshakeError::InvalidTimecodeSample)
        );
        assert_eq!(
            validate_timecode_sample(&TimecodeSample {
                rate: TimecodeRate::Fps25,
                frames: 24,
                drop_frame: true,
                ..valid
            }),
            Err(HandshakeError::InvalidTimecodeSample)
        );
    }
}
