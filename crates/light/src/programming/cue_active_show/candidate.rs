use super::{PreparedRecording, target::ResolvedCueTarget};
use crate::active_show::PreparedActiveShowTransaction;
use crate::{
    ActionError, ActionErrorKind, ActiveShowObjectKind, ProgrammingCueCommit,
    ProgrammingCueCommitResult, ProgrammingCueObjectProjection, ProgrammingCueProjections,
    ProgrammingRecordedCue, lossless_json, prepare_show_candidate,
};
use light_core::CueListId;
use light_playback::{CueListRecordingPlan, PlaybackDefinition, PlaybackPage};
use light_show::PortableShowDocument;
use std::collections::HashMap;
use std::sync::Arc;

// @tour cue-tracking-and-goto:10 Prepare the complete portable mutation
// Cue recording constructs every affected Cuelist, Playback, and page body in one candidate.
// Planning or compilation failure leaves the current portable show and runtime untouched.
pub(super) fn prepare_candidate(
    document: &PortableShowDocument,
    commit: &ProgrammingCueCommit,
    mut target: ResolvedCueTarget,
) -> Result<PreparedActiveShowTransaction<PreparedRecording>, ActionError> {
    let (mut plan, cue_list_id) = plan_recording(commit, &target)?;
    let catalog = crate::cuelist_pool_catalog(document)?;
    // Playback positions are never Cuelist addresses in the unified model.
    let independent = target.concrete_playback_address.is_some()
        || target.pool_number.is_some()
        || matches!(document.programming_contract_marker(), light_show::ProgrammingContractMarker::Declared(version) if version >= light_core::programming::INDEPENDENT_CUELIST_POOL_CONTRACT);
    if let Some(number) = target.pool_number {
        if target.cue_list.is_some() {
            catalog.migrate(&mut plan.cue_list).map_err(invalid)?;
        } else {
            plan.cue_list.pool_number = Some(number);
        }
        plan.changed |= target
            .cue_list
            .as_ref()
            .is_none_or(|stored| stored.typed != plan.cue_list);
    }
    if independent && target.pool_number.is_none() {
        if target.cue_list.is_some() {
            catalog.migrate(&mut plan.cue_list).map_err(invalid)?;
        } else {
            plan.cue_list.pool_number = Some(
                (1..=1000)
                    .find(|number| catalog.resolve(*number).is_none())
                    .ok_or_else(|| invalid("no free Cuelist address is available"))?,
            );
        }
        plan.changed |= target
            .cue_list
            .as_ref()
            .is_none_or(|stored| stored.typed != plan.cue_list);
    }
    if !plan.changed && !target.creates_topology() {
        return Ok(no_change(document, &target, plan));
    }
    let cue_list = cue_list_body(&target, &plan)?;
    let page = page_body(&mut target, cue_list_id)?;
    let mut transaction = document.transaction();
    let companions = if independent {
        crate::cuelist_pool::migrate_cuelist_pool(
            document,
            &mut transaction,
            &catalog,
            cue_list_id,
        )?
    } else {
        Vec::new()
    };
    transaction.put("cue_list", cue_list.object_id.clone(), cue_list.raw_body);
    if let Some(page) = &page {
        transaction.put(
            "playback_page",
            page.object_id.clone(),
            page.raw_body.clone(),
        );
    }
    let prepared = prepare_show_candidate(document, transaction)?;
    let result = candidate_result(
        document,
        &prepared,
        commit,
        &target,
        &plan,
        CandidateObjectIds {
            cue_list: cue_list.object_id,
            page: page.as_ref().map(|body| body.object_id.as_str()),
        },
    )?;
    let candidate = document
        .candidate(prepared.transaction())
        .map_err(invalid)?;
    validate_recorded_cuelist_catalog(&candidate)?;
    let companion_projections = companions
        .iter()
        .map(|id| candidate_projection(&candidate, ActiveShowObjectKind::CueList, id))
        .collect::<Result<Vec<_>, _>>()?;
    let mut changed_kinds = vec![ActiveShowObjectKind::CueList];
    if page.is_some() {
        changed_kinds.push(ActiveShowObjectKind::PlaybackPage);
    }
    Ok(PreparedActiveShowTransaction::PreparedCommit {
        prepared: Box::new(prepared),
        state: PreparedRecording {
            result,
            changed_kinds,
            companion_projections,
        },
    })
}

