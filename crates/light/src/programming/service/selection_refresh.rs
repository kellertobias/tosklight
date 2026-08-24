use super::ProgrammingService;
use crate::{
    ActionContext, ProgrammingInteractionChange, ProgrammingSelectionRefreshEvent,
    ProgrammingSelectionRefreshResult,
};

impl ProgrammingService {
    /// Runs one engine- or show-driven reconciliation of the desk's selection.
    ///
    /// The operation may refresh live Group membership and complete adapter reconciliation; the
    /// desk's final selection projection is then published once, if it changed. Use
    /// `run_selection_refresh_within_interaction` when the caller already holds the desk's
    /// Programming boundary.
    pub fn run_selection_refresh<T>(
        &self,
        context: &ActionContext,
        operation: impl FnOnce() -> T,
    ) -> ProgrammingSelectionRefreshResult<T> {
        self.run_selection_refresh_inner(context, false, operation)
    }

    /// Runs a refresh from inside the desk's already-held Programming interaction.
    ///
    /// The desk is observed and published but deliberately not re-locked. The outer interaction
    /// recognizes that selection revision as already published and emits only any remaining
    /// command-line component after the nested operation returns.
    pub fn run_selection_refresh_within_interaction<T>(
        &self,
        context: &ActionContext,
        operation: impl FnOnce() -> T,
    ) -> ProgrammingSelectionRefreshResult<T> {
        self.run_selection_refresh_inner(context, true, operation)
    }

    fn run_selection_refresh_inner<T>(
        &self,
        context: &ActionContext,
        within_interaction: bool,
        operation: impl FnOnce() -> T,
    ) -> ProgrammingSelectionRefreshResult<T> {
        let Some(interaction) = self.programmers.desk_interaction_context() else {
            // No surface has connected, so there is no command line or selection to reconcile.
            return ProgrammingSelectionRefreshResult {
                output: operation(),
                events: Vec::new(),
            };
        };
        // A refresh already running inside the desk's interaction must not take its gate again.
        let desk_gates: &[uuid::Uuid] = if within_interaction {
            &[]
        } else {
            std::slice::from_ref(&interaction.0)
        };
        self.programmers.serialized(|| {
            self.with_desk_gates(desk_gates, || {
                self.run_locked_selection_refresh(
                    context,
                    within_interaction,
                    interaction,
                    operation,
                )
            })
        })
    }

    fn run_locked_selection_refresh<T>(
        &self,
        context: &ActionContext,
        within_interaction: bool,
        interaction: light_core::SessionId,
        operation: impl FnOnce() -> T,
    ) -> ProgrammingSelectionRefreshResult<T> {
        let lifecycle_before = self.active_lifecycle_programmer();
        let before = self.programmers.interaction_context_version(interaction);
        let output = operation();
        let after = self.programmers.interaction_context_version(interaction);
        let command_line =
            (before.command_line != after.command_line).then_some(after.command_line);
        let selection = (before.selection_revision != after.selection_revision).then(|| {
            self.programmers
                .interaction_selection_for_context(interaction)
        });
        // Published under the acting context's desk, which is the key an outer interaction uses
        // to recognise a selection revision it has already sent.
        let desk_id = context.desk_id;
        let events =
            ProgrammingInteractionChange::from_components(desk_id, command_line, selection)
                .map(|change| {
                    let event_sequence =
                        self.publish_selection_refresh(context, change, within_interaction);
                    vec![ProgrammingSelectionRefreshEvent {
                        desk_id,
                        event_sequence,
                    }]
                })
                .unwrap_or_default();
        self.publish_lifecycle(context, lifecycle_before);
        ProgrammingSelectionRefreshResult { output, events }
    }
}
