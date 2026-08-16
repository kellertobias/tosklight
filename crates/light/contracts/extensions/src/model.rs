use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// The prototype's numeric major version.
///
/// This remains explicitly draft until the host and portable test extension pass on macOS,
/// Windows, and Linux; a numeric `1` is not a stability promise.
pub const DRAFT_PROTOCOL_V1: ProtocolVersion = ProtocolVersion(1);

/// A wire-compatible protocol version number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProtocolVersion(pub u16);

/// A privilege and message-family boundary declared during the handshake.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionCapability {
    ControlSurface,
    TelemetrySource,
    TimecodeSource,
}

/// One versioned, sequenced JSON payload. The codec adds a four-byte big-endian length prefix.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub version: ProtocolVersion,
    pub sequence: u64,
    pub message: Message,
}

impl Frame {
    pub fn v1(sequence: u64, message: Message) -> Self {
        Self {
            version: DRAFT_PROTOCOL_V1,
            sequence,
            message,
        }
    }
}

/// Messages in either direction. Each transport direction owns an independent frame sequence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "body", rename_all = "snake_case")]
pub enum Message {
    HostHello(HostHello),
    ExtensionHello(ExtensionHello),
    Configure(Configure),
    ControlInput(ControlInputEvent),
    /// Full replacement sent after reconnect or when an ordered delta queue overflows.
    FeedbackSnapshot(FeedbackSnapshot),
    FeedbackDelta(FeedbackDelta),
    TelemetrySample(TelemetrySample),
    DeviceActionRequest(DeviceActionRequest),
    DeviceActionResult(DeviceActionResult),
    TimecodeSample(TimecodeSample),
    Health(HealthReport),
    Shutdown(Shutdown),
    ProtocolError(ProtocolErrorMessage),
}

