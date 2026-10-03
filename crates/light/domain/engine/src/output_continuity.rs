use crate::{Engine, MoveInBlackKey, MoveInBlackRuntime};
use std::collections::HashMap;

/// Mutable history belonging to one output lane. A speculative baseline or Preload lane can
/// fork this state without advancing the Live lane's fades or Move-in-Black state machine.
///
/// Capturing currently clones the retained transition maps. Keep that cost visible when
/// measuring prepared frames; it is not an immutable source cache.
#[derive(Clone, Default)]
pub struct OutputContinuityState {
    revision: u64,
    /// Content-versioned (TL-639), so an unchanged history lets the Programmer memo reuse an
    /// evaluation. Never iterated in an order-dependent way.
    pub(crate) programmer_transitions: crate::programmer_memo::ProgrammerTransitions,
    pub(crate) move_in_black: HashMap<MoveInBlackKey, MoveInBlackRuntime>,
    pub(crate) mounts: crate::mount_projection::MountTransformWorkspace,
}

/// Only the Live owner may turn this capture token into a committed state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OutputContinuityRevision(u64);

impl OutputContinuityState {
    pub(crate) fn advance_revision(&mut self) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("output continuity revision exhausted");
    }
}

impl Engine {
    pub(crate) fn capture_output_continuity(
        &self,
    ) -> (OutputContinuityRevision, OutputContinuityState) {
        let continuity = self.output_continuity.lock();
        (
            OutputContinuityRevision(continuity.revision),
            continuity.clone(),
        )
    }

