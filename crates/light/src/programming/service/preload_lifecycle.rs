use super::{ProgrammingService, state::interaction_change, support::Snapshot};
use crate::{
    ActionEnvelope, ActionError, ProgrammingCaptureModeProjection,
    ProgrammingPreloadLifecycleAction, ProgrammingPreloadLifecyclePorts,
    ProgrammingPreloadLifecycleRequest, ProgrammingPreloadLifecycleResult,
    ProgrammingPreloadLifecycleState,
};
use light_core::{SessionId, UserId};
use std::sync::Arc;

use super::preload_lifecycle_replay::PreloadLifecycleReplayIdentity;
use super::preload_lifecycle_validation::{
    LifecycleIdentity, lifecycle_identity, preload_unavailable,
};

struct MutationResult {
    changed: bool,
    commit: Option<crate::ProgrammingPreloadCommitResult>,
    warning: Option<String>,
}

impl ProgrammingService {
    pub fn handle_preload_lifecycle(
        &self,
        action: ActionEnvelope<ProgrammingPreloadLifecycleRequest>,
        ports: &dyn ProgrammingPreloadLifecyclePorts,
    ) -> Result<ProgrammingPreloadLifecycleResult, ActionError> {
        let identity = lifecycle_identity(&action)?;
        self.with_user_and_desk_gate(action.context.desk_id, identity.user_id, || {
            self.apply_preload_lifecycle(action, ports, identity)
        })
    }

    fn apply_preload_lifecycle(
        &self,
        action: ActionEnvelope<ProgrammingPreloadLifecycleRequest>,
        ports: &dyn ProgrammingPreloadLifecyclePorts,
        identity: LifecycleIdentity,
    ) -> Result<ProgrammingPreloadLifecycleResult, ActionError> {
        ports.authorize_preload_lifecycle(&action.context)?;
        self.assert_preload_owner(identity.session_id, identity.user_id)?;
        let replay_identity = PreloadLifecycleReplayIdentity {
            user_id: identity.user_id,
            desk_id: action.context.desk_id,
            session_id: identity.session_id,
            request_id: identity.request_id.clone(),
        };
        if let Some(result) = self
            .preload_lifecycle_replay
            .lock()
            .get(&replay_identity, &action.command)?
        {
            return Ok(result);
        }
        self.assert_lifecycle_revisions(identity.session_id, identity.user_id, &action.command)?;
        let result = self.mutate_preload_lifecycle(&action, ports, &identity)?;
        self.preload_lifecycle_replay.lock().insert(
            replay_identity,
            action.command,
            result.clone(),
        );
        Ok(result)
    }

    fn mutate_preload_lifecycle(
        &self,
        action: &ActionEnvelope<ProgrammingPreloadLifecycleRequest>,
        ports: &dyn ProgrammingPreloadLifecyclePorts,
        identity: &LifecycleIdentity,
    ) -> Result<ProgrammingPreloadLifecycleResult, ActionError> {
        let lifecycle_before = self.active_lifecycle_programmer(identity.user_id);
        let before = Snapshot::read(
            &self.programmers,
            action.context.desk_id,
            identity.session_id,
            identity.user_id,
        )?;
        self.assert_go_is_armed(identity.session_id, &action.command.action)?;
        let mutation = self.run_preload_mutation(action, ports, identity.session_id, &before)?;
        let mutated = Snapshot::read(
            &self.programmers,
            action.context.desk_id,
            identity.session_id,
            identity.user_id,
        )?;
        if before.capture_mode != mutated.capture_mode {
            ports.reconcile_preload_capture(&action.context);
        }
        let after = Snapshot::read(
            &self.programmers,
            action.context.desk_id,
            identity.session_id,
            identity.user_id,
        )?;
        let result = self.finish_preload_lifecycle(action, identity, before, after, mutation)?;
        self.publish_lifecycle_for_context(&action.context, lifecycle_before);
        Ok(result)
    }

    fn run_preload_mutation(
        &self,
        action: &ActionEnvelope<ProgrammingPreloadLifecycleRequest>,
        ports: &dyn ProgrammingPreloadLifecyclePorts,
        session_id: SessionId,
        before: &Snapshot,
    ) -> Result<MutationResult, ActionError> {
        let (changed, commit, operation) = match action.command.action {
            ProgrammingPreloadLifecycleAction::Enter => {
                let capture = ports.capture_programmer_on_preload(&action.context);
                self.programmers.arm_preload(session_id, capture);
                let changed =
                    self.programmers.capture_mode(session_id) != Some(before.capture_mode);
                (changed, None, "preload.enter")
            }
            ProgrammingPreloadLifecycleAction::Go { .. } => {
                let commit = ports.commit_preload(&action.context, &action.command)?;
                (true, Some(commit), "preload.go")
            }
            ProgrammingPreloadLifecycleAction::ClearPending => {
                self.programmers.clear_preload_pending(session_id);
                let changed = self.programmers.preload_values_generation(session_id)
                    != Some(before.preload_values_generation)
                    || self
                        .programmers
                        .preload_playback_queue_generation(session_id)
                        != Some(before.preload_playback_queue_generation);
                (changed, None, "preload.clear")
            }
            ProgrammingPreloadLifecycleAction::Release => (
                self.programmers.release_preload(session_id),
                None,
                "preload.release",
            ),
        };
        let warning = if commit.is_none() && changed {
            ports.persist_preload_lifecycle(&action.context, operation)
        } else {
            None
        };
        Ok(MutationResult {
            changed,
            commit,
            warning,
        })
    }

