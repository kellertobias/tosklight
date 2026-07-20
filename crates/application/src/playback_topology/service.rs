use super::{
    PlaybackTopologyCommand, PlaybackTopologyOutcome, PlaybackTopologyPorts,
    PlaybackTopologyResult,
    candidate::prepare,
    change::PreparedTopology,
    replay::{ReplayCache, ReplayKey, fingerprint},
};
use crate::active_show::CompletedActiveShowTransaction;
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, ActiveShowObjectsChange, ActiveShowService,
    EventBus, EventDraft,
};
use parking_lot::Mutex;
use std::sync::Arc;

/// Feature-owned application boundary for portable Cuelist, Playback, and Page topology.
#[derive(Clone)]
pub struct PlaybackTopologyService {
    active_show: ActiveShowService,
    replay_order: Arc<Mutex<()>>,
    replay: Arc<Mutex<ReplayCache>>,
}

impl PlaybackTopologyService {
    pub fn new(active_show: ActiveShowService) -> Self {
        Self {
            active_show,
            replay_order: Arc::new(Mutex::new(())),
            replay: Arc::new(Mutex::new(ReplayCache::default())),
        }
    }

    pub fn events(&self) -> &EventBus {
        self.active_show.events()
    }

    pub fn handle<P: PlaybackTopologyPorts>(
        &self,
        envelope: ActionEnvelope<PlaybackTopologyCommand>,
        ports: &P,
    ) -> Result<PlaybackTopologyResult, ActionError> {
        ports.authorize_playback_topology(&envelope.context)?;
        let key = ReplayKey::from_context(&envelope.context)?;
        let expected_show_revision = envelope.context.expected_revision.ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::Invalid,
                "Playback topology action requires an expected Show revision",
            )
        })?;
        let fingerprint = fingerprint(&envelope.command, expected_show_revision)?;
        if let Some(result) = self.replay.lock().get(&key, fingerprint)? {
            return Ok(result);
        }
        let _replay_order = self.replay_order.lock();
        if let Some(result) = self.replay.lock().get(&key, fingerprint)? {
            return Ok(result);
        }
        let result = self.apply(envelope, ports, expected_show_revision, key.request_id())?;
        self.replay.lock().insert(key, fingerprint, result.clone());
        Ok(result)
    }

    fn apply<P: PlaybackTopologyPorts>(
        &self,
        envelope: ActionEnvelope<PlaybackTopologyCommand>,
        ports: &P,
        expected_show_revision: u64,
        request_id: &str,
    ) -> Result<PlaybackTopologyResult, ActionError> {
        let context = envelope.context;
        let command = envelope.command;
        let outcome = self.active_show.transact(
            &context,
            command.show_id,
            ports,
            "playback-topology",
            |document| prepare(document, &command, expected_show_revision),
            complete,
        )?;
        Ok(PlaybackTopologyResult {
            correlation_id: context.correlation_id,
            context,
            request_id: request_id.to_owned(),
            replayed: false,
            outcome,
        })
    }
}

fn complete<P: PlaybackTopologyPorts>(
    events: &EventBus,
    ports: &P,
    context: &crate::ActionContext,
    completed: CompletedActiveShowTransaction<PreparedTopology>,
) -> PlaybackTopologyOutcome {
    let PreparedTopology {
        show_id,
        mut show_revision,
        resolution,
        objects,
        changes,
    } = completed.state;
    let Some(commit) = completed.commit else {
        return PlaybackTopologyOutcome::NoChange {
            show_revision,
            resolution,
            objects,
        };
    };
    show_revision = commit.revision();
    ports.reconcile_playback_topology(&changes);
    let event_sequence = events
        .publish(EventDraft::active_show_objects_changed(
            context,
            ActiveShowObjectsChange {
                show_id,
                show_revision,
                changes,
            },
        ))
        .sequence;
    PlaybackTopologyOutcome::Changed {
        show_revision,
        resolution,
        objects,
        event_sequence,
    }
}

impl Default for PlaybackTopologyService {
    fn default() -> Self {
        Self::new(ActiveShowService::default())
    }
}
