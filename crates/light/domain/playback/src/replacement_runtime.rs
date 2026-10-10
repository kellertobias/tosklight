use crate::*;
use light_core::{ReplacementProgramProjection, ReplacementRuntimeMigration};

pub(crate) fn replacement_destinations(
    plan: &ReplacementRuntimeMigration,
    owner: FixtureId,
    attribute: &AttributeKey,
    existing: Option<&ReplacementProgramProjection>,
) -> Result<Vec<(FixtureId, Option<ReplacementProgramProjection>)>, String> {
    if existing.is_some_and(|value| {
        value.source_owner != plan.source_owner || value.target_profile != plan.source_profile
    }) {
        return Ok(vec![(owner, existing.cloned())]);
    }
    let projection = plan
        .project_existing(attribute, owner, existing)
        .map_err(|error| error.to_string())?;
    if owner == plan.source_owner && plan.root_attributes.contains(attribute) {
        if let Some(projection) = projection {
            projection.validate().map_err(|error| error.to_string())?;
            return Ok(projection
                .targets
                .iter()
                .map(|target| (target.fixture_id, Some(projection.clone())))
                .collect());
        }
    }
    if let Some(target) = plan.head_targets.get(&owner) {
        if projection.as_ref().is_some_and(|projection| {
            !projection
                .targets
                .iter()
                .any(|value| value.fixture_id == target.fixture_id)
        }) {
            return Ok(Vec::new());
        }
        return Ok(vec![(target.fixture_id, projection)]);
    }
    Ok(vec![(owner, projection)])
}

fn migrate_rows(
    rows: &[PlaybackRetainedValue],
    plan: &ReplacementRuntimeMigration,
) -> Result<Vec<PlaybackRetainedValue>, String> {
    let mut migrated = Vec::new();
    for row in rows {
        for (fixture, projection) in replacement_destinations(
            plan,
            row.timed.fixture_id,
            &row.timed.attribute,
            row.replacement_projection.as_ref(),
        )? {
            let mut value = row.clone();
            value.timed.fixture_id = fixture;
            value.replacement_projection = projection;
            // Unknown source evidence remains unknown, including captured physical starts.
            migrated.push(value);
        }
    }
    Ok(migrated)
}

impl PlaybackEngine {
    /// Retarget only already-captured samples on a detached, old-generation playback snapshot.
    /// The caller validates profile/mode ownership and explicit consent against both generations.
    pub fn apply_replacement_runtime_migration(
        &mut self,
        plan: &ReplacementRuntimeMigration,
    ) -> Result<(), String> {
        let mut active = self.active.clone();
        for playback in active.values_mut() {
            if let Some(rows) = &mut playback.deleted_cue_transition_source {
                *rows = migrate_rows(rows, plan)?;
            }
            if let Some(hold) = &mut playback.deleted_cue_hold {
                hold.contributions = migrate_rows(&hold.contributions, plan)?;
            }
            if let Some(history) = &mut playback.source_history {
                history.apply_replacement_runtime_migration(plan)?;
            }
        }
        self.active = active;
        Ok(())
    }
}
