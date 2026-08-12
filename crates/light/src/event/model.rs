use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    ActionContext, ActionSource, ActiveShowObjectKind, ActiveShowObjectsChange, OutputRouteChange,
    OutputRuntimeChange, PatchChange, SelectiveShowImportChange, SpeedGroupChange,
    playback::{PlaybackDeskProjection, PlaybackRuntimeChange, PlaybackRuntimeIdentity},
    programming::{
        ProgrammingCaptureModeChange, ProgrammingInteractionChange, ProgrammingLifecycleChange,
        ProgrammingPreloadPlaybackQueueChange, ProgrammingPreloadValuesChange,
        ProgrammingPriorityChange, ProgrammingValuesChange,
    },
};
use light_core::ShowId;
use light_programmer::HighlightState;

use super::routing::{active_show_routes, selective_import_routes};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EventSource {
    Action(ActionSource),
    Runtime,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EventClass {
    Transition,
    Projection,
    CommandOutcome,
    Error,
    Safety,
    Telemetry,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EventCapability {
    Programmer,
    Playback,
    Show,
    Desk,
    Output,
    System,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EventObject {
    pub capability: EventCapability,
    pub id: String,
}

impl EventObject {
    pub fn new(capability: EventCapability, id: impl Into<String>) -> Self {
        Self {
            capability,
            id: id.into(),
        }
    }

    pub fn show_objects(show_id: ShowId) -> Self {
        Self::new(EventCapability::Show, format!("objects:{}", show_id.0))
    }

    pub fn show_object_kind(show_id: ShowId, kind: ActiveShowObjectKind) -> Self {
        Self::show_storage_object_kind(show_id, kind.as_str())
    }

    pub fn show_storage_object_kind(show_id: ShowId, kind: &str) -> Self {
        Self::new(
            EventCapability::Show,
            format!("objects:{}:kind:{kind}", show_id.0),
        )
    }

    pub fn show_object(show_id: ShowId, kind: ActiveShowObjectKind, object_id: &str) -> Self {
        Self::show_storage_object(show_id, kind.as_str(), object_id)
    }

    pub fn show_storage_object(show_id: ShowId, kind: &str, object_id: &str) -> Self {
        Self::new(
            EventCapability::Show,
            format!("objects:{}:kind:{kind}:object:{object_id}", show_id.0),
        )
    }

    pub fn playback(number: u16) -> Self {
        Self::new(EventCapability::Playback, format!("playback:{number}"))
    }

    pub fn virtual_playback(address: light_playback::VirtualPlaybackAddress) -> Self {
        Self::new(
            EventCapability::Playback,
            format!(
                "virtual-playback:{}.{}",
                address.page(),
                address.number().get()
            ),
        )
    }

    /// The one shared route for sampled playback telemetry ticks.
    pub fn playback_telemetry() -> Self {
        Self::new(EventCapability::Playback, "telemetry")
    }

    pub fn cue_list(cue_list_id: Uuid) -> Self {
        Self::new(EventCapability::Playback, format!("cuelist:{cue_list_id}"))
    }

    pub fn group(group_id: &str) -> Self {
        Self::new(EventCapability::Playback, format!("group:{group_id}"))
    }

    pub fn playback_view(desk_id: Uuid) -> Self {
        Self::new(EventCapability::Desk, format!("playback-view:{desk_id}"))
    }

    pub fn global_output() -> Self {
        Self::new(EventCapability::Output, "runtime:global-master")
    }

    pub fn speed_groups() -> Self {
        Self::new(EventCapability::Playback, "speed-groups:manual")
    }

    pub fn dynamic_runtime() -> Self {
        Self::new(EventCapability::Output, "dynamics:runtime")
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeliveryPolicy {
    /// A queue overflow becomes an explicit sequence gap requiring snapshot repair.
    Lossless,
    /// An older queued event for the same object and class may be replaced by the newest value.
    Replaceable,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlaybackEvent {
    RuntimeChanged(Box<PlaybackRuntimeChange>),
    SpeedGroupsChanged(SpeedGroupChange),
    TelemetrySampled(crate::PlaybackTelemetryTick),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingEvent {
    InteractionChanged(ProgrammingInteractionChange),
    PriorityChanged(ProgrammingPriorityChange),
    LifecycleChanged(ProgrammingLifecycleChange),
    CaptureModeChanged(ProgrammingCaptureModeChange),
    ValuesChanged(ProgrammingValuesChange),
    PreloadValuesChanged(ProgrammingPreloadValuesChange),
    PreloadPlaybackQueueChanged(ProgrammingPreloadPlaybackQueueChange),
}

#[derive(Clone, Debug, PartialEq)]
pub enum DeskEvent {
    PlaybackViewChanged(PlaybackDeskProjection),
    ConfigurationChanged(NotificationRevision),
    ScreensChanged(ScreenNotification),
    HardwareConnectionChanged(HardwareConnectionNotification),
    VisualizerConnectionChanged(VisualizerConnectionNotification),
    MacroExecutionChanged(crate::CommandMacroExecutionSnapshot),
    TimecodeRuntimeChanged(crate::timeline::TimecodeRuntimeChange),
}

#[derive(Clone, Debug, PartialEq)]
pub enum OutputEvent {
    RuntimeChanged(OutputRuntimeChange),
    DynamicRuntimeChanged(DynamicRuntimeChange),
    HighlightChanged(HighlightChange),
    MediaChanged(MediaNotification),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicRuntimeChange {
    pub kind: DynamicRuntimeEventKind,
    pub dynamic_id: Option<Uuid>,
    pub runtime_instance_id: Option<Uuid>,
    pub controller_id: Option<Uuid>,
    pub winning_controller_id: Option<Uuid>,
    pub occurred_at_millis: u64,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HighlightChange {
    pub revision: u64,
    pub desk_id: Uuid,
    pub user_id: Uuid,
    pub action: Option<String>,
    pub source: Option<String>,
    pub state: HighlightState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ShowEvent {
    PatchChanged(PatchChange),
    OutputRouteChanged(OutputRouteChange),
    ObjectsChanged(ActiveShowObjectsChange),
    SelectiveImportApplied(SelectiveShowImportChange),
    VirtualPlaybackExclusionZonesChanged(VirtualPlaybackExclusionZonesChange),
    ShowLibraryChanged(ShowLibraryNotification),
    FixtureLibraryChanged(FixtureLibraryNotification),
    ScheduleRuntimeChanged(crate::ScheduleRuntimeChange),
}

#[derive(Clone, Debug, PartialEq)]
pub enum SystemEvent {
    Operator(OperatorNotification),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NotificationRevision {
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HardwareConnectionNotification {
    pub revision: u64,
    pub connected: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisualizerConnectionNotification {
    pub connected: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ScreenNotificationKind {
    Configuration,
    ScreenPage,
    PlaybackPage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScreenNotification {
    pub revision: u64,
    pub kind: ScreenNotificationKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ShowLibraryNotificationKind {
    ShowOpened,
    ShowRenamed,
    ShowRolledBack,
    ShowUploaded,
    ShowDeleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShowLibraryNotification {
    pub revision: u64,
    pub kind: ShowLibraryNotificationKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FixtureLibraryNotificationKind {
    Library,
    Profile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixtureLibraryNotification {
    pub revision: u64,
    pub kind: FixtureLibraryNotificationKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MediaNotificationKind {
    ThumbnailsRefreshed,
    PreviewRefreshed,
    ServerOffline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaNotification {
    pub revision: u64,
    pub kind: MediaNotificationKind,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct DeskActionNotification {
    pub action: Option<String>,
    pub control: Option<String>,
    pub value: Option<String>,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub desk_id: Option<String>,
    pub desk_alias: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct FileInputNotification {
    pub action: String,
    pub instance_id: String,
    pub session_id: String,
    pub source_session_id: Option<String>,
    pub desk_id: Option<String>,
    pub operation: Option<String>,
    pub source: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct FileOperationItemNotification {
    pub source_root_id: String,
    pub source: String,
    pub destination_root_id: Option<String>,
    pub destination: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct FileOperationNotification {
    pub operation: String,
    pub items: Vec<FileOperationItemNotification>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
pub struct GroupConfigurationNotification {
    pub group_id: String,
    pub desk_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateTargetFamilyNotification {
    Cue,
    Preset,
    Group,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct UpdateTargetNotification {
    pub family: UpdateTargetFamilyNotification,
    pub object_id: String,
    pub playback_number: Option<u16>,
    pub cue_id: Option<String>,
    pub cue_number: Option<f64>,
    pub validate_active_context: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
pub enum OperatorNotification {
    DeskAction {
        revision: u64,
        notification: DeskActionNotification,
    },
    FileInput {
        revision: u64,
        notification: FileInputNotification,
    },
    FileOperation {
        revision: u64,
        notification: FileOperationNotification,
    },
    GroupConfiguration {
        revision: u64,
        notification: GroupConfigurationNotification,
    },
    UpdateWorkflow {
        revision: u64,
        notification: UpdateWorkflowNotification,
    },
    CommandHistoryChanged {
        revision: u64,
        desk_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtualPlaybackExclusionZonesChange {
    pub show_id: ShowId,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ApplicationEvent {
    Programming(ProgrammingEvent),
    Playback(PlaybackEvent),
    Desk(DeskEvent),
    Output(OutputEvent),
    Show(ShowEvent),
    System(SystemEvent),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventDraft {
    /// `None` denotes an installation-global transition observed by every desk.
    pub desk_id: Option<Uuid>,
    pub class: EventClass,
    pub object: Option<EventObject>,
    /// Additional identities by which the same semantic event may be routed.
    pub related_objects: Vec<EventObject>,
    pub source: EventSource,
    pub correlation_id: Option<Uuid>,
    pub delivery: DeliveryPolicy,
    pub payload: ApplicationEvent,
}

impl EventDraft {
    pub fn macro_execution_changed(change: crate::CommandMacroExecutionSnapshot) -> Self {
        Self {
            desk_id: Some(change.desk_id),
            class: if change.state.is_terminal() {
                EventClass::CommandOutcome
            } else {
                EventClass::Transition
            },
            object: Some(EventObject::new(
                EventCapability::Desk,
                format!("macro-runtime:{}", change.desk_id),
            )),
            related_objects: vec![EventObject::new(
                EventCapability::Show,
                format!("macro:{}", change.macro_id),
            )],
            source: EventSource::Runtime,
            correlation_id: None,
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Desk(DeskEvent::MacroExecutionChanged(change)),
        }
    }

    pub fn timecode_runtime_changed(change: crate::timeline::TimecodeRuntimeChange) -> Self {
        Self {
            desk_id: None,
            class: match change.cause {
                crate::timeline::TimecodeRuntimeChangeCause::Tick { .. }
                | crate::timeline::TimecodeRuntimeChangeCause::ExternalSync { .. } => {
                    EventClass::Projection
                }
                crate::timeline::TimecodeRuntimeChangeCause::Installed
                | crate::timeline::TimecodeRuntimeChangeCause::Action(_) => EventClass::Transition,
            },
            object: Some(EventObject::new(EventCapability::Desk, "timecode-runtime")),
            related_objects: vec![EventObject::new(
                EventCapability::Show,
                format!("timecode:{}", change.snapshot.timecode_id.0),
            )],
            source: EventSource::Runtime,
            correlation_id: None,
            delivery: if matches!(
                change.cause,
                crate::timeline::TimecodeRuntimeChangeCause::Tick { .. }
                    | crate::timeline::TimecodeRuntimeChangeCause::ExternalSync { .. }
            ) {
                DeliveryPolicy::Replaceable
            } else {
                DeliveryPolicy::Lossless
            },
            payload: ApplicationEvent::Desk(DeskEvent::TimecodeRuntimeChanged(change)),
        }
    }

    pub fn system_event(event: SystemEvent) -> Self {
        Self::runtime_projection(
            EventCapability::System,
            "operator",
            ApplicationEvent::System(event),
        )
    }

    pub fn configuration_changed(change: NotificationRevision) -> Self {
        Self::runtime_projection(
            EventCapability::Desk,
            "configuration",
            ApplicationEvent::Desk(DeskEvent::ConfigurationChanged(change)),
        )
    }

    pub fn screens_changed(change: ScreenNotification) -> Self {
        Self::runtime_projection(
            EventCapability::Desk,
            "screens",
            ApplicationEvent::Desk(DeskEvent::ScreensChanged(change)),
        )
    }

    pub fn hardware_connection_changed(change: HardwareConnectionNotification) -> Self {
        Self::runtime_projection(
            EventCapability::Desk,
            "hardware-connections",
            ApplicationEvent::Desk(DeskEvent::HardwareConnectionChanged(change)),
        )
    }

    pub fn visualizer_connection_changed(change: VisualizerConnectionNotification) -> Self {
        Self::runtime_projection(
            EventCapability::Desk,
            "visualizer-connections",
            ApplicationEvent::Desk(DeskEvent::VisualizerConnectionChanged(change)),
        )
    }

    pub fn show_library_changed(change: ShowLibraryNotification) -> Self {
        Self::runtime_projection(
            EventCapability::Show,
            "show-library",
            ApplicationEvent::Show(ShowEvent::ShowLibraryChanged(change)),
        )
    }

    pub fn fixture_library_changed(change: FixtureLibraryNotification) -> Self {
        Self::runtime_projection(
            EventCapability::Show,
            "fixture-library",
            ApplicationEvent::Show(ShowEvent::FixtureLibraryChanged(change)),
        )
    }

    pub fn media_changed(change: MediaNotification) -> Self {
        Self::runtime_projection(
            EventCapability::Output,
            "media",
            ApplicationEvent::Output(OutputEvent::MediaChanged(change)),
        )
    }

    fn runtime_projection(
        capability: EventCapability,
        object_id: &'static str,
        payload: ApplicationEvent,
    ) -> Self {
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(EventObject::new(capability, object_id)),
            related_objects: Vec::new(),
            source: EventSource::Runtime,
            correlation_id: None,
            delivery: DeliveryPolicy::Lossless,
            payload,
        }
    }

    pub fn highlight_changed(context: &ActionContext, change: HighlightChange) -> Self {
        Self {
            desk_id: Some(change.desk_id),
            class: EventClass::Projection,
            object: Some(EventObject::new(EventCapability::Output, "highlight")),
            related_objects: Vec::new(),
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Output(OutputEvent::HighlightChanged(change)),
        }
    }

    pub fn virtual_playback_exclusion_zones_changed(
        change: VirtualPlaybackExclusionZonesChange,
    ) -> Self {
        let object = EventObject::new(
            EventCapability::Show,
            format!("virtual-playback-exclusion-zones:{}", change.show_id.0),
        );
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(object),
            related_objects: Vec::new(),
            source: EventSource::Action(ActionSource::Http),
            correlation_id: None,
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Show(ShowEvent::VirtualPlaybackExclusionZonesChanged(
                change,
            )),
        }
    }

    /// Volatile sampled playback telemetry: replaceable, telemetry-class, one shared route.
    pub fn playback_telemetry_sampled(tick: crate::PlaybackTelemetryTick) -> Self {
        Self {
            desk_id: None,
            class: EventClass::Telemetry,
            object: Some(EventObject::playback_telemetry()),
            related_objects: Vec::new(),
            source: EventSource::Runtime,
            correlation_id: None,
            delivery: DeliveryPolicy::Replaceable,
            payload: ApplicationEvent::Playback(PlaybackEvent::TelemetrySampled(tick)),
        }
    }

    pub fn dynamic_runtime_changed(
        context: Option<&ActionContext>,
        change: DynamicRuntimeChange,
    ) -> Self {
        Self {
            desk_id: context.map(|context| context.desk_id),
            class: if change.kind == DynamicRuntimeEventKind::FailedDependency {
                EventClass::Error
            } else {
                EventClass::Transition
            },
            object: Some(EventObject::dynamic_runtime()),
            related_objects: Vec::new(),
            source: context.map_or(EventSource::Runtime, |context| {
                EventSource::Action(context.source)
            }),
            correlation_id: context.map(|context| context.correlation_id),
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Output(OutputEvent::DynamicRuntimeChanged(change)),
        }
    }

    pub fn playback_runtime_changed(
        desk_id: Option<Uuid>,
        change: PlaybackRuntimeChange,
        source: EventSource,
        correlation_id: Option<Uuid>,
    ) -> Self {
        let (object, related_objects) = playback_routes(&change);
        let transition = change.transition.is_some();
        Self {
            desk_id,
            class: if transition {
                EventClass::Transition
            } else {
                EventClass::Projection
            },
            object,
            related_objects,
            source,
            correlation_id,
            delivery: if transition {
                DeliveryPolicy::Lossless
            } else {
                DeliveryPolicy::Replaceable
            },
            payload: ApplicationEvent::Playback(PlaybackEvent::RuntimeChanged(Box::new(change))),
        }
    }

    pub fn playback_view_changed(
        context: &ActionContext,
        projection: PlaybackDeskProjection,
    ) -> Self {
        Self {
            desk_id: Some(projection.desk_id),
            class: EventClass::Projection,
            object: Some(EventObject::playback_view(projection.desk_id)),
            related_objects: Vec::new(),
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Replaceable,
            payload: ApplicationEvent::Desk(DeskEvent::PlaybackViewChanged(projection)),
        }
    }

    pub fn output_runtime_changed(context: &ActionContext, change: OutputRuntimeChange) -> Self {
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(EventObject::global_output()),
            related_objects: Vec::new(),
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Replaceable,
            payload: ApplicationEvent::Output(OutputEvent::RuntimeChanged(change)),
        }
    }

    pub fn speed_groups_changed(context: &ActionContext, change: SpeedGroupChange) -> Self {
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(EventObject::speed_groups()),
            related_objects: Vec::new(),
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Playback(PlaybackEvent::SpeedGroupsChanged(change)),
        }
    }

    pub fn patch_changed(context: &ActionContext, change: PatchChange) -> Self {
        let object = EventObject::new(EventCapability::Show, format!("patch:{}", change.show_id.0));
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(object),
            related_objects: Vec::new(),
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Show(ShowEvent::PatchChanged(change)),
        }
    }

    pub fn output_route_changed(context: &ActionContext, change: OutputRouteChange) -> Self {
        let object = EventObject::new(
            EventCapability::Output,
            format!("route:{}:{}", change.show_id.0, change.route_id),
        );
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(object),
            related_objects: Vec::new(),
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Show(ShowEvent::OutputRouteChanged(change)),
        }
    }

    pub fn active_show_objects_changed(
        context: &ActionContext,
        change: ActiveShowObjectsChange,
    ) -> Self {
        let object = EventObject::show_objects(change.show_id);
        let related_objects = active_show_routes(&change);
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(object),
            related_objects,
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Show(ShowEvent::ObjectsChanged(change)),
        }
    }

    pub fn selective_import_applied(
        context: &ActionContext,
        change: SelectiveShowImportChange,
    ) -> Self {
        let object = EventObject::show_objects(change.show_id);
        let related_objects = selective_import_routes(&change);
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(object),
            related_objects,
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Show(ShowEvent::SelectiveImportApplied(change)),
        }
    }

    pub fn schedule_runtime_changed(change: crate::ScheduleRuntimeChange) -> Self {
        let object = EventObject::show_object(
            change.show_id,
            ActiveShowObjectKind::Schedule,
            &change.schedule_id.to_string(),
        );
        Self {
            desk_id: None,
            class: if change.validation_error.is_some()
                || change
                    .last_result
                    .as_ref()
                    .is_some_and(|result| result.status == crate::ScheduleOccurrenceStatus::Failed)
            {
                EventClass::Error
            } else {
                EventClass::Projection
            },
            object: Some(object),
            related_objects: vec![EventObject::show_object_kind(
                change.show_id,
                ActiveShowObjectKind::Schedule,
            )],
            source: EventSource::Runtime,
            correlation_id: None,
            delivery: DeliveryPolicy::Replaceable,
            payload: ApplicationEvent::Show(ShowEvent::ScheduleRuntimeChanged(change)),
        }
    }
}

fn playback_routes(change: &PlaybackRuntimeChange) -> (Option<EventObject>, Vec<EventObject>) {
    let mut routes = Vec::with_capacity(3);
    if let PlaybackRuntimeIdentity::Virtual(address) = change.projection.requested {
        routes.push(EventObject::virtual_playback(address));
    } else if let Some(number) = change.projection.playback_number {
        routes.push(EventObject::playback(number));
    }
    if let Some(cue_list_id) = change.projection.cue_list_id() {
        routes.push(EventObject::cue_list(cue_list_id.0));
    }
    if let Some(group_id) = change.projection.group_id() {
        routes.push(EventObject::group(group_id));
    }
    if routes.is_empty() {
        routes.push(match &change.projection.requested {
            PlaybackRuntimeIdentity::Playback(number) => EventObject::playback(*number),
            PlaybackRuntimeIdentity::Virtual(address) => EventObject::virtual_playback(*address),
            PlaybackRuntimeIdentity::CueList(id) => EventObject::cue_list(id.0),
            PlaybackRuntimeIdentity::Group(id) => EventObject::group(id.as_str()),
        });
    }
    let object = Some(routes.remove(0));
    (object, routes)
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventEnvelope {
    pub sequence: u64,
    pub occurred_at: DateTime<Utc>,
    pub desk_id: Option<Uuid>,
    pub class: EventClass,
    pub object: Option<EventObject>,
    pub related_objects: Vec<EventObject>,
    pub source: EventSource,
    pub correlation_id: Option<Uuid>,
    pub delivery: DeliveryPolicy,
    pub payload: ApplicationEvent,
}
