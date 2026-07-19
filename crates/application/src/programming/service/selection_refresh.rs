use super::ProgrammingService;
use crate::{
    ActionContext, ProgrammingInteractionChange, ProgrammingSelectionRefreshEvent,
    ProgrammingSelectionRefreshResult, ProgrammingSelectionTarget,
};

impl ProgrammingService {
    /// Runs one engine- or show-driven selection reconciliation across multiple desks.
    ///
    /// Targets are deduplicated and locked by desk UUID before the operation runs. The operation
    /// may refresh live Group membership and complete adapter reconciliation; final selection
    /// projections are then published once per changed desk in the same deterministic order. Use
    /// `run_selection_refresh_with_owned_target` when the caller already holds one target's desk
    /// boundary.
    pub fn run_selection_refresh<T>(
        &self,
        context: &ActionContext,
        targets: impl IntoIterator<Item = ProgrammingSelectionTarget>,
        operation: impl FnOnce() -> T,
    ) -> ProgrammingSelectionRefreshResult<T> {
        self.run_selection_refresh_inner(context, None, targets, operation)
    }

    /// Runs a shared refresh from inside `owned_target`'s already-held Programming interaction.
    /// The owner is observed and published with the peers but deliberately not re-locked. The
    /// outer interaction recognizes that selection revision as already published and emits only
    /// any remaining command-line component after the nested operation returns.
    pub fn run_selection_refresh_with_owned_target<T>(
        &self,
        context: &ActionContext,
        owned_target: ProgrammingSelectionTarget,
        targets: impl IntoIterator<Item = ProgrammingSelectionTarget>,
        operation: impl FnOnce() -> T,
    ) -> ProgrammingSelectionRefreshResult<T> {
        self.run_selection_refresh_inner(context, Some(owned_target), targets, operation)
    }

    fn run_selection_refresh_inner<T>(
        &self,
        context: &ActionContext,
        owned_target: Option<ProgrammingSelectionTarget>,
        targets: impl IntoIterator<Item = ProgrammingSelectionTarget>,
        operation: impl FnOnce() -> T,
    ) -> ProgrammingSelectionRefreshResult<T> {
        let locked_targets = normalized_targets(targets)
            .into_iter()
            .filter(|target| Some(target.desk_id) != owned_target.map(|owner| owner.desk_id))
            .collect::<Vec<_>>();
        let desk_ids = locked_targets
            .iter()
            .map(|target| target.desk_id)
            .collect::<Vec<_>>();
        let targets = normalized_targets(locked_targets.iter().copied().chain(owned_target));
        self.with_desk_gates(&desk_ids, || {
            let before = targets
                .iter()
                .filter_map(|target| {
                    self.programmers
                        .selection(target.interaction_id)
                        .map(|selection| (*target, selection.revision))
                })
                .collect::<Vec<_>>();
            let output = operation();
            let events = before
                .into_iter()
                .filter_map(|(target, before_revision)| {
                    let selection = self.programmers.selection(target.interaction_id)?;
                    if selection.revision == before_revision {
                        return None;
                    }
                    let change = ProgrammingInteractionChange::from_components(
                        target.desk_id,
                        None,
                        Some(selection),
                    )?;
                    let event_sequence = self.publish_selection_refresh(
                        context,
                        change,
                        Some(target.desk_id)
                            == owned_target.map(|owned_target| owned_target.desk_id),
                    );
                    Some(ProgrammingSelectionRefreshEvent {
                        desk_id: target.desk_id,
                        event_sequence,
                    })
                })
                .collect();
            ProgrammingSelectionRefreshResult { output, events }
        })
    }
}

fn normalized_targets(
    targets: impl IntoIterator<Item = ProgrammingSelectionTarget>,
) -> Vec<ProgrammingSelectionTarget> {
    let mut targets = targets.into_iter().collect::<Vec<_>>();
    targets.sort_unstable_by_key(|target| (target.desk_id, target.interaction_id.0));
    targets.dedup_by_key(|target| target.desk_id);
    targets
}
