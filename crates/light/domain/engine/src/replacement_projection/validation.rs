use super::*;
use crate::{EngineError, EngineSnapshot};
use light_core::{ReplacementProfileContext, ReplacementRuntimeMigration};

pub(crate) fn validate_migrations(
    previous: &EngineSnapshot,
    candidate: &EngineSnapshot,
    plans: &[ReplacementRuntimeMigration],
) -> Result<(), EngineError> {
    if plans.is_empty() {
        return Ok(());
    }
    let old_plan = ReplacementDestinationPlan::compile(&previous.fixtures)?;
    let new_plan = ReplacementDestinationPlan::compile(&candidate.fixtures)?;
    let mut owners = std::collections::HashSet::new();
    for plan in plans {
        if !owners.insert(plan.source_owner) {
            return Err(invalid(
                "replacement runtime contains duplicate physical owners",
            ));
        }
        let old = previous
            .fixtures
            .iter()
            .find(|fixture| fixture.fixture_id == plan.source_owner)
            .ok_or_else(|| invalid("replacement runtime source fixture no longer exists"))?;
        let new = candidate
            .fixtures
            .iter()
            .find(|fixture| fixture.fixture_id == plan.source_owner)
            .ok_or_else(|| invalid("replacement runtime destination fixture is missing"))?;
        if context(old)? != plan.source_profile || context(new)? != plan.target_profile {
            return Err(invalid(
                "replacement runtime profile context is stale; no captured sources were changed",
            ));
        }
        let old_mode = profile_mode(old)
            .ok_or_else(|| invalid("replacement runtime source mode is missing"))?;
        for (head_id, owner) in &plan.source_head_owners {
            let Some((index, head)) = old_mode
                .heads
                .iter()
                .enumerate()
                .find(|(_, head)| head.id == *head_id)
            else {
                return Err(invalid("replacement runtime source head is stale"));
            };
            if crate::fixture::profile_head_owner(old, index, head) != *owner {
                return Err(invalid(
                    "replacement runtime source head has a different physical owner",
                ));
            }
        }
        for attribute in &plan.root_attributes {
            let supported = old_mode
                .heads
                .iter()
                .filter(|head| head.master_shared)
                .any(|head| {
                    let source = ReplacementProgramProjection {
                        source_owner: plan.source_owner,
                        source_profile: plan.source_profile.clone(),
                        source_head_id: head.id,
                        target_profile: plan.source_profile.clone(),
                        targets: vec![light_core::ReplacementHeadTarget {
                            profile_head_id: head.id,
                            fixture_id: plan.source_owner,
                        }],
                    };
                    !old_plan.destinations(&source, attribute).is_empty()
                });
            if !supported {
                return Err(invalid(
                    "replacement runtime attribute does not belong to the old shared physical root",
                ));
            }
        }
        for (attribute, projection) in &plan.root_projections {
            projection
                .validate()
                .map_err(|error| invalid(error.to_string()))?;
            if projection.source_owner != plan.source_owner
                || projection.source_profile != plan.source_profile
                || projection.target_profile != plan.target_profile
                || !plan.root_attributes.contains(attribute)
                || !old_mode
                    .heads
                    .iter()
                    .any(|head| head.id == projection.source_head_id && head.master_shared)
            {
                return Err(invalid(
                    "replacement runtime consent has mismatched source context",
                ));
            }
            let old_projection = ReplacementProgramProjection {
                target_profile: plan.source_profile.clone(),
                targets: vec![light_core::ReplacementHeadTarget {
                    profile_head_id: projection.source_head_id,
                    fixture_id: plan.source_owner,
                }],
                ..projection.clone()
            };
            if old_plan.destinations(&old_projection, attribute).is_empty()
                || new_plan.destinations(projection, attribute).len() != projection.targets.len()
            {
                return Err(invalid(
                    "replacement runtime consent names an incompatible physical destination",
                ));
            }
        }
        for (owner, target) in &plan.head_targets {
            if !old
                .logical_heads
                .iter()
                .any(|head| head.fixture_id == *owner)
                || target.fixture_id != *owner
            {
                return Err(invalid(
                    "replacement runtime head mapping changed a stable logical owner",
                ));
            }
            let new_mode = profile_mode(new)
                .ok_or_else(|| invalid("replacement runtime target mode is missing"))?;
            let Some((index, head)) = new_mode
                .heads
                .iter()
                .enumerate()
                .find(|(_, head)| head.id == target.profile_head_id)
            else {
                return Err(invalid("replacement runtime target head is missing"));
            };
            if crate::fixture::profile_head_owner(new, index, head) != *owner {
                return Err(invalid(
                    "replacement runtime target has a different physical owner",
                ));
            }
        }
    }
    Ok(())
}

fn context(fixture: &PatchedFixture) -> Result<ReplacementProfileContext, EngineError> {
    let profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .ok_or_else(|| invalid("replacement runtime profile is unavailable"))?;
    let mode =
        profile_mode(fixture).ok_or_else(|| invalid("replacement runtime mode is unavailable"))?;
    Ok(ReplacementProfileContext {
        profile_id: profile.id,
        profile_revision: profile.revision.into(),
        mode_id: mode.id,
    })
}
fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::Invalid(message.into())
}
