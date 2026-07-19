use super::ProgrammingInteractionProjection;
use crate::{
    ActionContext, ApplicationEvent, DeliveryPolicy, EventCapability, EventClass, EventDraft,
    EventObject, EventSource, ProgrammingEvent,
};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingInteractionChange {
    pub projection: ProgrammingInteractionProjection,
}

impl EventObject {
    pub fn programming_interaction(desk_id: Uuid) -> Self {
        Self::new(
            EventCapability::Desk,
            format!("programming-interaction:{desk_id}"),
        )
    }
}

impl EventDraft {
    pub fn programming_interaction_changed(
        context: &ActionContext,
        projection: ProgrammingInteractionProjection,
    ) -> Self {
        Self {
            desk_id: Some(projection.desk_id),
            class: EventClass::Projection,
            object: Some(EventObject::programming_interaction(projection.desk_id)),
            related_objects: Vec::new(),
            source: EventSource::Action(context.source),
            correlation_id: Some(context.correlation_id),
            delivery: DeliveryPolicy::Replaceable,
            payload: ApplicationEvent::Programming(ProgrammingEvent::InteractionChanged(
                ProgrammingInteractionChange { projection },
            )),
        }
    }
}
