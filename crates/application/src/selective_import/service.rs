use super::{
    AppliedImportObject, ApplySelectiveShowImportCommand, ImportObjectAction, ImportProfileKey,
    SelectiveShowImportChange, SelectiveShowImportPorts, SelectiveShowImportPreview,
    SelectiveShowImportRequest, SelectiveShowImportResult, SelectiveShowObjectChange,
    SelectiveShowProfileChange,
    plan::{ImportPlan, build_plan},
};
use crate::active_show::{CompletedActiveShowTransaction, PreparedActiveShowTransaction};
use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActiveShowService, EventBus,
    EventDraft, prepare_show_candidate,
};
use light_show::{PortableShowDocument, PortableShowTransaction};
use std::{cell::RefCell, collections::BTreeSet};

/// Lossless, dependency-aware copying between a source show and the active show.
///
/// Import intentionally shares [`ActiveShowService`]'s ordering gate with every other active-show
/// mutation. A caller that also exposes ordinary object edits must inject the same service clone.
#[derive(Clone, Default)]
pub struct SelectiveShowImportService {
    active_show: ActiveShowService,
}

impl SelectiveShowImportService {
    pub fn new(active_show: ActiveShowService) -> Self {
        Self { active_show }
    }

    pub fn preview<P: SelectiveShowImportPorts>(
        &self,
        context: &ActionContext,
        request: SelectiveShowImportRequest,
        ports: &P,
    ) -> Result<SelectiveShowImportPreview, ActionError> {
        let source_snapshot = ports.open_import_source_snapshot(context, request.source_show_id)?;
        let source = ports.import_source_document(&source_snapshot);
        let target = self
            .active_show
            .snapshot(context, request.target_show_id, ports)?;
        validate_documents(&request, source, &target)?;
        let mut plan = build_plan(&request, &source_snapshot, source, &target, ports);
        preflight_plan(&target, &mut plan);
        Ok(plan.preview)
    }

    pub fn apply<P: SelectiveShowImportPorts>(
        &self,
        envelope: ActionEnvelope<ApplySelectiveShowImportCommand>,
        ports: &P,
    ) -> Result<SelectiveShowImportResult, ActionError> {
        let request = envelope.command.request.clone();
        let source_snapshot =
            ports.open_import_source_snapshot(&envelope.context, request.source_show_id)?;
        let source = ports.import_source_document(&source_snapshot);
        let target = self
            .active_show
            .snapshot(&envelope.context, request.target_show_id, ports)?;
        validate_documents(&request, source, &target)?;
        validate_revisions(&envelope.context, &envelope.command, source, &target)?;
        let mut plan = build_plan(&request, &source_snapshot, source, &target, ports);
        preflight_plan(&target, &mut plan);
        validate_plan(&plan)?;
        let prepared_assets = RefCell::new(prepare_assets(
            &envelope.context,
            &request,
            &source_snapshot,
            &plan,
            ports,
        )?);
        let prepared_assets_match = {
            let prepared_assets = prepared_assets.borrow();
            prepared_assets.as_ref().is_none_or(|prepared| {
                same_assets(ports.prepared_import_assets(prepared), &plan.asset_copies)
            })
        };
        if !prepared_assets_match {
            let error =
                invalid("managed-asset adapter prepared a different revision set than planned");
            return Err(compensate(error, prepared_assets.into_inner(), ports));
        }
        let result = self.active_show.transact(
            &envelope.context,
            request.target_show_id,
            ports,
            "selective-import",
            |target| {
                validate_documents(&request, source, target)?;
                validate_revisions(&envelope.context, &envelope.command, source, target)?;
                let state = PreparedImportState { plan };
                if state.plan.writes.is_empty() && state.plan.profiles.is_empty() {
                    return Ok(PreparedActiveShowTransaction::NoChange(state));
                }
                let transaction = stage_transaction(target, &state.plan)?;
                let prepared = prepare_show_candidate(target, transaction)?;
                Ok(PreparedActiveShowTransaction::PreparedCommit {
                    prepared: Box::new(prepared),
                    state,
                })
            },
            |events, ports, context, completed| {
                complete_import(events, ports, context, completed, &prepared_assets)
            },
        );
        match result {
            Ok(result) => Ok(result),
            Err(error) => Err(compensate(error, prepared_assets.into_inner(), ports)),
        }
    }
}

struct PreparedImportState {
    plan: ImportPlan,
}

fn complete_import<P: SelectiveShowImportPorts>(
    events: &EventBus,
    ports: &P,
    context: &ActionContext,
    completed: CompletedActiveShowTransaction<PreparedImportState>,
    prepared_assets: &RefCell<Option<P::PreparedImportAssets>>,
) -> SelectiveShowImportResult {
    let prepared_batch = prepared_assets.borrow_mut().take();
    let imported_assets = prepared_batch
        .map(|prepared| {
            ports.publish_import_assets(prepared);
            completed.state.plan.asset_copies.clone()
        })
        .unwrap_or_default();
    let outcomes = applied_outcomes(&completed.state.plan);
    let changed = completed.commit.is_some() || !imported_assets.is_empty();
    let show_revision = completed
        .commit
        .as_ref()
        .map_or(completed.state.plan.preview.target_revision, |commit| {
            commit.revision()
        });
    let objects = completed
        .commit
        .as_ref()
        .into_iter()
        .flat_map(|commit| commit.written_objects())
        .map(|object| SelectiveShowObjectChange {
            key: object.key().clone(),
            object_revision: object.revision(),
            body: object.body().clone(),
        })
        .collect();
    let profiles = completed
        .commit
        .as_ref()
        .into_iter()
        .flat_map(|commit| commit.fixture_profile_revisions())
        .map(|profile| {
            let destination = ImportProfileKey {
                profile_id: profile.id().profile_id(),
                revision: profile.id().revision(),
            };
            let source = completed
                .state
                .plan
                .profile_map
                .iter()
                .find_map(|(source, mapped)| (*mapped == destination).then_some(*source))
                .unwrap_or(destination);
            SelectiveShowProfileChange {
                source,
                destination,
                digest: profile.digest().as_str().to_owned(),
            }
        })
        .collect();
    let change = SelectiveShowImportChange {
        show_id: completed.state.plan.preview.request.target_show_id,
        show_revision,
        outcomes,
        objects,
        profiles,
        managed_assets: imported_assets,
    };
    let event_sequence = if changed {
        ports.reconcile_selective_import(&change);
        Some(
            events
                .publish(EventDraft::selective_import_applied(
                    context,
                    change.clone(),
                ))
                .sequence,
        )
    } else {
        None
    };
    SelectiveShowImportResult {
        context: context.clone(),
        changed,
        change,
        event_sequence,
    }
}

