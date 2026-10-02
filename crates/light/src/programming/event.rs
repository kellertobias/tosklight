use super::{
    ProgrammingCaptureModeChange, ProgrammingInteractionProjection, ProgrammingLifecycleChange,
    ProgrammingPreloadPlaybackQueueChange, ProgrammingPreloadValuesChange,
    ProgrammingPriorityChange, ProgrammingValuesChange,
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
    alignment: Option<light_programmer::ProgrammerAlignmentProjection>,
}

impl ProgrammingInteractionChange {
    pub fn from_components(
        desk_id: Uuid,
        command_line: Option<CommandLineState>,
        selection: Option<ProgrammerSelection>,
    ) -> Option<Self> {
        Self::with_alignment(desk_id, command_line, selection, None)
    }

    pub fn with_alignment(
        desk_id: Uuid,
        command_line: Option<CommandLineState>,
        selection: Option<ProgrammerSelection>,
        alignment: Option<light_programmer::ProgrammerAlignmentProjection>,
    ) -> Option<Self> {
        (command_line.is_some() || selection.is_some() || alignment.is_some()).then_some(Self {
            desk_id,
            command_line,
            selection,
            alignment,
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
        let alignment = (before.alignment != after.alignment).then(|| after.alignment.clone());
        Self::with_alignment(after.desk_id, command_line, selection, alignment)
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
        Self::with_alignment(self.desk_id, self.command_line, None, self.alignment)
    }

    pub fn alignment(&self) -> Option<&light_programmer::ProgrammerAlignmentProjection> {
        self.alignment.as_ref()
    }
}

impl EventObject {
    pub fn programming_alignment(desk_id: Uuid) -> Self {
        Self::new(
            EventCapability::Desk,
            format!("programming-alignment:{desk_id}"),
        )
    }
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

    pub fn programming_values() -> Self {
        Self::new(EventCapability::Programmer, "programming-values")
    }

    pub fn programming_priority() -> Self {
        Self::new(EventCapability::Programmer, "programming-priority")
    }

    pub fn programming_capture_mode() -> Self {
        Self::new(EventCapability::Programmer, "programming-capture-mode")
    }

    pub fn programming_preload_values() -> Self {
        Self::new(EventCapability::Programmer, "programming-preload-values")
    }

    pub fn programming_preload_playback_queue() -> Self {
        Self::new(
            EventCapability::Programmer,
            "programming-preload-playback-queue",
        )
    }

    pub fn programming_lifecycle() -> Self {
        Self::new(EventCapability::Programmer, "programming-lifecycle")
    }
}

impl EventDraft {
    pub fn programming_priority_changed(
        context: &ActionContext,
        change: ProgrammingPriorityChange,
    ) -> Self {
        let object = EventObject::programming_priority();
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(object),
            related_objects: Vec::new(),
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Replaceable,
            payload: ApplicationEvent::Programming(ProgrammingEvent::PriorityChanged(change)),
        }
    }

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
        let object = EventObject::programming_values();
        Self {
            desk_id: None,
            class: EventClass::Projection,
            object: Some(object),
            related_objects: Vec::new(),
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            // Address-level deltas are ordered and cannot supersede one another. A bounded
            // subscriber repairs any detected gap from the authoritative full snapshot.
            delivery: DeliveryPolicy::Lossless,
            payload: ApplicationEvent::Programming(ProgrammingEvent::ValuesChanged(change)),
        }
    }

    pub fn programming_capture_mode_changed(
        context: &ActionContext,
        change: ProgrammingCaptureModeChange,
    ) -> Self {
        let object = EventObject::programming_capture_mode();
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
        let object = EventObject::programming_preload_values();
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
        let object = EventObject::programming_preload_playback_queue();
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
    let mut routes = Vec::with_capacity(3);
    if change.command_line.is_some() {
        routes.push(EventObject::programming_command_line(change.desk_id));
    }
    if change.selection.is_some() {
        routes.push(EventObject::programming_selection(change.desk_id));
    }
    if change.alignment.is_some() {
        routes.push(EventObject::programming_alignment(change.desk_id));
    }
    let first = routes.remove(0);
    (first, routes)
}