    /// A reset or another committed evaluation after capture wins over this stale branch.
    /// Callers must discard a rejected branch before publishing its output.
    pub(crate) fn commit_output_continuity_if_unchanged(
        &self,
        expected: OutputContinuityRevision,
        mut next: OutputContinuityState,
    ) -> bool {
        let mut continuity = self.output_continuity.lock();
        if continuity.revision != expected.0 {
            return false;
        }
        next.revision = continuity.revision;
        next.advance_revision();
        *continuity = next;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration, Utc};
    use light_core::{
        AttributeKey, AttributeValue, CueListId, FixtureId, ManualClock, MergeMode, SessionId,
        TimedValue,
    };
    use std::sync::Arc;

    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp_millis(1_000_000).unwrap()
    }

    fn scalar(fixture: FixtureId, attribute: &str, value: f32) -> TimedValue {
        TimedValue {
            fixture_id: fixture,
            attribute: AttributeKey(attribute.into()),
            value: AttributeValue::Normalized(value),
            priority: 100,
            changed_at: now(),
            programmer_order: 0,
            merge_mode: MergeMode::Ltp,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        }
    }

    #[test]
    fn clear_rejects_a_previously_captured_continuity_branch() {
        let engine = Engine::new(Default::default());
        let (revision, state) = engine.capture_output_continuity();
        engine.clear_programmer_transitions();
        assert!(!engine.commit_output_continuity_if_unchanged(revision, state));
    }

    #[test]
    fn one_capture_can_only_commit_once() {
        let engine = Engine::new(Default::default());
        let (revision, state) = engine.capture_output_continuity();
        assert!(engine.commit_output_continuity_if_unchanged(revision, state.clone()));
        assert!(!engine.commit_output_continuity_if_unchanged(revision, state));
    }

    #[test]
    fn live_fade_mutation_rejects_an_older_capture() {
        let engine = Engine::new(Default::default());
        let (revision, state) = engine.capture_output_continuity();
        engine.faded_programmer_value(
            scalar(FixtureId::new(), "pan", 0.4),
            now(),
            None,
            light_core::ProgrammerId::new(),
            crate::ProgrammerTransitionSource::Programmer,
            false,
        );
        assert!(!engine.commit_output_continuity_if_unchanged(revision, state));
    }

    #[test]
    fn speculative_fade_does_not_seed_live_or_other_lanes_and_uses_captured_timing() {
        let clock = Arc::new(ManualClock::new(now()));
        let programmers = light_programmer::ProgrammerRegistry::with_clock(clock);
        let session = SessionId::new();
        let fixture = FixtureId::new();
        programmers.start(session);
        programmers.set_faded(
            session,
            fixture,
            AttributeKey("tilt".into()),
            AttributeValue::Normalized(1.0),
        );
        let engine = Engine::new(programmers.clone());
        let generation = engine.generation.load_full();
        let sources = programmers.active_output_states();
        let (revision, initial) = engine.capture_output_continuity();
        let mut speculative = initial.clone();
        let mut final_lane = initial;
        // The prepared frame captured 1s before this live control changed. Evaluation must
        // neither reread that timing nor let its baseline seed the final Dynamic underlay.
        engine.set_control_timing([120.0; 5], 10_000, 0, 0);
        let evaluate = |state: &mut OutputContinuityState, dynamic_value: f32| {
            let samples = [crate::ContributionBatch::new([
                crate::ContributionSample::independent(scalar(fixture, "tilt", dynamic_value)),
            ])];
            let mut underlay = crate::ResolvedContributionIndex::new(&[]);
            underlay.extend_sampled(crate::sampled_values(&samples));
            let values = engine.programmer_contributions_with_state(
                sources.clone(),
                &generation,
                now() + Duration::milliseconds(500),
                Some(&underlay),
                &samples,
                false,
                state,
                1_000,
                &HashMap::new(),
                &engine.programmer_addresses,
            );
            crate::ResolvedContributionIndex::new(&values[..])
                .value(fixture, &AttributeKey("tilt".into()))
                .and_then(AttributeValue::normalized)
                .unwrap()
        };
        assert!((evaluate(&mut speculative, 0.0) - 0.5).abs() < 0.0001);
        assert!((evaluate(&mut final_lane, 0.2) - 0.6).abs() < 0.0001);
        let (still_revision, live) = engine.capture_output_continuity();
        assert_eq!(still_revision, revision);
        assert!(live.programmer_transitions.is_empty());
        assert!(engine.commit_output_continuity_if_unchanged(revision, final_lane));
        assert!(!engine.commit_output_continuity_if_unchanged(revision, speculative));
    }

    #[test]
    fn speculative_move_in_black_does_not_change_live_diagnostics() {
        let engine = Engine::new(Default::default());
        let fixture = FixtureId::new();
        let cue_list = CueListId::new();
        let attribute = AttributeKey("pan".into());
        let candidate = light_playback::MoveInBlackCandidate {
            playback_number: Some(1),
            cue_list_id: cue_list,
            current_cue_id: uuid::Uuid::new_v4(),
            current_cue_number: 1_u16.into(),
            target_cue_id: uuid::Uuid::new_v4(),
            target_cue_number: 2_u16.into(),
            fixture_id: fixture,
            priority: 100,
            transition_ordinal: 1,
            values: vec![light_playback::MoveInBlackTargetValue {
                attribute: attribute.clone(),
                current: Some(AttributeValue::Normalized(0.2)),
                target: AttributeValue::Normalized(0.8),
                fade_millis: 1_000,
            }],
        };
        let key = MoveInBlackKey {
            playback_number: Some(1),
            cue_list_id: cue_list,
            fixture_id: fixture,
        };
        let prepared = crate::PreparedCandidate {
            key,
            candidate: candidate.clone(),
            enabled: true,
            delay_millis: 250,
            base_position: [(attribute, AttributeValue::Normalized(0.2))].into(),
            resolved_intensity: 0.0,
        };
        let mut runtime = MoveInBlackRuntime::new(&prepared, now());
        runtime.update(prepared, now());
        let (revision, mut initial) = engine.capture_output_continuity();
        initial.move_in_black.insert(key, runtime);
        assert!(engine.commit_output_continuity_if_unchanged(revision, initial));
        let (revision, mut speculative) = engine.capture_output_continuity();
        // This captured generation has no eligible fixture. Its private evaluation disables
        // the candidate, while Live must retain its original delayed-darkness state.
        let result = Engine::move_in_black_contributions_with_state(
            &engine.generation.load_full(),
            vec![candidate],
            &[],
            &crate::ResolvedValues::default(),
            now() + Duration::seconds(2),
            &mut speculative,
        );
        assert!(result.is_empty());
        assert_eq!(
            speculative.move_in_black[&key].diagnostic().state,
            crate::MoveInBlackState::Disabled,
        );
        let live = engine.move_in_black_runtime();
        assert_eq!(live[0].state, crate::MoveInBlackState::Delaying);
        assert_eq!(live[0].dark_since, Some(now()));
        assert_eq!(engine.capture_output_continuity().0, revision);
    }
}
