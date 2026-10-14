//! Exact captured Cue ownership for legacy scalar Dynamic output. Clock samples remain
//! ordered at the original roots; only an existing operator-consented envelope moves output.
use super::*;

#[derive(Clone)]
pub(super) struct CueScalarContext {
    pub(super) source: light_playback::SequenceMasterSource,
    pub(super) changed_at: chrono::DateTime<chrono::Utc>,
    pub(super) ordinal: u64,
    pub(super) merge_mode: MergeMode,
    pub(super) authored_fixture: FixtureId,
    pub(super) projection: Option<light_core::ReplacementProgramProjection>,
}

impl CueScalarContext {
    pub(super) fn capture(
        row: &light_playback::ActiveCueDynamicValue,
        snapshot: &light_engine::EngineSnapshot,
    ) -> Option<Self> {
        let list = snapshot
            .cue_lists
            .iter()
            .find(|list| list.id == row.cue_list_id)?;
        let cue = list.cues.iter().find(|cue| cue.id == row.authored_cue_id)?;
        // Direct assignments take precedence. An explicit empty/dormant envelope is retained,
        // never converted into a fallback to the current root or arbitrary descendants.
        let projection = match cue
            .changes
            .iter()
            .find(|change| change.fixture_id == row.fixture_id && change.attribute == row.attribute)
        {
            Some(change) => change.replacement_projection.clone(),
            None => cue
                .group_changes
                .iter()
                .filter(|change| change.attribute == row.attribute)
                .find_map(|change| change.replacement_projections.get(&row.fixture_id).cloned()),
        };
        Some(Self {
            source: row.source,
            changed_at: row.changed_at,
            ordinal: row.transition_ordinal,
            merge_mode: if row.attribute.is_intensity()
                && list.intensity_priority_mode == light_playback::IntensityPriorityMode::Htp
            {
                MergeMode::Htp
            } else {
                MergeMode::Ltp
            },
            authored_fixture: row.fixture_id,
            projection,
        })
    }
}
