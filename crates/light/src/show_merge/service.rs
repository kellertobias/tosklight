//! Previews and applies a merge against the open show through the ordered active-show gate.

use super::{MergeInputs, MergeObjects, MergeOutcome, MergeResolutions, merge_shows};
use crate::active_show::PreparedActiveShowTransaction;
use crate::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowService, EventDraft,
    SelectiveShowImportChange, SelectiveShowImportPorts, SelectiveShowImportResult,
    SelectiveShowImportUndoObject, SelectiveShowImportUndoTarget, SelectiveShowObjectChange,
    prepare_show_candidate,
};
use light_core::ShowId;
use light_show::{FixtureProfileRevision, PortableShowDocument};
use std::collections::BTreeMap;

/// The offline copy as the merge needs it: its objects, the objects it started from, and the
/// fixture profiles its patch may use.
#[derive(Clone, Debug, Default)]
pub struct OfflineCopy {
    pub base: MergeObjects,
    pub objects: MergeObjects,
    pub profiles: Vec<FixtureProfileRevision>,
}

/// A merge preview and the show revision it was computed against.
#[derive(Clone, Debug)]
pub struct ShowMergePreview {
    pub target_revision: u64,
    pub outcome: MergeOutcome,
}

#[derive(Clone, Default)]
pub struct ShowMergeService {
    active_show: ActiveShowService,
}

impl ShowMergeService {
    pub fn new(active_show: ActiveShowService) -> Self {
        Self { active_show }
    }

    /// Gives every Group and Preset of the open show its instance identity, so the copy about
    /// to be taken and the show it stays with agree on which object is which.
    pub fn prepare_for_offline_copy<P: SelectiveShowImportPorts>(
        &self,
        context: &ActionContext,
        show_id: ShowId,
        ports: &P,
    ) -> Result<u64, ActionError> {
        self.active_show.transact(
            context,
            show_id,
            ports,
            "offline-copy-identities",
            |document| {
                let mut transaction = document.transaction();
                crate::show_compiler::stage_instance_identities(document, &mut transaction)?;
                if transaction.is_empty() {
                    return Ok(PreparedActiveShowTransaction::NoChange(
                        document.revision().value(),
                    ));
                }
                let prepared = prepare_show_candidate(document, transaction)?;
                Ok(PreparedActiveShowTransaction::PreparedCommit {
                    prepared: Box::new(prepared),
                    state: 0,
                })
            },
            |_, _, _, completed| {
                completed
                    .commit
                    .map_or(completed.state, |commit| commit.revision().value())
            },
        )
    }

    pub fn preview<P: SelectiveShowImportPorts>(
        &self,
        context: &ActionContext,
        show_id: ShowId,
        copy: &OfflineCopy,
        resolutions: &MergeResolutions,
        ports: &P,
    ) -> Result<ShowMergePreview, ActionError> {
        let target = self.active_show.snapshot(context, show_id, ports)?;
        Ok(ShowMergePreview {
            target_revision: target.revision().value(),
            outcome: merge_shows(&inputs(&target, copy), resolutions),
        })
    }

    /// Applies the merge as one undoable change. Refuses while any conflict is undecided, and
    /// when the show changed since `expected_revision` was previewed.
    pub fn apply<P: SelectiveShowImportPorts>(
        &self,
        context: &ActionContext,
        show_id: ShowId,
        copy: &OfflineCopy,
        resolutions: &MergeResolutions,
        expected_revision: u64,
        ports: &P,
    ) -> Result<SelectiveShowImportResult, ActionError> {
        self.active_show.transact(
            context,
            show_id,
            ports,
            "show-merge",
            |target| {
                if target.revision().value() != expected_revision {
                    return Err(ActionError::new(
                        ActionErrorKind::Conflict,
                        "the show changed since the merge was previewed; preview it again",
                    )
                    .at_revision(target.revision().value()));
                }
                let outcome = merge_shows(&inputs(target, copy), resolutions);
                if let Some(conflict) = outcome.conflicts.first() {
                    return Err(ActionError::new(
                        ActionErrorKind::Invalid,
                        format!(
                            "{} conflicts still need a decision, starting with {} {}",
                            outcome.conflicts.len(),
                            conflict.key.kind(),
                            conflict.key.id()
                        ),
                    ));
                }
                let mut transaction = target.transaction();
                for (key, body) in &outcome.writes {
                    match body {
                        Some(body) => transaction.put(key.kind(), key.id(), body.clone()),
                        None => transaction.delete(key.kind(), key.id()),
                    };
                }
                if outcome
                    .writes
                    .keys()
                    .any(|key| key.kind() == "patched_fixture")
                {
                    transaction.mark_patch_changed();
                }
                for profile in &copy.profiles {
                    if target
                        .fixture_profile_revision(
                            profile.id().profile_id(),
                            profile.id().revision(),
                        )
                        .is_none()
                    {
                        transaction
                            .put_fixture_profile_revision(profile.clone())
                            .map_err(|error| {
                                ActionError::new(ActionErrorKind::Invalid, error.to_string())
                            })?;
                    }
                }
                if transaction.is_empty() {
                    return Ok(PreparedActiveShowTransaction::NoChange(BTreeMap::new()));
                }
                let prepared = prepare_show_candidate(target, transaction)?;
                let previous = prepared
                    .transaction()
                    .changed_object_keys()
                    .map(|key| {
                        let body = target
                            .object(key.kind(), key.id())
                            .map(|object| object.body().clone());
                        (key.clone(), body)
                    })
                    .collect::<BTreeMap<_, _>>();
                Ok(PreparedActiveShowTransaction::PreparedCommit {
                    prepared: Box::new(prepared),
                    state: previous,
                })
            },
            |events, ports, context, completed| {
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
                    .collect::<Vec<_>>();
                let undo = completed
                    .commit
                    .as_ref()
                    .map(|_| SelectiveShowImportUndoTarget {
                        show_id,
                        objects: objects
                            .iter()
                            .filter_map(|object| {
                                completed.state.get(&object.key).map(|previous| {
                                    SelectiveShowImportUndoObject {
                                        key: object.key.clone(),
                                        expected_object_revision: object.object_revision,
                                        previous_body: previous.clone(),
                                    }
                                })
                            })
                            .collect(),
                    });
                let show_revision = completed.commit.as_ref().map(|commit| commit.revision());
                let changed = show_revision.is_some();
                let change = SelectiveShowImportChange {
                    show_id,
                    show_revision: show_revision.unwrap_or_default(),
                    outcomes: Vec::new(),
                    objects,
                    profiles: Vec::new(),
                    managed_assets: Vec::new(),
                };
                if let Some(target) = undo.clone() {
                    ports.record_selective_import_undo(context, target);
                }
                let event_sequence = changed.then(|| {
                    ports.reconcile_selective_import(&change);
                    events
                        .publish(EventDraft::selective_import_applied(
                            context,
                            change.clone(),
                        ))
                        .sequence
                });
                SelectiveShowImportResult {
                    context: context.clone(),
                    changed,
                    change,
                    event_sequence,
                    undo,
                }
            },
        )
    }
}

fn inputs(target: &PortableShowDocument, copy: &OfflineCopy) -> MergeInputs {
    MergeInputs {
        base: copy.base.clone(),
        mine: target
            .objects()
            .map(|object| (object.key().clone(), object.body().clone()))
            .collect(),
        theirs: copy.objects.clone(),
    }
}
