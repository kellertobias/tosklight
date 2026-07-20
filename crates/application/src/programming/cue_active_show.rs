use super::{
    ProgrammingCueActiveShowPorts, ProgrammingCueCommit, ProgrammingCueCommitResult,
    ProgrammingCueObjectProjection, ProgrammingCueShowRevisionExpectation,
};
use crate::active_show::{CompletedActiveShowTransaction, PreparedActiveShowTransaction};
use crate::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowObjectChange, ActiveShowObjectKind,
    ActiveShowObjectsChange, ActiveShowService, EventBus, EventDraft,
};
use light_show::PortableShowDocument;

mod candidate;
mod target;

impl ActiveShowService {
    /// Commits one captured Cue through the centralized, lossless ActiveShow lifecycle.
    pub fn commit_programming_cue<P>(
        &self,
        context: &ActionContext,
        commit: &ProgrammingCueCommit,
        ports: &P,
    ) -> Result<ProgrammingCueCommitResult, ActionError>
    where
        P: ProgrammingCueActiveShowPorts,
    {
        self.transact(
            context,
            commit.show_id,
            ports,
            "record-cue",
            |document| prepare_recording(document, commit),
            complete_recording,
        )
    }
}

fn prepare_recording(
    document: &PortableShowDocument,
    commit: &ProgrammingCueCommit,
) -> Result<PreparedActiveShowTransaction<PreparedRecording>, ActionError> {
    validate_show(document, commit)?;
    let target = target::resolve_target(document, commit)?;
    candidate::prepare_candidate(document, commit, target)
}

fn complete_recording<P: ProgrammingCueActiveShowPorts>(
    events: &EventBus,
    ports: &P,
    context: &ActionContext,
    completed: CompletedActiveShowTransaction<PreparedRecording>,
) -> ProgrammingCueCommitResult {
    let PreparedRecording {
        mut result,
        changed_kinds,
    } = completed.state;
    let Some(commit) = completed.commit else {
        return result;
    };
    result.show_revision = commit.revision();
    ports.reconcile_programming_cue(&result.projections);
    let changes = changed_kinds
        .into_iter()
        .filter_map(|kind| projection_for_kind(&result, kind))
        .map(object_change)
        .collect();
    result.event_sequence = Some(
        events
            .publish(EventDraft::active_show_objects_changed(
                context,
                ActiveShowObjectsChange {
                    show_id: result.projections.show_id,
                    show_revision: result.show_revision,
                    changes,
                },
            ))
            .sequence,
    );
    result
}

pub(super) struct PreparedRecording {
    result: ProgrammingCueCommitResult,
    changed_kinds: Vec<ActiveShowObjectKind>,
}

fn projection_for_kind(
    result: &ProgrammingCueCommitResult,
    kind: ActiveShowObjectKind,
) -> Option<&ProgrammingCueObjectProjection> {
    match kind {
        ActiveShowObjectKind::CueList => Some(&result.projections.cue_list),
        ActiveShowObjectKind::Playback => result.projections.playback.as_ref(),
        ActiveShowObjectKind::PlaybackPage => result.projections.page.as_ref(),
        ActiveShowObjectKind::Group | ActiveShowObjectKind::Preset => None,
    }
}

fn object_change(projection: &ProgrammingCueObjectProjection) -> ActiveShowObjectChange {
    ActiveShowObjectChange {
        kind: projection.kind,
        object_id: projection.object_id.clone(),
        object_revision: projection.object_revision,
        body: Some(projection.raw_body.as_ref().clone()),
        deleted: false,
    }
}

fn validate_show(
    document: &PortableShowDocument,
    commit: &ProgrammingCueCommit,
) -> Result<(), ActionError> {
    if document.id() != commit.show_id {
        return Err(ActionError::new(
            ActionErrorKind::NotFound,
            "requested show is not active",
        ));
    }
    match commit.expected_show_revision {
        ProgrammingCueShowRevisionExpectation::Current => Ok(()),
        ProgrammingCueShowRevisionExpectation::Exact(expected)
            if expected == document.revision() =>
        {
            Ok(())
        }
        ProgrammingCueShowRevisionExpectation::Exact(_) => Err(ActionError::new(
            ActionErrorKind::Conflict,
            "stale active-show revision",
        )
        .at_revision(document.revision().value())),
    }
}

#[cfg(test)]
#[path = "cue_active_show_tests.rs"]
mod tests;
