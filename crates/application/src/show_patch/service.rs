use super::prepare::{PreparedPatch, plan_patch, prepare_patch};
use super::query::build_snapshot;
use super::replay::{ReplayCache, ReplayKey};
use super::validation::validate_action;
use super::{
    PatchChange, PatchFixturesCommand, PatchFixturesResult, PatchSnapshot, ShowPatchPorts,
};
use crate::active_show::{CompletedActiveShowTransaction, PreparedActiveShowTransaction};
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, ActiveShowService, EventBus, EventDraft,
};
use parking_lot::Mutex;
use std::sync::Arc;

/// Capability boundary for active-show patching.
///
/// Replay coordination remains patch-specific, while every document mutation is ordered by the
/// injected [`ActiveShowService`]. Slow immutable fixture-library reads therefore never hold the
/// shared mutation gate.
#[derive(Clone)]
pub struct ShowPatchService {
    active_show: ActiveShowService,
    replay_order: Arc<Mutex<()>>,
    replay: Arc<Mutex<ReplayCache>>,
}

impl ShowPatchService {
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

    pub fn snapshot<P: ShowPatchPorts>(
        &self,
        context: &crate::ActionContext,
        show_id: light_core::ShowId,
        ports: &P,
    ) -> Result<PatchSnapshot, ActionError> {
        ports.authorize_patch_read(context)?;
        let (document, event_sequence) = self
            .active_show
            .snapshot_with_event_sequence(context, show_id, ports)?;
        validate_active_show_id(&document, show_id)?;
        let mut snapshot = build_snapshot(&document)?;
        snapshot.event_sequence = event_sequence;
        Ok(snapshot)
    }

    pub fn handle<P: ShowPatchPorts>(
        &self,
        envelope: ActionEnvelope<PatchFixturesCommand>,
        ports: &P,
    ) -> Result<PatchFixturesResult, ActionError> {
        ports.authorize_patch(&envelope.context)?;
        validate_action(&envelope.context, &envelope.command)?;
        let key = required_replay_key(&envelope)?;
        if let Some(result) = self.cached(&key, &envelope)? {
            return Ok(result);
        }
        // This lock provides exact-once request replay, not show-mutation ordering. It may be held
        // during library reads because ordinary active-show mutations use only `active_show`.
        let _replay_order = self.replay_order.lock();
        if let Some(result) = self.cached(&key, &envelope)? {
            return Ok(result);
        }
        self.apply(key, envelope, ports)
    }

    fn apply<P: ShowPatchPorts>(
        &self,
        key: ReplayKey,
        envelope: ActionEnvelope<PatchFixturesCommand>,
        ports: &P,
    ) -> Result<PatchFixturesResult, ActionError> {
        let snapshot =
            self.active_show
                .snapshot(&envelope.context, envelope.command.show_id, ports)?;
        validate_active_document(&snapshot, &envelope)?;
        let plan = plan_patch(&snapshot, &envelope.command, ports)?;
        let transaction_context = envelope.context.clone();
        let replay = Arc::clone(&self.replay);
        self.active_show.transact(
            &transaction_context,
            envelope.command.show_id,
            ports,
            "patch",
            move |document| {
                validate_active_document(document, &envelope)?;
                match prepare_patch(document, &envelope.command, plan)? {
                    PreparedPatch::Noop(change) => {
                        Ok(PreparedActiveShowTransaction::NoChange(PatchCompletion {
                            key,
                            envelope,
                            change,
                        }))
                    }
                    PreparedPatch::Mutation(prepared) => {
                        Ok(PreparedActiveShowTransaction::PreparedCommit {
                            prepared: Box::new(prepared.candidate),
                            state: PatchCompletion {
                                key,
                                envelope,
                                change: prepared.change,
                            },
                        })
                    }
                }
            },
            move |events, ports, _context, completed| {
                complete_patch(events, ports, completed, &replay)
            },
        )
    }

    fn cached(
        &self,
        key: &ReplayKey,
        envelope: &ActionEnvelope<PatchFixturesCommand>,
    ) -> Result<Option<PatchFixturesResult>, ActionError> {
        self.replay
            .lock()
            .get(key, &envelope.context, &envelope.command)
    }
}

impl Default for ShowPatchService {
    fn default() -> Self {
        Self::new(ActiveShowService::default())
    }
}

struct PatchCompletion {
    key: ReplayKey,
    envelope: ActionEnvelope<PatchFixturesCommand>,
    change: PatchChange,
}

fn complete_patch<P: ShowPatchPorts>(
    events: &EventBus,
    ports: &P,
    completed: CompletedActiveShowTransaction<PatchCompletion>,
    replay: &Mutex<ReplayCache>,
) -> PatchFixturesResult {
    let PatchCompletion {
        key,
        envelope,
        mut change,
    } = completed.state;
    let event_sequence = completed.commit.map(|commit| {
        change.show_revision = commit.revision();
        change.patch_revision = commit.patch_revision();
        // Adapter projections and caches must match the installed runtime before subscribers can
        // observe the corresponding event sequence.
        ports.reconcile_patch_change(&change);
        events
            .publish(EventDraft::patch_changed(&envelope.context, change.clone()))
            .sequence
    });
    let result = PatchFixturesResult {
        context: envelope.context.clone(),
        request_id: key.request_id().to_owned(),
        replayed: false,
        changed: event_sequence.is_some(),
        change,
        event_sequence,
    };
    replay
        .lock()
        .insert(key, &envelope.context, envelope.command, result.clone());
    result
}

fn required_replay_key(
    envelope: &ActionEnvelope<PatchFixturesCommand>,
) -> Result<ReplayKey, ActionError> {
    ReplayKey::from_context(&envelope.context)
        .ok_or_else(|| ActionError::new(ActionErrorKind::Invalid, "patch request_id is required"))
}

fn validate_active_document(
    document: &light_show::PortableShowDocument,
    envelope: &ActionEnvelope<PatchFixturesCommand>,
) -> Result<(), ActionError> {
    validate_active_show_id(document, envelope.command.show_id)?;
    let Some(expected) = envelope.context.expected_revision else {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "patch operation requires an expected patch revision",
        ));
    };
    if document.patch_revision().value() == expected {
        Ok(())
    } else {
        Err(
            ActionError::new(ActionErrorKind::Conflict, "stale patch revision")
                .at_revision(document.patch_revision().value()),
        )
    }
}

fn validate_active_show_id(
    document: &light_show::PortableShowDocument,
    show_id: light_core::ShowId,
) -> Result<(), ActionError> {
    if document.id() == show_id {
        Ok(())
    } else {
        Err(ActionError::new(
            ActionErrorKind::NotFound,
            "requested show is not active",
        ))
    }
}
