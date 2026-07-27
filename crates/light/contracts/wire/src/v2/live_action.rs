//! Explicit typed action frames carried on the established desk WebSocket.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::{
    command_line::{CommandTarget, ProgrammingSelectionActionRequest},
    dynamics::{
        DynamicControllerLiveActionRequest, DynamicFixAtActionRequest, DynamicOffLiveActionRequest,
        DynamicStartLiveActionRequest,
    },
    output_control::{DmxOverrideRequest, HighlightActionRequest, PatchPreviewHighlightRequest},
    output_runtime::OutputRuntimeActionRequest,
    playback::PlaybackActionRequest,
    preload_lifecycle::ProgrammingPreloadLifecycleRequest,
    preload_values::ProgrammingPreloadValuesActionRequest,
    preset_recall::PresetRecallRequest,
    programmer_priority::ProgrammerPriorityActionRequest,
    programming::ProgrammingValuesActionRequest,
    speed_group::SpeedGroupActionRequest,
};

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct LiveActionFrame {
    #[serde(rename = "type")]
    pub message_type: LiveActionMessageType,
    pub protocol_version: u16,
    pub request_id: String,
    pub session_id: Uuid,
    pub action: LiveAction,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LiveActionMessageType {
    Action,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", content = "request", rename_all = "snake_case")]
pub enum LiveAction {
    ProgrammingSelection(ProgrammingSelectionActionRequest),
    ProgrammingValues(ProgrammingValuesActionRequest),
    ProgrammerCaptureMode(ProgrammerCaptureModeLiveActionRequest),
    ProgrammerPriority(ProgrammerPriorityActionRequest),
    ProgrammerPreloadLifecycle(ProgrammingPreloadLifecycleRequest),
    ProgrammerPreloadValues(ProgrammingPreloadValuesActionRequest),
    PresetRecall(PresetRecallLiveActionRequest),
    Playback(PlaybackActionRequest),
    SpeedGroup(SpeedGroupActionRequest),
    OutputRuntime(OutputRuntimeActionRequest),
    DmxOverride(DmxOverrideRequest),
    Highlight(HighlightActionRequest),
    PatchPreviewHighlight(PatchPreviewHighlightRequest),
    CommandLineReplace(CommandLineReplaceLiveActionRequest),
    CommandLineSet(CommandLineSetLiveActionRequest),
    CommandTarget(CommandTargetLiveActionRequest),
    CommandLineExecute(CommandLineExecuteLiveActionRequest),
    ProgrammerUndo,
    ProgrammingAlign(ProgrammingAlignLiveActionRequest),
    FixtureControl(FixtureControlLiveActionRequest),
    DynamicToggle(DynamicStartLiveActionRequest),
    DynamicStart(DynamicStartLiveActionRequest),
    DynamicOff(DynamicOffLiveActionRequest),
    DynamicSize(DynamicControllerLiveActionRequest),
    DynamicSpeed(DynamicControllerLiveActionRequest),
    DynamicPhase(DynamicControllerLiveActionRequest),
    DynamicFixAt(DynamicFixAtActionRequest),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PresetRecallLiveActionRequest {
    pub request_id: String,
    pub show_id: Uuid,
    pub request: PresetRecallRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CommandLineReplaceLiveActionRequest {
    #[ts(type = "number")]
    pub expected_revision: u64,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CommandLineSetLiveActionRequest {
    pub value: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CommandTargetLiveActionRequest {
    pub value: CommandTarget,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CommandLineExecuteLiveActionRequest {
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CommandTargetHttpActionRequest {
    pub value: CommandTarget,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CommandTargetHttpActionOutcome {
    pub request_id: String,
    pub target: CommandTarget,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammerUndoHttpActionOutcome {
    pub request_id: String,
    pub changed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammerCaptureModeLiveActionRequest {
    pub request_id: String,
    #[serde(default)]
    pub blind: Option<bool>,
    #[serde(default)]
    pub preview: Option<bool>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_active_context",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional = nullable, type = "string | null")]
    pub active_context: Option<Option<String>>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammerCaptureModeHttpActionRequest {
    #[serde(default)]
    pub blind: Option<bool>,
    #[serde(default)]
    pub preview: Option<bool>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_active_context",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional = nullable, type = "string | null")]
    pub active_context: Option<Option<String>>,
}

fn deserialize_optional_active_context<'de, D>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammerCaptureModeOutcome {
    pub request_id: String,
    pub blind: bool,
    pub preview: bool,
    pub active_context: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingAlignLiveActionRequest {
    pub request_id: String,
    pub attribute: String,
    pub mode: ProgrammingAlignMode,
    pub from: f32,
    pub to: f32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingAlignHttpActionRequest {
    pub attribute: String,
    pub mode: ProgrammingAlignMode,
    pub from: f32,
    pub to: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingAlignMode {
    Left,
    Right,
    Center,
    Out,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureControlLiveActionRequest {
    pub request_id: String,
    pub fixture_id: Uuid,
    pub action_id: Uuid,
    pub active: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureControlHttpActionRequest {
    pub fixture_id: Uuid,
    pub action_id: Uuid,
    pub active: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingAlignOutcome {
    pub request_id: String,
    pub unsupported_fixtures: Vec<Uuid>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FixtureControlKind {
    Latched,
    Momentary,
    Pulse,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureControlOutcome {
    pub request_id: String,
    pub action_id: Uuid,
    pub active: bool,
    pub kind: FixtureControlKind,
    #[ts(type = "number | null")]
    pub pulse_duration_millis: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct GenerateFixturePresetsRequest {
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_show_revision: u64,
    pub fixture_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct GeneratedFixturePreset {
    pub address: super::preset_recording::PresetRecordingAddress,
    pub number: u32,
    pub name: String,
    pub family: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct GenerateFixturePresetsOutcome {
    pub request_id: String,
    pub correlation_id: Uuid,
    pub replayed: bool,
    #[ts(type = "number")]
    pub show_revision: u64,
    #[ts(type = "number")]
    pub event_sequence: u64,
    pub created: Vec<GeneratedFixturePreset>,
}

impl LiveAction {
    pub fn embedded_request_id(&self) -> Option<&str> {
        match self {
            Self::ProgrammingSelection(request) => Some(&request.request_id),
            Self::ProgrammingValues(request) => Some(&request.request_id),
            Self::ProgrammerCaptureMode(request) => Some(&request.request_id),
            Self::ProgrammerPriority(request) => Some(&request.request_id),
            Self::ProgrammerPreloadLifecycle(request) => Some(&request.request_id),
            Self::ProgrammerPreloadValues(request) => Some(&request.request_id),
            Self::PresetRecall(request) => Some(&request.request_id),
            Self::Playback(request) => Some(&request.request_id),
            Self::SpeedGroup(request) => Some(&request.request_id),
            Self::OutputRuntime(request) => Some(&request.request_id),
            Self::DmxOverride(request) => Some(&request.request_id),
            Self::Highlight(request) => Some(&request.request_id),
            Self::PatchPreviewHighlight(request) => Some(&request.request_id),
            Self::CommandLineReplace(_)
            | Self::CommandLineSet(_)
            | Self::CommandTarget(_)
            | Self::CommandLineExecute(_)
            | Self::ProgrammerUndo => None,
            Self::ProgrammingAlign(request) => Some(&request.request_id),
            Self::FixtureControl(request) => Some(&request.request_id),
            Self::DynamicToggle(request) | Self::DynamicStart(request) => {
                Some(&request.request.request_id)
            }
            Self::DynamicOff(request) => Some(&request.request.request_id),
            Self::DynamicSize(request)
            | Self::DynamicSpeed(request)
            | Self::DynamicPhase(request) => Some(&request.request.request_id),
            Self::DynamicFixAt(request) => Some(&request.request_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_frames_require_an_explicit_action_discriminant() {
        let frame: LiveActionFrame = serde_json::from_value(serde_json::json!({
            "type": "action",
            "protocol_version": 2,
            "request_id": "undo-1",
            "session_id": Uuid::from_u128(1),
            "action": {"type": "programmer_undo"}
        }))
        .unwrap();
        assert!(matches!(frame.action, LiveAction::ProgrammerUndo));

        assert!(
            serde_json::from_value::<LiveActionFrame>(serde_json::json!({
                "protocol_version": 2,
                "request_id": "undo-1",
                "session_id": Uuid::from_u128(1),
                "action": {"type": "programmer_undo"}
            }))
            .is_err()
        );
    }

    #[test]
    fn malformed_action_payloads_fail_the_generated_boundary() {
        assert!(
            serde_json::from_value::<LiveActionFrame>(serde_json::json!({
                "type": "action",
                "protocol_version": 2,
                "request_id": "highlight-1",
                "session_id": Uuid::from_u128(1),
                "action": {
                    "type": "highlight",
                    "request": {
                        "request_id": "highlight-1",
                        "action": "sideways"
                    }
                }
            }))
            .is_err()
        );
    }

    #[test]
    fn future_fields_are_tolerated_at_frame_action_and_request_layers() {
        let frame: LiveActionFrame = serde_json::from_value(serde_json::json!({
            "type":"action",
            "protocol_version":2,
            "request_id":"priority-future-fields",
            "session_id":Uuid::from_u128(1),
            "future_frame":true,
            "action":{
                "type":"programmer_priority",
                "future_action":true,
                "request":{
                    "request_id":"priority-future-fields",
                    "expected_revision":0,
                    "priority":42,
                    "future_request":true
                }
            }
        }))
        .unwrap();
        assert!(matches!(frame.action, LiveAction::ProgrammerPriority(_)));
    }

    #[test]
    fn capture_mode_preserves_omitted_clear_and_named_active_context_intents() {
        let omitted: ProgrammerCaptureModeLiveActionRequest =
            serde_json::from_value(serde_json::json!({
                "request_id": "capture-omitted"
            }))
            .unwrap();
        assert_eq!(omitted.active_context, None);
        assert_eq!(
            serde_json::to_value(&omitted).unwrap(),
            serde_json::json!({"request_id": "capture-omitted", "blind": null, "preview": null})
        );

        let cleared: ProgrammerCaptureModeLiveActionRequest =
            serde_json::from_value(serde_json::json!({
                "request_id": "capture-cleared",
                "active_context": null
            }))
            .unwrap();
        assert_eq!(cleared.active_context, Some(None));

        let named: ProgrammerCaptureModeLiveActionRequest =
            serde_json::from_value(serde_json::json!({
                "request_id": "capture-named",
                "active_context": "preview"
            }))
            .unwrap();
        assert_eq!(named.active_context, Some(Some("preview".to_owned())));
    }
}
