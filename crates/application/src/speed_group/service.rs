use std::sync::Arc;

use parking_lot::Mutex;
use uuid::Uuid;

use crate::{ActionContext, ActionEnvelope, ActionError, EventBus, EventDraft};

use super::{
    SpeedGroupApplication, SpeedGroupAuthorityProjection, SpeedGroupChange, SpeedGroupCommand,
    SpeedGroupDurability, SpeedGroupOutcome, SpeedGroupPorts, SpeedGroupProjection,
    SpeedGroupResult, SpeedGroupSnapshot,
    planning::{plan, select_groups, validate_applied, validated_state},
    replay::AuthorityState,
};

#[derive(Clone)]
pub struct SpeedGroupService {
    authority: Arc<Mutex<AuthorityState>>,
    events: EventBus,
}

impl SpeedGroupService {
    pub fn new(events: EventBus) -> Self {
        Self::with_authority(events, Uuid::new_v4())
    }

    pub fn with_authority(events: EventBus, authority_id: Uuid) -> Self {
        Self {
            authority: Arc::new(Mutex::new(AuthorityState::new(authority_id))),
            events,
        }
    }

    pub const fn events(&self) -> &EventBus {
        &self.events
    }

    pub fn handle(
        &self,
        envelope: ActionEnvelope<SpeedGroupCommand>,
        ports: &dyn SpeedGroupPorts,
    ) -> Result<SpeedGroupResult, ActionError> {
        let mut authority = self.authority.lock();
        ports.authorize(&envelope.context)?;
        if let Some(result) = authority.cached(&envelope)? {
            return Ok(result);
        }
        let before = validated_state(ports.state(&envelope.context)?)?;
        authority.validate_expectation(envelope.command.expectation)?;
        let applied_at_millis = ports.application_millis(&envelope.context)?;
        let plan = plan(envelope.command.action, &before, applied_at_millis)?;
        let result = if plan.changed.is_empty() {
            unchanged(
                &envelope.context,
                &authority,
                applied_at_millis,
                plan.response,
            )
        } else {
            let revision = authority.next_revision()?;
            let application = ports.apply(&envelope.context, plan.resolved)?;
            let after = validated_state(ports.state(&envelope.context)?)?;
            validate_applied(&plan.expected, &after)?;
            authority.revision = revision;
            changed(
                &self.events,
                &envelope.context,
                &authority,
                applied_at_millis,
                application,
                select_groups(&after.groups, &plan.changed),
            )
        };
        authority.remember(&envelope, result.clone());
        Ok(result)
    }

    pub fn snapshot(
        &self,
        context: &ActionContext,
        ports: &dyn SpeedGroupPorts,
    ) -> Result<SpeedGroupSnapshot, ActionError> {
        let authority = self.authority.lock();
        ports.authorize(context)?;
        let state = validated_state(ports.state(context)?)?;
        Ok(SpeedGroupSnapshot {
            event_sequence: self.events.latest_sequence(),
            projection: SpeedGroupAuthorityProjection {
                authority_id: authority.id,
                revision: authority.revision,
                groups: state.groups,
            },
        })
    }
}

fn unchanged(
    context: &ActionContext,
    authority: &AuthorityState,
    applied_at_millis: u64,
    groups: Vec<SpeedGroupProjection>,
) -> SpeedGroupResult {
    SpeedGroupResult {
        context: context.clone(),
        authority_id: authority.id,
        revision: authority.revision,
        applied_at_millis,
        outcome: SpeedGroupOutcome::NoChange,
        durability: SpeedGroupDurability::Durable,
        warning: None,
        groups,
        event_sequence: None,
        replayed: false,
    }
}

fn changed(
    events: &EventBus,
    context: &ActionContext,
    authority: &AuthorityState,
    applied_at_millis: u64,
    application: SpeedGroupApplication,
    groups: Vec<SpeedGroupProjection>,
) -> SpeedGroupResult {
    let change = SpeedGroupChange {
        authority_id: authority.id,
        revision: authority.revision,
        applied_at_millis,
        groups: groups.clone(),
    };
    let event = events.publish(EventDraft::speed_groups_changed(context, change));
    SpeedGroupResult {
        context: context.clone(),
        authority_id: authority.id,
        revision: authority.revision,
        applied_at_millis,
        outcome: SpeedGroupOutcome::Applied,
        durability: application.durability,
        warning: application.warning,
        groups,
        event_sequence: Some(event.sequence),
        replayed: false,
    }
}
