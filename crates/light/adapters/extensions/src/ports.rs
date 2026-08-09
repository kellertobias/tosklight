use light_extensions_contract::{
    CanonicalControlIntent, ControlInputEvent, FeedbackDelta, FeedbackSnapshot, TelemetrySample,
    TimecodeSample,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct BoundControlInput {
    pub input: ControlInputEvent,
    pub intent: CanonicalControlIntent,
}

/// Host-owned authority supplied to an extension input. The child cannot select another desk or
/// pretend to be a user/network transport.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostControlContext {
    pub extension_id: String,
    pub extension_instance_id: String,
    pub desk_id: String,
    pub source: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TelemetryEnvelope {
    pub extension_id: String,
    pub extension_instance_id: String,
    /// Host receipt timestamp; extension-provided timestamps are never treated as host time.
    pub received_at_micros: u64,
    pub sample: TelemetrySample,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimecodeEnvelope {
    pub extension_id: String,
    pub extension_instance_id: String,
    pub received_at_micros: u64,
    pub sample: TimecodeSample,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortError {
    pub detail: String,
}

impl PortError {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for PortError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for PortError {}

/// The only application-facing seam in the Phase-0 host.
///
/// A production adapter will translate these calls into canonical application services. The
/// extension process never receives an HTTP/OSC client or an engine/output reference.
pub trait ExtensionApplicationPorts: Send + Sync + 'static {
    fn feedback_snapshot(
        &self,
        context: &HostControlContext,
        bindings: &BTreeMap<String, CanonicalControlIntent>,
    ) -> FeedbackSnapshot;

    fn apply_control(
        &self,
        context: &HostControlContext,
        input: BoundControlInput,
    ) -> Result<Option<FeedbackDelta>, PortError>;

    fn publish_telemetry(&self, telemetry: TelemetryEnvelope) -> Result<(), PortError>;

    fn publish_timecode(&self, _timecode: TimecodeEnvelope) -> Result<(), PortError> {
        Err(PortError::new("timecode application port is not connected"))
    }
}
