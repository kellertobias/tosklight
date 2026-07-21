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
    RuntimeChanged(PlaybackRuntimeChange),
    SpeedGroupsChanged(SpeedGroupChange),
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
}

#[derive(Clone, Debug, PartialEq)]
pub enum OutputEvent {
    RuntimeChanged(OutputRuntimeChange),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ShowEvent {
    PatchChanged(PatchChange),
    OutputRouteChanged(OutputRouteChange),
    ObjectsChanged(ActiveShowObjectsChange),
    SelectiveImportApplied(SelectiveShowImportChange),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ApplicationEvent {
    Programming(ProgrammingEvent),
    Playback(PlaybackEvent),
    Desk(DeskEvent),
    Output(OutputEvent),
    Show(ShowEvent),
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
            payload: ApplicationEvent::Playback(PlaybackEvent::RuntimeChanged(change)),
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
}

fn playback_routes(change: &PlaybackRuntimeChange) -> (Option<EventObject>, Vec<EventObject>) {
    let mut routes = Vec::with_capacity(3);
    if let Some(number) = change.projection.playback_number {
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
