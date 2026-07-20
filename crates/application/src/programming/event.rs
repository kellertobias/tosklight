use super::{
    ProgrammingCaptureModeChange, ProgrammingInteractionProjection, ProgrammingLifecycleChange,
    ProgrammingPreloadPlaybackQueueChange, ProgrammingPreloadValuesChange, ProgrammingValuesChange,
};
use crate::{
    ActionContext, ApplicationEvent, DeliveryPolicy, EventCapability, EventClass, EventDraft,
    EventObject, EventSource, ProgrammingEvent,
};
use light_programmer::{CommandLineState, ProgrammerSelection};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingInteractionChange {
    desk_id: Uuid,
    command_line: Option<CommandLineState>,
    selection: Option<ProgrammerSelection>,
}

impl ProgrammingInteractionChange {
    pub fn from_components(
        desk_id: Uuid,
        command_line: Option<CommandLineState>,
        selection: Option<ProgrammerSelection>,
    ) -> Option<Self> {
        (command_line.is_some() || selection.is_some()).then_some(Self {
            desk_id,
            command_line,
            selection,
        })
    }

    pub fn between(
        before: &ProgrammingInteractionProjection,
        after: &ProgrammingInteractionProjection,
    ) -> Option<Self> {
        if before.desk_id != after.desk_id {
            return None;
        }
        let command_line =
            (before.command_line != after.command_line).then(|| after.command_line.clone());
        let selection = (before.selection != after.selection).then(|| after.selection.clone());
        Self::from_components(after.desk_id, command_line, selection)
    }

    pub const fn desk_id(&self) -> Uuid {
        self.desk_id
    }

    pub const fn command_line(&self) -> Option<&CommandLineState> {
        self.command_line.as_ref()
    }

    pub const fn selection(&self) -> Option<&ProgrammerSelection> {
        self.selection.as_ref()
    }

    pub(super) fn without_selection(self) -> Option<Self> {
        Self::from_components(self.desk_id, self.command_line, None)
    }
}

impl EventObject {
    pub fn programming_command_line(desk_id: Uuid) -> Self {
        Self::new(
            EventCapability::Desk,
            format!("programming-command-line:{desk_id}"),
        )
    }

    pub fn programming_selection(desk_id: Uuid) -> Self {
        Self::new(
            EventCapability::Desk,
            format!("programming-selection:{desk_id}"),
        )
    }

    pub fn programming_values(user_id: Uuid) -> Self {
        Self::new(
            EventCapability::Programmer,
            format!("programming-values:{user_id}"),
        )
    }

    pub fn programming_capture_mode(user_id: Uuid) -> Self {
        Self::new(
            EventCapability::Programmer,
            format!("programming-capture-mode:{user_id}"),
        )
    }

    pub fn programming_preload_values(user_id: Uuid) -> Self {
        Self::new(
            EventCapability::Programmer,
            format!("programming-preload-values:{user_id}"),
        )
    }

    pub fn programming_preload_playback_queue(user_id: Uuid) -> Self {
        Self::new(
            EventCapability::Programmer,
            format!("programming-preload-playback-queue:{user_id}"),
        )
    }

    pub fn programming_lifecycle() -> Self {
        Self::new(EventCapability::Programmer, "programming-lifecycle")
    }

    pub fn programming_values_user_id(&self) -> Option<Uuid> {
        (self.capability == EventCapability::Programmer)
            .then(|| self.id.strip_prefix("programming-values:"))
            .flatten()
            .and_then(|value| Uuid::parse_str(value).ok())
    }

    pub fn programming_capture_mode_user_id(&self) -> Option<Uuid> {
        (self.capability == EventCapability::Programmer)
            .then(|| self.id.strip_prefix("programming-capture-mode:"))
            .flatten()
            .and_then(|value| Uuid::parse_str(value).ok())
    }