impl Message {
    /// Returns the negotiated capability required to carry this message, if any.
    pub fn required_capability(&self) -> Option<ExtensionCapability> {
        match self {
            Self::ControlInput(_) | Self::FeedbackSnapshot(_) | Self::FeedbackDelta(_) => {
                Some(ExtensionCapability::ControlSurface)
            }
            Self::TelemetrySample(_)
            | Self::DeviceActionRequest(_)
            | Self::DeviceActionResult(_) => Some(ExtensionCapability::TelemetrySource),
            Self::TimecodeSample(_) => Some(ExtensionCapability::TimecodeSource),
            Self::HostHello(_)
            | Self::ExtensionHello(_)
            | Self::Configure(_)
            | Self::Health(_)
            | Self::Shutdown(_)
            | Self::ProtocolError(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostHello {
    pub host_name: String,
    pub host_instance_id: String,
    pub supported_versions: Vec<ProtocolVersion>,
    pub requested_capabilities: BTreeSet<ExtensionCapability>,
    /// Fresh opaque challenge for the separately provisioned private channel credential.
    pub channel_challenge: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionHello {
    pub extension_id: String,
    /// Host-assigned ID for this one supervised process, distinct from the package ID.
    pub extension_instance_id: String,
    pub extension_version: String,
    /// Digest of the exact package image this process believes it is running.
    pub package_digest: String,
    pub selected_version: ProtocolVersion,
    pub capabilities: BTreeSet<ExtensionCapability>,
    /// Opaque response computed from the provisioned channel credential and current challenge.
    pub channel_response: String,
}

/// The host's authoritative startup configuration for the negotiated session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Configure {
    pub enabled_capabilities: BTreeSet<ExtensionCapability>,
    /// Control-surface extensions start from a full replaceable projection, never engine access.
    pub feedback: Option<FeedbackSnapshot>,
    /// Complete declarations for telemetry channels enabled in this session.
    #[serde(default)]
    pub telemetry_channels: Vec<TelemetryChannelDeclaration>,
    /// Only actions whose required permission was granted for this instance.
    #[serde(default)]
    pub device_actions: Vec<DeviceActionDeclaration>,
    /// Installation-owned mapping from declared logical controls to canonical desk intents.
    #[serde(default)]
    pub control_bindings: BTreeMap<String, CanonicalControlIntent>,
    /// Installation-owned opaque settings validated by the extension's package contract.
    #[serde(default)]
    pub settings: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlInputEvent {
    pub input_id: u64,
    pub occurred_at_micros: u64,
    pub control: ControlInput,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ControlInput {
    Button { control_id: String, pressed: bool },
    Absolute { control_id: String, value: f32 },
    Relative { control_id: String, delta: i32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CanonicalControlIntent {
    ProgrammerKey {
        key: ProgrammerKey,
    },
    Modifier {
        modifier: ModifierKey,
    },
    Navigation {
        action: NavigationAction,
    },
    Highlight {
        action: HighlightControlAction,
    },
    Encoder {
        index: u8,
    },
    PlaybackCurrent {
        slot: u16,
        control: PlaybackControl,
    },
    PlaybackExplicit {
        page: u16,
        slot: u16,
        control: PlaybackControl,
    },
    SpeedGroup {
        group: char,
        control: SpeedGroupControl,
    },
    GrandMaster,
    Blackout,
    DeskCommand {
        command: DeskCommand,
    },
}

/// Canonical Highlight controls exposed by an attached native surface.
///
/// Buttons are edge-triggered: the host applies the action on press and ignores release. This
/// keeps a physical HIGH key aligned with the software desk's latched toggle instead of turning
/// Highlight into a momentary look.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightControlAction {
    Toggle,
    Previous,
    Next,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammerKey {
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Plus,
    Minus,
    Point,
    At,
    Enter,
    Clear,
    Undo,
    Group,
    Cue,
    Playback,
    Off,
    Record,
    Preload,
    Delete,
    Copy,
    Move,
    Set,
    Time,
    Thru,
    Divide,
    Backspace,
    Escape,
    Highlight,
    Previous,
    Next,
    All,
    EncoderPlayback,
    PageUp,
    PageDown,
    Align,
    Fade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModifierKey {
    Shift,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationAction {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Menu,
    Escape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackControl {
    ButtonOne,
    ButtonTwo,
    ButtonThree,
    Master,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedGroupControl {
    Tap,
    Double,
    Half,
    Level,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeskCommand {
    Home,
    Stage,
    Fixtures,
    Channels,
    Groups,
    Presets,
    Cues,
    Playbacks,
    Setup,
    Help,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeedbackSnapshot {
    pub context: FeedbackContext,
    pub revision: u64,
    pub controls: BTreeMap<String, FeedbackValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeedbackDelta {
    pub context: FeedbackContext,
    pub base_revision: u64,
    pub revision: u64,
    pub changes: Vec<FeedbackChange>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackContext {
    /// Canonical host-owned logical desk identity, never supplied by the child.
    pub desk_id: String,
    /// Active show identity. `None` makes startup/no-show feedback explicit.
    pub show_id: Option<String>,
    /// Monotonic generation within the active show, distinct from the feedback stream revision.
    pub show_generation: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeedbackChange {
    pub control_id: String,
    pub value: Option<FeedbackValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FeedbackValue {
    Boolean(bool),
    Level(f32),
    Text(String),
    Rgb { red: u8, green: u8, blue: u8 },
    Control(FeedbackControlState),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct FeedbackControlState {
    pub available: bool,
    pub enabled: bool,
    pub selected: bool,
    pub warning: bool,
    pub error: bool,
    pub lamp: LampState,
    pub semantic_color: Option<String>,
    pub resolved_rgb: Option<[u8; 3]>,
    pub value: Option<f32>,
    pub ring_style: Option<EncoderRingStyle>,
    pub text: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LampState {
    #[default]
    Off,
    Dim,
    On,
    BlinkSlow,
    BlinkFast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncoderRingStyle {
    Dot,
    Bar,
    Spread,
    Pan,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TelemetrySample {
    pub sample_id: u64,
    pub observed_at_micros: u64,
    pub channel_id: String,
    pub value: TelemetryValue,
    pub quality: TelemetryQuality,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TelemetryChannelDeclaration {
    pub channel_id: String,
    /// Human-readable channel label suitable for diagnostics and operator surfaces.
    pub label: String,
    /// Human-readable physical or semantic quantity, for example `temperature` or `battery`.
    pub quantity: String,
    /// Explicit unit symbol or the literal `unitless`; never inferred from the channel name.
    pub unit: String,
    pub value_kind: TelemetryValueKind,
    /// Inclusive numeric bounds. Non-numeric channels leave both fields unset.
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    /// Decimal places meaningful at the source. This is metadata, not a rounding instruction.
    pub precision: Option<u8>,
    /// Expected source cadence; the host uses this to report stale channels.
    pub expected_interval_micros: Option<u64>,
    /// Quality values this channel can emit. Empty means only `good` is accepted.
    #[serde(default)]
    pub quality_flags: BTreeSet<TelemetryQuality>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryValueKind {
    Number,
    Integer,
    Boolean,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryQuality {
    Good,
    Stale,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TelemetryValue {
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceActionDeclaration {
    pub action_id: String,
    pub label: String,
    pub required_permission: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, TelemetryValueKind>,
    #[serde(default)]
    pub result_values: BTreeMap<String, TelemetryValueKind>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeviceActionRequest {
    pub request_id: u64,
    pub action_id: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, TelemetryValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeviceActionResult {
    pub request_id: u64,
    pub action_id: String,
    pub status: DeviceActionStatus,
    pub detail: Option<String>,
    #[serde(default)]
    pub values: BTreeMap<String, TelemetryValue>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceActionStatus {
    Completed,
    Rejected,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimecodeSample {
    pub sample_id: u64,
    pub observed_at_micros: u64,
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
    pub frames: u8,
    pub rate: TimecodeRate,
    pub drop_frame: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimecodeRate {
    Fps24,
    Fps25,
    Fps2997,
    Fps30,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: HealthStatus,
    pub detail: Option<String>,
    #[serde(default)]
    pub counters: BTreeMap<String, u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Starting,
    Ready,
    Degraded,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shutdown {
    pub reason: ShutdownReason,
    pub detail: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownReason {
    HostRequested,
    ExtensionRequested,
    Reconfigure,
    ProtocolFailure,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolErrorMessage {
    pub code: ProtocolErrorCode,
    pub detail: String,
    pub rejected_sequence: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolErrorCode {
    UnsupportedVersion,
    InvalidHandshake,
    CapabilityNotNegotiated,
    InvalidSequence,
    InvalidPayload,
    FrameTooLarge,
}