    fn finish_preload_lifecycle(
        &self,
        action: &ActionEnvelope<ProgrammingPreloadLifecycleRequest>,
        identity: &LifecycleIdentity,
        before: Snapshot,
        after: Snapshot,
        mutation: MutationResult,
    ) -> Result<ProgrammingPreloadLifecycleResult, ActionError> {
        let interaction = interaction_change(
            &self.programmers,
            action.context.desk_id,
            identity.session_id,
            &before,
            &after,
        );
        let capture =
            self.capture_mode_change(identity.user_id, before.capture_mode, after.capture_mode);
        let values = self.preload_values_change(
            identity.user_id,
            identity.session_id,
            before.preload_values_generation,
            after.preload_values_generation,
        )?;
        let queue = self.preload_playback_queue_change(
            identity.user_id,
            identity.session_id,
            before.preload_playback_queue_generation,
            after.preload_playback_queue_generation,
        )?;
        let interaction_event_sequence = self.publish_interaction(&action.context, interaction);
        let (capture_mode, capture_mode_event_sequence) =
            self.finish_capture(&action.context, identity, after.capture_mode, capture)?;
        let (values_revision, values_projection, values_event_sequence) =
            self.finish_preload_values(&action.context, identity.user_id, values);
        let (queue_revision, queue_projection, queue_event_sequence) =
            self.finish_preload_queue(&action.context, identity.user_id, queue);
        let changed = mutation.changed
            || interaction_event_sequence.is_some()
            || capture_mode_event_sequence.is_some()
            || values_event_sequence.is_some()
            || queue_event_sequence.is_some();
        let warning = lifecycle_warning(&mutation);
        Ok(ProgrammingPreloadLifecycleResult {
            context: action.context.clone(),
            request_id: identity.request_id.clone(),
            replayed: false,
            state: if changed {
                ProgrammingPreloadLifecycleState::Changed
            } else {
                ProgrammingPreloadLifecycleState::NoChange
            },
            active: self
                .programmers
                .has_active_preload(identity.session_id)
                .ok_or_else(preload_unavailable)?,
            capture_mode,
            capture_mode_event_sequence,
            values_revision,
            values_projection,
            values_event_sequence,
            queue_revision,
            queue_projection,
            queue_event_sequence,
            interaction_event_sequence,
            selection_revision: after.selection_revision,
            commit: mutation.commit,
            warning,
        })
    }

    fn finish_capture(
        &self,
        context: &crate::ActionContext,
        identity: &LifecycleIdentity,
        mode: light_programmer::ProgrammerCaptureMode,
        change: Option<crate::ProgrammingCaptureModeChange>,
    ) -> Result<(Arc<ProgrammingCaptureModeProjection>, Option<u64>), ActionError> {
        if let Some(change) = change {
            let projection = Arc::clone(&change.projection);
            let sequence = self.publish_capture_mode(context, Some(change));
            return Ok((projection, sequence));
        }
        let revision = self.programmers.capture_mode_revision(identity.user_id);
        Ok((
            Arc::new(ProgrammingCaptureModeProjection::from_mode(
                identity.user_id,
                revision,
                mode,
            )),
            None,
        ))
    }

    fn finish_preload_values(
        &self,
        context: &crate::ActionContext,
        user_id: UserId,
        change: Option<crate::ProgrammingPreloadValuesChange>,
    ) -> (
        u64,
        Option<Arc<crate::ProgrammingPreloadValuesProjection>>,
        Option<u64>,
    ) {
        let revision = change.as_ref().map_or_else(
            || self.programmers.preload_values_revision(user_id),
            |change| change.projection.revision,
        );
        let projection = change.as_ref().map(|change| Arc::clone(&change.projection));
        let sequence = self.publish_preload_values(context, change);
        (revision, projection, sequence)
    }

    fn finish_preload_queue(
        &self,
        context: &crate::ActionContext,
        user_id: UserId,
        change: Option<crate::ProgrammingPreloadPlaybackQueueChange>,
    ) -> (
        u64,
        Option<Arc<crate::ProgrammingPreloadPlaybackQueueProjection>>,
        Option<u64>,
    ) {
        let revision = change.as_ref().map_or_else(
            || self.programmers.preload_playback_queue_revision(user_id),
            |change| change.projection.revision,
        );
        let projection = change.as_ref().map(|change| Arc::clone(&change.projection));
        let sequence = self.publish_preload_playback_queue(context, change);
        (revision, projection, sequence)
    }
}

fn lifecycle_warning(mutation: &MutationResult) -> Option<String> {
    let mut warnings = mutation
        .commit
        .as_ref()
        .map_or_else(Vec::new, |commit| commit.warnings.clone());
    warnings.extend(mutation.warning.clone());
    (!warnings.is_empty()).then(|| warnings.join("; "))
}
