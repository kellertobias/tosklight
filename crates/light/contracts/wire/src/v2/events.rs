//! Stable filtered event-subscription and playback-repair DTOs.

use super::{
    command_line::ProgrammingInteractionChange,
    macros::MacroExecutionSnapshot,
    playback::{PlaybackDeskProjection, PlaybackRuntimeChange, PlaybackTelemetryTick},
    preload_playback_queue::ProgrammingPreloadPlaybackQueueChange,
    preload_values::ProgrammingPreloadValuesChange,
    programmer_lifecycle::ProgrammingLifecycleChange,
    programmer_priority::ProgrammerPriorityChange,
    programming::{ProgrammingCaptureModeChange, ProgrammingValuesChange},
    runtime::RuntimeHighlightState,
    schedules::ScheduleRuntimeChange,
    speed_group::SpeedGroupChange,
    timecode::TimecodeTransportSnapshot,
    virtual_playback_zones::VirtualPlaybackExclusionZonesChange,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct EventSubscriptionFilter {
    #[serde(default)]
    pub capabilities: Vec<EventCapability>,
    #[serde(default)]
    pub classes: Vec<EventClass>,
    #[serde(default)]
    pub objects: Vec<EventObject>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct EventRateLimit {
    pub capability: EventCapability,
    pub class: EventClass,
    pub object: Option<EventObject>,
    #[ts(type = "number")]
    pub min_interval_millis: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventClientMessage {
    Subscribe {
        #[serde(default)]
        filter: EventSubscriptionFilter,
        #[serde(default)]
        #[ts(as = "Option<f64>", optional = nullable)]
        after_sequence: Option<u64>,
        #[serde(default)]
        #[ts(optional = nullable)]
        capacity: Option<u16>,
        #[serde(default)]
        rate_limits: Vec<EventRateLimit>,
    },
    Repair {
        cursor: EventSnapshotCursor,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventServerMessage {
    Ready { cursor: EventSnapshotCursor },
    Event { event: Box<EventEnvelope> },
    Gap { gap: SequenceGap },
    Repaired { cursor: EventSnapshotCursor },
    Error { error: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct EventSnapshotCursor {
    #[ts(type = "number")]
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SequenceGap {
    #[ts(type = "number")]
    pub after_sequence: u64,
    #[ts(type = "number")]
    pub oldest_available: u64,
    #[ts(type = "number")]
    pub latest_sequence: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct EventEnvelope {
    #[ts(type = "number")]
    pub sequence: u64,
    pub occurred_at: String,
    pub desk_id: Option<Uuid>,
    pub class: EventClass,
    pub object: Option<EventObject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub related_objects: Option<Vec<EventObject>>,
    pub source: EventSource,
    pub correlation_id: Option<Uuid>,
    pub delivery: EventDeliveryPolicy,
    pub payload: EventPayload,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum EventCapability {
    Programmer,
    Playback,
    Show,
    Desk,
    Output,
    System,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum EventClass {
    Transition,
    Projection,
    CommandOutcome,
    Error,
    Safety,
    Telemetry,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
pub struct EventObject {
    pub capability: EventCapability,
    pub id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum EventDeliveryPolicy {
    Lossless,
    Replaceable,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventSource {
    Runtime,
    Action { source: EventActionSource },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum EventActionSource {
    UserInterface,
    Keyboard,
    Osc,
    Http,
    Extension,
    Matter,
    Cue,
    Timecode,
    Scheduler,
    Macro,
    System,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventPayload {
    ProgrammingInteractionChanged {
        change: ProgrammingInteractionChange,
    },
    ProgrammerPriorityChanged {
        change: ProgrammerPriorityChange,
    },
    ProgrammingValuesChanged {
        change: ProgrammingValuesChange,
    },
    ProgrammingCaptureModeChanged {
        change: ProgrammingCaptureModeChange,
    },
    ProgrammingPreloadValuesChanged {
        change: ProgrammingPreloadValuesChange,
    },
    ProgrammingPreloadPlaybackQueueChanged {
        change: ProgrammingPreloadPlaybackQueueChange,
    },
    ProgrammingLifecycleChanged {
        change: ProgrammingLifecycleChange,
    },
    PlaybackRuntimeChanged {
        change: PlaybackRuntimeChange,
    },
    PlaybackViewChanged {
        projection: PlaybackDeskProjection,
    },
    MacroExecutionChanged {
        execution: MacroExecutionSnapshot,
    },
    TimecodeRuntimeChanged {
        snapshot: TimecodeTransportSnapshot,
    },
    PlaybackTelemetrySampled {
        tick: PlaybackTelemetryTick,
    },
    OutputRuntimeChanged {
        change: OutputRuntimeChange,
    },
    DynamicRuntimeChanged {
        change: DynamicRuntimeChange,
    },
    SpeedGroupsChanged {
        change: SpeedGroupChange,
    },
    ShowPatchChanged {
        delta: super::patch::PatchDelta,
    },
    OutputRouteChanged {
        change: OutputRouteChange,
    },
    ShowObjectsChanged {
        change: ShowObjectsChange,
    },
    ScheduleRuntimeChanged {
        change: ScheduleRuntimeChange,
    },
    SelectiveImportApplied {
        change: Box<SelectiveImportChange>,
    },
    VirtualPlaybackExclusionZonesChanged {
        change: VirtualPlaybackExclusionZonesChange,
    },
    HighlightChanged {
        change: HighlightChange,
    },
    ServerConfigurationChanged {
        change: NotificationRevision,
    },
    ScreensChanged {
        change: ScreenNotification,
    },
    ShowLibraryChanged {
        change: ShowLibraryNotification,
    },
    FixtureLibraryChanged {
        change: FixtureLibraryNotification,
    },
    MediaChanged {
        change: MediaNotification,
    },
    HardwareConnectionChanged {
        change: HardwareConnectionNotification,
    },
    VisualizerConnectionChanged {
        change: VisualizerConnectionNotification,
    },
    OperatorNotification {
        notification: OperatorNotification,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRuntimeEventKind {
    InstanceStarted,
    InstancePending,
    InstanceActive,
    InstanceOff,
    InstanceRelease,
    ControllerUpdated,
    ControllerWinnerChanged,
    Paused,
    Resumed,
    FailedDependency,
    PreloadCommitted,
    TransitionCompleted,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicRuntimeChange {
    pub kind: DynamicRuntimeEventKind,
    pub dynamic_id: Option<Uuid>,
    pub runtime_instance_id: Option<Uuid>,
    pub controller_id: Option<Uuid>,
    pub winning_controller_id: Option<Uuid>,
    #[ts(type = "number")]
    pub occurred_at_millis: u64,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct NotificationRevision {
    #[ts(type = "number")]
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct HardwareConnectionNotification {
    #[ts(type = "number")]
    pub revision: u64,
    pub connected: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizerConnectionNotification {
    pub connected: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ScreenNotificationKind {
    Configuration,
    ScreenPage,
    PlaybackPage,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScreenNotification {
    #[ts(type = "number")]
    pub revision: u64,
    pub kind: ScreenNotificationKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ShowLibraryNotificationKind {
    ShowOpened,
    ShowRenamed,
    ShowRolledBack,
    ShowUploaded,
    ShowDeleted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowLibraryNotification {
    #[ts(type = "number")]
    pub revision: u64,
    pub kind: ShowLibraryNotificationKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FixtureLibraryNotificationKind {
    Library,
    Profile,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureLibraryNotification {
    #[ts(type = "number")]
    pub revision: u64,
    pub kind: FixtureLibraryNotificationKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MediaNotificationKind {
    ThumbnailsRefreshed,
    PreviewRefreshed,
    ServerOffline,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MediaNotification {
    #[ts(type = "number")]
    pub revision: u64,
    pub kind: MediaNotificationKind,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct HighlightChange {
    #[ts(type = "number")]
    pub revision: u64,
    pub desk_id: Uuid,
    pub action: Option<String>,
    pub source: Option<String>,
    pub state: RuntimeHighlightState,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct DeskActionNotification {
    pub action: Option<String>,
    pub control: Option<String>,
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub desk_id: Option<String>,
    pub path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct FileInputNotification {
    pub action: String,
    pub instance_id: String,
    pub session_id: String,
    pub source_session_id: Option<String>,
    pub desk_id: Option<String>,
    pub operation: Option<String>,
    pub source: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct FileOperationItemNotification {
    pub source_root_id: String,
    pub source: String,
    pub destination_root_id: Option<String>,
    pub destination: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct FileOperationNotification {
    pub operation: String,
    pub items: Vec<FileOperationItemNotification>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct GroupConfigurationNotification {
    pub group_id: String,
    pub desk_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackConfigurationNotification {
    pub desk_id: String,
    pub addressing: String,
    pub page: Option<u8>,
    pub slot: Option<u8>,
    pub playback: Option<u16>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UpdateTargetFamilyNotification {
    Cue,
    Preset,
    Group,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct UpdateTargetNotification {
    pub family: UpdateTargetFamilyNotification,
    pub object_id: String,
    pub playback_number: Option<u16>,
    pub cue_id: Option<String>,
    pub cue_number: Option<String>,
    pub validate_active_context: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdateWorkflowNotification {
    Armed {
        desk_id: String,
        armed: bool,
    },
    TargetRequested {
        desk_id: String,
        target: UpdateTargetNotification,
    },
    TargetRejected {
        desk_id: String,
        error: Option<String>,
    },
    TargetsRequested {
        desk_id: String,
    },
    SettingsRequested {
        desk_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OperatorNotification {
    DeskAction {
        #[ts(type = "number")]
        revision: u64,
        notification: DeskActionNotification,
    },
    FileInput {
        #[ts(type = "number")]
        revision: u64,
        notification: FileInputNotification,
    },
    FileOperation {
        #[ts(type = "number")]
        revision: u64,
        notification: FileOperationNotification,
    },
    GroupConfiguration {
        #[ts(type = "number")]
        revision: u64,
        notification: GroupConfigurationNotification,
    },
    PlaybackConfiguration {
        #[ts(type = "number")]
        revision: u64,
        notification: PlaybackConfigurationNotification,
    },
    UpdateWorkflow {
        #[ts(type = "number")]
        revision: u64,
        notification: UpdateWorkflowNotification,
    },
    CommandHistoryChanged {
        #[ts(type = "number")]
        revision: u64,
        desk_id: String,
    },
}

#[cfg(test)]
mod capture_mode_tests {
    use super::*;
    use crate::v2::programming::{ProgrammingCaptureModeChange, ProgrammingCaptureModeProjection};

    #[test]
    fn capture_mode_event_has_the_committed_wire_shape() {
        let payload = EventPayload::ProgrammingCaptureModeChanged {
            change: ProgrammingCaptureModeChange {
                projection: ProgrammingCaptureModeProjection {
                    revision: 4,
                    blind: true,
                    preview: false,
                    preload_capture_programmer: true,
                },
            },
        };

        let expected = serde_json::json!({
            "type": "programming_capture_mode_changed",
            "change": {
                "projection": {
                    "revision": 4,
                    "blind": true,
                    "preview": false,
                    "preload_capture_programmer": true
                }
            }
        });

        assert_eq!(serde_json::to_value(&payload).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<EventPayload>(expected).unwrap(),
            payload
        );
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OutputRuntimeIdentity {
    GlobalMaster,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct OutputRuntimeScope {
    pub show_id: Uuid,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct OutputRuntimeProjection {
    pub scope: OutputRuntimeScope,
    pub identity: OutputRuntimeIdentity,
    #[ts(type = "number")]
    pub revision: u64,
    pub grand_master: f32,
    pub blackout: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct OutputRuntimeChange {
    pub projection: OutputRuntimeProjection,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct OutputRuntimeSnapshot {
    pub cursor: EventSnapshotCursor,
    pub projection: OutputRuntimeProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowObjectsChange {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub changes: Vec<ShowObjectChange>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShowObjectChange {
    AttributeConfiguration {
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown | null")]
        body: Option<serde_json::Value>,
        deleted: bool,
    },
    Dynamic {
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown | null")]
        body: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        validation_error: Option<String>,
        deleted: bool,
    },
    CueList {
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown | null")]
        body: Option<serde_json::Value>,
        deleted: bool,
    },
    Group {
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown | null")]
        body: Option<serde_json::Value>,
        deleted: bool,
    },
    Macro {
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown | null")]
        body: Option<serde_json::Value>,
        deleted: bool,
    },
    PatchLayer {
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown | null")]
        body: Option<serde_json::Value>,
        deleted: bool,
    },
    Playback {
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown | null")]
        body: Option<serde_json::Value>,
        deleted: bool,
    },
    PlaybackPage {
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown | null")]
        body: Option<serde_json::Value>,
        deleted: bool,
    },
    Preset {
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown | null")]
        body: Option<serde_json::Value>,
        deleted: bool,
    },
    Schedule {
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown | null")]
        body: Option<serde_json::Value>,
        deleted: bool,
    },
    StageLayout {
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown | null")]
        body: Option<serde_json::Value>,
        deleted: bool,
    },
    Timecode {
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown | null")]
        body: Option<serde_json::Value>,
        deleted: bool,
    },
    UserLayout {
        object_id: String,
        #[ts(type = "number")]
        object_revision: u64,
        #[ts(type = "unknown | null")]
        body: Option<serde_json::Value>,
        deleted: bool,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ShowObjectKind {
    AttributeConfiguration,
    CueList,
    Dynamic,
    Group,
    Macro,
    PatchLayer,
    Playback,
    PlaybackPage,
    Preset,
    Schedule,
    StageLayout,
    Timecode,
    UserLayout,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportChange {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub objects: Vec<SelectiveImportObjectChange>,
    pub profile_revisions: Vec<FixtureProfileIdentity>,
    pub managed_assets: Vec<ManagedAssetReference>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportObjectChange {
    pub kind: String,
    pub object_id: String,
    #[ts(type = "number")]
    pub object_revision: u64,
    #[ts(type = "unknown")]
    pub body: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureProfileIdentity {
    pub profile_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ManagedAssetReference {
    pub asset_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputRouteChange {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub route_id: String,
    #[ts(type = "number")]
    pub object_revision: u64,
    pub route: Option<OutputRoute>,
    pub deleted: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputRoute {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub target: Option<OutputRouteTarget>,
    pub protocol: OutputProtocol,
    pub logical_universe: u16,
    pub destination_universe: u16,
    pub delivery_mode: OutputDeliveryMode,
    pub destination: Option<String>,
    pub enabled: bool,
    pub minimum_slots: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OutputRouteTarget {
    Network,
    UsbEndpoint { endpoint_id: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OutputProtocol {
    ArtNet,
    Sacn,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OutputDeliveryMode {
    Broadcast,
    Multicast,
    Unicast,
}

#[cfg(test)]
mod show_object_tests {
    use super::ShowObjectChange;
    use serde_json::json;

    #[test]
    fn show_object_change_is_discriminated_by_supported_family() {
        let changes = [
            ShowObjectChange::CueList {
                object_id: "main".into(),
                object_revision: 1,
                body: Some(json!({"id":"main"})),
                deleted: false,
            },
            ShowObjectChange::Group {
                object_id: "1".into(),
                object_revision: 1,
                body: Some(json!({"id":"1"})),
                deleted: false,
            },
            ShowObjectChange::PatchLayer {
                object_id: "main".into(),
                object_revision: 1,
                body: Some(json!({"id":"main"})),
                deleted: false,
            },
            ShowObjectChange::Playback {
                object_id: "1".into(),
                object_revision: 1,
                body: Some(json!({"number":1})),
                deleted: false,
            },
            ShowObjectChange::PlaybackPage {
                object_id: "1".into(),
                object_revision: 1,
                body: Some(json!({"number":1})),
                deleted: false,
            },
            ShowObjectChange::Preset {
                object_id: "1.1".into(),
                object_revision: 1,
                body: Some(json!({"number":1})),
                deleted: false,
            },
            ShowObjectChange::StageLayout {
                object_id: "main".into(),
                object_revision: 1,
                body: Some(json!({"positions":{}})),
                deleted: false,
            },
            ShowObjectChange::UserLayout {
                object_id: "main".into(),
                object_revision: 1,
                body: Some(json!({"desks":[]})),
                deleted: false,
            },
        ];

        let kinds = [
            "cue_list",
            "group",
            "patch_layer",
            "playback",
            "playback_page",
            "preset",
            "stage_layout",
            "user_layout",
        ];
        for (change, kind) in changes.into_iter().zip(kinds) {
            let encoded = serde_json::to_value(change).unwrap();
            assert_eq!(encoded["kind"], kind);
            assert!(serde_json::from_value::<ShowObjectChange>(encoded).is_ok());
        }
        assert!(
            serde_json::from_value::<ShowObjectChange>(
                json!({"kind":"fixture","object_id":"1","object_revision":1,"body":{},"deleted":false})
            )
            .is_err()
        );
    }
}