fn prepare_assets<P: SelectiveShowImportPorts>(
    context: &ActionContext,
    request: &SelectiveShowImportRequest,
    source: &P::ImportSourceSnapshot,
    plan: &ImportPlan,
    ports: &P,
) -> Result<Option<P::PreparedImportAssets>, ActionError> {
    if plan.asset_copies.is_empty() {
        return Ok(None);
    }
    ports
        .prepare_import_assets(context, source, request.target_show_id, &plan.asset_copies)
        .map(Some)
}

fn same_assets(left: &[crate::AssetReference], right: &[crate::AssetReference]) -> bool {
    left.iter().copied().collect::<BTreeSet<_>>() == right.iter().copied().collect::<BTreeSet<_>>()
        && left.len() == right.len()
}

fn compensate<P: SelectiveShowImportPorts>(
    mut error: ActionError,
    prepared: Option<P::PreparedImportAssets>,
    ports: &P,
) -> ActionError {
    if let Some(prepared) = prepared
        && let Err(compensation) = ports.compensate_import_assets(prepared)
    {
        error.message = format!(
            "{}; managed-asset compensation also failed: {}",
            error.message, compensation.message
        );
    }
    error
}

pub(super) fn stage_transaction(
    document: &PortableShowDocument,
    plan: &ImportPlan,
) -> Result<PortableShowTransaction, ActionError> {
    let mut transaction = document.transaction();
    let patch_changed = plan
        .writes
        .iter()
        .any(|write| write.destination.kind() == "patched_fixture");
    for write in &plan.writes {
        transaction.put(
            write.destination.kind(),
            write.destination.id(),
            write.body.clone(),
        );
    }
    for profile in &plan.profiles {
        transaction
            .put_fixture_profile_revision(profile.clone())
            .map_err(|error| invalid(error.to_string()))?;
    }
    if patch_changed {
        transaction.mark_patch_changed();
    }
    Ok(transaction)
}

fn applied_outcomes(plan: &ImportPlan) -> Vec<AppliedImportObject> {
    plan.preview
        .objects
        .iter()
        .filter(|object| !matches!(object.action, ImportObjectAction::BlockedConflict))
        .cloned()
        .collect()
}

fn validate_documents(
    request: &SelectiveShowImportRequest,
    source: &PortableShowDocument,
    target: &PortableShowDocument,
) -> Result<(), ActionError> {
    if source.id() != request.source_show_id {
        return Err(not_found("loaded source show identity changed"));
    }
    if target.id() != request.target_show_id {
        return Err(not_found("requested target show is not active"));
    }
    Ok(())
}

fn validate_revisions(
    context: &ActionContext,
    command: &ApplySelectiveShowImportCommand,
    source: &PortableShowDocument,
    target: &PortableShowDocument,
) -> Result<(), ActionError> {
    if source.revision() != command.expected_source_revision {
        return Err(conflict(
            "source show changed after preview",
            source.revision().value(),
        ));
    }
    if target.revision() != command.expected_target_revision {
        return Err(conflict(
            "active show changed after preview",
            target.revision().value(),
        ));
    }
    if context
        .expected_revision
        .is_some_and(|expected| expected != target.revision().value())
    {
        return Err(conflict(
            "action revision does not match the active show",
            target.revision().value(),
        ));
    }
    Ok(())
}

fn validate_plan(plan: &ImportPlan) -> Result<(), ActionError> {
    if plan.preview.can_apply() {
        return Ok(());
    }
    Err(ActionError::new(
        ActionErrorKind::Conflict,
        format!(
            "selective import has {} unresolved blocker(s): {:?}",
            plan.preview.blockers.len(),
            plan.preview.blockers
        ),
    )
    .at_revision(plan.preview.target_revision.value()))
}

fn preflight_plan(document: &PortableShowDocument, plan: &mut ImportPlan) {
    if !plan.preview.can_apply() || (plan.writes.is_empty() && plan.profiles.is_empty()) {
        return;
    }
    let result = stage_transaction(document, plan)
        .and_then(|transaction| prepare_show_candidate(document, transaction))
        .and_then(|prepared| {
            let (_, snapshot) = prepared.into_parts();
            snapshot
                .validate()
                .map_err(|error| invalid(error.to_string()))
        });
    if let Err(error) = result {
        plan.preview
            .blockers
            .push(super::ImportBlocker::CandidateInvalid {
                message: error.message,
            });
    }
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}

fn conflict(message: impl Into<String>, revision: u64) -> ActionError {
    ActionError::new(ActionErrorKind::Conflict, message).at_revision(revision)
}