struct PendingBody {
    object_id: String,
    raw_body: serde_json::Value,
}

struct CandidateObjectIds<'a> {
    cue_list: String,
    page: Option<&'a str>,
}

fn plan_recording(
    commit: &ProgrammingCueCommit,
    target: &ResolvedCueTarget,
) -> Result<(CueListRecordingPlan, CueListId), ActionError> {
    match &target.cue_list {
        Some(cue_list) => commit
            .plan_existing(&cue_list.typed)
            .map(|plan| (plan, cue_list.typed.id))
            .map_err(plan_error),
        None => {
            let playback = target
                .pool_number
                .or(target
                    .concrete_playback_address
                    .map(|address| address.position()))
                .ok_or_else(invalid_topology)?;
            let id = CueListId::new();
            commit
                .plan_new(id, format!("Cuelist {playback}"))
                .map(|plan| (plan, id))
                .map_err(plan_error)
        }
    }
}

fn cue_list_body(
    target: &ResolvedCueTarget,
    plan: &CueListRecordingPlan,
) -> Result<PendingBody, ActionError> {
    match &target.cue_list {
        Some(stored) => Ok(PendingBody {
            object_id: stored.object_id.clone(),
            raw_body: lossless_json::merge_typed(&stored.raw_body, &stored.typed, &plan.cue_list)
                .map_err(invalid)?,
        }),
        None => Ok(PendingBody {
            object_id: plan.cue_list.id.0.to_string(),
            raw_body: serde_json::to_value(&plan.cue_list).map_err(invalid)?,
        }),
    }
}

fn page_body(
    target: &mut ResolvedCueTarget,
    cue_list_id: CueListId,
) -> Result<Option<PendingBody>, ActionError> {
    if !target.creates_topology() {
        return Ok(None);
    }
    let Some(address) = target.concrete_playback_address else {
        return Ok(None);
    };
    let definition = PlaybackDefinition::new_cue_list(
        format!("Cuelist {}.{}", address.page(), address.position()),
        cue_list_id,
    );
    match &target.page {
        Some(stored) => {
            let mut page = stored.typed.clone();
            page.assignments.insert(address.position(), definition);
            Ok(Some(PendingBody {
                object_id: stored.object_id.clone(),
                raw_body: lossless_json::merge_typed(&stored.raw_body, &stored.typed, &page)
                    .map_err(invalid)?,
            }))
        }
        None => {
            let page = PlaybackPage {
                number: address.page(),
                name: format!("Page {}", address.page()),
                assignments: HashMap::from([(address.position(), definition)]),
            };
            Ok(Some(PendingBody {
                object_id: address.page().to_string(),
                raw_body: serde_json::to_value(page).map_err(invalid)?,
            }))
        }
    }
}

fn no_change(
    document: &PortableShowDocument,
    target: &ResolvedCueTarget,
    plan: CueListRecordingPlan,
) -> PreparedActiveShowTransaction<PreparedRecording> {
    let cue_list = target
        .cue_list
        .as_ref()
        .expect("verified no-change has an existing Cuelist");
    PreparedActiveShowTransaction::NoChange(PreparedRecording {
        result: ProgrammingCueCommitResult {
            changed: false,
            created_topology: false,
            projections: ProgrammingCueProjections {
                show_id: document.id(),
                cue_list: stored_projection(ActiveShowObjectKind::CueList, cue_list),
                page: target
                    .page
                    .as_ref()
                    .map(|stored| stored_projection(ActiveShowObjectKind::PlaybackPage, stored)),
            },
            recorded_cue: recorded_cue(&plan),
            show_revision: document.revision(),
            event_sequence: None,
            concrete_playback_address: target.concrete_playback_address,
        },
        changed_kinds: Vec::new(),
        companion_projections: Vec::new(),
    })
}

