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

pub(super) fn prepare_candidate(
    document: &PortableShowDocument,
    commit: &ProgrammingCueCommit,
    mut target: ResolvedCueTarget,
) -> Result<PreparedActiveShowTransaction<PreparedRecording>, ActionError> {
    let (plan, cue_list_id) = plan_recording(commit, &target)?;
    if !plan.changed && !target.creates_topology() {
        return Ok(no_change(document, &target, plan));
    }
    let cue_list = cue_list_body(&target, &plan)?;
    let playback = playback_body(&target, cue_list_id)?;
    let page = page_body(&mut target)?;
    let mut transaction = document.transaction();
    transaction.put("cue_list", cue_list.object_id.clone(), cue_list.raw_body);
    if let Some(playback) = &playback {
        transaction.put(
            "playback",
            playback.object_id.clone(),
            playback.raw_body.clone(),
        );
    }
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
            playback: playback.as_ref().map(|body| body.object_id.as_str()),
            page: page.as_ref().map(|body| body.object_id.as_str()),
        },
    )?;
    let mut changed_kinds = vec![ActiveShowObjectKind::CueList];
    if playback.is_some() {
        changed_kinds.push(ActiveShowObjectKind::Playback);
    }
    if page.is_some() {
        changed_kinds.push(ActiveShowObjectKind::PlaybackPage);
    }
    Ok(PreparedActiveShowTransaction::PreparedCommit {
        prepared: Box::new(prepared),
        state: PreparedRecording {
            result,
            changed_kinds,
        },
    })
}

struct PendingBody {
    object_id: String,
    raw_body: serde_json::Value,
}

struct CandidateObjectIds<'a> {
    cue_list: String,
    playback: Option<&'a str>,
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
                .concrete_playback_number
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

fn playback_body(
    target: &ResolvedCueTarget,
    cue_list_id: CueListId,
) -> Result<Option<PendingBody>, ActionError> {
    if target.playback.is_some() {
        return Ok(None);
    }
    let Some(number) = target.concrete_playback_number else {
        return Ok(None);
    };
    let name = format!("Cuelist {number}");
    let playback = PlaybackDefinition::new_cue_list(number, name, cue_list_id);
    Ok(Some(PendingBody {
        object_id: number.to_string(),
        raw_body: serde_json::to_value(playback).map_err(invalid)?,
    }))
}

fn page_body(target: &mut ResolvedCueTarget) -> Result<Option<PendingBody>, ActionError> {
    if !target.creates_topology() {
        return Ok(None);
    }
    let Some(slot) = target.page_slot else {
        return Ok(None);
    };
    let playback = target
        .concrete_playback_number
        .ok_or_else(invalid_topology)?;
    match &target.page {
        Some(stored) => {
            let mut page = stored.typed.clone();
            page.slots.insert(slot.slot, playback);
            Ok(Some(PendingBody {
                object_id: stored.object_id.clone(),
                raw_body: lossless_json::merge_typed(&stored.raw_body, &stored.typed, &page)
                    .map_err(invalid)?,
            }))
        }
        None => {
            let page = PlaybackPage {
                number: slot.page,
                name: format!("Page {}", slot.page),
                slots: HashMap::from([(slot.slot, playback)]),
            };
            Ok(Some(PendingBody {
                object_id: slot.page.to_string(),
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
            projections: ProgrammingCueProjections {
                show_id: document.id(),
                cue_list: stored_projection(ActiveShowObjectKind::CueList, cue_list),
                playback: target
                    .playback
                    .as_ref()
                    .map(|stored| stored_projection(ActiveShowObjectKind::Playback, stored)),
                page: target
                    .page
                    .as_ref()
                    .map(|stored| stored_projection(ActiveShowObjectKind::PlaybackPage, stored)),
            },
            recorded_cue: recorded_cue(&plan),
            show_revision: document.revision(),
            event_sequence: None,
            concrete_playback_number: target.concrete_playback_number,
        },
        changed_kinds: Vec::new(),
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
    let playback_id = ids
        .playback
        .or_else(|| target.playback.as_ref().map(|item| item.object_id.as_str()));
    let page_id = ids
        .page
        .or_else(|| target.page.as_ref().map(|item| item.object_id.as_str()));
    Ok(ProgrammingCueCommitResult {
        changed: true,
        projections: ProgrammingCueProjections {
            show_id: commit.show_id,
            cue_list: candidate_projection(
                &candidate,
                ActiveShowObjectKind::CueList,
                &ids.cue_list,
            )?,
            playback: playback_id
                .map(|id| candidate_projection(&candidate, ActiveShowObjectKind::Playback, id))
                .transpose()?,
            page: page_id
                .map(|id| candidate_projection(&candidate, ActiveShowObjectKind::PlaybackPage, id))
                .transpose()?,
        },
        recorded_cue: recorded_cue(plan),
        show_revision: candidate.revision(),
        event_sequence: None,
        concrete_playback_number: target.concrete_playback_number,
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
        number: crate::CueNumber::new(plan.cue_number),
        deleted: plan.deleted,
    }
}

fn plan_error(error: light_playback::CueRecordingPlanError) -> ActionError {
    let kind = match error {
        light_playback::CueRecordingPlanError::CueDoesNotExist { .. }
        | light_playback::CueRecordingPlanError::ActiveCueDoesNotExist { .. } => {
            ActionErrorKind::NotFound
        }
        light_playback::CueRecordingPlanError::CannotDeleteOnlyCue => ActionErrorKind::Conflict,
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
