use super::{
    ActiveMvrImportResult, ApplyActiveMvrImportCommand, PreparedActiveMvrImport,
    model::{PlannedPatchChange, PreparedMvrImportState},
    plan::plan_import,
};
use crate::active_show::{CompletedActiveShowTransaction, PreparedActiveShowTransaction};
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, ActiveShowService, EventBus, EventDraft,
    PatchChange, PatchFixtureProjection, ShowPatchPorts, prepare_show_candidate,
};
use light_show::{PortablePatchRevision, PortableShowCommit, PortableShowRevision};

#[derive(Clone)]
pub struct MvrImportService {
    active_show: ActiveShowService,
}

impl MvrImportService {
    pub const fn new(active_show: ActiveShowService) -> Self {
        Self { active_show }
    }

    /// Plans and compiles against one immutable active-show revision without retaining the shared
    /// mutation gate during MVR-sized fixture work.
    pub fn prepare<P: ShowPatchPorts>(
        &self,
        envelope: ActionEnvelope<ApplyActiveMvrImportCommand>,
        ports: &P,
    ) -> Result<PreparedActiveMvrImport, ActionError> {
        ports.authorize_patch(&envelope.context)?;
        let document =
            self.active_show
                .snapshot(&envelope.context, envelope.command.show_id, ports)?;
        let source_show_revision = document.revision();
        let source_patch_revision = document.patch_revision();
        let planned = plan_import(&document, envelope.context, &envelope.command)?;
        let candidate = if planned.transaction.is_empty() {
            None
        } else {
            Some(Box::new(prepare_show_candidate(
                &document,
                planned.transaction,
            )?))
        };
        Ok(PreparedActiveMvrImport {
            show_id: envelope.command.show_id,
            source_show_revision,
            source_patch_revision,
            candidate,
            state: planned.state,
        })
    }

    /// Revalidates and commits one prepared import through the active show's ordered lifecycle.
    pub fn commit<P: ShowPatchPorts>(
        &self,
        prepared: PreparedActiveMvrImport,
        ports: &P,
    ) -> Result<ActiveMvrImportResult, ActionError> {
        let PreparedActiveMvrImport {
            show_id,
            source_show_revision,
            source_patch_revision,
            candidate,
            state,
        } = prepared;
        let context = state.context.clone();
        self.active_show.transact(
            &context,
            show_id,
            ports,
            "mvr-import",
            move |document| {
                validate_target(document, show_id, source_show_revision)?;
                Ok(match candidate {
                    Some(prepared) => {
                        PreparedActiveShowTransaction::PreparedCommit { prepared, state }
                    }
                    None => PreparedActiveShowTransaction::NoChange(state),
                })
            },
            move |events, ports, context, completed| {
                complete_import(
                    events,
                    ports,
                    context,
                    completed,
                    show_id,
                    source_show_revision,
                    source_patch_revision,
                )
            },
        )
    }

    pub fn apply<P: ShowPatchPorts>(
        &self,
        envelope: ActionEnvelope<ApplyActiveMvrImportCommand>,
        ports: &P,
    ) -> Result<ActiveMvrImportResult, ActionError> {
        let prepared = self.prepare(envelope, ports)?;
        self.commit(prepared, ports)
    }
}

impl Default for MvrImportService {
    fn default() -> Self {
        Self::new(ActiveShowService::default())
    }
}

fn validate_target(
    document: &light_show::PortableShowDocument,
    show_id: light_core::ShowId,
    expected: PortableShowRevision,
) -> Result<(), ActionError> {
    if document.id() != show_id {
        return Err(ActionError::new(
            ActionErrorKind::NotFound,
            "requested show is not active",
        ));
    }
    if document.revision() == expected {
        Ok(())
    } else {
        Err(ActionError::new(
            ActionErrorKind::Conflict,
            "active show changed while the MVR import was being prepared",
        )
        .at_revision(document.revision().value()))
    }
}

fn complete_import<P: ShowPatchPorts>(
    events: &EventBus,
    ports: &P,
    context: &crate::ActionContext,
    completed: CompletedActiveShowTransaction<PreparedMvrImportState>,
    show_id: light_core::ShowId,
    source_show_revision: PortableShowRevision,
    source_patch_revision: PortablePatchRevision,
) -> ActiveMvrImportResult {
    let changed = completed.commit.is_some();
    let show_revision = completed
        .commit
        .as_ref()
        .map_or(source_show_revision, PortableShowCommit::revision);
    let patch_revision = completed
        .commit
        .as_ref()
        .map_or(source_patch_revision, PortableShowCommit::patch_revision);
    let change = patch_change(
        show_id,
        show_revision,
        patch_revision,
        &completed.state.patch,
        completed.commit.as_ref(),
    );
    let event_sequence = changed.then(|| {
        ports.reconcile_patch_change(&change);
        events
            .publish(EventDraft::patch_changed(context, change.clone()))
            .sequence
    });
    ActiveMvrImportResult {
        context: completed.state.context,
        changed,
        show_revision,
        patch_revision,
        imported_fixtures: completed.state.imported_fixtures,
        unresolved_fixtures: completed.state.unresolved_fixtures,
        warnings: completed.state.warnings,
        change,
        event_sequence,
    }
}

fn patch_change(
    show_id: light_core::ShowId,
    show_revision: PortableShowRevision,
    patch_revision: PortablePatchRevision,
    planned: &PlannedPatchChange,
    commit: Option<&PortableShowCommit>,
) -> PatchChange {
    let fixtures = planned
        .fixtures
        .iter()
        .map(|fixture| PatchFixtureProjection {
            fixture_revision: commit
                .and_then(|commit| {
                    commit
                        .written_object("patched_fixture", &fixture.patch.fixture_id.0.to_string())
                })
                .map_or(0, |object| object.revision()),
            profile: fixture.profile,
            patch: fixture.patch.clone(),
        })
        .collect();
    PatchChange {
        show_id,
        show_revision,
        patch_revision,
        fixtures,
        removed_fixture_ids: planned.removed_fixture_ids.clone(),
        profile_revisions: planned.profiles.clone(),
    }
}
