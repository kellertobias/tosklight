use super::{
    PlaybackAddress, PlaybackCommand, PlaybackExecution, PlaybackOperationResult, PlaybackOutcome,
    PlaybackPorts, PlaybackResult, PlaybackRuntimeIdentity, PlaybackRuntimeProjection,
    PlaybackRuntimeSnapshot, PlaybackUnitOfWork, ResolvedPlaybackAddress, committed_playback_event,
};
use crate::{ActionContext, ActionEnvelope, ActionError, ActionErrorKind, EventBus, EventDraft};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use uuid::Uuid;

use super::projection::validate_snapshot_identities;
use super::transition_set::RelatedTransitionSet;

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

    pub fn handle(
        &self,
        envelope: ActionEnvelope<PlaybackCommand>,
        ports: &dyn PlaybackPorts,
    ) -> Result<PlaybackResult, ActionError> {
        let _ordered = self.operation.lock();
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
        let related =
            RelatedTransitionSet::capture(envelope, resolved, identity, before.scope, ports)?;
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
            committed_playback_event(
                &envelope.context,
                envelope.command.action,
                configured_cause,
                before,
                projection.clone(),
            )
            .map(|draft| self.events.publish(draft).sequence)
        } else {
            None
        };
        let event_sequence =
            primary_event_sequence.or_else(|| related.last().map(|change| change.event_sequence));
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
            related,
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
