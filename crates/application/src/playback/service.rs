use super::{
    PlaybackAction, PlaybackAddress, PlaybackCommand, PlaybackCueTransition, PlaybackExecution,
    PlaybackOutcome, PlaybackPorts, PlaybackResult, PlaybackRuntimeChange, PlaybackRuntimeIdentity,
    PlaybackRuntimeProjection, PlaybackRuntimeSnapshot, PlaybackTransitionCause,
    ResolvedPlaybackAddress,
};
use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, EventBus, EventDraft, EventSource,
};
use parking_lot::{Mutex, MutexGuard};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use uuid::Uuid;

use super::projection::validate_snapshot_identities;

const REQUEST_CACHE_LIMIT: usize = 4_096;

#[derive(Clone)]
pub struct PlaybackService {
    operation: Arc<Mutex<()>>,
    replay: Arc<Mutex<ReplayCache>>,
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
            replay: Arc::default(),
            events,
        }
    }

    pub const fn events(&self) -> &EventBus {
        &self.events
    }

    /// Transitional access for page changes and deferred playback work that have not yet become
    /// application commands. Holding this guard orders them with all migrated playback actions.
    pub fn operation_lock(&self) -> MutexGuard<'_, ()> {
        self.operation.lock()
    }

    pub fn handle(
        &self,
        envelope: ActionEnvelope<PlaybackCommand>,
        ports: &dyn PlaybackPorts,
    ) -> Result<PlaybackResult, ActionError> {
        let _ordered = self.operation_lock();
        ports.authorize(&envelope.context)?;
        if let Some(result) = self.cached(&envelope)? {
            return Ok(result);
        }
        let result = self.apply(&envelope, ports)?;
        self.remember(&envelope, &result);
        Ok(result)
    }

    fn apply(
        &self,
        envelope: &ActionEnvelope<PlaybackCommand>,
        ports: &dyn PlaybackPorts,
    ) -> Result<PlaybackResult, ActionError> {
        let resolved = resolve(envelope.command.address, &envelope.context, ports)?;
        let identity = runtime_identity(resolved);
        let before = ports.projection(&envelope.context, identity)?;
        let before_desk = ports.desk_projection(&envelope.context)?;
        let configured_cause =
            ports.transition_cause(&envelope.context, resolved, envelope.command.action)?;
        let execution = ports.execute(
            &envelope.context,
            resolved,
            envelope.command.action,
            envelope.command.surface,
        )?;
        let durability = ports.durability();
        let outcome = outcome(&execution);
        let projection = ports.projection(&envelope.context, identity)?;
        let desk = ports.desk_projection(&envelope.context)?;
        let applied = outcome == PlaybackOutcome::Applied;
        let event_sequence = if applied && projection != before {
            Some(self.publish_change(
                &envelope.context,
                envelope.command.action,
                configured_cause,
                before,
                projection.clone(),
            ))
        } else {
            None
        };
        let desk_event_sequence = if applied && desk != before_desk {
            desk.map(|projection| {
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
            requested: envelope.command.address,
            resolved,
            outcome,
            durability,
            execution,
            projection,
            desk,
            event_sequence,
            desk_event_sequence,
            replayed: false,
        })
    }

    pub fn snapshot(
        &self,
        context: &ActionContext,
        identities: &[PlaybackRuntimeIdentity],
        ports: &dyn PlaybackPorts,
    ) -> Result<PlaybackRuntimeSnapshot, ActionError> {
        validate_snapshot_identities(identities)?;
        let _ordered = self.operation_lock();
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

    /// Publishes a projection change produced by an adapter-owned atomic batch. The caller must
    /// hold this service's operation guard from its before-read through the live commit and
    /// after-read, so the event describes one indivisible state transition.
    pub fn publish_committed_change(
        &self,
        context: &ActionContext,
        action: PlaybackAction,
        configured_cause: Option<PlaybackTransitionCause>,
        before: PlaybackRuntimeProjection,
        projection: PlaybackRuntimeProjection,
    ) -> Option<u64> {
        (before != projection)
            .then(|| self.publish_change(context, action, configured_cause, before, projection))
    }

    fn publish_change(
        &self,
        context: &ActionContext,
        action: PlaybackAction,
        configured_cause: Option<PlaybackTransitionCause>,
        before: PlaybackRuntimeProjection,
        projection: PlaybackRuntimeProjection,
    ) -> u64 {
        let transition = manual_transition(action, configured_cause, &before, &projection);
        self.events
            .publish(EventDraft::playback_runtime_changed(
                None,
                PlaybackRuntimeChange {
                    projection,
                    transition,
                },
                EventSource::Action(context.source),
                Some(context.correlation_id),
            ))
            .sequence
    }

    fn cached(
        &self,
        envelope: &ActionEnvelope<PlaybackCommand>,
    ) -> Result<Option<PlaybackResult>, ActionError> {
        let Some(key) = ReplayKey::from_envelope(envelope) else {
            return Ok(None);
        };
        self.replay.lock().get(&key, &envelope.command)
    }

    fn remember(&self, envelope: &ActionEnvelope<PlaybackCommand>, result: &PlaybackResult) {
        let Some(key) = ReplayKey::from_envelope(envelope) else {
            return;
        };
        self.replay
            .lock()
            .insert(key, envelope.command, result.clone());
    }
}

fn runtime_identity(address: ResolvedPlaybackAddress) -> PlaybackRuntimeIdentity {
    match address {
        ResolvedPlaybackAddress::CueList(id) => PlaybackRuntimeIdentity::CueList(id),
        ResolvedPlaybackAddress::Pool { number, .. } => PlaybackRuntimeIdentity::Playback(number),
    }
}

fn manual_transition(
    action: PlaybackAction,
    configured_cause: Option<PlaybackTransitionCause>,
    before: &PlaybackRuntimeProjection,
    after: &PlaybackRuntimeProjection,
) -> Option<PlaybackCueTransition> {
    let cause = configured_cause.or_else(|| navigation_cause(action))?;
    let previous = before.current_cue().cloned();
    let current = after.current_cue().cloned();
    if previous == current {
        return None;
    }
    let cue_list_id = after.cue_list_id().or_else(|| before.cue_list_id())?;
    Some(PlaybackCueTransition {
        playback_number: after.playback_number.or(before.playback_number),
        cue_list_id: cue_list_id.0,
        previous,
        current,
        cause,
        advanced_steps: 1,
    })
}

const fn navigation_cause(action: PlaybackAction) -> Option<PlaybackTransitionCause> {
    match action {
        PlaybackAction::Go { pressed: true } | PlaybackAction::FastForward { pressed: true } => {
            Some(PlaybackTransitionCause::Go)
        }
        PlaybackAction::Back { pressed: true } | PlaybackAction::FastRewind { pressed: true } => {
            Some(PlaybackTransitionCause::Back)
        }
        PlaybackAction::GoTo(_) => Some(PlaybackTransitionCause::Jump),
        _ => None,
    }
}

fn validate_snapshot_projections(
    identities: &[PlaybackRuntimeIdentity],
    projections: &[PlaybackRuntimeProjection],
    scope: super::PlaybackShowScope,
) -> Result<(), ActionError> {
    let requested = identities.iter().copied().collect::<HashSet<_>>();
    if projections
        .iter()
        .any(|projection| !requested.contains(&projection.requested))
    {
        return Err(ActionError::new(
            ActionErrorKind::Internal,
            "playback projection port returned an unrelated identity",
        ));
    }
    let projected = projections
        .iter()
        .map(|projection| projection.requested)
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
    address: PlaybackAddress,
    context: &crate::ActionContext,
    ports: &dyn PlaybackPorts,
) -> Result<ResolvedPlaybackAddress, ActionError> {
    match address {
        PlaybackAddress::CueList(id) => Ok(ResolvedPlaybackAddress::CueList(id)),
        PlaybackAddress::Pool(number) => Ok(pool(number, None, None)),
        PlaybackAddress::CurrentPage { slot } => {
            let page = ports.current_page(context)?;
            resolve_page(page, slot, ports)
        }
        PlaybackAddress::ExplicitPage { page, slot } => resolve_page(page, slot, ports),
    }
}

fn resolve_page(
    page: u8,
    slot: u8,
    ports: &dyn PlaybackPorts,
) -> Result<ResolvedPlaybackAddress, ActionError> {
    ports
        .playback_at(page, slot)?
        .map(|number| pool(number, Some(page), Some(slot)))
        .ok_or_else(|| ActionError::new(ActionErrorKind::NotFound, "paged playback"))
}

const fn pool(number: u16, page: Option<u8>, slot: Option<u8>) -> ResolvedPlaybackAddress {
    ResolvedPlaybackAddress::Pool { number, page, slot }
}

fn outcome(execution: &PlaybackExecution) -> PlaybackOutcome {
    match execution {
        PlaybackExecution::Pool {
            pending: Some(action),
            ..
        } => PlaybackOutcome::Captured(*action),
        PlaybackExecution::Pool { changed: false, .. } | PlaybackExecution::Released(false) => {
            PlaybackOutcome::NoChange
        }
        _ => PlaybackOutcome::Applied,
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    desk_id: Uuid,
    session_id: Option<Uuid>,
    request_id: String,
}

impl ReplayKey {
    fn from_envelope(envelope: &ActionEnvelope<PlaybackCommand>) -> Option<Self> {
        Some(Self {
            desk_id: envelope.context.desk_id,
            session_id: envelope.context.session_id,
            request_id: envelope.context.request_id.clone()?,
        })
    }
}

struct ReplayEntry {
    command: PlaybackCommand,
    result: PlaybackResult,
}

#[derive(Default)]
struct ReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl ReplayCache {
    fn get(
        &self,
        key: &ReplayKey,
        command: &PlaybackCommand,
    ) -> Result<Option<PlaybackResult>, ActionError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if entry.command != *command {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "request_id was already used for a different playback operation",
            ));
        }
        let mut result = entry.result.clone();
        result.replayed = true;
        Ok(Some(result))
    }

    fn insert(&mut self, key: ReplayKey, command: PlaybackCommand, result: PlaybackResult) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, ReplayEntry { command, result });
        while self.entries.len() > REQUEST_CACHE_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}
