//! What a stored Cue leaves on stage, for pictures of it.
//!
//! A Cue preview shows the look a Cue produces, not the look the desk shows now. This folds a
//! Cuelist up to one Cue with Group references expanded exactly as playback expands them, and
//! renders one fixture's DMX slots for those values on a private engine, so a picture drawn by an
//! external renderer (a Media Server) sees exactly the bytes the desk would transmit.

use std::collections::{HashMap, HashSet};

use light_core::{AttributeKey, AttributeValue, CueListId, FixtureId, SessionId};
use light_fixture::PatchedFixture;
use light_programmer::ProgrammerRegistry;
use uuid::Uuid;

use crate::lifecycle::expand_group_references_for_preview;
use crate::{Engine, EngineError, EngineSnapshot, RenderOptions, group_stage_positions};

/// One tracked value a Cue leaves in place.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackedCueValue {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
}

/// A Cue's own content and the complete tracked state it leaves.
#[derive(Clone, Debug, PartialEq)]
pub struct CuePreviewState {
    /// Every fixture the Cue itself addresses, Group references expanded, in first-use order.
    pub addressed: Vec<FixtureId>,
    /// Every value that is tracked into this Cue, including values set by earlier Cues.
    pub tracked: Vec<TrackedCueValue>,
}

/// Folds `cue_list_id` up to and including `cue_id`.
///
/// Returns `None` when the Cuelist or Cue does not exist. A released value (`None` in a Cue)
/// removes the tracked value, exactly as playback does.
pub fn cue_preview_state(
    snapshot: &EngineSnapshot,
    cue_list_id: CueListId,
    cue_id: Uuid,
) -> Option<CuePreviewState> {
    let source = snapshot
        .cue_lists
        .iter()
        .find(|cue_list| cue_list.id == cue_list_id)?;
    let target = source.cues.iter().position(|cue| cue.id == cue_id)?;
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect();
    let expanded =
        expand_group_references_for_preview(source, &groups, &group_stage_positions(snapshot));
    let mut order: Vec<(FixtureId, AttributeKey)> = Vec::new();
    let mut values: HashMap<(FixtureId, AttributeKey), AttributeValue> = HashMap::new();
    for cue in &expanded.cues[..=target] {
        for change in &cue.changes {
            let key = (change.fixture_id, change.attribute.clone());
            match &change.value {
                Some(value) => {
                    if values.insert(key.clone(), value.clone()).is_none() {
                        order.push(key);
                    }
                }
                None => {
                    values.remove(&key);
                }
            }
        }
    }
    let mut seen = HashSet::new();
    let addressed = expanded.cues[target]
        .changes
        .iter()
        .map(|change| change.fixture_id)
        .filter(|fixture_id| seen.insert(*fixture_id))
        .collect();
    let tracked = order
        .into_iter()
        .filter_map(|key| {
            let value = values.remove(&key)?;
            Some(TrackedCueValue {
                fixture_id: key.0,
                attribute: key.1,
                value,
            })
        })
        .collect();
    Some(CuePreviewState { addressed, tracked })
}

/// The DMX slots `fixture` would emit for `values`, one byte per slot of its footprint.
///
/// Values for other fixtures are ignored; values for the fixture's logical heads are applied to
/// those heads. The fixture is rendered alone, at full Grand Master, unpatched from the live
/// universe, so nothing the desk currently outputs can leak into the result.
pub fn render_fixture_slots(
    fixture: &PatchedFixture,
    values: &[TrackedCueValue],
) -> Result<Vec<u8>, EngineError> {
    let footprint = usize::from(fixture.definition.footprint);
    if footprint == 0 || footprint > 512 {
        return Err(EngineError::Invalid(format!(
            "fixture footprint {footprint} cannot be rendered into one universe"
        )));
    }
    let mut isolated = fixture.clone();
    isolated.universe = Some(1);
    isolated.address = Some(1);
    isolated.split_patches = Vec::new();
    isolated.multipatch = Vec::new();
    isolated.group_masters_enabled = false;
    isolated.grand_master_enabled = true;
    isolated.move_in_black_enabled = false;
    isolated.freeze = Default::default();
    let owned: HashSet<FixtureId> = std::iter::once(fixture.fixture_id)
        .chain(fixture.logical_heads.iter().map(|head| head.fixture_id))
        .collect();

    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    for value in values
        .iter()
        .filter(|value| owned.contains(&value.fixture_id))
    {
        programmers.set(
            session,
            value.fixture_id,
            value.attribute.clone(),
            value.value.clone(),
        );
    }
    let engine = Engine::new(programmers);
    engine.replace_snapshot(EngineSnapshot {
        fixtures: vec![isolated].into(),
        ..Default::default()
    })?;
    let rendered = engine.render(RenderOptions::default())?;
    let slots = rendered
        .universes
        .get(&1)
        .map(|frame| frame[..footprint].to_vec())
        .unwrap_or_else(|| vec![0; footprint]);
    Ok(slots)
}