fn candidate_result(
    document: &PortableShowDocument,
    prepared: &crate::PreparedShowCandidate,
    commit: &ProgrammingCueCommit,
    target: &ResolvedCueTarget,
    plan: &CueListRecordingPlan,
    ids: CandidateObjectIds<'_>,
) -> Result<ProgrammingCueCommitResult, ActionError> {
    let candidate = document
        .candidate(prepared.transaction())
        .map_err(invalid)?;
    let page_id = ids
        .page
        .or_else(|| target.page.as_ref().map(|item| item.object_id.as_str()));
    Ok(ProgrammingCueCommitResult {
        changed: true,
        created_topology: target.creates_topology(),
        projections: ProgrammingCueProjections {
            show_id: commit.show_id,
            cue_list: candidate_projection(
                &candidate,
                ActiveShowObjectKind::CueList,
                &ids.cue_list,
            )?,
            page: page_id
                .map(|id| candidate_projection(&candidate, ActiveShowObjectKind::PlaybackPage, id))
                .transpose()?,
        },
        recorded_cue: recorded_cue(plan),
        show_revision: candidate.revision(),
        event_sequence: None,
        concrete_playback_address: target.concrete_playback_address,
    })
}

fn candidate_projection(
    candidate: &light_show::PortableShowCandidate<'_>,
    kind: ActiveShowObjectKind,
    object_id: &str,
) -> Result<ProgrammingCueObjectProjection, ActionError> {
    let object = candidate
        .object(kind.as_str(), object_id)
        .ok_or_else(invalid_topology)?;
    Ok(ProgrammingCueObjectProjection {
        kind,
        object_id: object_id.to_owned(),
        object_revision: object.revision(),
        raw_body: Arc::new(object.body().clone()),
    })
}

fn stored_projection<T>(
    kind: ActiveShowObjectKind,
    stored: &super::target::Stored<T>,
) -> ProgrammingCueObjectProjection {
    ProgrammingCueObjectProjection {
        kind,
        object_id: stored.object_id.clone(),
        object_revision: stored.object_revision,
        raw_body: Arc::new(stored.raw_body.clone()),
    }
}

fn recorded_cue(plan: &CueListRecordingPlan) -> ProgrammingRecordedCue {
    ProgrammingRecordedCue {
        id: plan.cue_id,
        number: plan.cue_number.clone(),
        deleted: plan.deleted,
    }
}

fn plan_error(error: light_playback::CueRecordingPlanError) -> ActionError {
    let kind = match error {
        light_playback::CueRecordingPlanError::CueDoesNotExist { .. }
        | light_playback::CueRecordingPlanError::ActiveCueDoesNotExist { .. } => {
            ActionErrorKind::NotFound
        }
        light_playback::CueRecordingPlanError::CannotDeleteOnlyCue
        | light_playback::CueRecordingPlanError::CueAlreadyExists { .. } => {
            ActionErrorKind::Conflict
        }
        _ => ActionErrorKind::Invalid,
    };
    ActionError::new(kind, error.to_string())
}

fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}

fn invalid_topology() -> ActionError {
    ActionError::new(
        ActionErrorKind::Internal,
        "Cue recording produced an inconsistent portable topology",
    )
}

// Check the complete numbered namespace, including unnumbered assignment-derived entries,
// before any backup, commit or runtime installation can accept this recording.
fn validate_recorded_cuelist_catalog(
    candidate: &light_show::PortableShowCandidate<'_>,
) -> Result<(), ActionError> {
    let lists = candidate
        .objects_of_kind("cue_list")
        .map(|object| {
            serde_json::from_value::<light_playback::CueList>(object.body().clone())
                .map_err(invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut definitions = Vec::new();
    for object in candidate.objects_of_kind("playback_page") {
        let page: PlaybackPage = serde_json::from_value(object.body().clone()).map_err(invalid)?;
        for (position, definition) in page.assignments {
            definitions.push((
                light_playback::PlaybackAddress::new(page.number, position).map_err(invalid)?,
                definition,
            ));
        }
    }
    light_playback::CueListPoolCatalog::build(
        &lists,
        &definitions,
        candidate.objects_of_kind("playback_page").next().is_none(),
    )
    .map(|_| ())
    .map_err(invalid)
}
