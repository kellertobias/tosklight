use super::{
    PlaybackAction, PlaybackAddress, PlaybackCommand, PlaybackExecution, PlaybackOperationResult,
    PlaybackOutcome, PlaybackPorts, PlaybackResult, PlaybackRuntimeIdentity,
    PlaybackRuntimeProjection, PlaybackRuntimeSnapshot, PlaybackUnitOfWork,
    ResolvedPlaybackAddress, committed_playback_effect_event,
};
use crate::{ActionContext, ActionEnvelope, ActionError, ActionErrorKind, EventBus, EventDraft};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::Arc;

use super::projection::validate_snapshot_identities;
use super::transition_set::RelatedTransitionSet;

#[derive(Clone)]
pub struct PlaybackService {
    operation: Arc<Mutex<()>>,
    events: EventBus,
}

impl Default for PlaybackService {
    fn default() -> Self {
        Self::new(EventBus::default())
    }
}

impl PlaybackService {
    pub fn new(events: EventBus) -> Self {
        Self {
            operation: Arc::default(),
            events,
        }
    }

    pub const fn events(&self) -> &EventBus {
        &self.events
    }

    pub fn handle(
        &self,
        envelope: ActionEnvelope<PlaybackCommand>,
        ports: &dyn PlaybackPorts,
    ) -> Result<PlaybackResult, ActionError> {
        let _ordered = self.operation.lock();
        ports.authorize(&envelope.context)?;
        self.apply(&envelope, ports)
    }

    // @tour playback-runtime:10 Order one Playback action
    // One application operation resolves the address, captures before-state, executes the domain
    // action, reads authoritative projection, and publishes typed runtime and desk events.
    fn apply(
        &self,
        envelope: &ActionEnvelope<PlaybackCommand>,
        ports: &dyn PlaybackPorts,
    ) -> Result<PlaybackResult, ActionError> {
        let normalized = if let PlaybackAction::RuntimeStopDynamic {
            dynamic_id,
            instance_id,
            controller_id,
        } = envelope.command.action
        {
            if !matches!(envelope.command.address, PlaybackAddress::Playback(_)) {
                return Err(ActionError::new(
                    ActionErrorKind::Invalid,
                    "Runtime Dynamic Stop requires an exact Playback owner address",
                ));
            }
            let address = resolve(
                &envelope.command.address,
                &envelope.context,
                &envelope.command.control_source,
                ports,
            )?;
            ports.validate_runtime_dynamic_stop(
                &envelope.context,
                &address,
                dynamic_id,
                instance_id,
                controller_id,
            )?;
            let mut normalized = envelope.clone();
            normalized.command.action = PlaybackAction::Release;
            Some(normalized)
        } else {
            None
        };
        let envelope = normalized.as_ref().unwrap_or(envelope);
        validate_address_action(&envelope.command.address, envelope.command.action.clone())?;
        let resolved = resolve(
            &envelope.command.address,
            &envelope.context,
            &envelope.command.control_source,
            ports,
        )?;
        let identity = runtime_identity(&resolved);
        let before = ports.projection(&envelope.context, identity.clone())?;
        let related = RelatedTransitionSet::capture(
            envelope,
            resolved.clone(),
            identity.clone(),
            before.scope,
            ports,
        )?;
        let before_desk = ports.desk_projection(&envelope.context)?;
        let configured_cause = ports.transition_cause(
            &envelope.context,
            resolved.clone(),
            envelope.command.action.clone(),
        )?;
        let execution = ports.execute(
            &envelope.context,
            resolved.clone(),
            envelope.command.action.clone(),
            &envelope.command.control_source,
        )?;
        let durability = ports.durability();
        let outcome = outcome(&execution);
        let addressed_event_required = ports.addressed_runtime_event_required();
        let projection = ports.projection(&envelope.context, identity)?;
        if projection.scope != before.scope {
            return Err(ActionError::new(
                ActionErrorKind::Internal,
                "playback action projections span multiple show revisions",
            ));
        }
        let desk = ports.desk_projection(&envelope.context)?;
        let applied = outcome == PlaybackOutcome::Applied;
        let related = if applied {
            related.publish_changes(&self.events, &envelope.context, ports)?
        } else {
            Vec::new()
        };
        let primary_event_sequence = if applied {
            committed_playback_effect_event(
                &envelope.context,
                envelope.command.action.clone(),
                configured_cause,
                before,
                projection.clone(),
                addressed_event_required,
            )
            .map(|draft| self.events.publish(draft).sequence)
        } else {
            None
        };
        let event_sequence =
            primary_event_sequence.or_else(|| related.last().map(|change| change.event_sequence));
        let desk_event_sequence = if applied && desk != before_desk {
            desk.clone().map(|projection| {
                self.events
                    .publish(EventDraft::playback_view_changed(
                        &envelope.context,
                        projection,
                    ))
                    .sequence
            })
        } else {
            None
        };
        Ok(PlaybackResult {
            context: envelope.context.clone(),
            requested: envelope.command.address.clone(),
            resolved,
            outcome,
            durability,
            execution,
            projection,
            related,
            desk,
            event_sequence,
            desk_event_sequence,
        })
    }