    pub fn programming_preload_values_user_id(&self) -> Option<Uuid> {
        (self.capability == EventCapability::Programmer)
            .then(|| self.id.strip_prefix("programming-preload-values:"))
            .flatten()
            .and_then(|value| Uuid::parse_str(value).ok())
    }

    pub fn programming_user_id(&self) -> Option<Uuid> {
        self.programming_values_user_id()
            .or_else(|| self.programming_preload_values_user_id())
            .or_else(|| self.programming_preload_playback_queue_user_id())
            .or_else(|| self.programming_capture_mode_user_id())
    }

    pub fn programming_preload_playback_queue_user_id(&self) -> Option<Uuid> {
        (self.capability == EventCapability::Programmer)
            .then(|| self.id.strip_prefix("programming-preload-playback-queue:"))
            .flatten()
            .and_then(|value| Uuid::parse_str(value).ok())
    }
}

impl EventDraft {
    pub fn programming_lifecycle_changed(
        change: ProgrammingLifecycleChange,
        source: EventSource,
        correlation_id: Option<Uuid>,
    ) -> Self {
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(EventObject::programming_lifecycle()),
            related_objects: Vec::new(),
            source,
            correlation_id,
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Programming(ProgrammingEvent::LifecycleChanged(change)),
        }
    }

    pub fn programming_interaction_changed(
        context: &ActionContext,
        change: ProgrammingInteractionChange,
    ) -> Self {
        let (object, related_objects) = interaction_routes(&change);
        Self {
            desk_id: Some(change.desk_id),
            class: EventClass::Projection,
            object: Some(object),
            related_objects,
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            // Sparse component changes cannot be safely coalesced independently: replacing a
            // combined command-line + selection change with a command-only change would lose the
            // selection transition. Bounded subscribers repair overload through the snapshot.
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Programming(ProgrammingEvent::InteractionChanged(change)),
        }
    }

    pub fn programming_values_changed(
        context: &ActionContext,
        change: ProgrammingValuesChange,
    ) -> Self {
        let object = EventObject::programming_values(change.projection.user_id.0);
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(object),
            related_objects: Vec::new(),
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Replaceable,
            payload: ApplicationEvent::Programming(ProgrammingEvent::ValuesChanged(change)),
        }
    }

    pub fn programming_capture_mode_changed(
        context: &ActionContext,
        change: ProgrammingCaptureModeChange,
    ) -> Self {
        let object = EventObject::programming_capture_mode(change.projection.user_id.0);
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(object),
            related_objects: Vec::new(),
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Replaceable,
            payload: ApplicationEvent::Programming(ProgrammingEvent::CaptureModeChanged(change)),
        }
    }

    pub fn programming_preload_values_changed(
        context: &ActionContext,
        change: ProgrammingPreloadValuesChange,
    ) -> Self {
        let object = EventObject::programming_preload_values(change.projection.user_id.0);
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(object),
            related_objects: Vec::new(),
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Replaceable,
            payload: ApplicationEvent::Programming(ProgrammingEvent::PreloadValuesChanged(change)),
        }
    }

    pub fn programming_preload_playback_queue_changed(
        context: &ActionContext,
        change: ProgrammingPreloadPlaybackQueueChange,
    ) -> Self {
        let object = EventObject::programming_preload_playback_queue(change.projection.user_id.0);
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(object),
            related_objects: Vec::new(),
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Replaceable,
            payload: ApplicationEvent::Programming(ProgrammingEvent::PreloadPlaybackQueueChanged(
                change,
            )),
        }
    }
}

fn interaction_routes(change: &ProgrammingInteractionChange) -> (EventObject, Vec<EventObject>) {
    let command_line = EventObject::programming_command_line(change.desk_id);
    let selection = EventObject::programming_selection(change.desk_id);
    match (change.command_line.is_some(), change.selection.is_some()) {
        (true, true) => (command_line, vec![selection]),
        (true, false) => (command_line, Vec::new()),
        (false, true) => (selection, Vec::new()),
        (false, false) => unreachable!("Programming changes always contain a changed projection"),
    }
}
