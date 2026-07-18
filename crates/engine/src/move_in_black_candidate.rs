use crate::{EngineSnapshot, MoveInBlackKey, snapshot_attribute_is_snap};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_playback::MoveInBlackCandidate;
use std::collections::HashMap;

pub(crate) struct PreparedCandidate {
    pub(crate) key: MoveInBlackKey,
    pub(crate) candidate: MoveInBlackCandidate,
    pub(crate) enabled: bool,
    pub(crate) delay_millis: u64,
    pub(crate) base_position: HashMap<AttributeKey, AttributeValue>,
    pub(crate) resolved_intensity: f32,
}

impl PreparedCandidate {
    pub(crate) fn new(
        snapshot: &EngineSnapshot,
        mut candidate: MoveInBlackCandidate,
        base_resolved: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
    ) -> Self {
        apply_snap_attributes(snapshot, &mut candidate);
        let patch = snapshot.fixtures.iter().find(|fixture| {
            fixture.fixture_id == candidate.fixture_id
                || fixture
                    .logical_heads
                    .iter()
                    .any(|head| head.fixture_id == candidate.fixture_id)
        });
        Self {
            key: candidate_key(&candidate),
            enabled: patch.is_some_and(|fixture| fixture.move_in_black_enabled),
            delay_millis: patch
                .map(|fixture| fixture.move_in_black_delay_millis)
                .unwrap_or_default(),
            base_position: base_position(&candidate, base_resolved),
            resolved_intensity: resolved_intensity(candidate.fixture_id, base_resolved),
            candidate,
        }
    }
}

fn apply_snap_attributes(snapshot: &EngineSnapshot, candidate: &mut MoveInBlackCandidate) {
    for target in &mut candidate.values {
        if snapshot_attribute_is_snap(snapshot, candidate.fixture_id, &target.attribute) {
            target.fade_millis = 0;
        }
    }
}

fn candidate_key(candidate: &MoveInBlackCandidate) -> MoveInBlackKey {
    MoveInBlackKey {
        playback_number: candidate.playback_number,
        cue_list_id: candidate.cue_list_id,
        fixture_id: candidate.fixture_id,
    }
}

fn base_position(
    candidate: &MoveInBlackCandidate,
    base_resolved: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
) -> HashMap<AttributeKey, AttributeValue> {
    candidate
        .values
        .iter()
        .map(|value| {
            let key = (candidate.fixture_id, value.attribute.clone());
            let base = base_resolved
                .get(&key)
                .cloned()
                .unwrap_or_else(|| value.current.clone());
            (value.attribute.clone(), base)
        })
        .collect()
}

fn resolved_intensity(
    fixture_id: FixtureId,
    base_resolved: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
) -> f32 {
    base_resolved
        .iter()
        .filter(|((owner, attribute), _)| *owner == fixture_id && attribute.is_intensity())
        .filter_map(|(_, value)| value.normalized())
        .fold(0.0_f32, f32::max)
}