    // @tour playback-runtime:40 Capture a repair-safe runtime snapshot
    // The event cursor is captured before immutable projections are assembled, allowing a
    // duplicate during a race but never omitting a completed event.
    pub fn snapshot(
        &self,
        context: &ActionContext,
        identities: &[PlaybackRuntimeIdentity],
        ports: &dyn PlaybackPorts,
    ) -> Result<PlaybackRuntimeSnapshot, ActionError> {
        validate_snapshot_identities(identities)?;
        let _ordered = self.operation.lock();
        ports.authorize(context)?;
        // Capturing the cursor before reads permits duplicates on a race, but never misses an
        // event which completed while the immutable projections were being assembled.
        let event_sequence = self.events.latest_sequence();
        let projections = ports.projections(context, identities)?;
        let desk = ports.desk_projection(context)?.ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::Internal,
                "playback desk projection unavailable",
            )
        })?;
        validate_snapshot_projections(identities, &projections, desk.scope)?;
        Ok(PlaybackRuntimeSnapshot {
            event_sequence,
            desk,
            projections,
        })
    }

    pub fn run_unit_of_work<O>(&self, operation: O) -> PlaybackOperationResult<O::Output>
    where
        O: PlaybackUnitOfWork,
    {
        let _ordered = self.operation.lock();
        let completed = operation.execute();
        let event_sequences = completed
            .events
            .into_iter()
            .map(|draft| self.events.publish(draft).sequence)
            .collect();
        PlaybackOperationResult {
            output: completed.output,
            event_sequences,
        }
    }
}

fn runtime_identity(address: &ResolvedPlaybackAddress) -> PlaybackRuntimeIdentity {
    match address {
        ResolvedPlaybackAddress::CueList(id) => PlaybackRuntimeIdentity::CueList(*id),
        ResolvedPlaybackAddress::Group { group_id, .. } => {
            PlaybackRuntimeIdentity::Group(group_id.clone())
        }
        ResolvedPlaybackAddress::Playback(address) => PlaybackRuntimeIdentity::Playback(*address),
    }
}

fn validate_snapshot_projections(
    identities: &[PlaybackRuntimeIdentity],
    projections: &[PlaybackRuntimeProjection],
    scope: super::PlaybackShowScope,
) -> Result<(), ActionError> {
    let requested = identities.iter().cloned().collect::<HashSet<_>>();
    if projections
        .iter()
        .any(|projection| !requested.contains(&projection.identity))
    {
        return Err(ActionError::new(
            ActionErrorKind::Internal,
            "playback projection port returned an unrelated identity",
        ));
    }
    let projected = projections
        .iter()
        .map(|projection| projection.identity.clone())
        .collect::<HashSet<_>>();
    if requested != projected {
        return Err(ActionError::new(
            ActionErrorKind::Internal,
            "playback projection port omitted a requested identity",
        ));
    }
    if projections
        .iter()
        .any(|projection| projection.scope != scope)
    {
        return Err(ActionError::new(
            ActionErrorKind::Internal,
            "playback snapshot projections span multiple show revisions",
        ));
    }
    Ok(())
}

fn resolve(
    address: &PlaybackAddress,
    context: &ActionContext,
    source: &light_playback::PlaybackControlSource,
    ports: &dyn PlaybackPorts,
) -> Result<ResolvedPlaybackAddress, ActionError> {
    source
        .validate()
        .map_err(|message| ActionError::new(ActionErrorKind::Invalid, message))?;
    match address {
        PlaybackAddress::CueList(id) => Ok(ResolvedPlaybackAddress::CueList(*id)),
        PlaybackAddress::Group(group_id) => Ok(ResolvedPlaybackAddress::Group {
            group_id: group_id.clone(),
            playback_address: ports.group_playback(context, group_id.clone())?,
        }),
        PlaybackAddress::Playback(address) => resolve_assignment(*address, ports),
        PlaybackAddress::CurrentPage { position } => {
            let page = ports.current_page(context)?;
            let address = light_playback::PlaybackAddress::new(page, *position)
                .map_err(|message| ActionError::new(ActionErrorKind::Invalid, message))?;
            resolve_assignment(address, ports)
        }
    }
}
fn resolve_assignment(
    address: light_playback::PlaybackAddress,
    ports: &dyn PlaybackPorts,
) -> Result<ResolvedPlaybackAddress, ActionError> {
    if ports.playback_exists(address)? {
        Ok(ResolvedPlaybackAddress::Playback(address))
    } else {
        Err(ActionError::new(
            ActionErrorKind::NotFound,
            "Playback is not assigned",
        ))
    }
}

fn validate_address_action(
    address: &PlaybackAddress,
    action: super::PlaybackAction,
) -> Result<(), ActionError> {
    match action {
        super::PlaybackAction::ConfiguredButton { number, .. } if !(1..=6).contains(&number) => {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "configured button number must be within 1-6",
            ));
        }
        super::PlaybackAction::ConfiguredFader { number, .. } if !(1..=2).contains(&number) => {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "configured fader number must be within 1-2",
            ));
        }
        _ => {}
    }
    if matches!(address, PlaybackAddress::Group(_))
        && !matches!(
            action,
            super::PlaybackAction::Master(_)
                | super::PlaybackAction::MasterTransition { .. }
                | super::PlaybackAction::Flash { .. }
        )
    {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "Group runtime accepts only master and flash actions",
        ));
    }
    Ok(())
}

pub(super) fn outcome(execution: &PlaybackExecution) -> PlaybackOutcome {
    match execution {
        PlaybackExecution::Playback {
            pending: Some(action),
            ..
        } => PlaybackOutcome::Captured(*action),
        PlaybackExecution::ActiveList { changed: false, .. }
        | PlaybackExecution::Target { changed: false }
        | PlaybackExecution::Playback { changed: false, .. }
        | PlaybackExecution::Released(false) => PlaybackOutcome::NoChange,
        _ => PlaybackOutcome::Applied,
    }
}
